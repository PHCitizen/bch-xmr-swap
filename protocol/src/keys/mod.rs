use derivative::Derivative;
use serde::{Deserialize, Serialize};
use sigma_fun::ext::dl_secp256k1_ed25519_eq::CrossCurveDLEQProof;

use crate::utils::dbg_hexlify;

pub mod bitcoin;

#[derive(derivative::Derivative, Clone, Serialize, Deserialize)]
#[derivative(Debug)]
pub struct KeyPublic {
    #[derivative(Debug(format_with = "dbg_hexlify"))]
    pub locking_bytecode: Vec<u8>,
    pub ves: bitcoin::PublicKey,
    pub view: String,

    pub spend: monero::PublicKey,
    pub spend_bch: bitcoin::PublicKey,
    #[derivative(Debug = "ignore")]
    pub proof: CrossCurveDLEQProof,
}

#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub struct KeyPublicWithoutProof {
    #[derivative(Debug(format_with = "dbg_hexlify"))]
    pub locking_bytecode: Vec<u8>,
    pub ves: bitcoin::PublicKey,
    pub view: String,

    pub spend: monero::PublicKey,
    pub spend_bch: bitcoin::PublicKey,
}
