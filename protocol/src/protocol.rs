use std::sync::Arc;

use derivative::Derivative;
use ecdsa_fun::{adaptor::EncryptedSignature, Signature};
use monero::Address;

use crate::{
    blockchain::{types, BCH_MIN_CONFIRMATION},
    keys::KeyPublic,
};

#[derive(Debug)]
pub enum Response {
    Ok,
    Err(String),
    Exit(String),
}

#[derive(Debug)]
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

    /// The user of this transition must check if the contract
    /// received exact amount and confirmation
    BchLockVerified(bitcoincash::OutPoint),

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
