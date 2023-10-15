use crate::{
    contract::{Contract, ContractPair},
    keys::{KeyPublic, KeyPublicWithoutProof, Keys},
    proof,
    protocol::{Response, StateMachine, Transition},
};

#[derive(Debug)]
pub enum State {
    // If alice send keys with invalid proof, Response::Exit
    // Else
    //      proceed to State::WaitingForContract
    //      return Response::Continue
    WaitingForKeys,

    // If my_contract != alice_contract, Response::Exit
    // Else
    //      proceed to State::WaitingForEncSig
    //      return Response::Continue
    WaitingForContract {
        alice_keys: KeyPublicWithoutProof,
    },

    // If alice EncSig cannot unlock Refund.cash, return Response::Exit
    // Else:
    //      proceed to State::WaitingForXmrUnlockedBal
    //      return Action::BchTxHash
    WaitingForEncSig {
        alice_keys: KeyPublicWithoutProof,
    },

    // If Locked Monero does not satisfy initial "swap requirements".
    //    proceed to State::SwapFailed
    //    return Response::RefundTx
    // Else proceed to State::WaitingForXmrConf
    WaitingForXmrUnlockedBal {
        alice_keys: KeyPublicWithoutProof,
        enc_sig: String,
    },

    // Recover EncKey
    // If we can't recover, return Response::Continue
    //      ? alice may send junk transition. just ignore it
    //      ? the caller must impliment subscription directly from blockchain
    // Else:
    //      proceed to State::SwapSuccess
    //      return Response::End
    WaitingForDecSig {
        keys: KeyPublicWithoutProof,
        enc_sig: String,
    },

    SwapSuccess {
        keys: KeyPublicWithoutProof,
        enc_sig: String,
        decsig: String,
        monero_keypair: String,
    },
    SwapFailed,
}

#[derive(derivative::Derivative)]
#[derivative(Debug)]
pub struct Bob {
    #[derivative(Debug = "ignore")]
    pub keys: Keys,
    #[derivative(Debug = "ignore")]
    pub contract: Option<ContractPair>,
    pub refund_bch: Vec<u8>, // locking_bytecode

    pub xmr_amount: u64,
    pub bch_amount: u64,

    pub state: State,
}

impl Bob {
    pub fn new(refund_bch: Vec<u8>, xmr_amount: u64, bch_amount: u64) -> Self {
        Self {
            keys: Keys::random(),
            state: State::WaitingForKeys,
            contract: None,

            refund_bch,
            xmr_amount,
            bch_amount,
        }
    }
}

// Api endpoints that will be exposed to alice
impl Bob {
    pub fn get_keys(&self) -> Option<KeyPublic> {
        if let State::WaitingForKeys = self.state {
            let (proof, (spend_bch, _)) = self.keys.prove();
            return Some(KeyPublic {
                locking_bytecode: self.refund_bch.clone(),
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
        if let State::WaitingForContract { .. } = &self.state {
            // return self.bch_tx_hash();
            return Some("".to_owned());
        }

        return None;
    }

    pub fn swaplock_enc_sig(&self) -> Option<String> {
        if let State::WaitingForDecSig { .. } = &self.state {
            return Some(String::from(""));
        }

        return None;
    }
}

impl StateMachine for Bob {
    fn transition(&mut self, transition: Transition) -> Response {
        match (&self.state, transition) {
            (State::WaitingForKeys, Transition::Keys(alice_keys)) => {
                let is_valid_keys = proof::verify(
                    &alice_keys.proof,
                    (
                        alice_keys.spend_bch.clone().into(),
                        alice_keys.spend.clone().into(),
                    ),
                );

                if !is_valid_keys {
                    return Response::Exit("invalid proof".to_owned());
                }

                let contract = Contract::create(
                    1000,
                    self.refund_bch.clone(),
                    self.keys.ves.public_key(),
                    alice_keys.locking_bytecode.clone(),
                    alice_keys.ves.clone(),
                );

                self.contract = Some(contract);
                self.state = State::WaitingForContract {
                    alice_keys: KeyPublicWithoutProof {
                        locking_bytecode: alice_keys.locking_bytecode,
                        spend: alice_keys.spend,
                        spend_bch: alice_keys.spend_bch,
                        ves: alice_keys.ves,
                        view: alice_keys.view,
                    },
                };

                return Response::Ok;
            }

            (
                State::WaitingForContract { alice_keys },
                Transition::Contract { bch_address, .. },
            ) => {
                if self.contract.clone().unwrap().swaplock.cash_address() != bch_address {
                    return Response::Exit("contract mismatch".to_owned());
                }
                // todo: match for xmr. ExitCode::ContractMismatch

                self.state = State::WaitingForEncSig {
                    alice_keys: alice_keys.to_owned(),
                };
                return Response::Ok;
            }

            (State::WaitingForEncSig { alice_keys }, Transition::EncSig(enc_sig)) => {
                // TODO:
                // - check if enc_sig can unlock refund
                //      - if unlocked print contract address
                //      - if not, exit

                self.state = State::WaitingForXmrUnlockedBal {
                    alice_keys: alice_keys.to_owned(),
                    enc_sig: enc_sig.to_owned(),
                };

                return Response::Ok;
            }

            (
                State::WaitingForXmrUnlockedBal {
                    alice_keys,
                    enc_sig,
                },
                Transition::CheckXmr,
            ) => {
                // Todo:
                // - wait for contract to have correct unlocked balance
                //      - if it has, send Swaplock enc sig

                self.state = State::WaitingForDecSig {
                    keys: alice_keys.to_owned(),
                    enc_sig: enc_sig.to_owned(),
                };

                return Response::Ok;
            }

            (State::WaitingForDecSig { keys, enc_sig }, Transition::DecSig(decsig)) => {
                self.state = State::SwapSuccess {
                    keys: keys.to_owned(),
                    enc_sig: enc_sig.to_owned(),
                    decsig,
                    monero_keypair: "".to_owned(),
                };
                return Response::Exit("success".to_owned());
            }

            (_, _) => return Response::Err("invalid state-transition pair".to_owned()),
        }
    }
}
