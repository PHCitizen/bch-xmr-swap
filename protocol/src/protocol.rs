use std::str::FromStr;

use bitcoincash::OutPoint;
use derivative::Derivative;
use ecdsa_fun::{adaptor::EncryptedSignature, Signature};
use monero::Address;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    blockchain::{types, BCH_MIN_CONFIRMATION},
    keys::KeyPublic,
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

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Swap<S> {
    pub id: String,
    pub xmr_network: monero::Network,
    // #[derivative(Debug(ignore = "true"))]
    // pub xmr_daemon: Arc<monero_rpc::DaemonJsonRpcClient>,
    // #[derivative(Debug(ignore = "true"))]
    // pub xmr_wallet: monero_rpc::WalletClient,
    // #[derivative(Debug(ignore = "true"))]
    // pub bch_provider: Arc<BchProvider>,
    pub keys: crate::keys::KeyPrivate,
    pub bch_recv: bitcoincash::Script,

    pub xmr_amount: monero::Amount,
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

fn ser_outpoint<S>(outpoint: &OutPoint, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_str(&outpoint.to_string())
}

fn deser_outpoint<'de, D>(deserializer: D) -> Result<OutPoint, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    String::deserialize(deserializer).and_then(|string| {
        OutPoint::from_str(&string).map_err(|err| Error::custom(err.to_string()))
    })
}
