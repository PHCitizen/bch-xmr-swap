//! >> Keywords <<
//!     -> PrevTransition - The transition that makes the state-machine move to current state
//!     -> OEW (On Enter the Watcher) - If we enter to this state, the watcher must...
//!
//! >> State <<
//!     Init
//!     WithAliceKey
//!         -> Alice are able to get contract
//!     ContractMatch
//!     VerifiedEncSig
//!         -> OEW
//!             -> Send bch to SwapLock contract
//!             -> get the current xmr block. Will be used for `restore block`
//!     MoneroLocked
//!         -> OEW
//!             -> Watch the SwapLock contract if it is send to alice address
//!                 If it does, get decsig, Transition::DecSig
//!     SwapSuccess(monero::KeyPair, restore_height: u64)

use anyhow::bail;
use bitcoin_hashes::{sha256::Hash as sha256, Hash};
use ecdsa_fun::adaptor::EncryptedSignature;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::{
    adaptor_signature::AdaptorSignature,
    blockchain::TcpElectrum,
    contract::ContractPair,
    keys::{KeyPublic, KeyPublicWithoutProof},
    proof,
    protocol::{Action, Error, Swap, SwapEvents, Transition},
    utils::{monero_key_pair, monero_view_pair},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Value0 {
    alice_keys: KeyPublicWithoutProof,
    #[serde(with = "hex")]
    alice_bch_recv: Vec<u8>,
    contract_pair: ContractPair,
    #[serde(with = "monero_view_pair")]
    pub shared_keypair: monero::ViewPair,
    xmr_restore_height: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Value2 {
    alice_keys: KeyPublicWithoutProof,
    #[serde(with = "hex")]
    alice_bch_recv: Vec<u8>,
    // contract_pair: ContractPair,
    #[serde(with = "monero_view_pair")]
    shared_keypair: monero::ViewPair,
    // refund_unlocker: Signature,
    xmr_restore_height: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum State {
    Init,
    WithAliceKey(Value0),
    ContractMatch(Value0),
    VerifiedEncSig(Value0),
    MoneroLocked(Value2),
    SwapSuccess(#[serde(with = "monero_key_pair")] monero::KeyPair, u64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bob {
    pub state: State,
    pub swap: Swap,
}

impl Bob {
    pub fn new(swap: Swap) -> Self {
        Bob {
            state: State::Init,
            swap,
        }
    }

    pub fn get_public_keys(&self) -> KeyPublic {
        KeyPublic::from(self.swap.keys.clone())
    }

    pub fn get_contract(&self) -> Option<(String, monero::Address)> {
        let props = match &self.state {
            State::WithAliceKey(props) => props,
            State::ContractMatch(props) => props,
            _ => return None,
        };

        Some((
            props.contract_pair.swaplock.cash_address(),
            monero::Address::from_viewpair(self.swap.xmr_network, &props.shared_keypair),
        ))
    }

    pub fn get_swaplock_enc_sig(&self) -> Option<EncryptedSignature> {
        if let State::MoneroLocked(props) = &self.state {
            let hash = sha256::hash(&props.alice_bch_recv);
            let enc_sig = AdaptorSignature::encrypted_sign(
                &self.swap.keys.ves,
                &props.alice_keys.spend_bch,
                hash.as_byte_array(),
            );

            return Some(enc_sig);
        }

        return None;
    }
}

#[async_trait::async_trait]
impl SwapEvents for Bob {
    type State = Bob;
    fn transition(mut self, transition: Transition) -> (Self::State, Vec<Action>, Option<Error>) {
        match &self.state {
            State::Init => print!("Init - "),
            State::WithAliceKey(_) => print!("WithAliceKey - "),
            State::ContractMatch(_) => print!("ContractMatch - "),
            State::VerifiedEncSig(_) => print!("VerifiedEncSig - "),
            State::MoneroLocked(_) => print!("MoneroLocked - "),
            State::SwapSuccess(_, _) => print!("SwapSuccess - "),
        }
        println!("{}", &transition);

        if let Transition::SetXmrRestoreHeight(height) = transition {
            match &mut self.state {
                State::WithAliceKey(ref mut v) => v.xmr_restore_height = height,
                State::ContractMatch(ref mut v) => v.xmr_restore_height = height,
                State::VerifiedEncSig(ref mut v) => v.xmr_restore_height = height,
                State::MoneroLocked(ref mut v) => v.xmr_restore_height = height,
                _ => {}
            }
            return (self, vec![], None);
        }

        match (self.state.clone(), transition) {
            (State::Init, Transition::Msg0 { keys, receiving }) => {
                let is_valid_keys = proof::verify(&keys.proof, keys.spend_bch, keys.monero_spend);

                if !is_valid_keys {
                    return (self, vec![Action::SafeDelete], Some(Error::InvalidProof));
                }

                let secp = bitcoincash::secp256k1::Secp256k1::signing_only();
                let contract_pair = ContractPair::create(
                    1000,
                    self.swap.bch_recv.clone().into_bytes(),
                    self.swap.keys.ves.public_key(&secp),
                    receiving.clone().into_bytes(),
                    keys.ves.clone(),
                    self.swap.timelock1,
                    self.swap.timelock2,
                    self.swap.bch_network,
                    self.swap.bch_amount,
                );

                let shared_keypair = monero::ViewPair {
                    view: self.swap.keys.monero_view + keys.monero_view,
                    spend: monero::PublicKey::from_private_key(&self.swap.keys.monero_spend)
                        + keys.monero_spend,
                };

                self.state = State::WithAliceKey(Value0 {
                    alice_bch_recv: receiving.into_bytes(),
                    contract_pair,

                    shared_keypair,
                    alice_keys: keys.into(),
                    xmr_restore_height: 0,
                });

                return (self, vec![Action::CreateXmrView(shared_keypair)], None);
            }
            (
                State::WithAliceKey(props),
                Transition::Contract {
                    bch_address,
                    xmr_address,
                },
            ) => {
                if props.contract_pair.swaplock.cash_address() != bch_address {
                    return (self, vec![], Some(Error::InvalidBchAddress));
                }

                let xmr_derived =
                    monero::Address::from_viewpair(self.swap.xmr_network, &props.shared_keypair);
                if xmr_address != xmr_derived {
                    return (self, vec![], Some(Error::InvalidXmrAddress));
                }

                self.state = State::ContractMatch(props);
                return (self, vec![], None);
            }

            (State::ContractMatch(props), Transition::EncSig(enc_sig)) => {
                // check if decrypted sig can unlock Refund.cash contract
                let bob_receiving_hash = sha256::hash(self.swap.bch_recv.as_bytes());
                let dec_sig =
                    AdaptorSignature::decrypt_signature(&self.swap.keys.monero_spend, enc_sig);

                let is_valid = AdaptorSignature::verify(
                    props.alice_keys.ves.clone(),
                    bob_receiving_hash.as_byte_array(),
                    &dec_sig,
                );

                if !is_valid {
                    return (
                        self,
                        vec![Action::SafeDelete],
                        Some(Error::InvalidSignature),
                    );
                }

                let (bch_address, xmr_address) = self.get_contract().unwrap();

                self.state = State::VerifiedEncSig(props);

                return (
                    self,
                    vec![Action::LockBch(bch_address), Action::WatchXmr(xmr_address)],
                    None,
                );
            }

            (State::VerifiedEncSig(props), Transition::XmrLockVerified(amount)) => {
                if amount != self.swap.xmr_amount {
                    return (self, vec![], Some(Error::InvalidXmrAmount));
                }

                self.state = State::MoneroLocked(Value2 {
                    alice_keys: props.alice_keys,
                    alice_bch_recv: props.alice_bch_recv,
                    // contract_pair: props.contract_pair,
                    shared_keypair: props.shared_keypair,
                    // refund_unlocker: props.refund_unlocker,
                    xmr_restore_height: props.xmr_restore_height,
                });
                let (bch_address, _) = self.get_contract().unwrap();
                return (
                    self,
                    vec![Action::WatchBchAddress {
                        swaplock: bch_address,
                        refund: props.contract_pair.refund.cash_address(),
                    }],
                    None,
                );
            }

            (State::MoneroLocked(props), Transition::DecSig(decsig)) => {
                let alice_spend = AdaptorSignature::recover_decryption_key(
                    props.alice_keys.spend_bch,
                    decsig,
                    self.get_swaplock_enc_sig()
                        .expect("Enc sig should be open at current state"),
                );

                let key_pair = monero::KeyPair {
                    view: props.shared_keypair.view,
                    spend: self.swap.keys.monero_spend + alice_spend,
                };

                self.state = State::SwapSuccess(key_pair, props.xmr_restore_height);

                return (self, vec![Action::TradeSuccess], None);
            }

            (_, _) => return (self, vec![], Some(Error::InvalidStateTransition)),
        }
    }

    fn get_transition(&self) -> Option<Transition> {
        match &self.state {
            State::Init => None,
            State::WithAliceKey(_) => {
                let keys = self.get_public_keys();
                let receiving = self.swap.bch_recv.clone();
                Some(Transition::Msg0 { keys, receiving })
            }
            State::ContractMatch(_) => {
                let (bch_address, xmr_address) = self.get_contract().unwrap();
                Some(Transition::Contract {
                    bch_address,
                    xmr_address,
                })
            }
            State::MoneroLocked(_) => {
                let enc_sig = self.get_swaplock_enc_sig().unwrap();
                Some(Transition::EncSig(enc_sig))
            }
            _ => None,
        }
    }
}

pub struct Runner<'a> {
    pub inner: Bob,
    pub trade_id: String,
    pub bch: &'a TcpElectrum,
    pub monerod: &'a monero_rpc::DaemonJsonRpcClient,
    pub monero_wallet: &'a Mutex<monero_rpc::WalletClient>,
}

impl Runner<'_> {
    pub async fn check_xmr(&mut self) -> anyhow::Result<()> {
        let monero_wallet = self.monero_wallet.lock().await;
        monero_wallet
            .open_wallet(format!("{}_view", self.trade_id), Some("".to_owned()))
            .await?;
        let balance = monero_wallet.get_balance(0, None).await?;
        let (new_state, actions, _) = self
            .inner
            .clone()
            .transition(Transition::XmrLockVerified(balance.unlocked_balance));
        // TODO: check actions
        self.inner = new_state;

        Ok(())
    }

    pub async fn pub_transition(&mut self, transition: Transition) -> anyhow::Result<()> {
        match &transition {
            Transition::Msg0 { .. } => {}
            Transition::Contract { .. } => {}
            Transition::EncSig(_) => {}
            _ => bail!("priv transition"),
        }

        self.priv_transition(transition).await
    }

    pub async fn priv_transition(&mut self, transition: Transition) -> anyhow::Result<()> {
        let (mut new_state, actions, error) = self.inner.clone().transition(transition);
        if let Some(err) = error {
            bail!(err);
        }

        for action in actions {
            match action {
                Action::SafeDelete => {
                    todo!("bubble up?")
                }
                Action::CreateXmrView(keypair) => {
                    let address =
                        monero::Address::from_viewpair(self.inner.swap.xmr_network, &keypair);
                    let height = self.monerod.get_block_count().await?.get();

                    let monero_wallet = self.monero_wallet.lock().await;
                    let _ = monero_wallet
                        .generate_from_keys(monero_rpc::GenerateFromKeysArgs {
                            address,
                            restore_height: Some(height),
                            autosave_current: Some(true),
                            filename: format!("{}_view", self.trade_id),
                            password: "".to_owned(),
                            spendkey: None,
                            viewkey: keypair.view,
                        })
                        .await?;
                    monero_wallet.close_wallet().await?;
                    new_state = new_state
                        .transition(Transition::SetXmrRestoreHeight(height))
                        .0;
                }
                Action::TradeSuccess => {}
                Action::WatchBchAddress { .. } => {}
                Action::Refund => {}
                Action::LockBch(_) => {}
                Action::WatchXmr(_) => {}
            }
        }

        self.inner = new_state;
        Ok(())
    }
}
