use std::net::SocketAddr;

use axum::{
    extract::{ConnectInfo, Path},
    http::StatusCode,
    routing::{patch, post},
    Json, Router,
};
use fs4::tokio::AsyncFileExt;
use protocol::{
    alice::{self, Alice},
    bitcoincash,
    bob::{self, Bob},
    keys::{bitcoin::random_private_key, bitcoin::Network, KeyPrivate},
    monero,
    protocol::{Action, Swap, SwapEvents, SwapWrapper, Transition},
};
use serde::{Deserialize, Serialize};
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, ErrorKind},
};

use crate::utils::{random_str, ApiResult, Error, JsonRej};

pub fn trader() -> Router {
    Router::new()
        .route("/", post(create))
        .route("/:trade_id", patch(transition).get(get_transition))
}

// ==========================================
// SECTION: Self
// ==========================================

#[inline]
fn get_file_path(id: &str) -> String {
    format!("./.trades/ongoing/{id}-server.json")
}

#[derive(Serialize, Deserialize)]
struct TradePersist {
    swap: SwapWrapper,
    refund_private_key: bitcoincash::PrivateKey,
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
    let id = random_str(10);

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
        id: id.clone(),
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
        "bch->xmr" => SwapWrapper::Alice(Alice {
            state: alice::State::Init,
            swap,
        }),
        "xmr->bch" => SwapWrapper::Bob(Bob {
            state: bob::State::Init,
            swap,
        }),
        _ => {
            return Err(Error::new(
                StatusCode::NOT_IMPLEMENTED,
                "Pair not available",
            ))
        }
    };

    let serialized = serde_json::to_vec_pretty(&TradePersist {
        swap,
        refund_private_key: refund_priv,
    })?;

    fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(get_file_path(&id))
        .await?
        .write(&serialized)
        .await?;

    println!("[INFO] New Trade: {id}");
    println!("       Client IP: {addr}");

    Ok(Json(CreateResponse { trade_id: id }))
}

// ==========================================
// SECTION: Transition
// ==========================================

#[derive(Serialize)]
struct TransitionResponse {
    error: bool,
}

async fn transition(
    Path(trade_id): Path<String>,
    JsonRej(request): JsonRej<Transition>,
) -> ApiResult<Json<TransitionResponse>> {
    let (tr, transition) = match request {
        Transition::Msg0 { .. } => ("Msg0", request),
        Transition::Contract { .. } => ("Contract", request),
        Transition::EncSig(_) => ("EncSig", request),
        _ => {
            return Err(Error::new(
                StatusCode::FORBIDDEN,
                "Private transition not allowed",
            ))
        }
    };

    println!("Transition `{tr}` for {trade_id}");
    let mut file = match fs::OpenOptions::new()
        .write(true)
        .read(true)
        .open(get_file_path(&trade_id))
        .await
    {
        Err(e) => match e.kind() {
            ErrorKind::NotFound => {
                return Err(Error::new(StatusCode::NOT_FOUND, "Trade id not found"))
            }
            _ => return Err(Error::from(e.to_string())),
        },
        Ok(file) => file,
    };

    let mut buf = Vec::new();
    file.lock_exclusive()?;
    file.read_to_end(&mut buf).await?;

    let mut value = serde_json::from_slice::<TradePersist>(&buf)?;
    let (action, error) = value.swap.transition(transition);

    if let Some(action) = action {
        println!("Action: {:?}", action);

        match action {
            Action::TradeFailed => {
                todo!("delete trade")
            }
            Action::LockBchAndWatchXmr(bch_addr, monero_addr) => {
                // TODO:
            }
            a => todo!("Handle {:?}", a),
        }
    }

    if let Some(e) = error {
        return Err(Error::new(StatusCode::FORBIDDEN, e.to_string()));
    }

    let serialized = serde_json::to_vec_pretty(&value)?;
    file.set_len(0).await?;
    file.rewind().await?;
    file.write(&serialized).await?;
    file.unlock()?;

    Ok(Json(TransitionResponse { error: false }))
}

// ==========================================
// SECTION: Get Transition
// ==========================================

async fn get_transition(Path(trade_id): Path<String>) -> ApiResult<Json<Option<Transition>>> {
    match fs::OpenOptions::new()
        .read(true)
        .open(get_file_path(&trade_id))
        .await
    {
        Err(e) => match e.kind() {
            ErrorKind::NotFound => {
                return Err(Error::new(StatusCode::NOT_FOUND, "Trade id not found"))
            }
            _ => return Err(Error::from(e.to_string())),
        },
        Ok(mut f) => {
            let mut buf = Vec::new();
            f.read_to_end(&mut buf).await?;
            let value = serde_json::from_slice::<TradePersist>(&buf)?;
            Ok(Json(value.swap.get_transition()))
        }
    }
}
