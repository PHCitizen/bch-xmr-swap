use protocol::{
    bob::{Bob, State},
    keys::Keys,
    protocol::{Action, ExitCode, Response, StateMachine, Transition},
};

fn bob() {
    let mut bob = Bob::new();

    // bob allow to give KeyPublic to alice
    dbg!(bob.get_transition());

    let response = bob.transition(Transition::Keys(Keys::random().public()));
    match response {
        // alice send with invalid proof
        Response::Exit(ExitCode::InvalidProof) => {}
        // continue the flow
        Response::Continue(Action::None) => {}
        _ => {}
    };

    let response = bob.transition(Transition::Contract("".to_owned()));
    match response {
        // alice and bob contract does not match
        Response::Exit(ExitCode::ContractMismatch) => {}
        // continue the flow
        Response::Continue(Action::None) => {}
        _ => {}
    };

    let response = bob.transition(Transition::EncSig("".to_owned()));
    match response {
        // alice encsig cannot be used to unlock Refund.cash
        Response::Exit(ExitCode::InvalidEncSig) => {}
        // continue the flow
        Response::Continue(Action::BchTxHash) => {
            // get tx from user and use it as transition
        }
        _ => {}
    };

    // State::WaitingForBchTxHash
    let response = bob.transition(Transition::BchTxHash("".to_owned()));
    match response {
        // invalid bch tx hash, ask user to retry
        Response::Continue(Action::BchTxHash) => {}
        // continue the flow
        Response::Continue(Action::None) => {}
        _ => {}
    };

    // State::WaitingForXmrTx
    let response = bob.transition(Transition::XmrTxHash("".to_owned()));
    match response {
        // the monero tx received is invalid, ask alice to retry
        Response::Continue(Action::InvalidTx) => {}
        // continue the flow
        Response::Continue(Action::None) => {}
        _ => {}
    };

    // State::WaitingForXmrConf
    let response = bob.transition(Transition::XmrConfirmed);
    match response {
        // the monero tx received is not confirmed, ask alice to retry after some time
        Response::Continue(Action::WaitXmrConfirmation) => {}
        // continue the flow
        Response::Continue(Action::None) => {}
        _ => {}
    };

    // State::WaitingForDecSig
    let response = bob.transition(Transition::DecSig("".to_owned()));
    match response {
        // the decsig tx received is invalid, ask alice to retry
        Response::Continue(Action::WaitForDecSig) => {}
        // continue the flow
        Response::Continue(Action::None) => {}
        _ => {}
    };

    // State::SwapSuccess
    match bob.state {
        State::SwapSuccess { .. } => println!("> Bob Success"),
        _ => panic!("Bob state not ended successfully"),
    }
}

fn main() {
    bob()
}
