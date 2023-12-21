use std::time::Duration;

use anyhow::bail;
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::json;

use protocol::{
    alice,
    bitcoincash::{self},
    keys::{
        bitcoin::{self, random_private_key},
        KeyPrivate,
    },
    monero,
    protocol::Swap,
    protocol::{Action, SwapEvents, Transition},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufStream},
    net::TcpStream,
    time::sleep,
};

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
            bail!("Code: {code}\nBody: {body}");
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
            bail!("Code: {code}\nBody: {body}");
        }
    }
}

async fn send_transition(
    client: &reqwest::Client,
    trade_id: &str,
    transition: Transition,
) -> anyhow::Result<()> {
    let response = client
        .patch(format!("{BASE_URL}/trader/{trade_id}"))
        .json(&transition)
        .send()
        .await?;

    match response.status() {
        StatusCode::OK => Ok(()),
        code => {
            let body = response.text().await?;
            bail!("Code: {code}\nBody: {body}");
        }
    }
}

#[derive(Debug, Deserialize)]
struct Unspent {
    height: u64,
    tx_hash: String,
    tx_pos: u64,
    value: u64,
}

async fn watch_bch_address(address: &str) -> anyhow::Result<Vec<Unspent>> {
    let mut bch_server = TcpStream::connect("chipnet.imaginary.cash:50001").await?;

    let payload = json!({
        "id": 0,
        "method": "blockchain.address.listunspent",
        "params": [address, "exclude_tokens"]
    });

    let mut payload = serde_json::to_vec(&payload)?;
    payload.push(b'\n');
    bch_server.write(&payload).await?;

    let mut buf = Vec::new();
    loop {
        bch_server.read_buf(&mut buf).await?;
        if buf.ends_with(&[b'\n']) {
            break;
        }
    }

    #[derive(Deserialize)]
    struct UnspentResponse {
        result: Vec<Unspent>,
    }
    let txs = serde_json::from_slice::<UnspentResponse>(&buf)?.result;
    Ok(txs)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = reqwest::Client::new();

    let bch_network = bitcoin::Network::Testnet;

    println!("Generating new keys...");

    let refund_pk = random_private_key(bch_network);
    let secp = bitcoincash::secp256k1::Secp256k1::signing_only();
    let refund_pub = refund_pk.public_key(&secp);
    let refund_add = refund_pub.pubkey_hash();
    let refund_script = bitcoincash::Script::new_p2pkh(&refund_add);

    let timelock1 = 10000;
    let timelock2 = 10000;

    let swap = Swap {
        id: "".to_owned(),
        keys: KeyPrivate::random(bch_network),

        bch_amount: bitcoincash::Amount::from_sat(1000),
        xmr_amount: monero::Amount::from_pico(1000),

        xmr_network: monero::Network::Stagenet,
        bch_network,

        bch_recv: refund_script,

        timelock1,
        timelock2,
    };

    let mut swap = alice::Alice {
        state: alice::State::Init,
        swap,
    };

    let string_json = serde_json::to_string_pretty(&swap.swap.keys).unwrap();
    println!("Private Keys: {string_json}");
    println!("========================================");

    println!("Creating new trade...");
    let trade_id = create_new_trade(&client, timelock1, timelock2).await?;
    println!("Trade id: {trade_id}");
    println!("========================================");

    loop {
        let transition = swap.get_transition();
        if let Some(transition) = transition {
            send_transition(&client, &trade_id, transition).await?;
        }

        let transition = get_server_transition(&client, &trade_id).await?;
        if let Some(transition) = transition {
            let (action, error) = swap.transition(transition);
            if let Some(action) = action {
                println!("Action: {:?}", action);

                match action {
                    Action::WatchBchAddress(address) => loop {
                        println!("connecting to server");
                        let txs = watch_bch_address(&address).await?;
                        dbg!(txs);
                        sleep(Duration::from_secs(10)).await;
                    },
                    _ => todo!(),
                }
            }
            if let Some(error) = error {
                println!("Error: {:?}", error);
            }
            println!("========================================");
        }
    }
}
