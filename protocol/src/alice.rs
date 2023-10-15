// use crate::{
//     bob::BobTransition,
//     keys::{KeyPublic, KeyPublicWithoutProof, Keys},
//     proof, Response, StateMachine,
// };

// #[derive(Debug)]
// pub enum AliceTransition {
//     BobKeys(KeyPublic),
//     BCHTx(String),
//     BCHLocked,
//     XMRLocked,
//     EncSig(String),
// }

// #[derive(Debug, Clone)]
// pub enum AliceState {
//     WaitingForBobKeys,
//     WaitingForBCHTx {
//         bob_keys: KeyPublicWithoutProof,
//     },
//     WaitingForBCHConfirmation {
//         bob_keys: KeyPublicWithoutProof,
//         bch_tx: String,
//     },
//     WaitingForXMRConfirmation {
//         bob_keys: KeyPublicWithoutProof,
//         bch_tx: String,
//         xmr_tx: String,
//     },
//     WaitingForSwapLockEncSig {
//         bob_keys: KeyPublicWithoutProof,
//         bch_tx: String,
//         xmr_tx: String,
//     },
//     Success {
//         bob_keys: KeyPublicWithoutProof,
//         bch_tx_hash: String,
//         xmr_tx_hash: String,
//         enc_sig: String,
//     },
// }

// #[derive(derivative::Derivative)]
// #[derivative(Debug)]
// pub struct Alice {
//     pub state: AliceState,
//     #[derivative(Debug = "ignore")]
//     keys: Keys,
// }

// impl Default for Alice {
//     fn default() -> Self {
//         let keys = Keys::random();
//         Self {
//             keys,
//             state: AliceState::WaitingForBobKeys,
//         }
//     }
// }

// impl StateMachine<AliceTransition, BobTransition> for Alice {
//     fn get_transition(&self) -> Option<BobTransition> {
//         match &self.state {
//             AliceState::WaitingForBobKeys => Some(BobTransition::AliceKeys(self.keys.public())),
//             AliceState::WaitingForBCHTx { bob_keys: _ } => Some(BobTransition::ContractAndEncSig),
//             AliceState::WaitingForBCHConfirmation {
//                 bob_keys: _,
//                 bch_tx: _,
//             } => None,
//             AliceState::WaitingForXMRConfirmation {
//                 bob_keys: _,
//                 bch_tx: _,
//                 xmr_tx,
//             } => Some(BobTransition::XMRTx(xmr_tx.clone())),
//             AliceState::WaitingForSwapLockEncSig {
//                 bob_keys: _,
//                 bch_tx: _,
//                 xmr_tx: _,
//             } => None,
//             AliceState::Success { .. } => None,
//         }
//     }

//     fn transition(&mut self, transition: AliceTransition) -> Response {
//         match (&self.state, transition) {
//             (AliceState::WaitingForBobKeys, AliceTransition::BobKeys(pubkeys)) => {
//                 let verified = proof::verify(
//                     &pubkeys.proof,
//                     (
//                         pubkeys.spend_bch.clone().into(),
//                         pubkeys.spend.clone().into(),
//                     ),
//                 );
//                 if !verified {
//                     return Response::Exit(String::from("Invalid Proof"));
//                 }

//                 self.state = AliceState::WaitingForBCHTx {
//                     bob_keys: pubkeys.into(),
//                 };

//                 return Response::Continue;
//             }

//             (AliceState::WaitingForBCHTx { bob_keys }, AliceTransition::BCHTx(bch_tx)) => {
//                 self.state = AliceState::WaitingForBCHConfirmation {
//                     bob_keys: bob_keys.to_owned(),
//                     bch_tx,
//                 };

//                 return Response::Continue;
//             }

//             (
//                 AliceState::WaitingForBCHConfirmation { bob_keys, bch_tx },
//                 AliceTransition::BCHLocked,
//             ) => {
//                 self.state = AliceState::WaitingForXMRConfirmation {
//                     bob_keys: bob_keys.to_owned(),
//                     bch_tx: bch_tx.to_owned(),
//                     xmr_tx: String::from(""),
//                 };

//                 return Response::Continue;
//             }

//             (
//                 AliceState::WaitingForXMRConfirmation {
//                     bob_keys,
//                     bch_tx,
//                     xmr_tx,
//                 },
//                 AliceTransition::XMRLocked,
//             ) => {
//                 self.state = AliceState::WaitingForSwapLockEncSig {
//                     bob_keys: bob_keys.to_owned(),
//                     bch_tx: bch_tx.to_owned(),
//                     xmr_tx: xmr_tx.to_owned(),
//                 };

//                 return Response::Continue;
//             }

//             (
//                 AliceState::WaitingForSwapLockEncSig {
//                     bob_keys,
//                     bch_tx,
//                     xmr_tx,
//                 },
//                 AliceTransition::EncSig(sig),
//             ) => {
//                 self.state = AliceState::Success {
//                     bob_keys: bob_keys.to_owned(),
//                     bch_tx_hash: bch_tx.to_owned(),
//                     xmr_tx_hash: xmr_tx.to_owned(),
//                     enc_sig: sig,
//                 };

//                 return Response::End;
//             }

//             (_, _) => return Response::Exit(String::from("Nothing happend")),
//         }
//     }
// }
