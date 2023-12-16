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

use crate::utils::monero_view_pair;
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
    protocol::{Response, ResponseError, Swap, Transition},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Value0 {
    bob_keys: KeyPublicWithoutProof,
    bob_bch_recv: Vec<u8>,
    contract_pair: ContractPair,

    #[serde(with = "monero_view_pair")]
    shared_keypair: monero::ViewPair,
    spend_bch: bitcoincash::PublicKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Value1 {
    bob_keys: KeyPublicWithoutProof,
    bob_bch_recv: Vec<u8>,
    contract_pair: ContractPair,
    #[serde(with = "monero_view_pair")]
    shared_keypair: monero::ViewPair,
    spend_bch: bitcoincash::PublicKey,

    outpoint: OutPoint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Value2 {
    bob_keys: KeyPublicWithoutProof,
    bob_bch_recv: Vec<u8>,
    contract_pair: ContractPair,
    #[serde(with = "monero_view_pair")]
    shared_keypair: monero::ViewPair,
    spend_bch: bitcoincash::PublicKey,
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

// Api endpoints that will be exposed to bob
impl Swap<State> {
    pub fn get_keys(&self) -> KeyPublic {
        KeyPublic::from(self.keys.clone())
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
                let contract = ContractPair::create(
                    1000,
                    receiving.clone(),
                    keys.ves.clone(),
                    self.bch_recv.to_bytes().clone(),
                    self.keys.ves.public_key(&secp),
                );

                self.state = State::WithBobKeys(Value0 {
                    spend_bch: keys.spend_bch.clone(),
                    bob_bch_recv: receiving,
                    contract_pair: contract,
                    shared_keypair: monero::ViewPair {
                        view: self.keys.monero_view + keys.monero_view,
                        spend: monero::PublicKey::from_private_key(&self.keys.monero_spend)
                            + keys.monero_spend,
                    },
                    bob_keys: keys.into(),
                });

                return Response::Ok;
            }
            (
                State::WithBobKeys(props),
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
                return Response::WatchBchTx(bch_address);
            }

            (State::ContractMatch(props), Transition::BchConfirmedTx(transaction)) => {
                let mut outpoint = None;
                for (vout, txout) in transaction.output.iter().enumerate() {
                    if txout.value == self.bch_amount.to_sat()
                        && txout.script_pubkey.to_string()
                            == props.contract_pair.swaplock.cash_address()
                    {
                        outpoint = Some(bitcoincash::OutPoint {
                            txid: transaction.txid(),
                            vout: vout as u32,
                        });
                        break;
                    }
                }

                match outpoint {
                    None => return Response::Err(ResponseError::InvalidTransaction),
                    Some(outpoint) => {
                        self.state = State::BchLocked(Value1 {
                            bob_keys: props.bob_keys,
                            bob_bch_recv: props.bob_bch_recv,
                            contract_pair: props.contract_pair,
                            shared_keypair: props.shared_keypair,
                            spend_bch: props.spend_bch,

                            outpoint,
                        });
                        return Response::Ok;
                    }
                };
            }
            (State::BchLocked(props), Transition::EncSig(encsig)) => {
                let dec_sig =
                    AdaptorSignature::decrypt_signature(&self.keys.monero_spend, encsig.clone());

                {
                    // ? Check if the message by bob can unlock the swaplock contract
                    let alice_recv_hash = sha256::hash(&self.bch_recv.to_bytes());
                    let signer = props.bob_keys.ves.clone();
                    let message = alice_recv_hash.to_byte_array();

                    if !AdaptorSignature::verify(signer, &message, &dec_sig) {
                        return Response::Exit("Invalid signature".to_owned());
                        // Todo: procceed to refund
                    }
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
                return Response::Done;
            }
            (_, _) => return Response::Err(ResponseError::InvalidStateTransition),
        }
    }
}
