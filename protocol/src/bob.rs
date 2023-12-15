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
    protocol::{Response, ResponseError, Swap, Transition},
    utils::{monero_key_pair, monero_view_pair},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Value0 {
    alice_keys: KeyPublicWithoutProof,
    alice_bch_recv: Vec<u8>,
    contract_pair: ContractPair,
    #[serde(with = "monero_view_pair")]
    shared_keypair: monero::ViewPair,
    spend_bch: bitcoincash::PublicKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Value1 {
    alice_keys: KeyPublicWithoutProof,
    alice_bch_recv: Vec<u8>,
    // contract_pair: ContractPair,
    #[serde(with = "monero_view_pair")]
    shared_keypair: monero::ViewPair,
    spend_bch: bitcoincash::PublicKey,
    // refund_unlocker: Signature,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Value2 {
    alice_keys: KeyPublicWithoutProof,
    alice_bch_recv: Vec<u8>,
    // contract_pair: ContractPair,
    #[serde(with = "monero_view_pair")]
    shared_keypair: monero::ViewPair,
    spend_bch: bitcoincash::PublicKey,
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

// Api endpoints that will be exposed to alice
impl Swap<State> {
    pub fn get_keys(&self) -> KeyPublic {
        KeyPublic::from(self.keys.clone())
    }

    pub fn contract(&self) -> Option<(String, monero::Address)> {
        let props = match &self.state {
            State::WithAliceKey(props) => props,
            State::ContractMatch(props) => props,
            _ => return None,
        };

        Some((
            props.contract_pair.swaplock.cash_address(),
            monero::Address::from_viewpair(self.xmr_network, &props.shared_keypair),
        ))
    }

    pub fn swaplock_enc_sig(&self) -> Option<EncryptedSignature> {
        if let State::MoneroLocked(props) = &self.state {
            let hash = sha256::hash(&props.alice_bch_recv);
            let enc_sig = AdaptorSignature::encrypted_sign(
                &self.keys.ves,
                &props.spend_bch,
                hash.as_byte_array(),
            );

            return Some(enc_sig);
        }

        return None;
    }
}

impl Swap<State> {
    pub async fn transition(&mut self, transition: Transition) -> Response {
        let current_state = self.state.clone();
        match (current_state, transition) {
            (State::Init, Transition::Msg0 { keys, receiving }) => {
                let is_valid_keys =
                    proof::verify(&keys.proof, keys.spend_bch.clone(), keys.monero_spend);

                if !is_valid_keys {
                    return Response::Exit("invalid proof".to_owned());
                }

                let secp = bitcoincash::secp256k1::Secp256k1::signing_only();
                let contract_pair = ContractPair::create(
                    1000,
                    self.bch_recv.clone().into_bytes(),
                    self.keys.ves.public_key(&secp),
                    receiving.clone(),
                    keys.ves.clone(),
                );

                self.state = State::WithAliceKey(Value0 {
                    alice_bch_recv: receiving,
                    contract_pair,

                    shared_keypair: monero::ViewPair {
                        view: self.keys.monero_view + keys.monero_view,
                        spend: monero::PublicKey::from_private_key(&self.keys.monero_spend)
                            + keys.monero_spend,
                    },
                    spend_bch: keys.spend_bch.clone(),
                    alice_keys: keys.into(),
                });

                return Response::Ok;
            }
            (
                State::WithAliceKey(props),
                Transition::Contract {
                    bch_address,
                    xmr_address,
                },
            ) => {
                if props.contract_pair.swaplock.cash_address() != bch_address {
                    return Response::Err(ResponseError::InvalidBchAddress);
                }

                let xmr_derived =
                    monero::Address::from_viewpair(self.xmr_network, &props.shared_keypair);
                if xmr_address != xmr_derived {
                    return Response::Err(ResponseError::InvalidXmrAddress);
                }

                self.state = State::ContractMatch(props);
                return Response::Ok;
            }

            (State::ContractMatch(props), Transition::EncSig(enc_sig)) => {
                // check if decrypted sig can unlock Refund.cash contract
                let bob_receiving_hash = sha256::hash(self.bch_recv.as_bytes());
                let dec_sig = AdaptorSignature::decrypt_signature(&self.keys.monero_spend, enc_sig);

                let is_valid = AdaptorSignature::verify(
                    props.alice_keys.ves.clone(),
                    bob_receiving_hash.as_byte_array(),
                    &dec_sig,
                );

                if !is_valid {
                    return Response::Exit("Invalid signature".to_owned());
                }

                self.state = State::VerifiedEncSig(Value1 {
                    alice_keys: props.alice_keys,
                    alice_bch_recv: props.alice_bch_recv,
                    // contract_pair: props.contract_pair,
                    shared_keypair: props.shared_keypair,
                    spend_bch: props.spend_bch,
                    // refund_unlocker: dec_sig,
                });

                return Response::Ok;
            }

            (State::VerifiedEncSig(props), Transition::XmrLockVerified(restore_height)) => {
                self.state = State::MoneroLocked(Value2 {
                    alice_keys: props.alice_keys,
                    alice_bch_recv: props.alice_bch_recv,
                    // contract_pair: props.contract_pair,
                    shared_keypair: props.shared_keypair,
                    spend_bch: props.spend_bch,
                    // refund_unlocker: props.refund_unlocker,
                    restore_height,
                });
                return Response::Ok;
            }

            (State::MoneroLocked(props), Transition::DecSig(decsig)) => {
                let alice_spend = AdaptorSignature::recover_decryption_key(
                    props.alice_keys.spend_bch.clone(),
                    decsig,
                    self.swaplock_enc_sig()
                        .expect("Enc sig should be open at current state"),
                );

                let key_pair = monero::KeyPair {
                    view: props.shared_keypair.view,
                    spend: self.keys.monero_spend + alice_spend,
                };

                self.state = State::SwapSuccess(key_pair, props.restore_height);

                return Response::Done;
            }
            (_, _) => return Response::Err(ResponseError::InvalidStateTransition),
        };
    }
}
