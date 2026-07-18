use clap::{Parser, Subcommand};
use tracing::{error, info};

mod config;
mod ssh_client;
mod ssh_keys;
mod tunnel;

use ssh_keys::SshKeyManager;
use tunnel::TunnelManager;

#[derive(Parser)]
#[command(name = "edf")]
#[command(about = "FleetingDNS CLI for secure reverse tunnels")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(propagate_version = true)]
#[command(disable_help_flag = true)]
struct Cli {
    #[cfg(feature = "dev-overrides")]
    /// Override API URL (default: https://api.edf.run)
    #[arg(long)]
    api_url: Option<String>,

    #[cfg(feature = "dev-overrides")]
    /// Override Hub URL (default: https://hub.edf.run)
    #[arg(long)]
    hub_url: Option<String>,

    /// Enable verbose logging
    #[arg(long, short = 'v')]
    verbose: bool,

    /// Show configuration and exit
    #[arg(long)]
    show_config: bool,

    /// Print help information
    #[arg(long, short = 'h')]
    help: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Forward a local port through a secure tunnel
    Forward {
        /// Local port to forward
        #[arg(value_name = "PORT")]
        port: u16,

        /// Tunnel TTL in seconds (default: 1800)
        #[arg(long, default_value = "1800")]
        ttl: u32,

        /// Custom subdomain (optional)
        #[arg(long)]
        subdomain: Option<String>,
    },

    /// Manage SSH keys
    Keys {
        #[command(subcommand)]
        command: KeyCommands,
    },

    /// List active tunnels
    List,

    /// Close a tunnel
    Close {
        /// Tunnel ID
        #[arg(value_name = "ID")]
        id: String,
    },
}

#[derive(Subcommand)]
enum KeyCommands {
    /// Request new SSH key pair from API
    Request,

    /// Show current key status
    Status,

    /// Clean up SSH session
    Cleanup,

    /// Test key storage (for development)
    Test,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing with better error handling
    if let Err(e) = tracing_subscriber::fmt::try_init() {
        eprintln!("⚠️ Warning: Failed to initialize logging: {}", e);
        eprintln!("   Continuing without structured logging...");
    }

    let cli = Cli::parse();

    // Set up verbose logging if requested
    if cli.verbose {
        std::env::set_var("RUST_LOG", "debug");
        // Re-initialize tracing with debug level
        if let Err(e) = tracing_subscriber::fmt::try_init() {
            eprintln!("⚠️ Warning: Failed to initialize verbose logging: {}", e);
        } else {
            info!("Verbose logging enabled");
        }
    }

    // Auto-detect development mode based on build profile
    let is_dev_build = cfg!(debug_assertions);

    // Load configuration with better error handling
    let mut config = match config::CliConfig::load() {
        Ok(config) => {
            info!("Configuration loaded successfully");
            config
        }
        Err(e) => {
            eprintln!("⚠️ Failed to load config: {}", e);
            eprintln!("   Using default configuration...");
            config::CliConfig::default()
        }
    };

    if is_dev_build {
        // In debug builds, automatically set development endpoints if not overridden
        if config.api_url == "https://api.edf.run" {
            config.api_url = "http://localhost:8880".to_string();
            info!("🔧 Auto-detected development mode: API URL set to localhost");
        }
        if config.hub_url == "https://hub.edf.run" {
            config.hub_url = "localhost:2222".to_string();
            info!("🔧 Auto-detected development mode: Hub URL set to localhost");
        }
    } else {
        // In release builds, ensure production endpoints
        if config.api_url != "https://api.edf.run" {
            config.api_url = "https://api.edf.run".to_string();
            info!("🚀 Production mode: API URL set to production endpoint");
        }
        if config.hub_url != "https://hub.edf.run" {
            config.hub_url = "https://hub.edf.run".to_string();
            info!("🚀 Production mode: Hub URL set to production endpoint");
        }
    }

