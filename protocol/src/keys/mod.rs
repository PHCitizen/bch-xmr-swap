use serde::{Deserialize, Serialize};
use sigma_fun::{
    ed25519::curve25519_dalek::{edwards::EdwardsPoint, scalar::Scalar as ScalarD},
    ext::dl_secp256k1_ed25519_eq::CrossCurveDLEQProof,
};

pub mod bitcoin;
mod macros;
pub mod my_monero;

#[derive(Debug, Clone)]
pub struct Keys {
    pub bch: bitcoin::PrivateKey,
    pub ves: bitcoin::PrivateKey,
    pub spend: my_monero::PrivateKey,
    pub view: my_monero::PrivateKey,
}

impl Keys {
    pub fn random() -> Keys {
        Keys {
            bch: bitcoin::PrivateKey::random(),
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

    pub fn public(&self) -> KeyPublic {
        let (proof, (spend_bch, _)) = self.prove();
        KeyPublic {
            locking_bytecode: self.bch.public_key().pubkey_hash().get_P2PKH(),
            ves: self.ves.public_key(),
            view: self.view.clone(),
            spend: self.spend.public_key(),
            spend_bch,
            proof,
        }
    }
}

#[derive(derivative::Derivative, Clone, Serialize, Deserialize)]
#[derivative(Debug)]
pub struct KeyPublic {
    pub locking_bytecode: String,
    pub ves: bitcoin::PublicKey,
    pub view: my_monero::PrivateKey,

    pub spend: my_monero::PublicKey,
    pub spend_bch: bitcoin::PublicKey,
    #[derivative(Debug = "ignore")]
    pub proof: CrossCurveDLEQProof,
}

#[derive(Debug, Clone)]
pub struct KeyPublicWithoutProof {
    pub locking_bytecode: String,
    pub ves: bitcoin::PublicKey,
    pub view: my_monero::PrivateKey,

    pub spend: my_monero::PublicKey,
    pub spend_bch: bitcoin::PublicKey,
}

impl Into<KeyPublicWithoutProof> for KeyPublic {
    fn into(self) -> KeyPublicWithoutProof {
        KeyPublicWithoutProof {
            locking_bytecode: self.locking_bytecode,
            spend: self.spend,
            spend_bch: self.spend_bch,
            ves: self.ves,
            view: self.view,
        }
    }
}
