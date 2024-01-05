// #![allow(unused_variables, unused_imports, dead_code)]
use std::{env, net::SocketAddr, sync::Arc, time::Duration};

use axum::Router;
use protocol::{
    alice,
    blockchain::{self, TcpElectrum},
    bob,
    keys::bitcoin::Network,
    monero, monero_rpc,
    persist::TradePersist,
    protocol::SwapWrapper,
};
use serde_json::json;
use tokio::{fs, net::TcpStream, sync::Mutex, time::sleep};

use trader::get_file_path;

mod trader;
pub mod utils;

pub struct AppState {
    bch_server: TcpElectrum,
    monerod: monero_rpc::DaemonJsonRpcClient,
    monero_wallet: Mutex<monero_rpc::WalletClient>,
    bch_min_conf: u32,
    monero_network: monero::Network,
    bch_network: Network,
}

type TAppState = Arc<AppState>;

async fn check_xmr_wallets(state: &TAppState) {
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
        let mut trade = TradePersist::restore(get_file_path(&trade_id))
            .await
            .unwrap();
        match trade.config.swap {
            SwapWrapper::Bob(inner) => {
                let mut runner = bob::Runner {
                    inner,
                    trade_id,
                    bch: &state.bch_server,
                    monero_wallet: &state.monero_wallet,
                    monerod: &state.monerod,
                    min_bch_conf: state.bch_min_conf,
                };
                let _ = runner.check_xmr().await;
                trade.config.swap = SwapWrapper::Bob(runner.inner);
            }
            _ => {}
        }
        trade.save().await;
    }
}

async fn check_bch_wallets(state: &TAppState) {
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
        let mut trade = TradePersist::restore(get_file_path(&trade_id))
            .await
            .unwrap();

        match trade.config.swap {
            SwapWrapper::Bob(bob) => {
                let mut runner = bob::Runner {
                    trade_id,
                    inner: bob,
                    bch: &state.bch_server,
                    min_bch_conf: state.bch_min_conf,
                    monerod: &state.monerod,
                    monero_wallet: &state.monero_wallet,
                };
                let _ = runner.check_bch().await;
                trade.config.swap = SwapWrapper::Bob(runner.inner);
            }
            SwapWrapper::Alice(alice) => {
                let mut runner = alice::Runner {
                    inner: alice,
                    bch: &state.bch_server,
                    min_bch_conf: state.bch_min_conf,
                };
                let _ = runner.check_bch().await;
                trade.config.swap = SwapWrapper::Alice(runner.inner);
            }
        }
        trade.save().await;
    }
}

#[tokio::main]
async fn main() {
    let bch_min_conf = 1;

    let monerod_addr = "http://localhost:18081";
    let monero_wallet_addr = "http://localhost:8081";
    let fullcrum_tcp = "localhost:50001";

    let monero_network = monero::Network::Stagenet;
    let bch_network = Network::Testnet;

    // ===================================================

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

    let socket = TcpStream::connect(fullcrum_tcp).await.unwrap();
    let bch_server = blockchain::TcpElectrum::new(socket);

    let state = Arc::new(AppState {
        bch_server: bch_server.clone(),
        monerod,
        monero_wallet,
        bch_min_conf,
        monero_network,
        bch_network,
    });

    tokio::spawn({
        let state = state.clone();
        async move {
            loop {
                println!("Checking Wallet XMR...");
                check_xmr_wallets(&state).await;
                sleep(Duration::from_secs(20)).await;
            }
        }
    });

    tokio::spawn({
        let state = state.clone();
        let mut receiver = state.bch_server.subscribe();
        let _ = state
            .bch_server
            .send("blockchain.headers.subscribe", json!([]))
            .await
            .unwrap();

        async move {
            loop {
                let data = receiver.recv().await.unwrap();
                let data: serde_json::Value = serde_json::from_str(&data).unwrap();

                if data["method"].as_str().unwrap() != "blockchain.headers.subscribe" {
                    continue;
                }

                println!("New block found. Rescanning addresses");
                check_bch_wallets(&state).await
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
