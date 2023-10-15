use derivative::Derivative;
use serde::{Deserialize, Serialize};
use sigma_fun::ext::dl_secp256k1_ed25519_eq::CrossCurveDLEQProof;

use crate::{
    proof,
    utils::{dbg_hexlify, monero_priv_deser, monero_priv_ser},
};

pub mod bitcoin;

#[derive(Debug, Clone)]
pub struct KeyPrivate {
    pub monero_spend: monero::PrivateKey,
    pub monero_view: monero::PrivateKey,
    pub ves: bitcoin::PrivateKey,
}

impl KeyPrivate {
    pub fn to_public(&self) -> KeyPublic {
        let (proof, (spend_bch, _)) = proof::prove(&self.monero_spend);
        KeyPublic {
            monero_spend: monero::PublicKey::from_private_key(&self.monero_spend),
            monero_view: self.monero_view,
            ves: self.ves.public_key(),
            spend_bch,
            proof,
        }
    }
}

#[derive(Derivative, Clone, Serialize, Deserialize)]
#[derivative(Debug)]
pub struct KeyPublic {
    pub monero_spend: monero::PublicKey,
    #[rustfmt::skip]
    #[serde(serialize_with = "monero_priv_ser",deserialize_with = "monero_priv_deser")]
    pub monero_view: monero::PrivateKey,
    pub ves: bitcoin::PublicKey,

    pub spend_bch: bitcoin::PublicKey,
    #[derivative(Debug = "ignore")]
    pub proof: CrossCurveDLEQProof,
}

impl KeyPublic {
    pub fn remove_proof(&self) -> KeyPublicWithoutProof {
        KeyPublicWithoutProof {
            monero_spend: self.monero_spend,
            monero_view: self.monero_view,
            ves: self.ves.clone(),
            spend_bch: self.spend_bch.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyPublicWithoutProof {
    pub monero_spend: monero::PublicKey,
    #[rustfmt::skip]
    #[serde(serialize_with = "monero_priv_ser",deserialize_with = "monero_priv_deser")]
    pub monero_view: monero::PrivateKey,
    pub ves: bitcoin::PublicKey,

    pub spend_bch: bitcoin::PublicKey,
}
