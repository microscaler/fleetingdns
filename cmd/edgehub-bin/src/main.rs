use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;
use tokio::io::{AsyncRead, AsyncWriteExt};

use common::{AppResult, init_metrics, init_tracing, shutdown::GracefulShutdown};
use edgehub::{self, Config, SshConfig, SshServer};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tokio_rustls::server::TlsStream;

/// Simple HTTPS router that handles SNI-based routing
async fn serve_https_router(
    addr: SocketAddr,
    tls_config: Arc<rustls::ServerConfig>,
    redis_pool: edgehub::redis::RedisPool,
    mut shutdown_rx: tokio::sync::broadcast::Receiver<common::shutdown::ShutdownSignal>,
) -> AppResult<()> {
    let listener = TcpListener::bind(addr).await?;
    info!(addr = %listener.local_addr()?, "HTTPS router listening");

    let acceptor = TlsAcceptor::from(tls_config);

    loop {
        tokio::select! {
            // Handle new connections
            result = listener.accept() => {
                match result {
                    Ok((stream, peer)) => {
                        info!(peer = %peer, "New HTTPS connection");
                        
                        let acceptor = acceptor.clone();
                        let pool = redis_pool.clone();
                        
                        tokio::spawn(async move {
                            match acceptor.accept(stream).await {
                                Ok(mut tls_stream) => {
                                    // Read the HTTP request to extract the Host header
                                    let mut buffer = [0u8; 1024];
                                    let mut sni = None;
                                    
                                    // Read the first line of the HTTP request
                                    if let Ok(n) = tokio::io::AsyncReadExt::read(&mut tls_stream, &mut buffer).await {
                                        if n > 0 {
                                            let request = String::from_utf8_lossy(&buffer[..n]);
                                            info!(request = %request.lines().next().unwrap_or(""), "HTTP request received");
                                            
                                            // Extract Host header
                                            for line in request.lines() {
                                                if line.to_lowercase().starts_with("host:") {
                                                    let host = line[6..].trim();
                                                    sni = Some(host.to_string());
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                    
                                    if let Some(sni) = sni {
                                        info!(sni = %sni, "HTTPS request for SNI");
                                        
                                        // Extract subdomain from FQDN (e.g., "test-tunnel" from "test-tunnel.fleetingdns.run")
                                        let subdomain = if sni.contains('.') {
                                            sni.split('.').next().unwrap_or(&sni)
                                        } else {
                                            &sni
                                        };
                                        
                                        info!(sni = %sni, subdomain = %subdomain, "Extracted subdomain from SNI");
                                        
                                        // Look up tunnel in Redis
                                        info!(subdomain = %subdomain, "About to call get_tunnel_by_subdomain");
                                        match edgehub::redis::get_tunnel_by_subdomain(&pool, subdomain).await {
                                            Ok(Some(tunnel_info)) => {
                                                info!(sni = %sni, tunnel_id = %tunnel_info.id, "Routing to tunnel");
                                                
                                                // TODO: Implement actual HTTP forwarding to tunnel
                                                // For now, just send a placeholder response
                                                let response = format!(
                                                    "HTTP/1.1 200 OK\r\n\
                                                    Content-Type: text/plain\r\n\
                                                    Content-Length: {}\r\n\
                                                    \r\n\
                                                    Tunnel {} is active for {}\n",
                                                    25 + tunnel_info.id.len() + sni.len(),
                                                    tunnel_info.id,
                                                    sni
                                                );
                                                
                                                if let Err(e) = tls_stream.write_all(response.as_bytes()).await {
                                                    info!(error = %e, "Failed to write response");
                                                }
                                            }
                                            Ok(None) => {
                                                info!(sni = %sni, "No tunnel found for SNI");
                                                
                                                // Send 404 response
                                                let response = "HTTP/1.1 404 Not Found\r\n\
                                                    Content-Type: text/plain\r\n\
                                                    Content-Length: 13\r\n\
                                                    \r\n\
                                                    404 Not Found\n";
                                                
                                                if let Err(e) = tls_stream.write_all(response.as_bytes()).await {
                                                    info!(error = %e, "Failed to write 404 response");
                                                }
                                            }
                                            Err(e) => {
                                                info!(sni = %sni, error = %e, "Error looking up tunnel");
                                                
                                                // Send 500 response
                                                let response = "HTTP/1.1 500 Internal Server Error\r\n\
                                                    Content-Type: text/plain\r\n\
                                                    Content-Length: 25\r\n\
                                                    \r\n\
                                                    500 Internal Server Error\n";
                                                
                                                if let Err(e) = tls_stream.write_all(response.as_bytes()).await {
                                                    info!(error = %e, "Failed to write 500 response");
                                                }
                                            }
                                        }
                                    } else {
                                        info!("No Host header found in HTTP request");
                                        
                                        // Send 400 response
                                        let response = "HTTP/1.1 400 Bad Request\r\n\
                                            Content-Type: text/plain\r\n\
                                            Content-Length: 15\r\n\
                                            \r\n\
                                            400 Bad Request\n";
                                        
                                        if let Err(e) = tls_stream.write_all(response.as_bytes()).await {
                                            info!(error = %e, "Failed to write 400 response");
                                        }
                                    }
                                    
                                    if let Err(e) = tls_stream.shutdown().await {
                                        info!(error = %e, "Failed to shutdown TLS connection");
                                    }
                                }
                                Err(e) => {
                                    info!(error = %e, peer = %peer, "TLS handshake failed");
                                }
                            }
                        });
                    }
                    Err(e) => {
                        info!(error = %e, "Failed to accept connection");
                    }
                }
            }
            
            // Handle shutdown signal
            _ = shutdown_rx.recv() => {
                info!("HTTPS router received shutdown signal");
                break;
            }
        }
    }
    
    Ok(())
}

/// Extract SNI from TLS connection
fn extract_sni_from_tls(tls_stream: &tokio_rustls::server::TlsStream<tokio::net::TcpStream>) -> Option<String> {
    // For now, we'll extract the SNI from the Host header in the HTTP request
    // This is a simplified approach - in production we'd use proper TLS SNI extraction
    // The SNI will be passed via the Host header in the HTTP request
    None // We'll extract it from the HTTP request instead
}

/// EdgeHub command line arguments.
#[derive(Parser, Debug, Clone)]
struct Args {
    /// Address to bind the TLS listener.
    #[arg(long, default_value = "0.0.0.0:8443")]
    addr: SocketAddr,
    /// Address to bind the HTTPS router (port 443 for SNI-based routing).
    #[arg(long, default_value = "0.0.0.0:443")]
    https_addr: SocketAddr,
    /// Address to bind the SSH-over-TLS server (port 8443 for corporate firewall bypass).
    #[arg(long, default_value = "0.0.0.0:8443")]
    ssh_addr: SocketAddr,
    /// Path to SSH host key file.
    #[arg(long)]
    ssh_host_key: Option<String>,
    /// Public domain for tunnel URLs (e.g., fleetingdns.run).
    #[arg(long, default_value = "fleetingdns.run")]
    public_domain: String,
    /// Redis connection URL.
    #[arg(long, env = "REDIS_URL", default_value = "redis://127.0.0.1:6379")]
    redis: String,
    /// Path to control socket for graceful shutdown
    #[arg(long)]
    control_socket: Option<PathBuf>,
    /// Timeout for graceful shutdown in seconds
    #[arg(long, default_value = "30")]
    shutdown_timeout: u64,
}

async fn run(args: Args) -> AppResult<()> {
    let _ = init_tracing("edgehub-bin");
    init_metrics();

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
        https_addr = %args.https_addr,
        ssh_addr = %args.ssh_addr,
        control_socket = %shutdown.config.control_socket_path.display(),
        "edgehub starting with HTTPS router, TLS, and SSH servers"
    );

    let (tls_config, _) = common::tls::generate_tls_config(&["ssh"])?;
    let (https_config, _) = common::tls::generate_tls_config(&["http/1.1", "h2"])?;
    let pool = edgehub::redis::new_pool(&args.redis)
        .await
        .map_err(|e| common::AppError::Message(e.to_string()))?;

    // Create SSH server with development-friendly defaults
    let ssh_config = SshConfig {
        bind_addr: args.ssh_addr,
        host_key_path: args.ssh_host_key,
        public_domain: args.public_domain,
        ca_config: None, // No CA configuration for development mode
        // CRITICAL-3 ENHANCEMENT: Disable strict certificate validation for development
        require_client_certificates: false,
        certificate_pinning_enabled: false,
        ..Default::default()
    };
    let ssh_server = SshServer::new(ssh_config)
        .await
        .map_err(|e| common::AppError::Message(e.to_string()))?;

    // Get shutdown signals for all servers
    let tls_shutdown_rx = shutdown.subscribe();
    let https_shutdown_rx = shutdown.subscribe();
    let ssh_shutdown_rx = shutdown.subscribe();

    // Start all servers concurrently
    let tls_server = edgehub::serve_with_shutdown(
        Config {
            addr: args.addr,
            tls_config,
            redis_pool: pool.clone(),
        },
        tls_shutdown_rx,
    );

    let https_server = serve_https_router(
        args.https_addr,
        Arc::new(https_config),
        pool,
        https_shutdown_rx,
    );

    let ssh_server_task = ssh_server.run(ssh_shutdown_rx);

    // Run all servers concurrently
    let (tls_result, https_result, ssh_result) = tokio::join!(tls_server, https_server, ssh_server_task);

    // Wait for graceful shutdown to complete
    shutdown.wait_for_shutdown().await?;

    // Check results
    tls_result?;
    https_result?;
    ssh_result.map_err(|e| common::AppError::Message(e.to_string()))?;

    Ok(())
}

#[tokio::main]
async fn main() -> AppResult<()> {
    let args = Args::parse();
    run(args).await
}
