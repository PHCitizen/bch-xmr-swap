use crate::{
    contract::{Contract, ContractPair},
    keys::{bitcoin, KeyPrivate, KeyPublic, KeyPublicWithoutProof},
    proof,
    protocol::{Response, StateMachine, Transition},
};

#[derive(Debug, Clone)]
pub struct Value0 {
    bob_keys: KeyPublicWithoutProof,
    bob_bch_recv: Vec<u8>,
    contract_pair: ContractPair,
}

#[derive(Debug, Clone)]
pub struct Value1 {
    alice_keys: KeyPublicWithoutProof,
    alice_bch_recv: Vec<u8>,
    contract_pair: ContractPair,
    enc_sig: String,
}

#[derive(Debug, Clone)]
pub enum State {
    Init,
    WithBobKeys(Value0),
    ContractMatch(Value0),
    BchLocked {
        keys: KeyPublicWithoutProof,
        bch_tx_hash: String,
    },
    SwapSuccess {
        keys: KeyPublicWithoutProof,
        bch_tx_hash: String,
        enc_sig: String,
    },
    SwapFailed,
}

#[derive(Debug, Clone)]
pub struct Alice {
    pub alice_key: KeyPrivate,
    pub alice_bch_recv: Vec<u8>,

    pub xmr_amount: u64,
    pub bch_amount: u64,

    pub state: State,
}

impl Alice {
    pub fn get_keys(&self) -> KeyPublic {
        self.alice_key.to_public()
    }

    pub fn contract(&self) -> Option<String> {
        // todo
        if let State::WithBobKeys { .. } = &self.state {
            // return self.bch_tx_hash();
            return Some("".to_owned());
        }

        return None;
    }

    pub fn refunc_enc_sig(&self) -> Option<String> {
        // todo
        if let State::ContractMatch { .. } = &self.state {
            return Some(String::from(""));
        }

        return None;
    }
}

impl StateMachine for Alice {
    fn transition(&mut self, transition: Transition) -> Response {
        let current_state = self.state.clone();
        match (current_state, transition) {
            (State::Init, Transition::Msg0 { keys, receiving }) => {
                let is_valid_keys = proof::verify(
                    &keys.proof,
                    keys.spend_bch.clone(), //
                    keys.monero_spend,
                );

                if !is_valid_keys {
                    return Response::Exit("invalid proof".to_owned());
                }

                let contract = Contract::create(
                    1000,
                    receiving.clone(),
                    keys.ves.clone(),
                    self.alice_bch_recv.clone(),
                    self.alice_key.ves.public_key(),
                );

                self.state = State::WithBobKeys(Value0 {
                    bob_keys: keys.remove_proof(),
                    bob_bch_recv: receiving,
                    contract_pair: contract,
                });

                return Response::Ok;
            }
            (State::WithBobKeys(props), Transition::Contract { bch_address, .. }) => {
                if props.contract_pair.swaplock.cash_address() != bch_address {
                    return Response::Exit("contract mismatch".to_owned());
                }
                // todo: match for xmr. ExitCode::ContractMismatch

                self.state = State::ContractMatch(props);

                return Response::Ok;
            }
            (State::ContractMatch(props), Transition::CheckBch) => {
                // todo
                // - check if bch has correct amount, address, confirmation
                //      if true - show xmr locking address

                self.state = State::BchLocked {
                    keys: props.bob_keys.to_owned(),
                    bch_tx_hash: "".to_owned(),
                };
                return Response::Ok;
            }
            (State::BchLocked { keys, bch_tx_hash }, Transition::EncSig(encsig)) => {
                // TODO
                // If invalid decsig, Error
                // Else claim bch
                self.state = State::SwapSuccess {
                    keys: keys.to_owned(),
                    bch_tx_hash: bch_tx_hash.to_owned(),
                    enc_sig: encsig.to_owned(),
                };
                return Response::Exit("Success".to_owned());
            }
            (_, _) => return Response::Err("invalid state-transition pair".to_owned()),
        }
    }
}
