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

use crate::{
    protocol::{Action, SwapEvents},
    utils::monero_view_pair,
};
use bitcoin_hashes::{sha256::Hash as sha256, Hash};
use bitcoincash::{
    consensus::Encodable, OutPoint, PackedLockTime, Script, Sequence, Transaction, TxIn, TxOut,
};
use ecdsa_fun::{adaptor::EncryptedSignature, Signature};
use serde::{Deserialize, Serialize};

use crate::{
    adaptor_signature::AdaptorSignature,
    contract::ContractPair,
    keys::{KeyPublic, KeyPublicWithoutProof},
    proof,
    protocol::{Error, Swap, Transition},
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

    dec_sig: Signature,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum State {
    Init,
    WithBobKeys(Value0),
    ContractMatch(Value0),
    BchLocked(Value1),
    ValidEncSig(Value2),
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
            let hash = sha256::hash(&props.bob_bch_recv);
            let enc_sig = AdaptorSignature::encrypted_sign(
                &self.swap.keys.ves,
                &props.bob_keys.spend_bch,
                hash.as_byte_array(),
            );
            return Some(enc_sig);
        }

        return None;
    }
}

// private api
impl Alice {
    pub fn get_swap_tx(&self) -> Option<Vec<u8>> {
        if let State::ValidEncSig(props) = &self.state {
            let unlocker = props
                .contract_pair
                .swaplock
                .unlocking_script(&props.dec_sig.to_bytes());

            let mut buffer = Vec::new();
            Transaction {
                version: 2,
                lock_time: PackedLockTime(812991),
                input: vec![TxIn {
                    sequence: Sequence(0),
                    previous_output: props.outpoint,
                    script_sig: Script::from(unlocker),
                    ..Default::default()
                }],
                output: vec![TxOut {
                    value: self.swap.bch_amount.to_sat(),
                    script_pubkey: self.swap.bch_recv.clone(),
                    token: None,
                }],
            }
            .consensus_encode(&mut buffer)
            .expect("cannot encode tx");

            return Some(buffer);
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
                let mut outpoint = None;
                let swaplock_script = props.contract_pair.swaplock.locking_script();
                for (vout, txout) in transaction.output.iter().enumerate() {
                    if txout.value == self.swap.bch_amount.to_sat()
                        && txout.script_pubkey.as_bytes() == swaplock_script
                    {
                        outpoint = Some(bitcoincash::OutPoint {
                            txid: transaction.txid(),
                            vout: vout as u32,
                        });
                        break;
                    }
                }

                match outpoint {
                    None => return (self, vec![], Some(Error::InvalidTransaction)),
                    Some(outpoint) => {
                        self.state = State::BchLocked(Value1 {
                            bob_keys: props.bob_keys,
                            bob_bch_recv: props.bob_bch_recv,
                            contract_pair: props.contract_pair,
                            shared_keypair: props.shared_keypair,

                            outpoint,
                        });
                        return (self, vec![], None);
                    }
                };
            }
            (State::BchLocked(props), Transition::EncSig(encsig)) => {
                let dec_sig = AdaptorSignature::decrypt_signature(
                    &self.swap.keys.monero_spend,
                    encsig.clone(),
                );

                {
                    // ? Check if the message by bob can unlock the swaplock contract
                    let alice_recv_hash = sha256::hash(&self.swap.bch_recv.to_bytes());
                    let signer = props.bob_keys.ves.clone();
                    let message = alice_recv_hash.to_byte_array();

                    if !AdaptorSignature::verify(signer, &message, &dec_sig) {
                        return (self, vec![Action::Refund], Some(Error::InvalidSignature));
                        // Todo: procceed to refund
                    }
                }

                self.state = State::ValidEncSig(Value2 {
                    bob_keys: props.bob_keys,
                    bob_bch_recv: props.bob_bch_recv,
                    contract_pair: props.contract_pair,
                    shared_keypair: props.shared_keypair,
                    outpoint: props.outpoint,
                    dec_sig,
                });
                return (self, vec![Action::TradeSuccess], None);
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
