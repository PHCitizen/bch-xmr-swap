use std::str::FromStr;

use bitcoin_hashes::Hash;
use bitcoincash::blockdata::{opcodes::all as opcodes, script::Builder};
use ecdsa_fun::{
    fun::{Point, Scalar},
    ECDSA,
};
use serde::{Deserialize, Serialize};
use sigma_fun::secp256k1::fun::{hex::HexError, Point as PointP};

use crate::utils::impl_debug_display;

pub mod address;

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

    pub fn from_str(str: &str) -> Result<Self, HexError> {
        Ok(Self(Point::from_str(str)?))
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
        let little_endian_bytes = value.to_bytes();
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
    pub fn P2PKH_locking_bytecode(&self) -> Vec<u8> {
        Builder::new()
            .push_opcode(opcodes::OP_DUP)
            .push_opcode(opcodes::OP_HASH160)
            .push_slice(&self.0)
            .push_opcode(opcodes::OP_EQUALVERIFY)
            .push_opcode(opcodes::OP_CHECKSIG)
            .into_script()
            .to_bytes()
    }
}

// #[test]
// fn te() {
//     let mut res = vec![];
//     let t = Transaction {
//         version: 2,
//         lock_time: PackedLockTime(812991),
//         input: vec![TxIn {
//             sequence: Sequence(4294967294),
//             previous_output: bitcoincash::OutPoint {
//                 txid: Txid::from_str(
//                     "92b210c45c874992335067d2bc29d4e1636795f38f1b72efcadc4bba77172be7",
//                 )
//                 .unwrap(),
//                 vout: 2,
//             },
//             script_sig: Script::from_hex("4003aa7f31b5914321c03bd9c57c88862475c281dc887fc1016d959aee2a43fe03aa7f31b5914321c03bd9c57c888624892475c281dc887fc1016d959aee2a43fe4c8b1976a91447fe8a0ca161ebc0090c9d46f81582c579c594a788ac02e8032102ee2cbe75e3d2a9b5049ac73122c229627a49bd289f71e05075b2c600907661281976a91447fe8a0ca161ebc0090c9d46f81582c579c594a788ac02e803c3519dc4519d00c600cc949d00cb009c6300cd7888547978a85379bb675279b27500cd54798854790088686d6d7551").unwrap(),
//             ..Default::default()
//         }],
//         output: vec![TxOut {
//             value: 1000,
//             script_pubkey: Script::from_hex("76a91447fe8a0ca161ebc0090c9d46f81582c579c594a788ac").unwrap(),
//             token: None,
//         }],
//     }
//     .consensus_encode(&mut res)
//     .unwrap();

//     let strin = res.encode_hex::<String>();
//     dbg!(&strin);
//     assert!(strin == "0200000001e72b1777ba4bdccaef721b8ff3956763e1d429bcd26750339249875cc410b29202000000ce4003aa7f31b5914321c03bd9c57c88862475c281dc887fc1016d959aee2a43fe03aa7f31b5914321c03bd9c57c888624892475c281dc887fc1016d959aee2a43fe4c8b1976a91447fe8a0ca161ebc0090c9d46f81582c579c594a788ac02e8032102ee2cbe75e3d2a9b5049ac73122c229627a49bd289f71e05075b2c600907661281976a91447fe8a0ca161ebc0090c9d46f81582c579c594a788ac02e803c3519dc4519d00c600cc949d00cb009c6300cd7888547978a85379bb675279b27500cd54798854790088686d6d7551feffffff01e8030000000000001976a91447fe8a0ca161ebc0090c9d46f81582c579c594a788acbf670c00");
// }
