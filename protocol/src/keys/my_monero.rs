use monero::PrivateKey as MoneroPrivk;
use monero::PublicKey as MoneroPubk;
use serde::{Deserialize, Serialize};
use sigma_fun::ed25519::curve25519_dalek::edwards::CompressedEdwardsY;
use sigma_fun::ed25519::curve25519_dalek::edwards::EdwardsPoint;
use sigma_fun::ed25519::curve25519_dalek::scalar::Scalar;

use crate::utils::impl_debug_display;

#[derive(Clone, Serialize, Deserialize)]
pub struct PrivateKey(Scalar);
impl_debug_display!(PrivateKey);

impl PrivateKey {
    pub fn random() -> Self {
        let mut rng = rand::thread_rng();
        let scalar = Scalar::random(&mut rng);
        PrivateKey(scalar)
    }

    pub fn public_key(&self) -> PublicKey {
        let privkey = MoneroPrivk::from_slice(self.0.as_bytes()).unwrap();
        let pubkey = MoneroPubk::from_private_key(&privkey);
        PublicKey(pubkey)
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        self.0.to_bytes()
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PublicKey(MoneroPubk);
impl_debug_display!(PublicKey);

impl PublicKey {
    fn to_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl Into<EdwardsPoint> for PublicKey {
    fn into(self) -> EdwardsPoint {
        CompressedEdwardsY::from_slice(self.0.as_bytes())
            .decompress()
            .unwrap()
    }
}
