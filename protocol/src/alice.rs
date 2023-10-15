use crate::{
    keys::{KeyPublicWithoutProof, Keys},
    proof,
    protocol::{Action, ExitCode, Response, StateMachine, Transition},
};

#[derive(Debug)]
pub enum State {
    WaitingForKeys,
    WaitingForContract {
        keys: KeyPublicWithoutProof,
    },
    WaitingForBchTxHash {
        keys: KeyPublicWithoutProof,
    },
    WaitingForBchConf {
        keys: KeyPublicWithoutProof,
        bch_tx_hash: String,
    },
    WaitingForXmrTxHash {
        keys: KeyPublicWithoutProof,
        bch_tx_hash: String,
    },
    WaitingForEncSig {
        keys: KeyPublicWithoutProof,
        bch_tx_hash: String,
        xmr_tx_hash: String,
    },
    SwapSuccess {
        keys: KeyPublicWithoutProof,
        bch_tx_hash: String,
        xmr_tx_hash: String,
        enc_sig: String,
    },
    SwapFailed,
}

#[derive(derivative::Derivative)]
#[derivative(Debug)]
pub struct Alice {
    #[derivative(Debug = "ignore")]
    keys: Keys,
    pub state: State,
}

impl Alice {
    pub fn new() -> Self {
        Self {
            keys: Keys::random(),
            state: State::WaitingForKeys,
        }
    }
}

impl StateMachine for Alice {
    fn get_transition(&self) -> Transition {
        match &self.state {
            State::WaitingForKeys => Transition::Keys(self.keys.public()),
            State::WaitingForContract { .. } => Transition::Contract("".to_owned()),
            State::WaitingForBchTxHash { .. } => Transition::EncSig("".to_owned()),
            State::WaitingForBchConf { .. } => Transition::None,
            State::WaitingForXmrTxHash { .. } => Transition::None,
            State::WaitingForEncSig { .. } => Transition::XmrTxHash("".to_owned()),
            State::SwapSuccess { .. } => Transition::None,
            State::SwapFailed { .. } => Transition::None,
        }
    }

    fn transition(&mut self, transition: Transition) -> Response {
        match (&self.state, transition) {
            (State::WaitingForKeys, Transition::Keys(keys)) => {
                self.state = State::WaitingForContract { keys: keys.into() };
                return Response::Continue(Action::None);
            }
            (State::WaitingForContract { keys }, Transition::Contract(_)) => {
                // todo: ExitCode::ContractMismatch
                self.state = State::WaitingForBchTxHash {
                    keys: keys.to_owned(),
                };
                return Response::Continue(Action::None);
            }
            (State::WaitingForBchTxHash { keys }, Transition::BchTxHash(bch_tx_hash)) => {
                // todo: check if bch has correct amount
                // Action::BchTxHash
                self.state = State::WaitingForBchConf {
                    keys: keys.to_owned(),
                    bch_tx_hash,
                };
                return Response::Continue(Action::None);
            }

            (
                State::WaitingForBchConf {
                    keys,
                    bch_tx_hash: bch_tx,
                },
                Transition::BchConfirmed,
            ) => {
                // todo: check if xmr has correct amount
                // Action::InvalidTx

                self.state = State::WaitingForXmrTxHash {
                    keys: keys.to_owned(),
                    bch_tx_hash: bch_tx.to_owned(),
                };

                return Response::Continue(Action::WaitXmrConfirmation);
            }

            (
                State::WaitingForXmrTxHash {
                    keys,
                    bch_tx_hash: bch_tx,
                },
                Transition::XmrTxHash(xmr_tx_hash),
            ) => {
                // todo: if not confirmed, Action::WaitXmrConfirmation

                self.state = State::WaitingForEncSig {
                    keys: keys.to_owned(),
                    bch_tx_hash: bch_tx.to_owned(),
                    xmr_tx_hash: xmr_tx_hash.to_owned(),
                };

                return Response::Continue(Action::WaitForDecSig);
            }

            (
                State::WaitingForEncSig {
                    keys,
                    bch_tx_hash,
                    xmr_tx_hash,
                },
                Transition::EncSig(encsig),
            ) => {
                // If invalid decsig, return Action::WaitForDecSig
                self.state = State::SwapSuccess {
                    keys: keys.to_owned(),
                    bch_tx_hash: bch_tx_hash.to_owned(),
                    xmr_tx_hash: xmr_tx_hash.to_owned(),
                    enc_sig: encsig.to_owned(),
                };
                return Response::End(Action::SwapLockTx(vec![]));
            }

            (_, _) => return Response::Continue(Action::None),
        }
    }
}
