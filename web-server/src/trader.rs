use std::{fs, io::Write, net::SocketAddr};

use axum::{
    extract::{ConnectInfo, Path, State},
    http::StatusCode,
    routing::{patch, post},
    Json, Router,
};
use protocol::{
    alice::{self, Alice},
    bitcoincash,
    bob::{self, Bob},
    keys::{bitcoin::random_private_key, bitcoin::Network, KeyPrivate},
    monero, monero_rpc,
    persist::{Config, Error as PersistError, TradePersist},
    protocol::{Action, Swap, SwapEvents, SwapWrapper, Transition},
};
use serde::{Deserialize, Serialize};

use crate::{
    utils::{random_str, ApiResult, Error, JsonRej},
    TAppState,
};

pub fn trader(state: TAppState) -> Router {
    Router::new()
        .route("/", post(create))
        .route("/:trade_id", patch(transition).get(get_transition))
        .with_state(state)
}

#[inline]
pub fn get_file_path(trade_id: &str) -> String {
    format!("./.trades/ongoing/{trade_id}-server.json")
}

// ==========================================
// SECTION: Create Trade
// ==========================================

#[derive(Deserialize)]
struct CreateRequest {
    path: String,
    timelock1: i64,
    timelock2: i64,
}

#[derive(Debug, Serialize)]
struct CreateResponse {
    trade_id: String,
}

async fn create(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    JsonRej(request): JsonRej<CreateRequest>,
) -> ApiResult<Json<CreateResponse>> {
    let trade_id = random_str(10);

    let bch_network = Network::Testnet;
    let xmr_network = monero::Network::Stagenet;

    let bch_amount = bitcoincash::Amount::from_sat(1000);
    let xmr_amount = monero::Amount::from_pico(10000);

    let (refund_priv, refund_script) = {
        let refund_priv = random_private_key(bch_network);
        let secp = bitcoincash::secp256k1::Secp256k1::signing_only();
        let refund_pkh = refund_priv.public_key(&secp).pubkey_hash();
        let script = bitcoincash::Script::new_p2pkh(&refund_pkh);
        (refund_priv, script)
    };

    let swap = Swap {
        id: trade_id.clone(),
        keys: KeyPrivate::random(bch_network),
        bch_amount,
        xmr_amount,
        xmr_network,
        bch_network,
        bch_recv: refund_script,
        timelock1: request.timelock1,
        timelock2: request.timelock2,
    };

    let swap = match request.path.as_str() {
        // TODO:
        // "bch->xmr" => SwapWrapper::Alice(Alice {
        //     state: alice::State::Init,
        //     swap,
        // }),
        "xmr->bch" => SwapWrapper::Bob(Bob::new(swap)),
        _ => {
            return Err(Error::new(
                StatusCode::NOT_IMPLEMENTED,
                "Pair not available",
            ))
        }
    };

    let serialized = serde_json::to_vec_pretty(&Config {
        swap,
        refund_private_key: refund_priv,
    })?;

    fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(get_file_path(&trade_id))?
        .write(&serialized)?;

    println!("[INFO] New Trade: {trade_id}");
    println!("       Client IP: {addr}");

    Ok(Json(CreateResponse { trade_id }))
}

// ==========================================
// SECTION: Transition
// ==========================================

#[derive(Serialize)]
struct TransitionResponse {
    error: bool,
}

async fn transition(
    State(state): State<TAppState>,
    Path(trade_id): Path<String>,
    JsonRej(request): JsonRej<Transition>,
) -> ApiResult<Json<TransitionResponse>> {
    // ! we always open the file even on private transition
    // ! we can put a matcher here to reduce file opening

    let mut trade = match TradePersist::restore(get_file_path(&trade_id)).await {
        Ok(v) => v,
        Err(e) => match e {
            PersistError::NotFound => {
                return Err(Error::new(StatusCode::NOT_FOUND, "Trade id not found"))
            }
            PersistError::Unknown(e) => return Err(Error::from(e.to_string())),
        },
    };

    match trade.config.swap {
        SwapWrapper::Bob(inner) => {
            let mut bob = bob::Runner {
                inner,
                trade_id,
                bch: &state.bch_server,
                monero_wallet: &state.monero_wallet,
                monerod: &state.monerod,
            };
            bob.pub_transition(request).await?;

            trade.config.swap = SwapWrapper::Bob(bob.inner);
            trade.save();
        }
        SwapWrapper::Alice(_) => {}
    }

    Ok(Json(TransitionResponse { error: false }))
}

// ==========================================
// SECTION: Get Transition
// ==========================================

async fn get_transition(Path(trade_id): Path<String>) -> ApiResult<Json<Option<Transition>>> {
    match TradePersist::restore(get_file_path(&trade_id)).await {
        Ok(value) => match value.config.swap {
            SwapWrapper::Alice(alice) => Ok(Json(alice.get_transition())),
            SwapWrapper::Bob(bob) => Ok(Json(bob.get_transition())),
        },
        Err(e) => match e {
            PersistError::NotFound => {
                return Err(Error::new(StatusCode::NOT_FOUND, "Trade id not found"))
            }
            PersistError::Unknown(e) => return Err(Error::from(e.to_string())),
        },
    }
}
