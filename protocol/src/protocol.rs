use std::{fmt::Display, sync::Arc};

use crate::{blockchain::BchProvider, keys::KeyPublic};
use derivative::Derivative;
use ecdsa_fun::{adaptor::EncryptedSignature, Signature};
use monero::{Address, Amount};

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

    CheckXmr,
    CheckBch,
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Swap<S> {
    pub id: String,
    pub xmr_network: monero::Network,
    #[derivative(Debug(ignore = "true"))]
    pub xmr_daemon: Arc<monero_rpc::DaemonJsonRpcClient>,
    #[derivative(Debug(ignore = "true"))]
    pub xmr_wallet: monero_rpc::WalletClient,
    #[derivative(Debug(ignore = "true"))]
    pub bch_provider: Arc<BchProvider>,

    pub keys: crate::keys::KeyPrivate,
    pub bch_recv: bitcoincash::Script,

    pub xmr_amount: monero::Amount,
    pub bch_amount: bitcoincash::Amount,

    pub state: S,
}
