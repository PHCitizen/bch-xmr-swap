pub mod transaction {
    use serde::{Deserialize, Deserializer};

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Root {
        pub id: i64,
        pub jsonrpc: String,
        pub result: RpcResult,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct RpcResult {
        pub blockhash: String,
        pub blocktime: i64,
        pub confirmations: i64,
        pub hash: String,
        // pub hex: String,
        pub locktime: i64,
        pub size: i64,
        pub time: i64,
        pub txid: String,
        pub version: i64,
        pub vin: Vec<Vin>,
        pub vout: Vec<Vout>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Vin {
        pub script_sig: ScriptSig,
        pub sequence: i64,
        pub txid: String,
        pub vout: u32,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ScriptSig {
        // pub asm: String,
        pub hex: String,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Vout {
        pub n: u32,
        pub script_pub_key: ScriptPubKey,
        #[serde(deserialize_with = "btc_amount")]
        pub value: bitcoincash::Amount,
    }

    fn btc_amount<'de, D>(deserializer: D) -> Result<bitcoincash::Amount, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;

        f64::deserialize(deserializer).and_then(|val| {
            bitcoincash::Amount::from_btc(val).map_err(|err| Error::custom(err.to_string()))
        })
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ScriptPubKey {
        // pub asm: String,
        pub hex: String,
        #[serde(rename = "type")]
        pub type_field: String,
        #[serde(default)]
        pub addresses: Vec<String>,
        pub req_sigs: Option<i64>,
    }
}

pub mod history {
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    pub struct Root {
        pub id: i64,
        pub jsonrpc: String,
        pub result: Vec<Root2>,
    }

    #[derive(Debug, Deserialize)]
    pub struct Root2 {
        pub height: u64,
        pub tx_hash: String,
    }
}
