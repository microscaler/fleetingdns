use std::net::SocketAddr;

use tracing::info;

use common::init_tracing;
use dnsd::Config;

#[tokio::main]
async fn main() -> common::AppResult<()> {
    init_tracing();
    let addr: SocketAddr = "0.0.0.0:5353".parse().unwrap();
    info!(addr=%addr, "dnsd-bin starting server");
    dnsd::serve(Config { addr }).await
}