    #[cfg(feature = "dev-overrides")]
    {
        // Apply CLI overrides with validation
        if let Some(api_url) = cli.api_url {
            if !api_url.starts_with("http://") && !api_url.starts_with("https://") {
                eprintln!("⚠️ Warning: API URL should start with http:// or https://");
            }
            println!("🔧 Overriding API URL: {}", api_url);
            config.api_url = api_url;
        }

        if let Some(hub_url) = cli.hub_url {
            if hub_url.contains("://") {
                eprintln!("⚠️ Warning: Hub URL should not include protocol (use localhost:2222, not http://localhost:2222)");
            }
            println!("🔧 Overriding Hub URL: {}", hub_url);
            config.hub_url = hub_url;
        }

        // Display final configuration
        println!("🔧 Final Configuration:");
        println!("   API URL: {}", config.api_url);
        println!("   Hub URL: {}", config.hub_url);
        println!("   Key Directory: {}", config.key_directory.display());
        println!("   Default TTL: {} seconds", config.default_ttl);
        println!("   Log Level: {}", config.log_level);
        println!(
            "   Build Profile: {}",
            if is_dev_build {
                "debug (dev mode)"
            } else {
                "release"
            }
        );
    }

    // Set environment variables for other components
    std::env::set_var("EDF_API_URL", &config.api_url);

    // Validate configuration before proceeding
    if let Err(e) = validate_config(&config) {
        eprintln!("❌ Configuration validation failed: {}", e);
        eprintln!("   Please check your configuration and try again");
        std::process::exit(1);
    }

    // Handle --show-config flag
    if cli.show_config {
        display_configuration(&config, is_dev_build);
        return Ok(());
    }

    // Handle --help flag
    if cli.help {
        println!("FleetingDNS CLI for secure reverse tunnels");
        println!("Version: {}", env!("CARGO_PKG_VERSION"));
        println!();
        println!("Usage: edf [OPTIONS] <COMMAND>");
        println!();
        println!("Commands:");
        println!("  forward <PORT>     Forward a local port through a secure tunnel");
        println!("  keys               Manage SSH keys");
        println!("  list               List active tunnels");
        println!("  close <ID>         Close a tunnel");
        println!();
        println!("Options:");
        println!("  -v, --verbose      Enable verbose logging");
        println!("      --show-config  Show configuration and exit");

        #[cfg(feature = "dev-overrides")]
        {
            println!("      --api-url      Override API URL (development only)");
            println!("      --hub-url      Override Hub URL (development only)");
        }

        println!("  -h, --help         Print this help message");
        println!("  -V, --version      Print version information");
        println!();
        println!("Examples:");
        println!("  edf forward 8080                    # Forward port 8080");
        println!("  edf keys request                     # Request new SSH keys");
        println!("  edf --show-config                    # Show current configuration");
        println!("  edf --verbose forward 3000          # Forward port 3000 with verbose logging");

        #[cfg(feature = "dev-overrides")]
        {
            println!("  edf --api-url http://localhost:8880 forward 3000  # Override API URL");
            println!("  edf --hub-url localhost:2222 forward 3000         # Override Hub URL");
        }

        println!();
        println!("For more information, visit: https://github.com/microscaler/fleetingdns");
        return Ok(());
    }

    // Execute command with comprehensive error handling
    match execute_command(cli.command, &config).await {
        Ok(()) => {
            info!("Command executed successfully");
            Ok(())
        }
        Err(e) => {
            error!("Command execution failed: {}", e);

            // Provide helpful error messages based on error type
            if let Some(io_error) = e.downcast_ref::<std::io::Error>() {
                match io_error.kind() {
                    std::io::ErrorKind::ConnectionRefused => {
                        eprintln!("❌ Connection refused. Please check:");
                        eprintln!("   - Are the services running?");
                        eprintln!("   - Are the URLs correct?");
                        eprintln!("   - Is Docker Compose running?");
                    }
                    std::io::ErrorKind::TimedOut => {
                        eprintln!("❌ Connection timed out. Please check:");
                        eprintln!("   - Network connectivity");
                        eprintln!("   - Firewall settings");
                        eprintln!("   - Service responsiveness");
                    }
                    _ => {
                        eprintln!("❌ I/O error: {}", io_error);
                    }
                }
            } else if e.to_string().contains("authentication") {
                eprintln!("❌ Authentication failed. Please check:");
                eprintln!("   - Your credentials are correct");
                eprintln!("   - You have the necessary permissions");
                eprintln!("   - The service is accessible");
            } else {
                eprintln!("❌ Unexpected error: {}", e);
            }

            // Provide development hints
            if is_dev_build {
                eprintln!("\n💡 Development hints:");
                eprintln!("   - Debug build detected: endpoints auto-set to localhost");
                eprintln!("   - Check Docker Compose logs: docker compose logs");
                eprintln!("   - Verify service health: docker compose ps");
            }

            #[cfg(feature = "dev-overrides")]
            {
                eprintln!("   - Use --api-url and --hub-url to override endpoints");
            }

            Err(e)
        }
    }
}

