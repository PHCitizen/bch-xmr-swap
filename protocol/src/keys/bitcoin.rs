use bitcoin_hashes::Hash;
use ecdsa_fun::{
    fun::{Point, Scalar},
    ECDSA,
};
use hex::ToHex;
use serde::{Deserialize, Serialize};
use sigma_fun::secp256k1::fun::Point as PointP;

use super::macros::impl_debug_display;

#[derive(Clone)]
pub struct PrivateKey(Scalar);
impl_debug_display!(PrivateKey);

impl PrivateKey {
    pub fn random() -> Self {
        let mut rng = rand::thread_rng();
        PrivateKey(Scalar::random(&mut rng))
    }

    pub fn public_key(&self) -> PublicKey {
        let ecdsa = ECDSA::<()>::default();
        let public = ecdsa.verification_key_for(&self.0);
        PublicKey(public)
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        self.0.to_bytes()
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PublicKey(Point);
impl_debug_display!(PublicKey);

impl PublicKey {
    pub fn from_point(point: Point) -> Self {
        Self(point)
    }

    pub fn pubkey_hash(&self) -> PublicKeyHash {
        let hash = bitcoin_hashes::hash160::Hash::hash(&self.0.to_bytes()).to_byte_array();
        PublicKeyHash(hash)
    }

    pub fn to_bytes(&self) -> [u8; 33] {
        self.0.to_bytes()
    }
}

impl From<PointP> for PublicKey {
    fn from(value: PointP) -> Self {
        let mut little_endian_bytes = value.to_bytes();
        PublicKey(Point::from_bytes(little_endian_bytes).unwrap())
    }
}

impl Into<PointP> for PublicKey {
    fn into(self) -> PointP {
        PointP::from_bytes(self.to_bytes()).unwrap()
    }
}

pub struct PublicKeyHash([u8; 20]);
impl_debug_display!(PublicKeyHash);

impl PublicKeyHash {
    pub fn to_bytes(&self) -> [u8; 20] {
        self.0
    }

    #[allow(non_snake_case)]
    pub fn get_P2PKH(&self) -> String {
        format!("76a914{}88ac", self.0.encode_hex::<String>())
    }
}
