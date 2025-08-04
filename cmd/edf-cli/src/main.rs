use clap::{Parser, Subcommand};
use tracing::{info, error};

mod ssh_keys;
mod tunnel;
mod config;

use ssh_keys::SshKeyManager;
use tunnel::TunnelManager;

#[derive(Parser)]
#[command(name = "edf")]
#[command(about = "FleetingDNS CLI for secure reverse tunnels")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
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
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Forward { port, ttl, subdomain } => {
            info!("Starting tunnel for port {} with TTL {}", port, ttl);
            
            let mut tunnel_manager = TunnelManager::new()?;
            tunnel_manager.forward(port, ttl, subdomain).await?;
        }
        
        Commands::Keys { command } => {
            let key_manager = SshKeyManager::new()?;
            
            match command {
                KeyCommands::Request => {
                    info!("Requesting new SSH key pair from API");
                    let key_pair = key_manager.request_key_pair(1800).await?;
                    
                    // Get the private key path for user feedback
                    let ssh_dir = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from(".")).join(".ssh");
                    let expiry_str = key_pair.expires_at.format("%Y%m%d-%H%M%S").to_string();
                    let filename = format!("edf-cli-{}-{}.priv", expiry_str, key_pair.session_id);
                    let private_key_path = ssh_dir.join(&filename);
                    
                    println!("✅ SSH key pair requested successfully");
                    println!("   Fingerprint: {}", key_pair.fingerprint);
                    println!("   Type: {}", key_pair.key_type);
                    println!("   Session ID: {}", key_pair.session_id);
                    println!("   Expires: {}", key_pair.expires_at);
                    println!("   Private key stored at: {}", private_key_path.display());
                    println!("   ⚠️  This key will be automatically removed when the session expires");
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
                        }
                        Err(ssh_keys::SshKeyError::KeyFileNotFound(_)) => {
                            println!("❌ No SSH keys found");
                        }
                        Err(e) => {
                            error!("Failed to check key status: {:?}", e);
                            return Err(anyhow::anyhow!("Key status check failed: {:?}", e));
                        }
                    }
                }
                
                KeyCommands::Cleanup => {
                    info!("Cleaning up SSH session");
                    key_manager.cleanup_session()?;
                    println!("✅ SSH session cleaned up successfully");
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
                    let ssh_dir = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from(".")).join(".ssh");
                    let expiry_str = key_pair.expires_at.format("%Y%m%d-%H%M%S").to_string();
                    let filename = format!("edf-cli-{}-{}.priv", expiry_str, key_pair.session_id);
                    let private_key_path = ssh_dir.join(&filename);
                    println!("   Private key stored at: {}", private_key_path.display());
                }
            }
        }
        
        Commands::List => {
            info!("Listing active tunnels");
            let tunnel_manager = TunnelManager::new()?;
            tunnel_manager.list_tunnels().await?;
        }
        
        Commands::Close { id } => {
            info!("Closing tunnel {}", id);
            let mut tunnel_manager = TunnelManager::new()?;
            tunnel_manager.close_tunnel(&id).await?;
        }
    }
    
    Ok(())
} 