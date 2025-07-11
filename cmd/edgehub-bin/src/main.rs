use clap::Parser;
use std::net::SocketAddr;
use tracing::info;

use common::{AppResult, init_tracing, tls};
use edgehub::{self, Config};

/// EdgeHub command line arguments.
#[derive(Parser, Debug, Clone)]
struct Args {
    /// Address to bind the TLS listener.
    #[arg(long, default_value = "0.0.0.0:2222")]
    addr: SocketAddr,
    /// Redis connection URL.
    #[arg(long, default_value = "redis://127.0.0.1:6379")]
    redis: String,
}

async fn run(args: Args) -> AppResult<()> {
    init_tracing();
    info!(addr=%args.addr, "edgehub listening");
    let (tls_config, _) = tls::generate_tls_config(&["ssh"])?;
    let pool = edgehub::redis::new_pool(&args.redis)
        .await
        .map_err(|e| common::AppError::Message(e.to_string()))?;
    edgehub::serve(Config {
        addr: args.addr,
        tls_config,
        redis_pool: pool,
    })
    .await
}

#[tokio::main]
async fn main() -> AppResult<()> {
    let args = Args::parse();
    run(args).await
}
