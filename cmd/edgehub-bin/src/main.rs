use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;
use tracing::info;

use common::{AppResult, init_tracing, tls, shutdown::GracefulShutdown};
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
    /// Path to control socket for graceful shutdown
    #[arg(long)]
    control_socket: Option<PathBuf>,
    /// Timeout for graceful shutdown in seconds
    #[arg(long, default_value = "30")]
    shutdown_timeout: u64,
}

async fn run(args: Args) -> AppResult<()> {
    init_tracing();
    
    // Initialize graceful shutdown framework
    let mut shutdown = if let Some(socket_path) = args.control_socket {
        let mut config = common::shutdown::ShutdownConfig::default();
        config.control_socket_path = socket_path;
        config.component_name = "edgehub".to_string();
        config.graceful_timeout = std::time::Duration::from_secs(args.shutdown_timeout);
        GracefulShutdown::with_config(config)?
    } else {
        GracefulShutdown::new("edgehub")?
    };
    
    // Start shutdown framework
    shutdown.start().await?;
    
    info!(
        addr = %args.addr,
        control_socket = %shutdown.config.control_socket_path.display(),
        "edgehub starting with graceful shutdown support"
    );
    
    let (tls_config, _) = tls::generate_tls_config(&["ssh"])?;
    let pool = edgehub::redis::new_pool(&args.redis)
        .await
        .map_err(|e| common::AppError::Message(e.to_string()))?;
    
    // Start EdgeHub server with shutdown signal
    let shutdown_rx = shutdown.subscribe();
    let serve_result = edgehub::serve_with_shutdown(
        Config {
            addr: args.addr,
            tls_config,
            redis_pool: pool,
        },
        shutdown_rx,
    )
    .await;
    
    // Wait for graceful shutdown to complete
    shutdown.wait_for_shutdown().await?;
    
    serve_result
}

#[tokio::main]
async fn main() -> AppResult<()> {
    let args = Args::parse();
    run(args).await
}
