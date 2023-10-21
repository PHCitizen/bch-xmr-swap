use anyhow::{bail, Context, Result};
use bitcoincash::{consensus::Encodable, Transaction};
use hex::ToHex;
use serde_json::json;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufStream},
    net::TcpStream,
};
use types::{history, transaction};

pub mod types;

pub const BCH_MIN_CONFIRMATION: i64 = 6;

#[derive(Debug, Clone)]
pub struct BchProvider {
    address: String,
}

impl BchProvider {
    pub async fn new(address: String) -> Result<Self, std::io::Error> {
        Ok(Self { address })
    }

    async fn send<T: serde::de::DeserializeOwned>(&self, payload: String) -> Result<T> {
        let socket = TcpStream::connect(&self.address).await?;
        let mut socket = BufStream::new(socket);

        let payload = payload + "\n";
        socket.write_all(payload.as_bytes()).await?;
        socket.flush().await?;

        let mut line = String::new();
        socket.read_line(&mut line).await?;

        if line.is_empty() {
            bail!("Closed connection")
        }

        socket.shutdown().await?;

        serde_json::from_str(&line).context(format!("failed to deserialize json: {line}"))
    }
}

impl BchProvider {
    pub async fn broadcast(&self, tx: Transaction) -> Result<String> {
        let mut buffer = Vec::new();
        tx.consensus_encode(&mut buffer)?;
        let buffer: String = buffer.encode_hex();

        let payload = json!({
            "id": 0,
            "jsonrpc": "2.0",
            "method": "blockchain.transaction.broadcast",
            "params": [buffer],
        })
        .to_string();
        self.send(payload).await
    }

    pub async fn get_tx(&self, hash: &str) -> Result<transaction::Root> {
        let payload = json!({
            "id": 0,
            "jsonrpc": "2.0",
            "method": "blockchain.transaction.get",
            "params": [hash, true]
        })
        .to_string();

        self.send(payload).await
    }

    pub async fn get_address_history(&self, address: &str) -> Result<history::Root> {
        let payload = json!({
            "id": 0,
            "jsonrpc": "2.0",
            "method": "blockchain.address.get_history",
            "params": [address]
        })
        .to_string();

        self.send(payload).await
    }
}

// pub async fn is_valid_tx(
//     hash: &str,
//     out_hex: &str,
//     out_val: u64,
// ) -> Result<bool, Box<dyn std::error::Error>> {
//     let response = Bch::get_tx(hash).await?;
//     for vout in response.result.vout {
//         if vout.script_pub_key.hex == out_hex && vout.value == out_val {
//             return Ok(true);
//         }
//     }

//     return Ok(false);
// }

// pub async fn is_confirmed(hash: &str) -> Result<bool, Box<dyn std::error::Error>> {
//     let response = Bch::get_tx(hash).await?;
//     Ok(response.result.confirmations >= BCH_MIN_CONFIRMATION)
// }

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
