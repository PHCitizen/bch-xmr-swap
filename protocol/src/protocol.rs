use crate::keys::KeyPublic;

#[derive(Debug)]
pub enum ExitCode {
    InvalidProof,
    ContractMismatch,
    InvalidEncSig,
}

#[derive(Debug)]
pub enum Action {
    BchTxHash,
    WaitBchConfirmation,
    XmrTxHash,
    WaitXmrConfirmation,
    InvalidTx,
    WaitForDecSig,
    RefundTx(Vec<u8>),
    SwapLockTx(Vec<u8>),
    None,
}

#[derive(Debug)]
pub enum Response {
    /// do some action first before successfuly ending the swap
    End(Action),
    /// continue the swap and proceed with the next step
    /// the caller is responsible for doing some action
    Continue(Action),
    /// exit the program and don't proceed the swap
    Exit(ExitCode),
}

#[derive(Debug)]
pub enum Transition {
    None,
    Keys(KeyPublic),
    Contract(String),
    BchTxHash(String),
    BchConfirmed,
    XmrTxHash(String),
    XmrConfirmed,
    EncSig(String),
    DecSig(String),
}

pub trait StateMachine {
    fn get_transition(&self) -> Transition;
    fn transition(&mut self, transition: Transition) -> Response;
}
