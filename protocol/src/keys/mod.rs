use derivative::Derivative;
use serde::{Deserialize, Serialize};
use sigma_fun::{
    ed25519::curve25519_dalek::scalar::Scalar, ext::dl_secp256k1_ed25519_eq::CrossCurveDLEQProof,
};

use crate::{
    proof,
    utils::{monero_priv_deser, monero_priv_ser},
};

pub mod bitcoin;

#[derive(Debug, Clone)]
pub struct KeyPrivate {
    pub monero_spend: monero::PrivateKey,
    pub monero_view: monero::PrivateKey,
    pub ves: bitcoin::PrivateKey,
}

impl KeyPrivate {
    pub fn random() -> KeyPrivate {
        let mut rng = rand::thread_rng();
        let monero_spend = Scalar::random(&mut rng);
        let monero_view = Scalar::random(&mut rng);
        Self {
            monero_spend: monero::PrivateKey::from_slice(monero_spend.as_bytes()).unwrap(),
            monero_view: monero::PrivateKey::from_slice(monero_view.as_bytes()).unwrap(),
            ves: bitcoin::PrivateKey::random(),
        }
    }

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

#[cfg(test)]
mod test {
    use monero::ViewPair;

    /// Our assumption on monero keys:
    ///
    /// alice_public + bob_public = shared_public
    /// alice_private + bob_private = shared_private
    /// monero::PublicKey::from_private_key(shared_private) == shared_public

    #[test]
    fn test() {
        let bob = {
            use sigma_fun::ed25519::curve25519_dalek::scalar::Scalar;

            let mut rng = rand::thread_rng();
            let priv_spend = Scalar::random(&mut rng);
            let p_spend = monero::PrivateKey::from_slice(&priv_spend.to_bytes()).unwrap();
            let p_view = monero::PrivateKey::from_slice(&priv_spend.to_bytes()).unwrap();

            let address = monero::Address::from_viewpair(
                monero::Network::Stagenet,
                &ViewPair {
                    spend: monero::PublicKey::from_private_key(&p_spend),
                    view: p_view,
                },
            );

            (p_spend, p_view, address)
        };

        let alice = {
            use sigma_fun::ed25519::curve25519_dalek::scalar::Scalar;

            let mut rng = rand::thread_rng();
            let priv_spend = Scalar::random(&mut rng);
            let p_spend = monero::PrivateKey::from_slice(&priv_spend.to_bytes()).unwrap();
            let p_view = monero::PrivateKey::from_slice(&priv_spend.to_bytes()).unwrap();

            let address = monero::Address::from_viewpair(
                monero::Network::Stagenet,
                &ViewPair {
                    spend: monero::PublicKey::from_private_key(&p_spend),
                    view: p_view,
                },
            );

            (p_spend, p_view, address)
        };

        let add_priv_spend = bob.0 + alice.0;
        let add_priv_spend_pub = monero::PublicKey::from_private_key(&add_priv_spend);

        let add_pub_spend = monero::PublicKey::from_private_key(&bob.0)
            + monero::PublicKey::from_private_key(&alice.0);

        assert_eq!(add_priv_spend_pub, add_pub_spend);
    }
}
