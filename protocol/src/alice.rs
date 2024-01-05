//! >> Keywords <<
//!     -> PrevTransition - The transition that makes the state-machine move to current state
//!     -> OEW (On Enter the Watcher) - If we enter to this state, the watcher must...
//!
//! >> State <<
//!     Init
//!     WithBobKeys
//!         -> Bob are able to get contract
//!     ContractMatch
//!         -> OEW
//!             -> Watch the SwapLock contract if it receive tx with correct amount
//!                 Watch that tx for BCH_MIN_CONFIRMATION
//!                 If BCH_MIN_CONFIRMATION satisfied, Transition::BchLockVerified
//!         -> Bob are able to get refund encrypted signature
//!     BchLocked
//!         -> OEW
//!             -> Must send the XMR to shared address
//!     ValidEncSig
//!         -> Alice can get swap_tx and broadcast it
//!

use std::fmt;

use anyhow::bail;
use bitcoin_hashes::{sha256::Hash as sha256, Hash};
use bitcoincash::{
    consensus::Encodable, OutPoint, PackedLockTime, Script, Sequence, Transaction, TxIn, TxOut,
};
use ecdsa_fun::adaptor::EncryptedSignature;
use hex::ToHex;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    adaptor_signature::AdaptorSignature,
    bitcoincash::secp256k1::ecdsa,
    blockchain::{scan_address_conf_tx, TcpElectrum},
    contract::{ContractPair, TransactionType},
    keys::{KeyPublic, KeyPublicWithoutProof},
    proof,
    protocol::{Action, Error, Swap, SwapEvents, Transition},
    utils::monero_view_pair,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Value0 {
    bob_keys: KeyPublicWithoutProof,
    #[serde(with = "hex")]
    bob_bch_recv: Vec<u8>,
    contract_pair: ContractPair,

    #[serde(with = "monero_view_pair")]
    shared_keypair: monero::ViewPair,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Value1 {
    bob_keys: KeyPublicWithoutProof,
    #[serde(with = "hex")]
    bob_bch_recv: Vec<u8>,
    contract_pair: ContractPair,
    #[serde(with = "monero_view_pair")]
    shared_keypair: monero::ViewPair,

    outpoint: OutPoint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Value2 {
    bob_keys: KeyPublicWithoutProof,
    #[serde(with = "hex")]
    bob_bch_recv: Vec<u8>,
    contract_pair: ContractPair,
    #[serde(with = "monero_view_pair")]
    shared_keypair: monero::ViewPair,
    outpoint: OutPoint,

    dec_sig: ecdsa::Signature,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum State {
    Init,
    WithBobKeys(Value0),
    ContractMatch(Value0),
    BchLocked(Value1),
    ValidEncSig(Value2),
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            State::Init => write!(f, "AliceState:Init"),
            State::WithBobKeys(_) => write!(f, "AliceState:WithBobKeys"),
            State::ContractMatch(_) => write!(f, "AliceState:ContractMatch"),
            State::BchLocked(_) => write!(f, "AliceState:BchLocked"),
            State::ValidEncSig(_) => write!(f, "AliceState:ValidEncSig"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alice {
    pub state: State,
    pub swap: Swap,
}

impl Alice {
    pub fn get_public_keys(&self) -> KeyPublic {
        KeyPublic::from(self.swap.keys.clone())
    }

    pub fn get_contract(&self) -> Option<(String, monero::Address)> {
        if let State::WithBobKeys(props) = &self.state {
            return Some((
                props.contract_pair.swaplock.cash_address(),
                monero::Address::from_viewpair(self.swap.xmr_network, &props.shared_keypair),
            ));
        }

        return None;
    }

    pub fn get_refunc_enc_sig(&self) -> Option<EncryptedSignature> {
        if let State::ContractMatch(props) = &self.state {
            let hash = sha256::hash(&props.bob_bch_recv).to_byte_array();
            let hash = sha256::hash(&hash).to_byte_array();
            let enc_sig = AdaptorSignature::encrypted_sign(
                &self.swap.keys.ves,
                &props.bob_keys.spend_bch,
                &hash,
            );
            return Some(enc_sig);
        }

        return None;
    }

    pub fn get_contract_pair(&self) -> Option<ContractPair> {
        match self.state.clone() {
            State::Init => None,
            State::WithBobKeys(v) => Some(v.contract_pair),
            State::ContractMatch(v) => Some(v.contract_pair),
            State::BchLocked(v) => Some(v.contract_pair),
            State::ValidEncSig(v) => Some(v.contract_pair),
        }
    }

    pub fn get_unlock_normal_tx(&self) -> Option<Transaction> {
        if let State::ValidEncSig(props) = &self.state {
            let unlocker = props
                .contract_pair
                .swaplock
                .unlocking_script(&props.dec_sig.serialize_der());

            let mining_fee = props.contract_pair.swaplock.mining_fee;
            let transaction = Transaction {
                version: 2,
                lock_time: PackedLockTime(0), // TODO: Should we use current time?
                input: vec![TxIn {
                    sequence: Sequence(0),
                    previous_output: props.outpoint,
                    script_sig: Script::from(unlocker),
                    ..Default::default()
                }],
                output: vec![TxOut {
                    value: self.swap.bch_amount.to_sat() - mining_fee,
                    script_pubkey: self.swap.bch_recv.clone(),
                    token: None,
                }],
            };

            return Some(transaction);
        }

        None
    }
}

#[async_trait::async_trait]
impl SwapEvents for Alice {
    type State = Alice;

    fn transition(mut self, transition: Transition) -> (Self::State, Vec<Action>, Option<Error>) {
        match &self.state {
            State::Init => print!("Init - "),
            State::WithBobKeys(_) => print!("WithBobKeys - "),
            State::ContractMatch(_) => print!("ContractMatch - "),
            State::BchLocked(_) => print!("BchLocked - "),
            State::ValidEncSig(_) => print!("ValidEncSig - "),
        }
        println!("{}", &transition);

        let current_state = self.state.clone();
        match (current_state, transition) {
            (State::Init, Transition::Msg0 { keys, receiving }) => {
                let is_valid_keys = proof::verify(&keys.proof, keys.spend_bch, keys.monero_spend);
                if !is_valid_keys {
                    return (self, vec![Action::SafeDelete], Some(Error::InvalidProof));
                }

                let secp = bitcoincash::secp256k1::Secp256k1::signing_only();
                let contract = ContractPair::create(
                    1000,
                    receiving.clone().into_bytes(),
                    keys.ves.clone(),
                    self.swap.bch_recv.to_bytes().clone(),
                    self.swap.keys.ves.public_key(&secp),
                    self.swap.timelock1,
                    self.swap.timelock2,
                    self.swap.bch_network,
                    self.swap.bch_amount,
                );

                self.state = State::WithBobKeys(Value0 {
                    bob_bch_recv: receiving.into_bytes(),
                    contract_pair: contract,
                    shared_keypair: monero::ViewPair {
                        view: self.swap.keys.monero_view + keys.monero_view,
                        spend: monero::PublicKey::from_private_key(&self.swap.keys.monero_spend)
                            + keys.monero_spend,
                    },
                    bob_keys: keys.into(),
                });

                return (self, vec![], None);
            }
            (
                State::WithBobKeys(props),
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

                let refund = props.contract_pair.refund.cash_address();
                self.state = State::ContractMatch(props);
                return (
                    self,
                    vec![Action::WatchBchAddress {
                        swaplock: bch_address,
                        refund,
                    }],
                    None,
                );
            }

            (State::ContractMatch(props), Transition::BchConfirmedTx(transaction)) => {
                match props.contract_pair.analyze_tx(transaction) {
                    Some((outpoint, TransactionType::ToSwapLock)) => {
                        self.state = State::BchLocked(Value1 {
                            bob_keys: props.bob_keys,
                            bob_bch_recv: props.bob_bch_recv,
                            contract_pair: props.contract_pair,
                            shared_keypair: props.shared_keypair,

                            outpoint,
                        });

                        let xmr_amount = self.swap.xmr_amount;
                        let address = monero::Address::from_viewpair(
                            self.swap.xmr_network,
                            &props.shared_keypair,
                        );
                        return (self, vec![Action::LockXmr(xmr_amount, address)], None);
                    }
                    _ => return (self, vec![], Some(Error::InvalidTransaction)),
                }
            }

            (State::ValidEncSig(_), Transition::EncSig(_)) => {
                return (self, vec![], None);
            }

            (State::BchLocked(props), Transition::EncSig(encsig)) => {
                let dec_sig = AdaptorSignature::decrypt_signature(
                    &self.swap.keys.monero_spend,
                    encsig.clone(),
                );

                {
                    // ? Check if the message by bob can unlock the swaplock contract
                    let recv_hash = sha256::hash(&self.swap.bch_recv.to_bytes()).to_byte_array();
                    let recv_hash = sha256::hash(&recv_hash).to_byte_array();
                    let signer = props.bob_keys.ves.clone();

                    if !AdaptorSignature::verify(signer, &recv_hash, &dec_sig) {
                        return (self, vec![Action::Refund], Some(Error::InvalidSignature));
                        // Todo: procceed to refund
                    }
                }

                let dec_sig = match ecdsa::Signature::from_compact(&dec_sig.to_bytes()) {
                    Ok(v) => v,
                    Err(_) => return (self, vec![Action::Refund], Some(Error::InvalidSignature)),
                };

                self.state = State::ValidEncSig(Value2 {
                    bob_keys: props.bob_keys,
                    bob_bch_recv: props.bob_bch_recv,
                    contract_pair: props.contract_pair,
                    shared_keypair: props.shared_keypair,
                    outpoint: props.outpoint,
                    dec_sig,
                });
                return (self, vec![Action::UnlockBchNormal], None);
            }
            (_, _) => return (self, vec![], Some(Error::InvalidStateTransition)),
        }
    }

    fn get_transition(&self) -> Option<Transition> {
        match &self.state {
            State::Init => {
                let keys = self.get_public_keys();
                let receiving = self.swap.bch_recv.clone();
                Some(Transition::Msg0 { keys, receiving })
            }
            State::WithBobKeys(_) => {
                let (bch_address, xmr_address) = self.get_contract().unwrap();
                Some(Transition::Contract {
                    bch_address,
                    xmr_address,
                })
            }
            State::ContractMatch(_) => {
                let enc_sig = self.get_refunc_enc_sig().unwrap();
                Some(Transition::EncSig(enc_sig))
            }
            _ => None,
        }
    }
}

pub struct Runner<'a> {
    pub inner: Alice,
    pub bch: &'a TcpElectrum,
    // pub monerod: &'a monero_rpc::DaemonJsonRpcClient,
    // pub monero_wallet: &'a Mutex<monero_rpc::WalletClient>,
    pub min_bch_conf: i64,
}

impl Runner<'_> {
    pub async fn check_bch(&mut self) -> anyhow::Result<()> {
        let contract = self.inner.get_contract_pair();
        if let Some(contract) = contract {
            let swaplock = contract.swaplock.cash_address();
            let refund = contract.refund.cash_address();
            for address in [swaplock, refund].into_iter() {
                let txs = scan_address_conf_tx(&self.bch, &address, self.min_bch_conf).await;
                println!("{}txs address {}", txs.len(), address);
                for tx in txs {
                    let _ = self.priv_transition(Transition::BchConfirmedTx(tx)).await;
                }
            }
        }

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
        let (new_state, actions, error) = self.inner.clone().transition(transition);
        if let Some(err) = error {
            bail!(err);
        }

        for action in actions {
            match action {
                Action::LockXmr(amount, addr) => {
                    let msg = format!("  Send {} to {}  ", amount, addr.to_string());
                    println!("|{:=^width$}|", "", width = msg.len());
                    println!("|{msg}|");
                    println!("|{:=^width$}|", "", width = msg.len());
                }
                Action::UnlockBchNormal => {
                    let mut buffer = Vec::new();
                    let transaction = new_state.get_unlock_normal_tx().unwrap();
                    transaction.consensus_encode(&mut buffer).unwrap();
                    let tx_hex: String = buffer.encode_hex();

                    println!("Broadcasting tx. Expected txid: {}", transaction.txid());
                    println!("Hex: {}", tx_hex);
                    let transaction_resp = self
                        .bch
                        .send("blockchain.transaction.broadcast", json!([tx_hex]))
                        .await
                        .unwrap();
                    dbg!(transaction_resp);
                }
                _ => {}
            }
        }

        self.inner = new_state;
        Ok(())
    }
}