/// Validate configuration before execution
fn validate_config(config: &config::CliConfig) -> Result<(), Box<dyn std::error::Error>> {
    // Validate TTL range
    if config.default_ttl < 60 || config.default_ttl > 86400 {
        return Err("Default TTL must be between 60 and 86400 seconds".into());
    }

    // Validate log level
    let valid_log_levels = ["trace", "debug", "info", "warn", "error"];
    if !valid_log_levels.contains(&config.log_level.as_str()) {
        return Err(format!(
            "Invalid log level '{}'. Must be one of: {}",
            config.log_level,
            valid_log_levels.join(", ")
        )
        .into());
    }

    Ok(())
}

/// Execute the specified command with comprehensive error handling
async fn execute_command(
    command: Option<Commands>,
    config: &config::CliConfig,
) -> anyhow::Result<()> {
    match command {
        Some(Commands::Forward {
            port,
            ttl,
            subdomain,
        }) => {
            info!("Starting tunnel for port {} with TTL {}", port, ttl);

            // Validate port range
            if port < 1024 {
                eprintln!("⚠️ Warning: Port {} is below 1024 (privileged range)", port);
                eprintln!("   This may require elevated permissions");
            }

            // Validate TTL
            if ttl < 60 {
                return Err(anyhow::anyhow!("TTL must be at least 60 seconds"));
            }
            if ttl > 86400 {
                return Err(anyhow::anyhow!(
                    "TTL cannot exceed 86400 seconds (24 hours)"
                ));
            }

            let mut tunnel_manager = TunnelManager::new()?;

            // Extract hub configuration from CLI config
            let hub_port = if config.hub_url.contains(':') {
                config
                    .hub_url
                    .split(':')
                    .nth(1)
                    .and_then(|p| p.parse::<u16>().ok())
                    .unwrap_or(2222)
            } else {
                2222
            };

            tunnel_manager
                .forward(
                    port,
                    ttl,
                    subdomain,
                    Some(config.hub_url.clone()),
                    Some(hub_port),
                )
                .await?;

            println!("✅ Tunnel creation initiated successfully!");
            println!("   Note: This creates the tunnel via API. SSH connection to EdgeHub not yet implemented.");
            println!("   See T-26b in production readiness PRD for full functionality.");
        }

        Some(Commands::Keys { command }) => {
            let key_manager = SshKeyManager::new()?;

            match command {
                KeyCommands::Request => {
                    info!("Requesting new SSH key pair from API");
                    let key_pair = key_manager.request_key_pair(1800, 8080, None).await?;

                    // Get the private key path for user feedback
                    let ssh_dir = dirs::home_dir()
                        .unwrap_or_else(|| std::path::PathBuf::from("."))
                        .join(".ssh");
                    let expiry_str = key_pair.expires_at.format("%Y%m%d-%H%M%S").to_string();
                    let filename = format!("edf-cli-{}-{}.priv", expiry_str, key_pair.session_id);
                    let private_key_path = ssh_dir.join(&filename);

                    println!("✅ SSH key pair requested successfully");
                    println!("   Fingerprint: {}", key_pair.fingerprint);
                    println!("   Type: {}", key_pair.key_type);
                    println!("   Session ID: {}", key_pair.session_id);
                    println!("   Expires: {}", key_pair.expires_at);
                    println!("   Private key stored at: {}", private_key_path.display());
                    println!(
                        "   ⚠️  This key will be automatically removed when the session expires"
                    );
                }

                KeyCommands::Status => {
                    info!("Checking SSH key status");
                    match key_manager.load_existing_key_pair() {
                        Ok(key_pair) => {
                            println!("✅ SSH keys found");
                            println!("   Fingerprint: {}", key_pair.fingerprint);
                            println!("   Type: {}", key_pair.key_type);
                            println!("   Session ID: {}", key_pair.session_id);
                            println!("   Created: {}", key_pair.created_at);
                            println!("   Expires: {}", key_pair.expires_at);

                            // Check if key is expired
                            if key_pair.expires_at < chrono::Utc::now() {
                                println!("   ⚠️  WARNING: This key has expired!");
                                println!("   💡 Run 'edf keys cleanup' to remove expired keys");
                            }
                        }
                        Err(ssh_keys::SshKeyError::NotFound(_)) => {
                            println!("❌ No SSH keys found");
                            println!("   💡 Run 'edf keys request' to create new keys");
                        }
                        Err(e) => {
                            error!("Failed to check key status: {:?}", e);
                            return Err(anyhow::anyhow!("Key status check failed: {:?}", e));
                        }
                    }
                }

                KeyCommands::Cleanup => {
                    info!("Cleaning up SSH session");
                    match key_manager.cleanup_session() {
                        Ok(()) => println!("✅ SSH session cleaned up successfully"),
                        Err(e) => {
                            eprintln!("⚠️ Warning: Some cleanup operations failed: {}", e);
                            eprintln!("   You may need to manually remove expired keys");
                        }
                    }
                }

                KeyCommands::Test => {
                    info!("Testing SSH key storage");
                    let key_pair = key_manager.test_key_storage().await?;
                    println!("✅ Test SSH key pair created successfully");
                    println!("   Fingerprint: {}", key_pair.fingerprint);
                    println!("   Type: {}", key_pair.key_type);
                    println!("   Session ID: {}", key_pair.session_id);
                    println!("   Expires: {}", key_pair.expires_at);

                    // Get the private key path for user feedback
                    let ssh_dir = dirs::home_dir()
                        .unwrap_or_else(|| std::path::PathBuf::from("."))
                        .join(".ssh");
                    let expiry_str = key_pair.expires_at.format("%Y%m%d-%H%M%S").to_string();
                    let filename = format!("edf-cli-{}-{}.priv", expiry_str, key_pair.session_id);
                    let private_key_path = ssh_dir.join(&filename);
                    println!("   Private key stored at: {}", private_key_path.display());

                    println!("   💡 This is a test key for development purposes only");
                }
            }
        }

        Some(Commands::List) => {
            info!("Listing active tunnels");
            let tunnel_manager = TunnelManager::new()?;
            match tunnel_manager.list_tunnels().await {
                Ok(()) => println!("✅ Tunnel listing completed"),
                Err(e) => {
                    eprintln!("⚠️ Warning: Some tunnel operations failed: {}", e);
                    println!("   This may be expected if no tunnels are active");
                }
            }
        }

        Some(Commands::Close { id }) => {
            info!("Closing tunnel {}", id);
            let mut tunnel_manager = TunnelManager::new()?;
            match tunnel_manager.close_tunnel(&id).await {
                Ok(()) => println!("✅ Tunnel {} closed successfully", id),
                Err(e) => {
                    eprintln!("❌ Failed to close tunnel {}: {}", id, e);
                    eprintln!("   The tunnel may have already been closed or may not exist");
                }
            }
        }

        None => {
            // If no subcommand, show help
            println!("FleetingDNS CLI for secure reverse tunnels");
            println!("Usage: edf <COMMAND>");
            println!("Use --help for more information");
            println!("Use --show-config to see current configuration");
        }
    }

    Ok(())
}

