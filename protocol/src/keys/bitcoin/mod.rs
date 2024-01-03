use ecdsa_fun::fun::Scalar;
use serde::{Deserialize, Serialize};

pub mod address;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Network {
    Mainnet,
    Testnet,
    Regtest,
}

pub fn random_private_key(network: Network) -> bitcoincash::PrivateKey {
    let mut rng = rand::thread_rng();
    let scalar = Scalar::random(&mut rng);

    let network = match network {
        Network::Mainnet => bitcoincash::Network::Bitcoin,
        Network::Testnet => bitcoincash::Network::Testnet,
        Network::Regtest => bitcoincash::Network::Regtest,
    };
    bitcoincash::PrivateKey::from_slice(&scalar.to_bytes(), network).unwrap()
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
