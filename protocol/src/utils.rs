use serde::{Deserialize, Deserializer, Serializer};
use std::str::FromStr;

macro_rules! impl_debug_display {
    ($struct_name:ident) => {
        impl std::fmt::Debug for $struct_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let hash: String = hex::encode(self.to_bytes());
                f.write_str(&hash)
            }
        }

        impl std::fmt::Display for $struct_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let hash: String = hex::encode(self.to_bytes());
                f.write_str(&hash)
            }
        }
    };
}

pub(crate) use impl_debug_display;

pub fn dbg_hexlify<T: AsRef<[u8]>>(
    slice: &T,
    f: &mut std::fmt::Formatter,
) -> Result<(), std::fmt::Error> {
    let hexlify = hex::encode(slice);
    f.write_str(&hexlify)
}

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
