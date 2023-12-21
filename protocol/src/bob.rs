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

use bitcoin_hashes::{sha256::Hash as sha256, Hash};
use ecdsa_fun::adaptor::EncryptedSignature;
use serde::{Deserialize, Serialize};

use crate::{
    adaptor_signature::AdaptorSignature,
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
    shared_keypair: monero::ViewPair,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Value1 {
    alice_keys: KeyPublicWithoutProof,
    #[serde(with = "hex")]
    alice_bch_recv: Vec<u8>,
    // contract_pair: ContractPair,
    #[serde(with = "monero_view_pair")]
    shared_keypair: monero::ViewPair,
    // refund_unlocker: Signature,
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
    restore_height: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum State {
    Init,
    WithAliceKey(Value0),
    ContractMatch(Value0),
    VerifiedEncSig(Value1),
    MoneroLocked(Value2),
    SwapSuccess(#[serde(with = "monero_key_pair")] monero::KeyPair, u64),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Bob {
    pub state: State,
    pub swap: Swap,
}

impl Bob {
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
    fn transition(&mut self, transition: Transition) -> (Option<Action>, Option<Error>) {
        let current_state = self.state.clone();
        match (current_state, transition) {
            (State::Init, Transition::Msg0 { keys, receiving }) => {
                let is_valid_keys = proof::verify(&keys.proof, keys.spend_bch, keys.monero_spend);

                if !is_valid_keys {
                    return (Some(Action::TradeFailed), Some(Error::InvalidProof));
                }

                let secp = bitcoincash::secp256k1::Secp256k1::signing_only();
                let contract_pair = ContractPair::create(
                    1000,
                    self.swap.bch_recv.clone().into_bytes(),
                    self.swap.keys.ves.public_key(&secp),
                    receiving.clone().into_bytes(),
                    keys.ves.clone(),
                );

                self.state = State::WithAliceKey(Value0 {
                    alice_bch_recv: receiving.into_bytes(),
                    contract_pair,

                    shared_keypair: monero::ViewPair {
                        view: self.swap.keys.monero_view + keys.monero_view,
                        spend: monero::PublicKey::from_private_key(&self.swap.keys.monero_spend)
                            + keys.monero_spend,
                    },
                    alice_keys: keys.into(),
                });

                return (None, None);
            }
            (
                State::WithAliceKey(props),
                Transition::Contract {
                    bch_address,
                    xmr_address,
                },
            ) => {
                if props.contract_pair.swaplock.cash_address() != bch_address {
                    return (None, Some(Error::InvalidBchAddress));
                }

                let xmr_derived =
                    monero::Address::from_viewpair(self.swap.xmr_network, &props.shared_keypair);
                if xmr_address != xmr_derived {
                    return (None, Some(Error::InvalidXmrAddress));
                }

                self.state = State::ContractMatch(props);
                return (None, None);
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
                    return (Some(Action::TradeFailed), Some(Error::InvalidSignature));
                }

                let (bch_address, xmr_address) = self.get_contract().unwrap();
                let action = Action::LockBchAndWatchXmr(bch_address, xmr_address);

                self.state = State::VerifiedEncSig(Value1 {
                    alice_keys: props.alice_keys,
                    alice_bch_recv: props.alice_bch_recv,
                    // contract_pair: props.contract_pair,
                    shared_keypair: props.shared_keypair,
                    // refund_unlocker: dec_sig,
                });

                return (Some(action), None);
            }

            (State::VerifiedEncSig(props), Transition::XmrLockVerified(restore_height, amount)) => {
                if amount != self.swap.xmr_amount {
                    return (None, Some(Error::InvalidXmrAddress));
                }

                self.state = State::MoneroLocked(Value2 {
                    alice_keys: props.alice_keys,
                    alice_bch_recv: props.alice_bch_recv,
                    // contract_pair: props.contract_pair,
                    shared_keypair: props.shared_keypair,
                    // refund_unlocker: props.refund_unlocker,
                    restore_height,
                });
                let (bch_address, _) = self.get_contract().unwrap();
                return (Some(Action::WatchBchAddress(bch_address)), None);
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

                self.state = State::SwapSuccess(key_pair, props.restore_height);

                return (Some(Action::TradeSuccess), None);
            }
            (_, _) => return (None, Some(Error::InvalidStateTransition)),
        };
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
