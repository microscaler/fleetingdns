use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;
use tracing::info;

use common::{AppResult, init_tracing, shutdown::GracefulShutdown, tls};
use edgehub::{self, Config, SshConfig, SshServer};

/// EdgeHub command line arguments.
#[derive(Parser, Debug, Clone)]
struct Args {
    /// Address to bind the TLS listener.
    #[arg(long, default_value = "0.0.0.0:8443")]
    addr: SocketAddr,
    /// Address to bind the SSH-over-TLS server (port 443 for corporate firewall bypass).
    #[arg(long, default_value = "0.0.0.0:443")]
    ssh_addr: SocketAddr,
    /// Path to SSH host key file.
    #[arg(long)]
    ssh_host_key: Option<String>,
    /// Public domain for tunnel URLs (e.g., fleetingdns.run).
    #[arg(long, default_value = "fleetingdns.run")]
    public_domain: String,
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
        let config = common::shutdown::ShutdownConfig {
            control_socket_path: socket_path,
            component_name: "edgehub".to_string(),
            graceful_timeout: std::time::Duration::from_secs(args.shutdown_timeout),
            ..Default::default()
        };
        GracefulShutdown::with_config(config)?
    } else {
        GracefulShutdown::new("edgehub")?
    };

    // Start shutdown framework
    shutdown.start().await?;

    info!(
        addr = %args.addr,
        ssh_addr = %args.ssh_addr,
        control_socket = %shutdown.config.control_socket_path.display(),
        "edgehub starting with TLS and SSH servers"
    );

    let (tls_config, _) = tls::generate_tls_config(&["ssh"])?;
    let pool = edgehub::redis::new_pool(&args.redis)
        .await
        .map_err(|e| common::AppError::Message(e.to_string()))?;

    // Create SSH server
    let ssh_config = SshConfig {
        bind_addr: args.ssh_addr,
        host_key_path: args.ssh_host_key,
        public_domain: args.public_domain,
        ca_config: None, // No CA configuration for now
    };
    let ssh_server = SshServer::new(ssh_config)
        .await
        .map_err(|e| common::AppError::Message(e.to_string()))?;

    // Get shutdown signals for both servers
    let tls_shutdown_rx = shutdown.subscribe();
    let ssh_shutdown_rx = shutdown.subscribe();

    // Start both servers concurrently
    let tls_server = edgehub::serve_with_shutdown(
        Config {
            addr: args.addr,
            tls_config,
            redis_pool: pool,
        },
        tls_shutdown_rx,
    );

    let ssh_server_task = ssh_server.run(ssh_shutdown_rx);

    // Run both servers concurrently
    let (tls_result, ssh_result) = tokio::join!(tls_server, ssh_server_task);

    // Wait for graceful shutdown to complete
    shutdown.wait_for_shutdown().await?;

    // Check results
    tls_result?;
    ssh_result.map_err(|e| common::AppError::Message(e.to_string()))?;

    Ok(())
}

#[tokio::main]
async fn main() -> AppResult<()> {
    let args = Args::parse();
    run(args).await
}
