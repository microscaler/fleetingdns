//! TLS Router for EdgeHub
//!
//! This module handles incoming HTTPS connections on port 443 and routes them
//! to the appropriate SSH tunnels based on the SNI (Server Name Indication).
//! This is the core USP that differentiates FleetingDNS from competitors.

use anyhow::{Context, Result};
use rustls::{ServerConfig, ServerConnection};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use crate::redis_auth::SessionData;
use crate::ssh_server::{ReverseTunnelInfo, SshServerState};

/// Configuration for the TLS router
#[derive(Debug, Clone)]
pub struct TlsRouterConfig {
    /// Address to bind the TLS listener to
    pub bind_addr: SocketAddr,
    /// TLS server configuration
    pub tls_config: ServerConfig,
    /// Domain for certificate generation
    pub public_domain: String,
    /// Redis URL for tunnel lookup
    pub redis_url: String,
    /// Maximum concurrent connections
    pub max_connections: usize,
}

impl Default for TlsRouterConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:443".parse().unwrap(),
            tls_config: ServerConfig::builder()
                .with_safe_defaults()
                .with_no_client_auth()
                .build(),
            public_domain: "fleetingdns.run".to_string(),
            redis_url: "redis://localhost:6379".to_string(),
            max_connections: 1000,
        }
    }
}

/// TLS Router that handles incoming HTTPS connections
pub struct TlsRouter {
    config: TlsRouterConfig,
    state: Arc<SshServerState>,
    active_connections: Arc<Mutex<HashMap<String, usize>>>, // SNI -> connection count
}

