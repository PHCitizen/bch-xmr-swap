use std::{collections::HashSet, sync::Arc, time::Duration};

use anyhow::bail;
use reqwest::StatusCode;
use serde_json::json;

use protocol::{
    alice,
    bitcoincash::{self},
    blockchain::{self, scan_address_conf_tx},
    keys::{
        bitcoin::{self, random_private_key},
        KeyPrivate,
    },
    monero::{self},
    protocol::Swap,
    protocol::{Action, SwapEvents, SwapWrapper, Transition},
};
use tokio::{net::TcpStream, sync::Mutex, time::sleep};

const BASE_URL: &str = "http://localhost:8080";

async fn create_new_trade(
    client: &reqwest::Client,
    timelock1: i64,
    timelock2: i64,
) -> anyhow::Result<String> {
    let response = client
        .post(format!("{BASE_URL}/trader"))
        .json(&json!({
           "path": "xmr->bch",
           "timelock1": timelock1,
           "timelock2": timelock2
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
    const BCH_MIN_CONFIRMATION: i64 = 2;
    let bch_network = bitcoin::Network::Testnet;
    // let socket = TcpStream::connect("chipnet.imaginary.cash:50001").await?;
    let req_client = reqwest::Client::new();
    let socket = TcpStream::connect("localhost:50001").await?;
    let bch_server = Arc::new(blockchain::TcpElectrum::new(socket));
    let bch_subcriber_addr = Arc::new(Mutex::new(HashSet::<String>::new()));

    println!("Subscribing for new block");
    let _ = bch_server
        .send("blockchain.headers.subscribe", json!([]))
        .await?;
    println!("========================================");

    println!("Generating new keys...");
    let refund_pk = random_private_key(bch_network);
    let secp = bitcoincash::secp256k1::Secp256k1::signing_only();
    let refund_pub = refund_pk.public_key(&secp);
    let refund_add = refund_pub.pubkey_hash();
    let refund_script = bitcoincash::Script::new_p2pkh(&refund_add);

    let timelock1 = 10000;
    let timelock2 = 10000;

    let swap = alice::Alice {
        state: alice::State::Init,
        swap: Swap {
            id: "".to_owned(),
            keys: KeyPrivate::random(bch_network),

            bch_amount: bitcoincash::Amount::from_sat(1000),
            xmr_amount: monero::Amount::from_pico(1000),

            xmr_network: monero::Network::Stagenet,
            bch_network,

            bch_recv: refund_script,

            timelock1,
            timelock2,
        },
    };

    let string_json = serde_json::to_string_pretty(&swap.swap.keys).unwrap();
    println!("Private Keys: {string_json}");

    let swap = Arc::new(Mutex::new(SwapWrapper::Alice(swap)));
    tokio::spawn({
        // process subscription
        let bch_server = bch_server.clone();
        let swap = swap.clone();
        let bch_subcriber_addr = bch_subcriber_addr.clone();

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
                let guard = bch_subcriber_addr.lock().await;
                let addresses = guard.clone();
                drop(guard);

                for address in addresses {
                    let txs =
                        scan_address_conf_tx(&bch_server, &address, BCH_MIN_CONFIRMATION).await;
                    for tx in txs {
                        let mut guard = swap.lock().await;
                        match &mut *guard {
                            SwapWrapper::Alice(alice) => {
                                *alice = alice.clone().transition(Transition::BchConfirmedTx(tx)).0;
                            }
                            SwapWrapper::Bob(bob) => {
                                *bob = bob.clone().transition(Transition::BchConfirmedTx(tx)).0;
                            }
                        }
                    }
                }
            }
        }
    });
    println!("========================================");

    println!("Creating new trade...");
    let trade_id = create_new_trade(&req_client, timelock1, timelock2).await?;
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
                None => {}
                Some(v) => {
                    let transition = match v {
                        Transition::Msg0 { .. } => v,
                        Transition::Contract { .. } => v,
                        Transition::EncSig(_) => v,
                        _ => {
                            bail!("Private transition receive from server")
                        }
                    };

                    // let (action, error) = swap.lock().await.transition(transition);
                    let mut guard = swap.lock().await;
                    let (actions, error) = match &mut *guard {
                        SwapWrapper::Alice(alice) => {
                            let (new, actions, error) = alice.clone().transition(transition);
                            *alice = new;
                            (actions, error)
                        }
                        SwapWrapper::Bob(bob) => {
                            let (new, actions, error) = bob.clone().transition(transition);
                            *bob = new;
                            (actions, error)
                        }
                    };
                    drop(guard);

                    for action in actions {
                        println!("Action: {:?}", action);

                        match action {
                            Action::WatchBchAddress { swaplock, refund } => {
                                println!("Waiting for bch to be locked");
                                let mut guard = bch_subcriber_addr.lock().await;
                                let _ = guard.insert(swaplock);
                                let _ = guard.insert(refund);
                            }
                            _ => todo!(),
                        }
                    }

                    if let Some(error) = error {
                        println!("Error: {:?}", error);
                    }
                }
            },
        };

        println!("========================================");
        sleep(Duration::from_secs(5)).await;
    }
}
