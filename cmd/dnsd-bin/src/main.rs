use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;
use tracing::info;

use common::shutdown::GracefulShutdown;
#[cfg(feature = "dot")]
use common::tls;

#[derive(Parser)]
#[command(name = "dnsd-bin")]
#[command(about = "FleetingDNS DNS server")]
struct Args {
    /// Address to bind to (e.g., 127.0.0.1:6353 or 0.0.0.0:6353)
    #[arg(long, default_value = "0.0.0.0:6353")]
    addr: SocketAddr,

    /// Path to control socket for graceful shutdown
    #[arg(long)]
    control_socket: Option<PathBuf>,

    /// Timeout for graceful shutdown in seconds
    #[arg(long, default_value = "30")]
    shutdown_timeout: u64,
}

#[tokio::main]
async fn main() -> common::AppResult<()> {
    let args = Args::parse();

    // Initialize comprehensive telemetry
    let telemetry_config = common::telemetry::TelemetryConfig {
        service_name: "dnsd".to_string(),
        service_version: "0.1.0".to_string(),
        environment: std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
        ..Default::default()
    };

    common::telemetry::init_telemetry(telemetry_config)
        .map_err(|e| common::AppError::Message(format!("Failed to initialize telemetry: {e}")))?;

    // Initialize graceful shutdown framework
    let mut shutdown = if let Some(socket_path) = args.control_socket {
        let config = common::shutdown::ShutdownConfig {
            control_socket_path: socket_path,
            component_name: "dnsd".to_string(),
            graceful_timeout: std::time::Duration::from_secs(args.shutdown_timeout),
            ..Default::default()
        };
        GracefulShutdown::with_config(config)?
    } else {
        GracefulShutdown::new("dnsd")?
    };

    // Start shutdown framework (signal handlers and control socket)
    shutdown.start().await?;

    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
    let pool = common::redis::new_pool(&redis_url)
        .await
        .map_err(|e| common::AppError::Message(e.to_string()))?;

    info!(
        addr = %args.addr,
        control_socket = %shutdown.config.control_socket_path.display(),
        "dnsd-bin starting server with graceful shutdown support"
    );

    #[cfg(feature = "dot")]
    let (tls_config, _) = tls::generate_tls_config(&["dot"])?;

    // Start DNS server with shutdown signal
    let serve_result = dnsd::serve_with_shutdown(
        dnsd::Config {
            addr: args.addr,
            redis_pool: pool,
            ddos_config: common::ddos_protection::DdosConfig::default(),
            enable_ddos_protection: true,
            dnssec_config: dnsd::sign::DnssecConfig::default(),
            performance_config: dnsd::dns_handler::PerformanceConfig::default(),
        },
        shutdown.clone(),
    )
    .await;

    // Wait for graceful shutdown to complete
    shutdown.wait_for_shutdown().await?;

    serve_result
}
