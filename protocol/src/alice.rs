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

use bitcoin_hashes::{sha256::Hash as sha256, Hash};
use bitcoincash::{
    consensus::Encodable, OutPoint, PackedLockTime, Script, Sequence, Transaction, TxIn, TxOut,
};
use ecdsa_fun::{adaptor::EncryptedSignature, Signature};

use crate::{
    adaptor_signature::AdaptorSignature,
    contract::ContractPair,
    keys::{bitcoin, KeyPublic, KeyPublicWithoutProof},
    proof,
    protocol::{Response, Swap, Transition},
};

#[derive(Debug, Clone)]
pub struct Value0 {
    bob_keys: KeyPublicWithoutProof,
    bob_bch_recv: Vec<u8>,
    contract_pair: ContractPair,

    shared_keypair: monero::ViewPair,
    spend_bch: bitcoin::PublicKey,
}

#[derive(Debug, Clone)]
pub struct Value1 {
    bob_keys: KeyPublicWithoutProof,
    bob_bch_recv: Vec<u8>,
    contract_pair: ContractPair,
    shared_keypair: monero::ViewPair,
    spend_bch: bitcoin::PublicKey,

    outpoint: OutPoint,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Value2 {
    bob_keys: KeyPublicWithoutProof,
    bob_bch_recv: Vec<u8>,
    contract_pair: ContractPair,
    shared_keypair: monero::ViewPair,
    spend_bch: bitcoin::PublicKey,
    outpoint: OutPoint,

    dec_sig: Signature,
}

#[derive(Debug, Clone)]
pub enum State {
    Init,
    WithBobKeys(Value0),
    ContractMatch(Value0),
    BchLocked(Value1),
    ValidEncSig(Value2),
}

// Api endpoints that will be exposed to bob
impl Swap<State> {
    pub fn get_keys(&self) -> KeyPublic {
        self.keys.to_public()
    }

    pub fn contract(&self) -> Option<(String, monero::Address)> {
        // todo
        if let State::WithBobKeys(props) = &self.state {
            return Some((
                props.contract_pair.swaplock.cash_address(),
                monero::Address::from_viewpair(self.xmr_network, &props.shared_keypair),
            ));
        }

        return None;
    }

    pub fn refunc_enc_sig(&self) -> Option<EncryptedSignature> {
        if let State::ContractMatch(props) = &self.state {
            let hash = sha256::hash(&props.bob_bch_recv);
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

// private api
impl Swap<State> {
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
                    value: self.bch_amount.to_sat(),
                    script_pubkey: self.bch_recv.clone(),
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

impl Swap<State> {
    pub async fn transition(&mut self, transition: Transition) -> anyhow::Result<Response> {
        let current_state = self.state.clone();
        match (current_state, transition) {
            (State::Init, Transition::Msg0 { keys, receiving }) => {
                let is_valid_keys =
                    proof::verify(&keys.proof, keys.spend_bch.clone(), keys.monero_spend);

                if !is_valid_keys {
                    return Ok(Response::Exit("invalid proof".to_owned()));
                }

                let contract = ContractPair::create(
                    1000,
                    receiving.clone(),
                    keys.ves.clone(),
                    self.bch_recv.to_bytes().clone(),
                    self.keys.ves.public_key(),
                );

                self.state = State::WithBobKeys(Value0 {
                    bob_keys: keys.remove_proof(),
                    bob_bch_recv: receiving,
                    contract_pair: contract,
                    shared_keypair: monero::ViewPair {
                        view: self.keys.monero_view + keys.monero_view,
                        spend: monero::PublicKey::from_private_key(&self.keys.monero_spend)
                            + keys.monero_spend,
                    },
                    spend_bch: keys.spend_bch,
                });

                return Ok(Response::Ok);
            }
            (
                State::WithBobKeys(props),
                Transition::Contract {
                    bch_address,
                    xmr_address,
                },
            ) => {
                if props.contract_pair.swaplock.cash_address() != bch_address {
                    return Ok(Response::Exit("bch address not match".to_owned()));
                }

                let xmr_derived =
                    monero::Address::from_viewpair(self.xmr_network, &props.shared_keypair);
                if xmr_address != xmr_derived {
                    return Ok(Response::Exit("xmr address not match".to_owned()));
                }

                self.state = State::ContractMatch(props);
                return Ok(Response::Ok);
            }

            (State::ContractMatch(props), Transition::BchLockVerified(outpoint)) => {
                self.state = State::BchLocked(Value1 {
                    bob_keys: props.bob_keys,
                    bob_bch_recv: props.bob_bch_recv,
                    contract_pair: props.contract_pair,
                    shared_keypair: props.shared_keypair,
                    spend_bch: props.spend_bch,

                    outpoint,
                });
                return Ok(Response::Ok);
            }
            (State::BchLocked(props), Transition::EncSig(encsig)) => {
                let bob_receiving_hash = sha256::hash(&self.bch_recv.to_bytes());
                let dec_sig =
                    AdaptorSignature::decrypt_signature(&self.keys.monero_spend, encsig.clone());

                let is_valid = AdaptorSignature::verify(
                    props.bob_keys.ves.clone(),
                    bob_receiving_hash.as_byte_array(),
                    &dec_sig,
                );

                if !is_valid {
                    return Ok(Response::Exit("Invalid signature".to_owned()));
                    // Todo: procceed to refund
                }

                self.state = State::ValidEncSig(Value2 {
                    bob_keys: props.bob_keys,
                    bob_bch_recv: props.bob_bch_recv,
                    contract_pair: props.contract_pair,
                    shared_keypair: props.shared_keypair,
                    spend_bch: props.spend_bch,
                    outpoint: props.outpoint,
                    dec_sig,
                });
                return Ok(Response::Ok);
            }
            (_, _) => return Ok(Response::Err("invalid state-transition pair".to_owned())),
        }
    }
}