impl TlsRouter {
    /// Create a new TLS router
    pub fn new(config: TlsRouterConfig, state: Arc<SshServerState>) -> Self {
        Self {
            config,
            state,
            active_connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Start the TLS router
    pub async fn run(self) -> Result<()> {
        let listener = TcpListener::bind(self.config.bind_addr).await
            .context("Failed to bind TLS listener")?;
        
        info!(addr = %listener.local_addr()?, "TLS router listening on port 443");

        loop {
            match listener.accept().await {
                Ok((stream, peer_addr)) => {
                    info!(peer = %peer_addr, "New TLS connection");
                    
                    let router = self.clone();
                    tokio::spawn(async move {
                        if let Err(e) = router.handle_tls_connection(stream, peer_addr).await {
                            error!(peer = %peer_addr, error = %e, "TLS connection error");
                        }
                    });
                }
                Err(e) => {
                    error!(error = %e, "Failed to accept TLS connection");
                }
            }
        }
    }

    /// Handle an individual TLS connection
    async fn handle_tls_connection(
        &self,
        stream: TcpStream,
        peer_addr: SocketAddr,
    ) -> Result<()> {
        // Create TLS acceptor
        let tls_config = Arc::new(self.config.tls_config.clone());
        let mut tls_stream = tokio_rustls::TlsAcceptor::from(tls_config)
            .accept(stream)
            .await
            .context("TLS handshake failed")?;

        // Extract SNI from TLS connection
        let sni = self.extract_sni(&tls_stream)
            .context("Failed to extract SNI")?;

        info!(sni = %sni, peer = %peer_addr, "Processing TLS connection");

        // Look up tunnel for this SNI
        let tunnel_info = self.lookup_tunnel(&sni).await?;
        
        if let Some(tunnel) = tunnel_info {
            // Route the connection to the tunnel
            self.route_to_tunnel(tls_stream, tunnel).await?;
        } else {
            // No tunnel found, return 404
            self.send_404_response(tls_stream).await?;
        }

        Ok(())
    }

    /// Extract Server Name Indication from TLS connection
    fn extract_sni(&self, tls_stream: &tokio_rustls::TlsStream<TcpStream>) -> Result<String> {
        // Get the negotiated SNI from the TLS connection
        if let Some(server_name) = tls_stream.get_ref().1.server_name() {
            Ok(server_name.to_string())
        } else {
            Err(anyhow::anyhow!("No SNI provided"))
        }
    }

    /// Look up tunnel information for a given SNI
    async fn lookup_tunnel(&self, sni: &str) -> Result<Option<ReverseTunnelInfo>> {
        // First check if it's a valid subdomain of our domain
        if !sni.ends_with(&self.config.public_domain) {
            return Ok(None);
        }

        // Extract subdomain from SNI
        let subdomain = sni
            .strip_suffix(&self.config.public_domain)
            .and_then(|s| s.strip_suffix('.'))
            .unwrap_or(sni);

        debug!(sni = %sni, subdomain = %subdomain, "Looking up tunnel");

        // Look up tunnel in Redis
        let tunnel_info = self.state.find_reverse_tunnel(subdomain).await;
        
        match tunnel_info {
            Some(tunnel) => {
                info!(
                    sni = %sni,
                    subdomain = %subdomain,
                    local_port = %tunnel.local_port,
                    "Found tunnel for SNI"
                );
                Ok(Some(tunnel))
            }
            None => {
                warn!(
                    sni = %sni,
                    subdomain = %subdomain,
                    "No tunnel found for SNI"
                );
                Ok(None)
            }
        }
    }

    /// Route TLS connection to the appropriate tunnel
    async fn route_to_tunnel(
        &self,
        mut tls_stream: tokio_rustls::TlsStream<TcpStream>,
        tunnel: ReverseTunnelInfo,
    ) -> Result<()> {
        // Connect to the local service
        let local_addr = format!("127.0.0.1:{}", tunnel.local_port);
        let mut local_stream = TcpStream::connect(&local_addr).await
            .context("Failed to connect to local service")?;

        info!(
            subdomain = %tunnel.subdomain,
            local_port = %tunnel.local_port,
            "Routing TLS connection to local service"
        );

        // Create bidirectional proxy between TLS stream and local service
        let (mut tls_read, mut tls_write) = tokio::io::split(tls_stream);
        let (mut local_read, mut local_write) = tokio::io::split(local_stream);

        // Spawn tasks for bidirectional forwarding
        let upstream_task = tokio::spawn(async move {
            let mut buffer = vec![0u8; 8192];
            loop {
                match tls_read.read(&mut buffer).await {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        if local_write.write_all(&buffer[..n]).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let downstream_task = tokio::spawn(async move {
            let mut buffer = vec![0u8; 8192];
            loop {
                match local_read.read(&mut buffer).await {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        if tls_write.write_all(&buffer[..n]).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        // Wait for either direction to complete
        tokio::select! {
            _ = upstream_task => {},
            _ = downstream_task => {},
        }

        info!(
            subdomain = %tunnel.subdomain,
            "TLS connection routing completed"
        );

        Ok(())
    }

    /// Send a 404 response when no tunnel is found
    async fn send_404_response(
        &self,
        mut tls_stream: tokio_rustls::TlsStream<TcpStream>,
    ) -> Result<()> {
        let response = "HTTP/1.1 404 Not Found\r\n\
                       Content-Length: 0\r\n\
                       Connection: close\r\n\
                       \r\n";

        tls_stream.write_all(response.as_bytes()).await
            .context("Failed to write 404 response")?;

        Ok(())
    }

    /// Generate ephemeral certificate for a subdomain
    pub async fn generate_certificate(&self, subdomain: &str) -> Result<Vec<u8>> {
        // TODO: Implement certificate generation
        // This would use the certificate authority to generate an ephemeral cert
        // for the subdomain with appropriate SANs
        
        info!(subdomain = %subdomain, "Generating ephemeral certificate");
        
        // Placeholder - in real implementation, this would generate an actual cert
        Ok(vec![])
    }

    /// Get connection statistics
    pub async fn get_connection_stats(&self) -> HashMap<String, usize> {
        self.active_connections.lock().await.clone()
    }
}

impl Clone for TlsRouter {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            state: self.state.clone(),
            active_connections: self.active_connections.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    #[test]
    fn test_tls_router_config_default() {
        let config = TlsRouterConfig::default();
        assert_eq!(config.bind_addr, "0.0.0.0:443".parse::<SocketAddr>().unwrap());
        assert_eq!(config.public_domain, "fleetingdns.run");
        assert_eq!(config.max_connections, 1000);
    }

    #[test]
    fn test_sni_extraction() {
        // This would test SNI extraction from TLS connections
        // For now, just test the structure
        assert!(true);
    }

    #[test]
    fn test_subdomain_validation() {
        let config = TlsRouterConfig::default();
        let router = TlsRouter::new(config, Arc::new(SshServerState::default()));
        
        // Test valid subdomain
        assert!(router.is_valid_subdomain("test.fleetingdns.run"));
        
        // Test invalid subdomain
        assert!(!router.is_valid_subdomain("test.example.com"));
    }

    impl TlsRouter {
        fn is_valid_subdomain(&self, sni: &str) -> bool {
            sni.ends_with(&self.config.public_domain)
        }
    }
} 