/// Display current configuration
fn display_configuration(config: &config::CliConfig, is_dev_build: bool) {
    println!("🔧 FleetingDNS CLI Configuration");
    println!("=================================");
    println!("API URL: {}", config.api_url);
    println!("Hub URL: {}", config.hub_url);
    println!("Key Directory: {}", config.key_directory.display());
    println!("Default TTL: {} seconds", config.default_ttl);
    println!("Log Level: {}", config.log_level);
    println!(
        "Build Profile: {}",
        if is_dev_build {
            "debug (dev mode)"
        } else {
            "release"
        }
    );

    if is_dev_build {
        println!("\n💡 Development Mode Active:");
        println!("   - Endpoints automatically set to localhost");
        println!("   - Perfect for Docker Compose development");
        println!("   - Use --api-url and --hub-url to override if needed");
    } else {
        println!("\n🚀 Production Mode Active:");
        println!("   - Endpoints set to production URLs");
        println!("   - Secure by default");
        println!("   - Use --api-url and --hub-url to override if needed");
    }

    println!("\n📚 Available Commands:");
    println!("   edf forward <PORT>     - Create a tunnel");
    println!("   edf keys request       - Request SSH keys");
    println!("   edf keys status        - Check key status");
    println!("   edf keys cleanup       - Clean up expired keys");
    println!("   edf list               - List active tunnels");
    println!("   edf close <ID>         - Close a tunnel");
    println!("   edf --help             - Show detailed help");
}
