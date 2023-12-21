// #![allow(unused_variables, unused_imports, dead_code)]
use std::{env, net::SocketAddr};

use axum::Router;

mod trader;
pub mod utils;

#[tokio::main]
async fn main() {
    let app = Router::new().nest("/trader", trader::trader());

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
