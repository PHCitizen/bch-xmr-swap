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

