//! `FleetingDNS` Control CLI
//!
//! Command-line tool for controlling `FleetingDNS` daemon processes via Unix socket.

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

#[derive(Subcommand, Clone)]
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

#[derive(clap::ValueEnum, Clone, PartialEq, Debug)]
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
    if let Err(e) = common::init_tracing("fleetingdns-ctl") {
        eprintln!("Failed to initialize tracing: {e}");
    }

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
    use common::shutdown::{ControlCommand, ControlResponse, ShutdownState};
    use std::time::Duration;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixListener;

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

    #[test]
    fn test_format_duration_edge_cases() {
        assert_eq!(format_duration(Duration::from_secs(0)), "0s");
        assert_eq!(format_duration(Duration::from_secs(59)), "59s");
        assert_eq!(format_duration(Duration::from_secs(60)), "1m 0s");
        assert_eq!(format_duration(Duration::from_secs(3600)), "1h 0m 0s");
        assert_eq!(format_duration(Duration::from_mins(61)), "1h 1m 0s");
        assert_eq!(format_duration(Duration::from_secs(7323)), "2h 2m 3s");
    }

    #[test]
    fn test_shutdown_signal_arg_clone() {
        let signal = ShutdownSignalArg::Graceful;
        let cloned = signal.clone();
        assert!(matches!(cloned, ShutdownSignalArg::Graceful));
    }

    #[test]
    fn test_cli_parsing() {
        use clap::Parser;

        // Test basic parsing
        let cli = Cli::try_parse_from(["fleetingdns-ctl", "status"]).unwrap();
        assert_eq!(cli.component, "dnsd");
        assert!(cli.socket.is_none());
        assert!(matches!(cli.command, Commands::Status));

        // Test with component flag
        let cli =
            Cli::try_parse_from(["fleetingdns-ctl", "--component", "edgehub", "ping"]).unwrap();
        assert_eq!(cli.component, "edgehub");
        assert!(matches!(cli.command, Commands::Ping));

        // Test with socket flag
        let cli = Cli::try_parse_from(["fleetingdns-ctl", "--socket", "/tmp/test.sock", "status"])
            .unwrap();
        assert_eq!(cli.socket.unwrap(), PathBuf::from("/tmp/test.sock"));

        // Test shutdown with signal
        let cli =
            Cli::try_parse_from(["fleetingdns-ctl", "shutdown", "--signal", "immediate"]).unwrap();
        match cli.command {
            Commands::Shutdown { signal } => {
                assert_eq!(signal, ShutdownSignalArg::Immediate);
            }
            _ => panic!("Expected shutdown command"),
        }

        // Test version flag
        let result = Cli::try_parse_from(["fleetingdns-ctl", "--version"]);
        assert!(result.is_err()); // Should exit with version info

        // Test help flag
        let result = Cli::try_parse_from(["fleetingdns-ctl", "--help"]);
        assert!(result.is_err()); // Should exit with help info
    }

    #[test]
    fn test_cli_version() {
        let result = Cli::try_parse_from(["fleetingdns-ctl", "--version"]);
        // This will fail because --version exits, but we can test that it's recognized
        assert!(result.is_err());
    }

    #[test]
    fn test_cli_help() {
        let result = Cli::try_parse_from(["fleetingdns-ctl", "--help"]);
        // This will fail because --help exits, but we can test that it's recognized
        assert!(result.is_err());
    }

    #[test]
    fn test_commands_variants() {
        // Test all command variants can be matched
        let commands = vec![
            Commands::Status,
            Commands::Ping,
            Commands::Reload,
            Commands::Shutdown {
                signal: ShutdownSignalArg::Graceful,
            },
        ];

        for cmd in commands {
            match cmd {
                Commands::Status => { /* OK */ }
                Commands::Ping => { /* OK */ }
                Commands::Reload => { /* OK */ }
                Commands::Shutdown { signal: _ } => { /* OK */ }
            }
        }
    }

    #[tokio::test]
    async fn test_execute_command_mock_server() {
        // Create a temporary socket path
        let socket_path = std::env::temp_dir().join("test_fleetingdns_ctl.sock");

        // Remove socket if it exists
        let _ = std::fs::remove_file(&socket_path);

        // Create mock server
        let listener = UnixListener::bind(&socket_path).unwrap();

        let server_handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();

            // Parse the command
            let _command: ControlCommand = serde_json::from_str(line.trim()).unwrap();

            // Send mock response
            let response = ControlResponse {
                component: "test".to_string(),
                status: "running".to_string(),
                uptime: Duration::from_secs(3600),
                active_connections: 5,
                shutdown_state: ShutdownState::Running,
            };

            let response_json = serde_json::to_string(&response).unwrap();
            let mut stream = reader.into_inner();
            stream.write_all(response_json.as_bytes()).await.unwrap();
            stream.write_all(b"\n").await.unwrap();
            stream.flush().await.unwrap();
        });

        // Test execute_command
        let result = execute_command(&socket_path, &Commands::Status).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.component, "test");
        assert_eq!(response.status, "running");
        assert_eq!(response.uptime, Duration::from_secs(3600));
        assert_eq!(response.active_connections, 5);
        assert!(matches!(response.shutdown_state, ShutdownState::Running));

        server_handle.await.unwrap();

        // Clean up
        let _ = std::fs::remove_file(&socket_path);
    }

    #[tokio::test]
    async fn test_execute_command_different_commands() {
        let socket_path = std::env::temp_dir().join("test_fleetingdns_ctl_2.sock");
        let _ = std::fs::remove_file(&socket_path);

        let commands_to_test = vec![
            Commands::Status,
            Commands::Ping,
            Commands::Reload,
            Commands::Shutdown {
                signal: ShutdownSignalArg::Graceful,
            },
            Commands::Shutdown {
                signal: ShutdownSignalArg::Immediate,
            },
            Commands::Shutdown {
                signal: ShutdownSignalArg::Force,
            },
        ];

        for command in commands_to_test {
            let listener = UnixListener::bind(&socket_path).unwrap();

            let command_clone = command.clone();
            let server_handle = tokio::spawn(async move {
                let (stream, _) = listener.accept().await.unwrap();
                let mut reader = BufReader::new(stream);
                let mut line = String::new();
                reader.read_line(&mut line).await.unwrap();

                // Parse and verify the command
                let received_command: ControlCommand = serde_json::from_str(line.trim()).unwrap();

                // Verify command type matches
                match (&command_clone, &received_command) {
                    (Commands::Status, ControlCommand::Status) => { /* OK */ }
                    (Commands::Ping, ControlCommand::Ping) => { /* OK */ }
                    (Commands::Reload, ControlCommand::Reload) => { /* OK */ }
                    (
                        Commands::Shutdown { signal },
                        ControlCommand::Shutdown {
                            signal: recv_signal,
                        },
                    ) => {
                        let expected_signal = ShutdownSignal::from(signal.clone());
                        assert_eq!(recv_signal, &expected_signal);
                    }
                    _ => panic!("Command mismatch"),
                }

                // Send mock response
                let response = ControlResponse {
                    component: "test".to_string(),
                    status: "running".to_string(),
                    uptime: Duration::from_mins(30),
                    active_connections: 3,
                    shutdown_state: ShutdownState::Running,
                };

                let response_json = serde_json::to_string(&response).unwrap();
                let mut stream = reader.into_inner();
                stream.write_all(response_json.as_bytes()).await.unwrap();
                stream.write_all(b"\n").await.unwrap();
                stream.flush().await.unwrap();
            });

            let result = execute_command(&socket_path, &command).await;
            assert!(result.is_ok());

            server_handle.await.unwrap();
            let _ = std::fs::remove_file(&socket_path);
        }
    }

    #[tokio::test]
    async fn test_execute_command_connection_error() {
        let socket_path = std::env::temp_dir().join("nonexistent_socket.sock");
        let _ = std::fs::remove_file(&socket_path);

        let result = execute_command(&socket_path, &Commands::Status).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_command_invalid_response() {
        let socket_path = std::env::temp_dir().join("test_fleetingdns_ctl_invalid.sock");
        let _ = std::fs::remove_file(&socket_path);

        let listener = UnixListener::bind(&socket_path).unwrap();

        let server_handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();

            // Send invalid JSON response
            let mut stream = reader.into_inner();
            stream.write_all(b"invalid json\n").await.unwrap();
            stream.flush().await.unwrap();
        });

        let result = execute_command(&socket_path, &Commands::Status).await;
        assert!(result.is_err());

        server_handle.await.unwrap();
        let _ = std::fs::remove_file(&socket_path);
    }

    #[test]
    fn test_print_response() {
        let response = ControlResponse {
            component: "test-component".to_string(),
            status: "healthy".to_string(),
            uptime: Duration::from_secs(7323), // 2h 2m 3s
            active_connections: 42,
            shutdown_state: ShutdownState::Running,
        };

        // We can't easily test stdout, but we can verify the function doesn't panic
        print_response(&response);
    }

    #[test]
    fn test_default_socket_path_usage() {
        // Test that get_default_socket_path is called with the component name
        let path = get_default_socket_path("test-component");
        assert!(path.to_string_lossy().contains("test-component"));
    }

    #[test]
    fn test_shutdown_signal_arg_value_enum() {
        use clap::ValueEnum;

        // Test that all variants can be parsed
        assert!(ShutdownSignalArg::from_str("graceful", true).is_ok());
        assert!(ShutdownSignalArg::from_str("immediate", true).is_ok());
        assert!(ShutdownSignalArg::from_str("force", true).is_ok());

        // Test invalid variant
        assert!(ShutdownSignalArg::from_str("invalid", true).is_err());
    }

    #[test]
    fn test_cli_struct_fields() {
        let cli = Cli {
            socket: Some(PathBuf::from("/test/path")),
            component: "test-component".to_string(),
            command: Commands::Status,
        };

        assert_eq!(cli.socket, Some(PathBuf::from("/test/path")));
        assert_eq!(cli.component, "test-component");
        assert!(matches!(cli.command, Commands::Status));
    }

    #[test]
    fn test_format_duration_large_values() {
        // Test very large durations
        assert_eq!(format_duration(Duration::from_hours(24)), "24h 0m 0s");
        assert_eq!(format_duration(Duration::from_secs(90061)), "25h 1m 1s");
    }
}
