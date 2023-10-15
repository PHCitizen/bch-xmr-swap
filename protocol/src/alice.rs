use crate::{
    contract::{Contract, ContractPair},
    keys::{KeyPublic, KeyPublicWithoutProof, Keys},
    proof,
    protocol::{Response, StateMachine, Transition},
};

#[derive(Debug)]
pub enum State {
    WaitingForKeys,
    WaitingForContract {
        keys: KeyPublicWithoutProof,
    },
    WaitingForLockedBch {
        keys: KeyPublicWithoutProof,
    },
    WaitingForEncSig {
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

#[derive(derivative::Derivative)]
#[derivative(Debug)]
pub struct Alice {
    #[derivative(Debug = "ignore")]
    keys: Keys,

    pub contract: Option<ContractPair>,
    pub receiving_bch: Vec<u8>, // locking_bytecode

    pub xmr_amount: u64,
    pub bch_amount: u64,

    pub state: State,
}

impl Alice {
    pub fn new(receiving_bch: Vec<u8>, xmr_amount: u64, bch_amount: u64) -> Self {
        Self {
            keys: Keys::random(),
            state: State::WaitingForKeys,
            contract: None,

            receiving_bch,
            xmr_amount,
            bch_amount,
        }
    }
}

impl Alice {
    pub fn get_keys(&self) -> Option<KeyPublic> {
        if let State::WaitingForKeys = self.state {
            let (proof, (spend_bch, _)) = self.keys.prove();
            return Some(KeyPublic {
                locking_bytecode: self.receiving_bch.clone(),
                ves: self.keys.ves.public_key(),
                view: self.keys.view.clone(),
                spend: self.keys.spend.public_key(),
                spend_bch,
                proof,
            });
        }

        return None;
    }

    pub fn contract(&self) -> Option<String> {
        // todo
        if let State::WaitingForContract { .. } = &self.state {
            // return self.bch_tx_hash();
            return Some("".to_owned());
        }

        return None;
    }

    pub fn refunc_enc_sig(&self) -> Option<String> {
        // todo
        if let State::WaitingForLockedBch { .. } = &self.state {
            return Some(String::from(""));
        }

        return None;
    }
}

impl StateMachine for Alice {
    fn transition(&mut self, transition: Transition) -> Response {
        match (&self.state, transition) {
            (State::WaitingForKeys, Transition::Keys(keys)) => {
                let is_valid_keys = proof::verify(
                    &keys.proof,
                    (keys.spend_bch.clone().into(), keys.spend.clone().into()),
                );

                if !is_valid_keys {
                    return Response::Exit("invalid proof".to_owned());
                }

                let contract = Contract::create(
                    1000,
                    keys.locking_bytecode.clone(),
                    keys.ves.clone(),
                    self.receiving_bch.clone(),
                    self.keys.ves.public_key(),
                );

                self.contract = Some(contract);
                self.state = State::WaitingForContract {
                    keys: KeyPublicWithoutProof {
                        locking_bytecode: keys.locking_bytecode,
                        spend: keys.spend,
                        spend_bch: keys.spend_bch,
                        ves: keys.ves,
                        view: keys.view,
                    },
                };

                return Response::Ok;
            }
            (State::WaitingForContract { keys }, Transition::Contract { bch_address, .. }) => {
                if self.contract.clone().unwrap().swaplock.cash_address() != bch_address {
                    return Response::Exit("contract mismatch".to_owned());
                }
                // todo: match for xmr. ExitCode::ContractMismatch

                self.state = State::WaitingForLockedBch {
                    keys: keys.to_owned(),
                };

                return Response::Ok;
            }
            (State::WaitingForLockedBch { keys }, Transition::CheckBch) => {
                // todo
                // - check if bch has correct amount, address, confirmation
                //      if true - show xmr locking address

                self.state = State::WaitingForEncSig {
                    keys: keys.to_owned(),
                    bch_tx_hash: "".to_owned(),
                };
                return Response::Ok;
            }
            (State::WaitingForEncSig { keys, bch_tx_hash }, Transition::EncSig(encsig)) => {
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
