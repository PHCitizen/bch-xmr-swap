use crate::{
    keys::{KeyPublicWithoutProof, Keys},
    proof,
    protocol::{ExitCode, Response, StateMachine, Transition},
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
    //      proceed to State::WaitingForXmrTx
    //      return tx sending bch to Swaplock.cash
    //          the caller is responsible in brodcasting the tx
    WaitingForEncSig {
        keys: KeyPublicWithoutProof,
    },

    // If Locked Monero does not satisfy initial "swap requirements".
    //    proceed to State::SwapFailed
    //    return Response::RefundTx
    // Else proceed to State::WaitingForXmrConf
    WaitingForXmrTx {
        keys: KeyPublicWithoutProof,
        enc_sig: String,
        bch_tx: String,
    },

    // Verify that monero has enough confirmation.
    // If not, return Response::Continue
    // Else proceed to State::WaitingForDecSig
    WaitingForXmrConf {
        keys: KeyPublicWithoutProof,
        enc_sig: String,
        bch_tx: String,
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
        bch_tx: String,
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
    pub state: State,
    #[derivative(Debug = "ignore")]
    keys: Keys,
}

impl StateMachine for Bob {
    fn get_transition(&self) -> Transition {
        match &self.state {
            State::WaitingForKeys => Transition::Keys(self.keys.public()),
            State::WaitingForContract { .. } => Transition::None,
            State::WaitingForEncSig { .. } => Transition::None,
            State::WaitingForXmrTx { bch_tx, .. } => Transition::BchTx(bch_tx.clone()),
            State::WaitingForXmrConf { .. } => Transition::None,
            State::WaitingForDecSig { .. } => Transition::EncSig(String::from("")),
            State::SwapSuccess { .. } => Transition::None,
            State::SwapFailed => Transition::None,
        }
    }

    fn transition(&mut self, transition: Transition) -> Response {
        match (&self.state, transition) {
            (State::WaitingForKeys, Transition::Keys(keys)) => {
                let is_valid_keys = proof::verify(
                    &keys.proof,
                    (keys.spend_bch.clone().into(), keys.spend.clone().into()),
                );

                if !is_valid_keys {
                    return Response::Exit(ExitCode::InvalidProof);
                }

                self.state = State::WaitingForContract { keys: keys.into() };
                return Response::Continue;
            }
            (State::WaitingForContract { keys }, Transition::Contract(_)) => {
                // todo: Check Contract
                self.state = State::WaitingForEncSig {
                    keys: keys.to_owned(),
                };
                return Response::Continue;
            }
            (State::WaitingForEncSig { keys }, Transition::EncSig(enc_sig)) => {
                // todo: check if enc_sig can unlock Refund.cash
                // send bch to SwapLock contract
                let bch_tx = String::from("value");
                self.state = State::WaitingForXmrTx {
                    keys: keys.to_owned(),
                    enc_sig,
                    bch_tx,
                };
                return Response::BchRawTx(String::from("BCH TX"));
            }
            (
                State::WaitingForXmrTx {
                    keys,
                    enc_sig,
                    bch_tx,
                },
                Transition::XmrTx(tx),
            ) => {
                // todo: check if xmr has correct amount
                self.state = State::WaitingForDecSig {
                    keys: keys.to_owned(),
                    enc_sig: enc_sig.to_owned(),
                    bch_tx: bch_tx.to_owned(),
                    xmr_tx: tx,
                };

                return Response::Continue;
            }
            (
                State::WaitingForDecSig {
                    keys,
                    enc_sig,
                    bch_tx,
                    xmr_tx,
                },
                Transition::DecSig(decsig),
            ) => {
                self.state = State::SwapSuccess {
                    keys: keys.to_owned(),
                    enc_sig: enc_sig.to_owned(),
                    bch_tx: bch_tx.to_owned(),
                    xmr_tx: xmr_tx.to_owned(),
                    decsig,
                };
                return Response::End;
            }
            (_, _) => return Response::Continue,
        }
    }
}
