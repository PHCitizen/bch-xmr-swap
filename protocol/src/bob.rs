use crate::{
    contract::{Contract, ContractPair},
    keys::{KeyPrivate, KeyPublic, KeyPublicWithoutProof},
    proof,
    protocol::{Response, StateMachine, Transition},
};

#[derive(Debug, Clone)]
pub struct Value0 {
    alice_keys: KeyPublicWithoutProof,
    alice_bch_recv: Vec<u8>,
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
    // If alice send keys with invalid proof, Response::Exit
    // Else
    //      proceed to State::WaitingForContract
    //      return Response::Continue
    // WaitingForKeys,

    // If my_contract != alice_contract, Response::Exit
    // Else
    //      proceed to State::WaitingForEncSig
    //      return Response::Continue
    WithAliceKey(Value0),

    // If alice EncSig cannot unlock Refund.cash, return Response::Exit
    // Else:
    //      proceed to State::WaitingForXmrUnlockedBal
    //      return Action::BchTxHash
    ContractMatch(Value0),

    // If Locked Monero does not satisfy initial "swap requirements".
    //    proceed to State::SwapFailed
    //    return Response::RefundTx
    // Else proceed to State::WaitingForXmrConf
    VerifiedEncSig(Value1),

    // Recover EncKey
    // If we can't recover, return Response::Continue
    //      ? alice may send junk transition. just ignore it
    //      ? the caller must impliment subscription directly from blockchain
    // Else:
    //      proceed to State::SwapSuccess
    //      return Response::End
    MoneroLocked(Value1),

    SwapSuccess {
        keys: KeyPublicWithoutProof,
        alice_bch_recv: Vec<u8>,
        enc_sig: String,
        decsig: String,
        monero_keypair: String,
    },
    SwapFailed,
}

#[derive(Debug)]
pub struct Bob {
    pub bob_keys: KeyPrivate,
    pub bob_bch_recv: Vec<u8>,

    pub xmr_amount: u64,
    pub bch_amount: u64,

    pub state: State,
}

// Api endpoints that will be exposed to alice
impl Bob {
    pub fn get_keys(&self) -> KeyPublic {
        self.bob_keys.to_public()
    }

    pub fn contract(&self) -> Option<String> {
        if let State::WithAliceKey { .. } = &self.state {
            // return self.bch_tx_hash();
            return Some("".to_owned());
        }

        return None;
    }

    pub fn swaplock_enc_sig(&self) -> Option<String> {
        if let State::MoneroLocked { .. } = &self.state {
            return Some(String::from(""));
        }

        return None;
    }
}

impl StateMachine for Bob {
    fn transition(&mut self, transition: Transition) -> Response {
        let current_state = self.state.clone();
        match (current_state, transition) {
            (State::Init, Transition::Msg0 { keys, receiving }) => {
                let is_valid_keys =
                    proof::verify(&keys.proof, keys.spend_bch.clone(), keys.monero_spend);

                if !is_valid_keys {
                    return Response::Exit("invalid proof".to_owned());
                }

                let contract = ContractPair::create(
                    1000,
                    self.bob_bch_recv.clone(),
                    self.bob_keys.ves.public_key(),
                    receiving.clone(),
                    keys.ves.clone(),
                );

                self.state = State::WithAliceKey(Value0 {
                    alice_keys: keys.remove_proof(),
                    alice_bch_recv: receiving,
                    contract_pair: contract,
                });

                return Response::Ok;
            }
            (State::WithAliceKey(props), Transition::Contract { bch_address, .. }) => {
                if props.contract_pair.swaplock.cash_address() != bch_address {
                    return Response::Exit("contract mismatch".to_owned());
                }
                // todo: match for xmr. ExitCode::ContractMismatch

                self.state = State::ContractMatch(props);
                return Response::Ok;
            }
            (State::ContractMatch(props), Transition::EncSig(enc_sig)) => {
                // TODO:
                // - check if enc_sig can unlock refund
                //      - if unlocked print contract address
                //      - if not, exit

                self.state = State::VerifiedEncSig(Value1 {
                    alice_keys: props.alice_keys,
                    alice_bch_recv: props.alice_bch_recv,
                    contract_pair: props.contract_pair,
                    enc_sig: enc_sig,
                });

                return Response::Ok;
            }
            // the state above will give the address of SwapLock contract
            // the user is responsible for funding it
            // even user are not done in funding or we have insufficient confirmation
            //      we assume that it's already done, and we proceed to waiting xmrlocked
            (State::VerifiedEncSig(props), Transition::CheckXmr) => {
                // Todo:
                // - wait for contract to have correct unlocked balance
                //      - if it has, send Swaplock enc sig

                self.state = State::MoneroLocked(props);

                return Response::Ok;
            }
            (State::MoneroLocked(props), Transition::DecSig(decsig)) => {
                self.state = State::SwapSuccess {
                    keys: props.alice_keys,
                    alice_bch_recv: props.alice_bch_recv,
                    enc_sig: props.enc_sig,
                    decsig,
                    monero_keypair: "".to_owned(),
                };
                return Response::Exit("success".to_owned());
            }
            (_, _) => return Response::Err("invalid state-transition pair".to_owned()),
        }
    }
}
