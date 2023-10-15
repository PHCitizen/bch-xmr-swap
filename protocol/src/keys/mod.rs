use derivative::Derivative;
use serde::{Deserialize, Serialize};
use sigma_fun::{
    ed25519::curve25519_dalek::{edwards::EdwardsPoint, scalar::Scalar as ScalarD},
    ext::dl_secp256k1_ed25519_eq::CrossCurveDLEQProof,
};

use crate::utils::dbg_hexlify;

pub mod bitcoin;
pub mod my_monero;

#[derive(Debug, Clone)]
pub struct Keys {
    pub ves: bitcoin::PrivateKey,
    pub spend: my_monero::PrivateKey,
    pub view: my_monero::PrivateKey,
}

impl Keys {
    pub fn random() -> Keys {
        Keys {
            ves: bitcoin::PrivateKey::random(),
            spend: my_monero::PrivateKey::random(),
            view: my_monero::PrivateKey::random(),
        }
    }

    pub fn prove(&self) -> (CrossCurveDLEQProof, (bitcoin::PublicKey, EdwardsPoint)) {
        let scalar = ScalarD::from_bytes_mod_order(self.spend.to_bytes());
        let (proof, (bch, xmr)) = crate::proof::prove(&scalar);

        (proof, (bitcoin::PublicKey::from_point(bch), xmr))
    }
}

#[derive(derivative::Derivative, Clone, Serialize, Deserialize)]
#[derivative(Debug)]
pub struct KeyPublic {
    #[derivative(Debug(format_with = "dbg_hexlify"))]
    pub locking_bytecode: Vec<u8>,
    pub ves: bitcoin::PublicKey,
    pub view: my_monero::PrivateKey,

    pub spend: my_monero::PublicKey,
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
    pub view: my_monero::PrivateKey,

    pub spend: my_monero::PublicKey,
    pub spend_bch: bitcoin::PublicKey,
}
