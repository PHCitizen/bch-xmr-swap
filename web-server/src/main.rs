// #![allow(unused_variables, unused_imports, dead_code)]
use std::{collections::HashMap, env, net::SocketAddr, str::FromStr, sync::Arc, time::Duration};

use axum::Router;
use protocol::{
    blockchain::{self, scan_address_conf_tx, TcpElectrum},
    bob, monero_rpc,
    persist::TradePersist,
    protocol::{SwapEvents, SwapWrapper, Transition},
};
use serde_json::json;
use tokio::{fs, net::TcpStream, sync::Mutex, time::sleep};

use trader::get_file_path;

mod trader;
pub mod utils;

pub struct AppState {
    bch_server: TcpElectrum,
    bch_addrs: Mutex<HashMap<String, String>>,
    monerod: monero_rpc::DaemonJsonRpcClient,
    monero_wallet: Mutex<monero_rpc::WalletClient>,
}

type TAppState = Arc<AppState>;

async fn check_wallet_xmr(state: &TAppState) {
    let base_path = "./.trades/ongoing/";
    let mut entries = fs::read_dir(base_path).await.unwrap();
    while let Some(entry) = entries.next_entry().await.unwrap() {
        if !entry.path().is_file() {
            continue;
        }
        let filename = entry.file_name().into_string().unwrap();
        if !filename.ends_with("-server.json") {
            continue;
        }

        let trade_id = filename.split("-").next().unwrap().to_string();
        let trade = TradePersist::restore(get_file_path(&trade_id))
            .await
            .unwrap();
        match trade.config.swap {
            SwapWrapper::Bob(inner) => {
                let _ = bob::Runner {
                    inner,
                    trade_id,
                    bch: &state.bch_server,
                    monero_wallet: &state.monero_wallet,
                    monerod: &state.monerod,
                }
                .check_xmr()
                .await;
            }
            _ => {}
        }
    }
}

#[tokio::main]
async fn main() {
    let monerod_addr = "http://localhost:18081";
    let monero_wallet_addr = "http://localhost:8081";

    let monerod = monero_rpc::RpcClientBuilder::new()
        .build(monerod_addr)
        .unwrap()
        .daemon();
    let monero_wallet = Mutex::new(
        monero_rpc::RpcClientBuilder::new()
            .build(monero_wallet_addr)
            .unwrap()
            .wallet(),
    );

    let socket = TcpStream::connect("localhost:50001").await.unwrap();
    let bch_server = blockchain::TcpElectrum::new(socket);

    let _ = bch_server
        .send("blockchain.headers.subscribe", json!([]))
        .await
        .unwrap();

    let state = Arc::new(AppState {
        bch_server: bch_server.clone(),
        bch_addrs: Mutex::new(HashMap::new()),
        monerod,
        monero_wallet,
    });

    tokio::spawn({
        let state = state.clone();
        async move {
            loop {
                println!("Checking Wallet XMR...");
                check_wallet_xmr(&state).await;
                sleep(Duration::from_secs(10));
            }
        }
    });

    tokio::spawn({
        let state = state.clone();
        let mut receiver = state.bch_server.subscribe();

        const BCH_MIN_CONFIRMATION: i64 = 2;

        async move {
            loop {
                let data = receiver.recv().await.unwrap();
                let data: serde_json::Value = serde_json::from_str(&data).unwrap();

                if data["method"].as_str().unwrap() != "blockchain.headers.subscribe" {
                    continue;
                }

                println!("New block found. Rescanning addresses");
                let guard = state.bch_addrs.lock().await;
                let addresses = guard.clone();
                drop(guard);

                for (address, trade_id) in addresses {
                    let txs =
                        scan_address_conf_tx(&state.bch_server, &address, BCH_MIN_CONFIRMATION)
                            .await;

                    if let Ok(val) = TradePersist::restore(get_file_path(&trade_id)).await {
                        match val.config.swap {
                            SwapWrapper::Bob(mut v) => {
                                for transaction in txs {
                                    v = v.transition(Transition::BchConfirmedTx(transaction)).0;
                                }
                            }
                            SwapWrapper::Alice(mut v) => {
                                for transaction in txs {
                                    v = v.transition(Transition::BchConfirmedTx(transaction)).0;
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    let app = Router::new().nest("/trader", trader::trader(state));

    let port = env::var("PORT").unwrap_or("8080".to_owned());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .unwrap();

    println!("listening on http://{}", listener.local_addr().unwrap());
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
