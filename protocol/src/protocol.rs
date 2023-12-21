use std::fmt::{self, Debug};

use ecdsa_fun::{adaptor::EncryptedSignature, Signature};
use monero::Address;
use serde::{Deserialize, Serialize};

use crate::{
    alice::Alice,
    blockchain::{types, BCH_MIN_CONFIRMATION},
    bob::Bob,
    keys::{bitcoin, KeyPublic},
    utils::{bch_amount, monero_amount, monero_network},
};

#[derive(Debug)]
pub enum Error {
    InvalidProof,
    InvalidStateTransition,
    InvalidTransaction,
    InvalidBchAddress,
    InvalidXmrAddress,
    InvalidSignature,
    InvalidXmrAmount,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(self, f)
    }
}

#[derive(Debug)]
pub enum Action {
    /// Trade must stop
    TradeFailed,
    /// No further transition needed
    TradeSuccess,
    /// The server must watch address for send/receive tx
    /// and make Transition::BchTx(Transaction)
    WatchBchAddress(String),
    Refund,

    LockBchAndWatchXmr(String, monero::Address),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Transition {
    Msg0 {
        keys: KeyPublic,
        receiving: bitcoincash::Script,
    },
    Contract {
        bch_address: String,
        xmr_address: Address,
    },

    EncSig(EncryptedSignature),
    DecSig(Signature),

    /// You are responsible to only use on confirmed tx
    #[serde(skip)]
    BchConfirmedTx(bitcoincash::Transaction),
    XmrLockVerified(u64, #[serde(with = "monero_amount")] monero::Amount), // restore_height, amount
}

#[derive(Deserialize, Serialize)]
pub struct Swap {
    pub id: String,
    #[serde(with = "monero_network")]
    pub xmr_network: monero::Network,
    pub bch_network: bitcoin::Network,

    pub keys: crate::keys::KeyPrivate,
    pub bch_recv: bitcoincash::Script,

    #[serde(with = "monero_amount")]
    pub xmr_amount: monero::Amount,
    #[serde(with = "bch_amount")]
    pub bch_amount: bitcoincash::Amount,
}

impl Debug for Swap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Swap {{\n\
                \tid: {:?},\n\
                \txmr_network: {:?},\n\
                \tbch_network: {:?},\n\
                \tkeys: KeyPrivate {{\n\
                    \t\tmonero_spend: monero::PrivateKey({}),\n\
                    \t\tmonero_view: monero::PrivateKey({}),\n\
                    \t\tves: bitcoincash::PrivateKey({}),\n\
                \t}},\n\
                \tbch_recv: {:?},\n\
                \txmr_amount: {:?},\n\
                \tbch_amount: {:?},\n\
            }}\n\
            ",
            self.id,
            self.xmr_network,
            self.bch_network,
            self.keys.monero_spend,
            self.keys.monero_view,
            self.keys.ves,
            self.bch_recv,
            self.xmr_amount,
            self.bch_amount,
        )
    }
}

pub trait SwapEvents {
    /// Most of the time only one from the return type are `not None`
    /// but there are special case that we both error and action
    ///
    /// Example: (Action::TradeFailed, Error::InvalidProof)
    ///        : this means that we must stop the trade because other give invalid proof
    fn transition(&mut self, transition: Transition) -> (Option<Action>, Option<Error>);
    fn get_transition(&self) -> Option<Transition>;
}

#[derive(Debug, Deserialize, Serialize)]
pub enum SwapWrapper {
    Alice(Alice),
    Bob(Bob),
}

impl SwapEvents for SwapWrapper {
    fn transition(&mut self, transition: Transition) -> (Option<Action>, Option<Error>) {
        match self {
            SwapWrapper::Alice(alice) => alice.transition(transition),
            SwapWrapper::Bob(bob) => bob.transition(transition),
        }
    }
    fn get_transition(&self) -> Option<Transition> {
        match self {
            SwapWrapper::Alice(alice) => alice.get_transition(),
            SwapWrapper::Bob(bob) => bob.get_transition(),
        }
    }
}

pub fn tx_has_correct_amount_and_conf(
    tx: types::transaction::RpcResult,
    locking_script: &str,
    amount: bitcoincash::Amount,
) -> bool {
    if tx.confirmations < BCH_MIN_CONFIRMATION {
        return false;
    }

    for vout in tx.vout {
        if vout.value == amount && vout.script_pub_key.hex == locking_script {
            return true;
        }
    }

    return false;
}
