use bitcoin_hashes::{sha256::Hash as sha256, Hash};
use ecdsa_fun::{adaptor::EncryptedSignature, Signature};

use crate::{
    adaptor_signature::AdaptorSignature,
    blockchain::BchProvider,
    contract::ContractPair,
    keys::{bitcoin, KeyPublic, KeyPublicWithoutProof},
    proof,
    protocol::{Response, Swap, Transition},
};

#[derive(Debug, Clone)]
pub struct Value0 {
    alice_keys: KeyPublicWithoutProof,
    alice_bch_recv: Vec<u8>,
    contract_pair: ContractPair,

    shared_keypair: monero::ViewPair,
    spend_bch: bitcoin::PublicKey,
}

#[derive(Debug, Clone)]
pub struct Value1 {
    alice_keys: KeyPublicWithoutProof,
    alice_bch_recv: Vec<u8>,
    contract_pair: ContractPair,

    shared_keypair: monero::ViewPair,
    spend_bch: bitcoin::PublicKey,

    #[allow(dead_code)]
    refund_unlocker: Signature,
    restore_height: u64,
}

#[derive(Debug, Clone)]
pub enum State {
    Init,
    WithAliceKey(Value0),
    ContractMatch(Value0),
    VerifiedEncSig(Value1),
    MoneroLocked(Value1),
    SwapSuccess(monero::KeyPair),
    SwapFailed,
}

// Api endpoints that will be exposed to alice
impl Swap<State> {
    pub fn get_keys(&self) -> KeyPublic {
        self.keys.to_public()
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
    /// Ok variant can contain handled/known error
    /// Err variant has unhandle/internal error
    pub async fn transition(&mut self, transition: Transition) -> anyhow::Result<Response> {
        let current_state = self.state.clone();
        match (current_state, transition) {
            (State::Init, Transition::Msg0 { keys, receiving }) => {
                let is_valid_keys =
                    proof::verify(&keys.proof, keys.spend_bch.clone(), keys.monero_spend);

                if !is_valid_keys {
                    return Ok(Response::Exit("invalid proof".to_owned()));
                }

                let contract_pair = ContractPair::create(
                    1000,
                    self.bch_recv.clone().into_bytes(),
                    self.keys.ves.public_key(),
                    receiving.clone(),
                    keys.ves.clone(),
                );

                self.state = State::WithAliceKey(Value0 {
                    alice_keys: keys.remove_proof(),
                    alice_bch_recv: receiving,
                    contract_pair,

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
                State::WithAliceKey(props),
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
                    return Ok(Response::Exit("Invalid signature".to_owned()));
                }

                let restore_height = self.xmr_daemon.get_block_count().await?.get();
                self.xmr_wallet
                    .generate_from_keys(monero_rpc::GenerateFromKeysArgs {
                        restore_height: Some(restore_height),
                        filename: format!("view_{}", self.id),
                        address: monero::Address::from_viewpair(
                            self.xmr_network,
                            &props.shared_keypair,
                        ),
                        spendkey: None,
                        viewkey: props.shared_keypair.view,
                        password: "".to_owned(),
                        autosave_current: Some(true),
                    })
                    .await?;

                println!("=============================");
                println!(
                    "Send BCH here: {} Amount: {}",
                    props.contract_pair.swaplock.cash_address(),
                    self.bch_amount
                );
                println!("=============================");

                self.state = State::VerifiedEncSig(Value1 {
                    alice_keys: props.alice_keys,
                    alice_bch_recv: props.alice_bch_recv,
                    contract_pair: props.contract_pair,
                    shared_keypair: props.shared_keypair,
                    spend_bch: props.spend_bch,

                    refund_unlocker: dec_sig,
                    restore_height,
                });

                return Ok(Response::Ok);
            }
            // the state above will give the address of SwapLock contract
            // the user is responsible for funding it
            // even user are not done in funding or we have insufficient confirmation
            //      we assume that it's already done, and we proceed to waiting xmrlocked
            (State::VerifiedEncSig(props), Transition::CheckXmr) => {
                self.xmr_wallet
                    .open_wallet(format!("view_{}", self.id), None)
                    .await?;

                let balance = self.xmr_wallet.get_balance(0, None).await?;
                if balance.unlocked_balance >= self.xmr_amount {
                    self.state = State::MoneroLocked(props);
                }

                return Ok(Response::Ok);
            }
            (State::MoneroLocked(props), Transition::CheckBch) => {
                let transactions = self
                    .bch_provider
                    .get_address_history(&props.contract_pair.swaplock.cash_address())
                    .await?
                    .result;

                for transaction in transactions {
                    let tx = self.bch_provider.get_tx(&transaction.tx_hash).await?.result;
                    if tx.vin.len() != 1 || tx.vout.len() != 1 {
                        continue;
                    }

                    // Contract.swap
                    let vout_hex = hex::decode(tx.vout[0].script_pub_key.hex.clone())?;
                    if vout_hex == props.alice_bch_recv {
                        println!("Alice Swap Tx found. TxHash: {}", tx.hash);
                        let mut signature = [0u8; 64];
                        signature.copy_from_slice(&vout_hex[0..64].to_vec());
                        let dec_sig = Signature::from_bytes(signature).unwrap();
                        let alice_spend = AdaptorSignature::recover_decryption_key(
                            props.alice_keys.spend_bch.clone(),
                            dec_sig,
                            self.swaplock_enc_sig().unwrap(),
                        );

                        let key_pair = monero::KeyPair {
                            view: props.shared_keypair.view,
                            spend: self.keys.monero_spend + alice_spend,
                        };

                        self.xmr_wallet
                            .generate_from_keys(monero_rpc::GenerateFromKeysArgs {
                                restore_height: Some(props.restore_height),
                                filename: format!("spend_{}", self.id),
                                address: monero::Address::from_keypair(self.xmr_network, &key_pair),
                                spendkey: Some(key_pair.spend),
                                viewkey: key_pair.view,
                                password: "".to_owned(),
                                autosave_current: Some(true),
                            })
                            .await?;

                        self.state = State::SwapSuccess(key_pair);

                        return Ok(Response::Exit("success".to_owned()));
                    }

                    // Contract.forwardToRefund
                    if vout_hex == props.contract_pair.refund.locking_script() {
                        todo!("Contract.forwardToRefund");
                    }
                }

                return Ok(Response::Exit("success".to_owned()));
            }
            (_, _) => return Ok(Response::Err("invalid state-transition pair".to_owned())),
        }
    }
}
