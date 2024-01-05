use std::{collections::HashMap, sync::Arc, time::Duration};

use bitcoincash::Transaction;
use serde::Deserialize;
use serde_json::json;
use tokio::{
    io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
    sync::{broadcast, oneshot, Mutex},
    time::sleep,
};

#[derive(Deserialize)]
struct HasId {
    id: u64,
}

pub struct TcpElectrum {
    futures: Arc<Mutex<HashMap<u64, oneshot::Sender<String>>>>,
    producer: broadcast::Sender<String>,

    id: Arc<Mutex<u64>>,
    stream_write: Arc<Mutex<OwnedWriteHalf>>,
}

#[derive(Debug)]
pub enum TcpElectrumError {
    IoError(io::Error),
    RecvError(oneshot::error::RecvError),
}

impl std::fmt::Display for TcpElectrumError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError(e) => write!(f, "IoError {e}"),
            Self::RecvError(e) => write!(f, "RecvError {e}"),
        }
    }
}

impl std::error::Error for TcpElectrumError {}

impl TcpElectrum {
    pub fn new(stream: TcpStream) -> Self {
        let (producer, _) = broadcast::channel(10);
        let (stream_read, stream_write) = stream.into_split();

        let id = Arc::new(Mutex::new(0));
        let futures = Arc::new(Mutex::new(HashMap::new()));
        let stream_write = Arc::new(Mutex::new(stream_write));

        tokio::spawn({
            let producer = producer.clone();
            let futures = futures.clone();
            async move {
                let stream_read = BufReader::new(stream_read);
                TcpElectrum::process_reads(stream_read, producer, futures).await;
            }
        });

        let server = TcpElectrum {
            id,
            futures,
            producer,
            stream_write,
        };

        tokio::spawn({
            let server = server.clone();
            async move {
                loop {
                    let _ = server.send("server.ping", json!([])).await;
                    sleep(Duration::from_secs(5)).await;
                }
            }
        });

        server
    }

    async fn process_reads(
        mut reader: BufReader<OwnedReadHalf>,
        producer: broadcast::Sender<String>,
        futures: Arc<Mutex<HashMap<u64, oneshot::Sender<String>>>>,
    ) {
        loop {
            let mut buf = String::new();
            let _ = reader.read_line(&mut buf).await.unwrap();
            if buf == "" {
                break;
            }

            match serde_json::from_str::<HasId>(&buf) {
                Err(_) => {
                    let _ = producer.send(buf);
                }
                Ok(HasId { id }) => {
                    if let Some(recv) = futures.lock().await.remove(&id) {
                        let _ = recv.send(buf);
                    }
                }
            }
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.producer.subscribe()
    }

    pub async fn send(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<String, TcpElectrumError> {
        let mut guard = self.id.lock().await;
        let id = guard.clone();
        *guard += 1;
        drop(guard);

        let payload = json!({"id": id, "method": method, "params": params});
        let mut payload = serde_json::to_vec(&payload).unwrap();
        payload.push(b'\n');

        let (sender, recv) = oneshot::channel();
        let mut guard = self.futures.lock().await;
        let _ = guard.insert(id, sender);
        drop(guard);

        let mut guard = self.stream_write.lock().await;
        let _ = guard
            .write(&payload)
            .await
            .map_err(|e| TcpElectrumError::IoError(e))?;
        drop(guard);

        let result = recv.await.map_err(|e| TcpElectrumError::RecvError(e))?;
        Ok(result)
    }
}

impl Clone for TcpElectrum {
    fn clone(&self) -> Self {
        TcpElectrum {
            id: self.id.clone(),
            futures: self.futures.clone(),
            producer: self.producer.clone(),
            stream_write: self.stream_write.clone(),
        }
    }
}

#[derive(Deserialize)]
pub struct TxInfo0 {
    confirmations: u32,
    #[serde(with = "hex")]
    hex: Vec<u8>,
}

#[derive(Deserialize)]
pub struct TxInfo {
    result: TxInfo0,
}

pub async fn scan_address_conf_tx(
    bch_server: &TcpElectrum,
    address: &str,
    min_conf: u32,
) -> Vec<(Transaction, u32)> {
    let response = bch_server
        .send("blockchain.address.get_history", json!([address, true]))
        .await
        .unwrap();

    let tx_hashes = serde_json::from_str::<serde_json::Value>(&response).unwrap()["result"]
        .as_array()
        .unwrap()
        .to_owned();

    let mut txs = Vec::new();
    for tx in tx_hashes {
        // in mempool
        if tx["height"].as_u64().unwrap() == 0 {
            continue;
        }

        let tx_hash = tx["tx_hash"].as_str().unwrap();
        let tx_info = bch_server
            .send("blockchain.transaction.get", json!([tx_hash, true]))
            .await
            .unwrap();

        let tx_info = serde_json::from_str::<TxInfo>(&tx_info).unwrap().result;
        if tx_info.confirmations < min_conf {
            continue;
        }

        txs.push((
            bitcoincash::consensus::deserialize::<bitcoincash::Transaction>(&tx_info.hex).unwrap(),
            tx_info.confirmations,
        ));
    }

    txs
}
