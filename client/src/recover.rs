use std::{env, sync::Arc};

use protocol::{alice, blockchain, persist::TradePersist, protocol::SwapWrapper};
use tokio::net::TcpStream;

pub fn get_file_path(trade_id: &str) -> String {
    format!("./.trades/ongoing/{trade_id}-client.json")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let trade_id = env::args().nth(1).expect("Trade id required");

    let fullcrum_tcp = "localhost:50001";
    let socket = TcpStream::connect(fullcrum_tcp).await?;
    let bch_server = Arc::new(blockchain::TcpElectrum::new(socket));

    let mut trade = TradePersist::restore(get_file_path(&trade_id))
        .await
        .unwrap();
    match trade.config.swap {
        SwapWrapper::Bob(_) => {}
        SwapWrapper::Alice(inner) => {
            let mut runner = alice::Runner {
                inner,
                bch: &bch_server,
                min_bch_conf: 0,
            };
            let _ = runner.check_bch().await;
            trade.config.swap = SwapWrapper::Alice(runner.inner);
            trade.save().await;
        }
    };

    Ok(())
}
