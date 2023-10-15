use crate::keys::KeyPublic;

#[derive(Debug)]
pub enum Response {
    Ok,
    BchTxHash(String),
    SubscribeXmrTx(String),

    Err(String),
    Exit(String),
}

#[derive(Debug)]
pub enum Transition {
    None,
    Keys(KeyPublic),
    Contract {
        bch_address: String,
        xmr_address: String,
    },
    BchTxHash(String),
    BchConfirmed,
    XmrTxHash(String),
    XmrConfirmed,
    EncSig(String),
    DecSig(String),

    CheckXmr,
    CheckBch,
}

pub trait StateMachine {
    fn transition(&mut self, transition: Transition) -> Response;
}
