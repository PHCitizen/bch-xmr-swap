pub mod transaction {
    use serde::{Deserialize, Serialize};

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Root {
        pub id: i64,
        pub jsonrpc: String,
        pub result: Result,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Result {
        pub blockhash: String,
        pub blocktime: i64,
        pub confirmations: i64,
        pub hash: String,
        pub hex: String,
        pub locktime: i64,
        pub size: i64,
        pub time: i64,
        pub txid: String,
        pub version: i64,
        pub vin: Vec<Vin>,
        pub vout: Vec<Vout>,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Vin {
        pub script_sig: ScriptSig,
        pub sequence: i64,
        pub txid: String,
        pub vout: u32,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ScriptSig {
        pub asm: String,
        pub hex: String,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Vout {
        pub n: u32,
        pub script_pub_key: ScriptPubKey,
        pub value: u64,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ScriptPubKey {
        pub asm: String,
        pub hex: String,
        #[serde(rename = "type")]
        pub type_field: String,
        #[serde(default)]
        pub addresses: Vec<String>,
        pub req_sigs: Option<i64>,
    }
}

pub mod history {
    use serde::{Deserialize, Serialize};

    pub type Root = Vec<Root2>;

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Root2 {
        pub height: u64,
        pub tx_hash: String,
    }
}
