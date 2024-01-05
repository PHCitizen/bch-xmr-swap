// #[serde(with = "path")]

use bitcoincash::{secp256k1::ecdsa::Signature, Script};

pub mod monero_private_key {
    use serde::{de::Error, Deserialize, Deserializer, Serializer};
    use std::str::FromStr;

    type Type = monero::PrivateKey;

    pub fn serialize<S>(privkey: &Type, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_str(&privkey.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Type, D::Error>
    where
        D: Deserializer<'de>,
    {
        let string = String::deserialize(deserializer)?;
        monero::PrivateKey::from_str(&string).map_err(|err| Error::custom(err.to_string()))
    }
}

pub mod monero_view_pair {
    use serde::{de::Error, Deserialize, Deserializer, Serialize, Serializer};
    use std::str::FromStr;

    type Type = monero::ViewPair;

    #[derive(Deserialize, Serialize)]
    struct MoneroViewPair {
        spend: String,
        view: String,
    }

    pub fn serialize<S>(key: &Type, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        MoneroViewPair {
            spend: key.spend.to_string(),
            view: key.view.to_string(),
        }
        .serialize(s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Type, D::Error>
    where
        D: Deserializer<'de>,
    {
        let string = MoneroViewPair::deserialize(deserializer)?;
        Ok(monero::ViewPair {
            spend: monero::PublicKey::from_str(&string.spend)
                .map_err(|err| Error::custom(err.to_string()))?,
            view: monero::PrivateKey::from_str(&string.view)
                .map_err(|err| Error::custom(err.to_string()))?,
        })
    }
}

pub mod monero_key_pair {
    use std::str::FromStr;

    use serde::{de::Error, Deserialize, Deserializer, Serialize, Serializer};

    type Type = monero::KeyPair;

    #[derive(Deserialize, Serialize)]
    struct MoneroKeyPair {
        spend: String,
        view: String,
    }

    pub fn serialize<S>(key: &Type, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        MoneroKeyPair {
            spend: key.spend.to_string(),
            view: key.view.to_string(),
        }
        .serialize(s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Type, D::Error>
    where
        D: Deserializer<'de>,
    {
        let string = MoneroKeyPair::deserialize(deserializer)?;
        Ok(monero::KeyPair {
            spend: monero::PrivateKey::from_str(&string.spend)
                .map_err(|err| Error::custom(err.to_string()))?,
            view: monero::PrivateKey::from_str(&string.view)
                .map_err(|err| Error::custom(err.to_string()))?,
        })
    }
}

pub mod monero_network {
    use serde::{de, Deserialize, Deserializer, Serializer};

    type Type = monero::Network;

    pub fn serialize<S>(key: &Type, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let string = match *key {
            monero::Network::Mainnet => "Mainnet",
            monero::Network::Testnet => "Testnet",
            monero::Network::Stagenet => "Stagenet",
        };
        s.serialize_str(string)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Type, D::Error>
    where
        D: Deserializer<'de>,
    {
        let network = match String::deserialize(deserializer)?.as_str() {
            "Mainnet" => monero::Network::Mainnet,
            "Testnet" => monero::Network::Testnet,
            "Stagenet" => monero::Network::Stagenet,
            _ => return Err(de::Error::custom("Invalid monero network")),
        };
        Ok(network)
    }
}

pub mod monero_amount {
    use serde::{Deserialize, Deserializer, Serializer};

    type Type = monero::Amount;

    pub fn serialize<S>(key: &Type, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_u64(key.as_pico())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Type, D::Error>
    where
        D: Deserializer<'de>,
    {
        let amount = u64::deserialize(deserializer)?;
        Ok(monero::Amount::from_pico(amount))
    }
}

pub mod bch_amount {
    use serde::{Deserialize, Deserializer, Serializer};

    type Type = bitcoincash::Amount;

    pub fn serialize<S>(key: &Type, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_u64(key.to_sat())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Type, D::Error>
    where
        D: Deserializer<'de>,
    {
        let amount = u64::deserialize(deserializer)?;
        Ok(bitcoincash::Amount::from_sat(amount))
    }
}

pub mod monero_public_key {
    use std::str::FromStr;

    use serde::{de::Error, Deserialize, Deserializer, Serializer};

    type Type = monero::PublicKey;

    pub fn serialize<S>(key: &Type, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_str(&key.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Type, D::Error>
    where
        D: Deserializer<'de>,
    {
        let amount = String::deserialize(deserializer)?;
        Ok(monero::PublicKey::from_str(&amount).map_err(|e| Error::custom(e.to_string()))?)
    }
}

pub fn get_signature(script: Script) -> Option<Signature> {
    for instruction in script.instructions_minimal() {
        match instruction {
            Ok(bitcoincash::blockdata::script::Instruction::PushBytes(bytes)) => {
                match bitcoincash::secp256k1::ecdsa::Signature::from_der(bytes) {
                    Ok(sig) => return Some(sig),
                    _ => {}
                }
            }
            _ => {}
        }
    }
    return None;
}
