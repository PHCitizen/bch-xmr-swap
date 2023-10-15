use crate::{
    keys::{KeyPublicWithoutProof, Keys},
    proof,
    protocol::{Action, ExitCode, Response, StateMachine, Transition},
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
        keys: KeyPublicWithoutProof,
    },

    // If alice EncSig cannot unlock Refund.cash, return Response::Exit
    // Else:
    //      proceed to State::WaitingForBchTxHash
    //      return Action::BchTxHash
    WaitingForEncSig {
        keys: KeyPublicWithoutProof,
    },

    // If tx has correct amount and destination
    //      proceed to State::WaitingForXmrTx
    // Else
    //      return Action::BchTxHash - ask for new tx
    WaitingForBchTxHash {
        keys: KeyPublicWithoutProof,
        enc_sig: String,
    },

    // If Locked Monero does not satisfy initial "swap requirements".
    //    proceed to State::SwapFailed
    //    return Response::RefundTx
    // Else proceed to State::WaitingForXmrConf
    WaitingForXmrTxHash {
        keys: KeyPublicWithoutProof,
        enc_sig: String,
        bch_tx_hash: String,
    },

    // Verify that monero has enough confirmation.
    // If not, return Response::Continue
    // Else proceed to State::WaitingForDecSig
    WaitingForXmrConf {
        keys: KeyPublicWithoutProof,
        enc_sig: String,
        bch_tx_hash: String,
        xmr_tx: String,
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
        bch_tx_hash: String,
        xmr_tx: String,
    },

    SwapSuccess {
        keys: KeyPublicWithoutProof,
        enc_sig: String,
        bch_tx: String,
        xmr_tx: String,
        decsig: String,
    },
    SwapFailed,
}

#[derive(derivative::Derivative)]
#[derivative(Debug)]
pub struct Bob {
    #[derivative(Debug = "ignore")]
    keys: Keys,
    pub state: State,
}

impl Bob {
    pub fn new() -> Self {
        Self {
            keys: Keys::random(),
            state: State::WaitingForKeys,
        }
    }
}

impl StateMachine for Bob {
    fn get_transition(&self) -> Transition {
        match &self.state {
            State::WaitingForKeys => Transition::Keys(self.keys.public()),
            State::WaitingForContract { .. } => Transition::None,
            State::WaitingForEncSig { .. } => Transition::None,
            State::WaitingForBchTxHash { .. } => Transition::None,
            State::WaitingForXmrTxHash { bch_tx_hash, .. } => {
                Transition::BchTxHash(bch_tx_hash.clone())
            }
            State::WaitingForXmrConf { .. } => Transition::None,
            State::WaitingForDecSig { .. } => Transition::EncSig(String::from("")),
            State::SwapSuccess { .. } => Transition::None,
            State::SwapFailed => Transition::None,
        }
    }

    fn transition(&mut self, transition: Transition) -> Response {
        match (&self.state, transition) {
            (State::WaitingForKeys, Transition::Keys(keys)) => {
                // let is_valid_keys = proof::verify(
                //     &keys.proof,
                //     (keys.spend_bch.clone().into(), keys.spend.clone().into()),
                // );

                // if !is_valid_keys {
                //     return Response::Exit(ExitCode::InvalidProof);
                // }

                self.state = State::WaitingForContract { keys: keys.into() };
                return Response::Continue(Action::None);
            }
            (State::WaitingForContract { keys }, Transition::Contract(_)) => {
                // todo: ExitCode::ContractMismatch
                self.state = State::WaitingForEncSig {
                    keys: keys.to_owned(),
                };
                return Response::Continue(Action::None);
            }
            (State::WaitingForEncSig { keys }, Transition::EncSig(enc_sig)) => {
                // todo: ExitCode::EncSigFailed
                let bch_tx = String::from("value");
                self.state = State::WaitingForBchTxHash {
                    keys: keys.to_owned(),
                    enc_sig,
                };
                return Response::Continue(Action::BchTxHash);
            }
            (State::WaitingForBchTxHash { keys, enc_sig }, Transition::BchTxHash(bch_tx_hash)) => {
                // todo: check if bch has correct amount
                // Action::BchTxHash
                self.state = State::WaitingForXmrTxHash {
                    keys: keys.to_owned(),
                    enc_sig: enc_sig.to_owned(),
                    bch_tx_hash,
                };
                return Response::Continue(Action::None);
            }

            (
                State::WaitingForXmrTxHash {
                    keys,
                    enc_sig,
                    bch_tx_hash: bch_tx,
                },
                Transition::XmrTxHash(tx),
            ) => {
                // todo: check if xmr has correct amount
                // Action::InvalidTx

                self.state = State::WaitingForXmrConf {
                    keys: keys.to_owned(),
                    enc_sig: enc_sig.to_owned(),
                    bch_tx_hash: bch_tx.to_owned(),
                    xmr_tx: tx,
                };

                return Response::Continue(Action::WaitXmrConfirmation);
            }

            (
                State::WaitingForXmrConf {
                    keys,
                    enc_sig,
                    bch_tx_hash: bch_tx,
                    xmr_tx,
                },
                Transition::XmrConfirmed,
            ) => {
                // todo: if not confirmed, Action::WaitXmrConfirmation

                self.state = State::WaitingForDecSig {
                    keys: keys.to_owned(),
                    enc_sig: enc_sig.to_owned(),
                    bch_tx_hash: bch_tx.to_owned(),
                    xmr_tx: xmr_tx.to_owned(),
                };

                return Response::Continue(Action::WaitForDecSig);
            }

            (
                State::WaitingForDecSig {
                    keys,
                    enc_sig,
                    bch_tx_hash,
                    xmr_tx,
                },
                Transition::DecSig(decsig),
            ) => {
                // If invalid decsig, return Action::WaitForDecSig
                self.state = State::SwapSuccess {
                    keys: keys.to_owned(),
                    enc_sig: enc_sig.to_owned(),
                    bch_tx: bch_tx_hash.to_owned(),
                    xmr_tx: xmr_tx.to_owned(),
                    decsig,
                };
                return Response::End(Action::None);
            }
            (_, _) => return Response::Continue(Action::None),
        }
    }
}
