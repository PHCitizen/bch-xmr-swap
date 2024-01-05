use std::{sync::Arc, time::Duration};

use anyhow::bail;
use reqwest::StatusCode;
use serde_json::json;

use protocol::{
    alice,
    bitcoincash::{self},
    blockchain::{self},
    keys::{
        bitcoin::{self, random_private_key},
        KeyPrivate,
    },
    monero::{self},
    protocol::Swap,
    protocol::{SwapEvents, SwapWrapper, Transition},
};
use tokio::{net::TcpStream, sync::Mutex, time::sleep};

const BASE_URL: &str = "http://localhost:8080";

async fn create_new_trade(
    client: &reqwest::Client,
    timelock1: u32,
    timelock2: u32,
    bch_amount: bitcoincash::Amount,
    xmr_amount: monero::Amount,
) -> anyhow::Result<String> {
    let response = client
        .post(format!("{BASE_URL}/trader"))
        .json(&json!({
           "path": "xmr->bch",
           "timelock1": timelock1,
           "timelock2": timelock2,
           "bch_amount": bch_amount.to_sat(),
           "xmr_amount": xmr_amount.as_pico()
        }))
        .send()
        .await?;

    match response.status() {
        StatusCode::OK => {
            let body = response.json::<serde_json::Value>().await?;
            return Ok(body["trade_id"].as_str().unwrap().to_string());
        }
        code => {
            let body = response.text().await?;
            bail!("[ERROR]: {code} - {body}");
        }
    }
}

async fn get_server_transition(
    client: &reqwest::Client,
    trade_id: &str,
) -> anyhow::Result<Option<Transition>> {
    let response = client
        .get(format!("{BASE_URL}/trader/{trade_id}"))
        .send()
        .await?;

    match response.status() {
        StatusCode::OK => Ok(response.json::<Option<Transition>>().await?),
        code => {
            let body = response.text().await?;
            bail!("[ERROR]: {code} - {body}");
        }
    }
}

async fn send_transition(
    client: &reqwest::Client,
    trade_id: &str,
    transition: &Transition,
) -> anyhow::Result<()> {
    let response = client
        .patch(format!("{BASE_URL}/trader/{trade_id}"))
        .json(transition)
        .send()
        .await?;

    match response.status() {
        StatusCode::OK => Ok(()),
        code => {
            let body = response.text().await?;
            bail!("[ERROR] {code} - {body}");
        }
    }
}

/// This only check if we already sent transition,
/// if we do just skip it and return success
struct TransitionManager {
    enc_sig_sent: bool,
    wrapper: Arc<Mutex<SwapWrapper>>,
}

impl TransitionManager {
    fn new(wrapper: Arc<Mutex<SwapWrapper>>) -> Self {
        Self {
            wrapper,
            enc_sig_sent: false,
        }
    }

    async fn send_transition(&mut self) -> Option<Transition> {
        let guard = self.wrapper.lock().await;
        let transition = match &*guard {
            SwapWrapper::Alice(v) => v.get_transition(),
            SwapWrapper::Bob(v) => v.get_transition(),
        };
        drop(guard);

        if let Some(transition) = transition {
            // Check if we already sent it, skip if we do
            match transition {
                Transition::EncSig(_) if self.enc_sig_sent => return None,
                _ => {
                    return Some(transition);
                }
            }
        }

        None
    }

    fn sent(&mut self, transition: Transition) {
        match transition {
            Transition::EncSig(_) => self.enc_sig_sent = true,
            _ => {}
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let bch_min_confirmation = 1;

    let fullcrum_tcp = "localhost:50001";
    let monero_network = monero::Network::Stagenet;
    let bch_network = bitcoin::Network::Testnet;

    // ===================================================

    let req_client = reqwest::Client::new();
    let socket = TcpStream::connect(fullcrum_tcp).await?;
    let bch_server = Arc::new(blockchain::TcpElectrum::new(socket));

    println!("Subscribing for new block");
    let _ = bch_server
        .send("blockchain.headers.subscribe", json!([]))
        .await?;
    println!("========================================");

    println!("Generating new keys...");
    let recv_pk = random_private_key(bch_network);
    let secp = bitcoincash::secp256k1::Secp256k1::signing_only();
    let recv_pub = recv_pk.public_key(&secp);
    let recv_addr = recv_pub.pubkey_hash();
    let recv_script = bitcoincash::Script::new_p2pkh(&recv_addr);

    let timelock1 = 20;
    let timelock2 = 20;

    let bch_amount = bitcoincash::Amount::from_sat(100000);
    let xmr_amount = monero::Amount::from_pico(100000);

    let swap = alice::Alice {
        state: alice::State::Init,
        swap: Swap {
            id: "".to_owned(),
            keys: KeyPrivate::random(bch_network),

            bch_amount,
            xmr_amount,

            xmr_network: monero_network,
            bch_network,

            bch_recv: recv_script,

            timelock1,
            timelock2,
        },
    };

    let string_json = serde_json::to_string_pretty(&swap.swap.keys).unwrap();
    println!("Private Keys: {string_json}");
    println!("Bch recv private key: {}", recv_pk);

    let swap = Arc::new(Mutex::new(SwapWrapper::Alice(swap)));
    tokio::spawn({
        // process subscription
        let bch_server = bch_server.clone();
        let swap = swap.clone();

        async move {
            let mut receiver = bch_server.subscribe();

            loop {
                let data = receiver.recv().await.unwrap();
                let data = serde_json::from_str::<serde_json::Value>(&data).unwrap();

                let method = data["method"].as_str().unwrap();
                if method != "blockchain.headers.subscribe" {
                    eprintln!("Unknown method: {method}");
                    continue;
                }

                println!("New block found. Rescanning addresses");

                let mut guard = swap.lock().await;
                match &mut *guard {
                    SwapWrapper::Alice(alice) => {
                        let mut runner = alice::Runner {
                            inner: alice.clone(),
                            bch: &bch_server,
                            min_bch_conf: bch_min_confirmation,
                        };
                        let _ = runner.check_bch().await;
                        *alice = runner.inner;
                    }
                    SwapWrapper::Bob(_) => {}
                }
            }
        }
    });
    println!("========================================");

    println!("Creating new trade...");
    let trade_id =
        create_new_trade(&req_client, timelock1, timelock2, bch_amount, xmr_amount).await?;
    println!("Trade id: {trade_id}");
    println!("========================================");

    let mut transition_manager = TransitionManager::new(swap.clone());

    loop {
        if let Some(tr) = transition_manager.send_transition().await {
            match send_transition(&req_client, &trade_id, &tr).await {
                Ok(_) => transition_manager.sent(tr),
                Err(e) => {
                    println!("{:?}", e);
                    sleep(Duration::from_secs(10)).await;
                }
            }
        }

        match get_server_transition(&req_client, &trade_id).await {
            Err(e) => println!("============= {:?}", e),
            Ok(transition) => match transition {
                None => {
                    sleep(Duration::from_secs(5)).await;
                }
                Some(transition) => {
                    let mut guard = swap.lock().await;
                    match &mut *guard {
                        SwapWrapper::Alice(alice) => {
                            let mut runner = alice::Runner {
                                inner: alice.clone(),
                                min_bch_conf: bch_min_confirmation,
                                bch: &bch_server,
                            };

                            if let Err(e) = runner.pub_transition(transition).await {
                                dbg!(e);
                            }

                            *alice = runner.inner;
                        }
                        SwapWrapper::Bob(_) => {}
                    };
                }
            },
        };

        sleep(Duration::from_secs(5)).await;
    }
}
