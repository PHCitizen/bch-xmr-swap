use anyhow::Result;
use bitcoincash::{consensus::Encodable, Transaction};
use hex::ToHex;
use serde_json::json;
use types::{history, transaction};

mod types;

const BCH_API_WRAPPER: &str = "https://fulcrum-http.pat.mn";
const BCH_API: &str = "wss://chipnet.imaginary.cash:50004";
pub const BCH_MIN_CONFIRMATION: i64 = 6;

#[derive(Debug)]
pub struct Bch;

impl Bch {
    pub async fn broadcast(tx: Transaction) -> Result<String> {
        let mut buffer = vec![];
        tx.consensus_encode(&mut buffer)?;
        let buffer: String = buffer.encode_hex();

        let payload = json!({
            "jsonrpc": "2.0",
            "method": "blockchain.transaction.broadcast",
            "params": [buffer]
        });

        let response = reqwest::Client::new()
            .post(BCH_API_WRAPPER)
            .header("Content-Type", "application/json")
            .header("server", BCH_API)
            .header("accept", "application/json")
            .json(&payload)
            .send()
            .await?
            .json::<String>()
            .await?;

        Ok(response)
    }

    pub async fn get_tx(hash: &str) -> Result<transaction::Root> {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "blockchain.transaction.get",
            "params": [hash, true]
        });

        let response = reqwest::Client::new()
            .post(BCH_API_WRAPPER)
            .header("Content-Type", "application/json")
            .header("server", BCH_API)
            .header("accept", "application/json")
            .json(&payload)
            .send()
            .await?
            .json::<transaction::Root>()
            .await?;

        Ok(response)
    }

    pub async fn get_address_history(address: &str) -> Result<history::Root> {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "blockchain.address.get_history",
            "params": [address]
        });

        let response = reqwest::Client::new()
            .post(BCH_API_WRAPPER)
            .header("Content-Type", "application/json")
            .header("server", BCH_API)
            .header("accept", "application/json")
            .json(&payload)
            .send()
            .await?
            .json::<history::Root>()
            .await?;

        Ok(response)
    }

    pub async fn is_valid_tx(
        hash: &str,
        out_hex: &str,
        out_val: u64,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let response = Bch::get_tx(hash).await?;
        for vout in response.result.vout {
            if vout.script_pub_key.hex == out_hex && vout.value == out_val {
                return Ok(true);
            }
        }

        return Ok(false);
    }

    pub async fn is_confirmed(hash: &str) -> Result<bool, Box<dyn std::error::Error>> {
        let response = Bch::get_tx(hash).await?;
        Ok(response.result.confirmations >= BCH_MIN_CONFIRMATION)
    }
}

// Example code to create xmr_wallet_rpc
//
// pub fn new(exe_path: &str, ip: &str, port: u16) -> (WalletClient, DaemonJsonRpcClient) {
//     let rpc_server = Command::new(exe_path)
//         .env("LANG", "en_AU.UTF-8")
//         .kill_on_drop(true)
//         .args([
//             "--stagenet",
//             "--disable-rpc-login",
//             "--log-level=1",
//             "--daemon-address=http://stagenet.xmr-tw.org:38081",
//             "--untrusted-daemon",
//             "--rpc-bind-ip",
//             ip,
//             "--rpc-bind-port",
//             port.to_string().as_str(),
//             "--wallet-dir=wallet_dir",
//         ])
//         .spawn()
//         .unwrap();
//
//     (
//         RpcClientBuilder::new()
//             .build(format!("http://{ip}:{port}"))
//             .unwrap()
//             .wallet(),
//         RpcClientBuilder::new()
//             .build("http://stagenet.xmr-tw.org:38081")
//             .unwrap()
//             .daemon(),
//     )
// }
