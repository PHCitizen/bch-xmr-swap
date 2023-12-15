use ecdsa_fun::{adaptor::EncryptedSignature, Signature};
use monero::Address;
use serde::{Deserialize, Serialize};

use crate::{
    blockchain::{types, BCH_MIN_CONFIRMATION},
    keys::{bitcoin, KeyPublic},
    utils::{bch_amount, monero_amount, monero_network},
};

#[derive(Debug)]
pub enum ResponseError {
    InvalidStateTransition,
    InvalidTransaction,
    InvalidBchAddress,
    InvalidXmrAddress,
}

#[derive(Debug)]
pub enum Response {
    Ok,
    Err(ResponseError),
    Exit(String),
    Done,

    /// if you receive this response,
    /// you must check if it has valid output address and confirmation.
    ///
    /// If it does `Transition::BchLockVerified`
    WatchBchTx(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Transition {
    Msg0 {
        keys: KeyPublic,
        receiving: Vec<u8>,
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

    /// The user of this transition must check if the shared address
    /// received exact amount that is already spendable or 'unlocked'
    XmrLockVerified(u64),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Swap<S> {
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

    pub state: S,
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
