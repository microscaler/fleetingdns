//! FleetingDNS Control CLI
//!
//! Command-line tool for controlling FleetingDNS daemon processes via Unix socket.

use std::path::PathBuf;
use std::time::Duration;

use clap::{Parser, Subcommand};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tracing::{error, info};

use common::shutdown::{ControlCommand, ControlResponse, ShutdownSignal, get_default_socket_path};

#[derive(Parser)]
#[command(name = "fleetingdns-ctl")]
#[command(about = "FleetingDNS daemon control tool")]
#[command(version)]
struct Cli {
    /// Path to control socket
    #[arg(long, short)]
    socket: Option<PathBuf>,

    /// Component name (used to find default socket path)
    #[arg(long, short, default_value = "dnsd")]
    component: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Shutdown the daemon gracefully
    Shutdown {
        /// Shutdown signal type
        #[arg(long, default_value = "graceful")]
        signal: ShutdownSignalArg,
    },
    /// Get daemon status
    Status,
    /// Ping the daemon (health check)
    Ping,
    /// Reload daemon configuration (future use)
    Reload,
}

#[derive(clap::ValueEnum, Clone)]
enum ShutdownSignalArg {
    Graceful,
    Immediate,
    Force,
}

impl From<ShutdownSignalArg> for ShutdownSignal {
    fn from(arg: ShutdownSignalArg) -> Self {
        match arg {
            ShutdownSignalArg::Graceful => ShutdownSignal::Graceful,
            ShutdownSignalArg::Immediate => ShutdownSignal::Immediate,
            ShutdownSignalArg::Force => ShutdownSignal::Force,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    common::init_tracing();

    let cli = Cli::parse();

    // Determine socket path
    let socket_path = cli
        .socket
        .unwrap_or_else(|| get_default_socket_path(&cli.component));

    info!("Connecting to control socket: {}", socket_path.display());

    // Execute command
    match execute_command(&socket_path, &cli.command).await {
        Ok(response) => {
            print_response(&response);
            Ok(())
        }
        Err(e) => {
            error!("Command failed: {}", e);
            std::process::exit(1);
        }
    }
}

async fn execute_command(
    socket_path: &PathBuf,
    command: &Commands,
) -> Result<ControlResponse, Box<dyn std::error::Error>> {
    // Connect to Unix socket
    let stream = UnixStream::connect(socket_path).await?;
    let reader = BufReader::new(stream);

    // Build command
    let control_command = match command {
        Commands::Shutdown { signal } => ControlCommand::Shutdown {
            signal: signal.clone().into(),
        },
        Commands::Status => ControlCommand::Status,
        Commands::Ping => ControlCommand::Ping,
        Commands::Reload => ControlCommand::Reload,
    };

    // Send command
    let command_json = serde_json::to_string(&control_command)?;
    let mut stream = reader.into_inner();
    stream.write_all(command_json.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    stream.flush().await?;

    // Read response
    let mut reader = BufReader::new(stream);
    let mut response_line = String::new();
    reader.read_line(&mut response_line).await?;

    // Parse response
    let response: ControlResponse = serde_json::from_str(response_line.trim())?;
    Ok(response)
}

fn print_response(response: &ControlResponse) {
    println!("Component: {}", response.component);
    println!("Status: {}", response.status);
    println!("Uptime: {}", format_duration(response.uptime));
    println!("Active connections: {}", response.active_connections);
    println!("Shutdown state: {:?}", response.shutdown_state);
}

fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        format!("{hours}h {minutes}m {seconds}s")
    } else if minutes > 0 {
        format!("{minutes}m {seconds}s")
    } else {
        format!("{seconds}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(format_duration(Duration::from_secs(90)), "1m 30s");
        assert_eq!(format_duration(Duration::from_secs(3661)), "1h 1m 1s");
    }

    #[test]
    fn test_shutdown_signal_conversion() {
        assert!(matches!(
            ShutdownSignal::from(ShutdownSignalArg::Graceful),
            ShutdownSignal::Graceful
        ));
        assert!(matches!(
            ShutdownSignal::from(ShutdownSignalArg::Immediate),
            ShutdownSignal::Immediate
        ));
        assert!(matches!(
            ShutdownSignal::from(ShutdownSignalArg::Force),
            ShutdownSignal::Force
        ));
    }
}
