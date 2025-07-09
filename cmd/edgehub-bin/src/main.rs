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
}

async fn run(args: Args) -> AppResult<()> {
    init_tracing();
    info!(addr=%args.addr, "edgehub listening");
    let (tls_config, _) = tls::generate_tls_config(&["ssh"])?;
    edgehub::serve(Config {
        addr: args.addr,
        tls_config,
    })
    .await
}

#[tokio::main]
async fn main() -> AppResult<()> {
    let args = Args::parse();
    run(args).await
}
