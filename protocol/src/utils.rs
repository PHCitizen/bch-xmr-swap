use serde::{Deserialize, Deserializer, Serializer};
use std::str::FromStr;

pub fn monero_priv_ser<S>(privkey: &monero::PrivateKey, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_str(&privkey.to_string())
}

pub fn monero_priv_deser<'de, D>(deserializer: D) -> Result<monero::PrivateKey, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    String::deserialize(deserializer).and_then(|string| {
        monero::PrivateKey::from_str(&string).map_err(|err| Error::custom(err.to_string()))
    })
}
