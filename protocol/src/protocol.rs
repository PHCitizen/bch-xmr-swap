use crate::keys::KeyPublic;

#[derive(Debug)]
pub enum ExitCode {
    InvalidProof,
}

#[derive(Debug)]
pub enum Response {
    End,
    Continue,
    BchRawTx(String),
    XmrRawTx(String),
    RefundTx(String),
    // ? exit the program and dont proceed the swap. Exit Code enum?
    Exit(ExitCode),
}

#[derive(Debug)]
pub enum Transition {
    None,
    Keys(KeyPublic),
    Contract(String),
    BchTx(String),
    BchConfirmed,
    XmrTx(String),
    XmrConfirmed,
    EncSig(String),
    DecSig(String),
}

pub trait StateMachine {
    fn get_transition(&self) -> Transition;
    fn transition(&mut self, transition: Transition) -> Response;
}
