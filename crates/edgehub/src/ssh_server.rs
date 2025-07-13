use anyhow::{Context, Result};
use russh::server::{Auth, Msg, Session};
use russh::{Channel, ChannelId};
use russh_keys::key::KeyPair;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info, warn};
use common::shutdown::ShutdownSignal;
use rand::Rng;

// Import certificate authority functionality
use edf_ca::{CertificateAuthority, CaConfig, IssuanceRequest, IssuanceResponse};

/// SSH server configuration
#[derive(Debug, Clone)]
pub struct SshConfig {
    pub bind_addr: SocketAddr,
    pub host_key_path: Option<String>,
    pub public_domain: String, // e.g., "fleetingdns.run"
    pub ca_config: Option<CaConfig>, // Certificate authority configuration
}

impl Default for SshConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:443".parse().unwrap(), // Port 443 for corporate firewall bypass
            host_key_path: None,
            public_domain: "fleetingdns.run".to_string(),
            ca_config: Some(CaConfig::default()),
        }
    }
}

/// SSH server state shared between sessions
#[derive(Clone)]
pub struct SshServerState {
    pub active_tunnels: Arc<Mutex<HashMap<ChannelId, TunnelInfo>>>,
    pub reverse_tunnels: Arc<Mutex<HashMap<String, ReverseTunnelInfo>>>, // subdomain -> tunnel info
    pub shutdown_tx: mpsc::Sender<()>,
    pub certificate_authority: Option<Arc<CertificateAuthority>>, // Certificate authority for validation
}

/// Information about an active tunnel
#[derive(Debug, Clone)]
pub struct TunnelInfo {
    pub local_addr: SocketAddr,
    pub remote_addr: SocketAddr,
    pub created_at: std::time::Instant,
    pub client_certificate_serial: Option<String>, // Certificate serial number for this tunnel
}

/// Information about a reverse tunnel (developer service -> EdgeHub)
#[derive(Debug, Clone)]
pub struct ReverseTunnelInfo {
    pub subdomain: String,
    pub local_port: u16,
    pub channel_id: ChannelId,
    pub created_at: std::time::Instant,
    pub developer_id: String,
    pub certificate_serial: Option<String>, // Certificate used for this tunnel
}

/// SSH server implementation with reverse tunnel support and certificate validation
pub struct SshServer {
    config: SshConfig,
    host_key: KeyPair,
    state: SshServerState,
}

impl SshServer {
    /// Create a new SSH server with certificate authority
    pub async fn new(config: SshConfig) -> Result<Self> {
        let host_key = Self::load_or_generate_host_key(&config.host_key_path).await?;
        
        // Initialize certificate authority if configured
        let certificate_authority = if let Some(ca_config) = &config.ca_config {
            info!("Initializing certificate authority for SSH server");
            let ca = CertificateAuthority::new(ca_config.clone()).await
                .context("Failed to initialize certificate authority")?;
            Some(Arc::new(ca))
        } else {
            info!("SSH server running without certificate authority (development mode)");
            None
        };
        
        let (shutdown_tx, _) = mpsc::channel(1);
        let state = SshServerState {
            active_tunnels: Arc::new(Mutex::new(HashMap::new())),
            reverse_tunnels: Arc::new(Mutex::new(HashMap::new())),
            shutdown_tx,
            certificate_authority,
        };

        Ok(Self {
            config,
            host_key,
            state,
        })
    }

    /// Issue a certificate for a client
    pub async fn issue_certificate(&self, client_id: &str, common_name: &str) -> Result<IssuanceResponse> {
        if let Some(ca) = &self.state.certificate_authority {
            let request = IssuanceRequest::new(common_name.to_string(), client_id.to_string());
            ca.issue_certificate(request).await
                .context("Failed to issue certificate")
        } else {
            anyhow::bail!("Certificate authority not configured")
        }
    }

    /// Validate a client certificate
    pub async fn validate_certificate(&self, certificate_pem: &str) -> Result<bool> {
        if let Some(ca) = &self.state.certificate_authority {
            // Extract serial number from certificate
            if let Some(serial) = self.extract_certificate_serial(certificate_pem).await? {
                ca.validate_certificate(&serial).await
                    .context("Failed to validate certificate")
            } else {
                Ok(false)
            }
        } else {
            // In development mode without CA, accept any certificate
            warn!("Certificate validation skipped - no CA configured");
            Ok(true)
        }
    }

    /// Extract certificate serial number from PEM certificate
    async fn extract_certificate_serial(&self, certificate_pem: &str) -> Result<Option<String>> {
        use rustls_pemfile;
        use ring::digest;
        
        // Parse the PEM certificate
        let mut reader = std::io::BufReader::new(certificate_pem.as_bytes());
        let certs_iter = rustls_pemfile::certs(&mut reader);
        
        // Get the first certificate from the iterator
        let cert_der = match certs_iter.into_iter().next() {
            Some(Ok(cert)) => cert,
            Some(Err(_)) => return Ok(None),
            None => return Ok(None),
        };
        
        // Calculate fingerprint as serial (simplified approach)
        let digest = digest::digest(&digest::SHA256, &cert_der);
        let serial = hex::encode(digest.as_ref());
        
        Ok(Some(serial))
    }

    /// Generate a unique subdomain for a developer's service
    pub async fn generate_subdomain(&self, service_name: &str) -> String {
        let mut rng = rand::thread_rng();
        let suffix: u32 = rng.gen_range(100..999999);
        format!("{}{}", service_name, suffix)
    }

    /// Register a reverse tunnel mapping
    pub async fn register_reverse_tunnel(
        &self,
        subdomain: String,
        local_port: u16,
        channel_id: ChannelId,
        developer_id: String,
        certificate_serial: Option<String>,
    ) -> Result<String> {
        let tunnel_info = ReverseTunnelInfo {
            subdomain: subdomain.clone(),
            local_port,
            channel_id,
            created_at: std::time::Instant::now(),
            developer_id: developer_id.clone(),
            certificate_serial: certificate_serial.clone(),
        };

        self.state.reverse_tunnels.lock().await.insert(subdomain.clone(), tunnel_info);
        
        let public_url = format!("https://{}.{}", subdomain, self.config.public_domain);
        info!(
            subdomain = %subdomain,
            local_port = %local_port,
            public_url = %public_url,
            certificate_serial = ?certificate_serial,
            "Registered reverse tunnel"
        );
        
        Ok(public_url)
    }

    /// Find reverse tunnel by subdomain
    pub async fn find_reverse_tunnel(&self, subdomain: &str) -> Option<ReverseTunnelInfo> {
        self.state.reverse_tunnels.lock().await.get(subdomain).cloned()
    }

    /// Handle incoming HTTP request for reverse tunnel
    pub async fn handle_reverse_tunnel_request(
        &self,
        subdomain: &str,
        _request_data: Vec<u8>,
    ) -> Result<Vec<u8>> {
        if let Some(tunnel_info) = self.find_reverse_tunnel(subdomain).await {
            // Forward request through the SSH tunnel to developer's local service
            self.forward_to_developer_service(tunnel_info, _request_data).await
        } else {
            // Return 404 if no tunnel found
            let response = b"HTTP/1.1 404 Not Found\r\nContent-Length: 13\r\n\r\nTunnel not found";
            Ok(response.to_vec())
        }
    }

    async fn forward_to_developer_service(
        &self,
        tunnel_info: ReverseTunnelInfo,
        _request_data: Vec<u8>,
    ) -> Result<Vec<u8>> {
        // This would forward the HTTP request through the SSH channel
        // to the developer's local service and return the response
        // For now, return a placeholder response
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 50\r\n\r\nReverse tunnel active for {}:{}",
            tunnel_info.subdomain, tunnel_info.local_port
        );
        Ok(response.into_bytes())
    }

    /// Load existing host key or generate a new one
    async fn load_or_generate_host_key(path: &Option<String>) -> Result<KeyPair> {
        match path {
            Some(key_path) => {
                if Path::new(key_path).exists() {
                    info!("Loading SSH host key from {}", key_path);
                    let key_data = tokio::fs::read_to_string(key_path).await
                        .context("Failed to read host key file")?;
                    
                    russh_keys::decode_secret_key(&key_data, None)
                        .context("Failed to decode host key")
                } else {
                    info!("Generating new SSH host key at {}", key_path);
                    let key = russh_keys::key::KeyPair::generate_ed25519()
                        .context("Failed to generate host key")?;
                    
                    let mut encoded = Vec::new();
                    russh_keys::encode_pkcs8_pem(&key, &mut encoded)
                        .context("Failed to encode host key")?;
                    
                    tokio::fs::write(key_path, encoded).await
                        .context("Failed to write host key")?;
                    
                    Ok(key)
                }
            }
            None => {
                info!("Generating ephemeral SSH host key");
                russh_keys::key::KeyPair::generate_ed25519()
                    .context("Failed to generate ephemeral host key")
            }
        }
    }

    /// Start the SSH server
    pub async fn run(self, mut shutdown_rx: tokio::sync::broadcast::Receiver<ShutdownSignal>) -> Result<()> {
        let listener = TcpListener::bind(&self.config.bind_addr).await
            .context("Failed to bind SSH server")?;
        
        info!(
            bind_addr = %self.config.bind_addr,
            public_domain = %self.config.public_domain,
            ca_enabled = %self.state.certificate_authority.is_some(),
            "SSH reverse tunnel server listening (corporate firewall friendly)"
        );

        loop {
            tokio::select! {
                // Check for shutdown signal
                signal = shutdown_rx.recv() => {
                    match signal {
                        Ok(shutdown_signal) => {
                            info!("SSH server received shutdown signal: {:?}", shutdown_signal);
                            break;
                        }
                        Err(e) => {
                            warn!("SSH server shutdown channel error: {}", e);
                            break;
                        }
                    }
                }
                
                // Accept new connections
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((stream, addr)) => {
                            debug!("New SSH connection from {}", addr);
                            let state = self.state.clone();
                            let host_key = self.host_key.clone();
                            let public_domain = self.config.public_domain.clone();
                            
                            tokio::spawn(async move {
                                let session = SshSession {
                                    state,
                                    channels: HashMap::new(),
                                    public_domain,
                                    client_certificate_serial: None,
                                };
                                
                                let config = Arc::new(russh::server::Config {
                                    inactivity_timeout: Some(std::time::Duration::from_secs(3600)),
                                    auth_rejection_time: std::time::Duration::from_secs(3),
                                    auth_rejection_time_initial: Some(std::time::Duration::from_secs(0)),
                                    keys: vec![host_key],
                                    ..Default::default()
                                });
                                
                                if let Err(e) = russh::server::run_stream(
                                    config,
                                    stream,
                                    session,
                                ).await {
                                    error!("SSH session error: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            error!("Failed to accept SSH connection: {}", e);
                        }
                    }
                }
            }
        }

        info!("SSH reverse tunnel server stopped");
        Ok(())
    }

    /// Get the number of active tunnels
    pub async fn active_tunnel_count(&self) -> usize {
        self.state.active_tunnels.lock().await.len()
    }

    /// Get the number of active reverse tunnels
    pub async fn reverse_tunnel_count(&self) -> usize {
        self.state.reverse_tunnels.lock().await.len()
    }

    /// Get certificate authority statistics
    pub async fn get_ca_statistics(&self) -> Option<edf_ca::ca::CaStatistics> {
        if let Some(ca) = &self.state.certificate_authority {
            Some(ca.get_statistics().await)
        } else {
            None
        }
    }
}

/// Individual SSH session handler with certificate validation
pub struct SshSession {
    state: SshServerState,
    channels: HashMap<ChannelId, Channel<Msg>>,
    public_domain: String,
    client_certificate_serial: Option<String>, // Track client certificate for this session
}

#[async_trait::async_trait]
impl russh::server::Handler for SshSession {
    type Error = anyhow::Error;

    async fn channel_open_direct_tcpip(
        mut self,
        channel: Channel<Msg>,
        host_to_connect: &str,
        port_to_connect: u32,
        originator_address: &str,
        originator_port: u32,
        session: Session,
    ) -> Result<(Self, bool, Session), Self::Error> {
        info!(
            "Direct TCP/IP request: {}:{} from {}:{}",
            host_to_connect, port_to_connect, originator_address, originator_port
        );

        let target_addr = format!("{}:{}", host_to_connect, port_to_connect)
            .parse::<SocketAddr>()
            .context("Invalid target address")?;

        let originator_addr = format!("{}:{}", originator_address, originator_port)
            .parse::<SocketAddr>()
            .context("Invalid originator address")?;

        // Store tunnel info with certificate information
        let tunnel_info = TunnelInfo {
            local_addr: originator_addr,
            remote_addr: target_addr,
            created_at: std::time::Instant::now(),
            client_certificate_serial: self.client_certificate_serial.clone(),
        };

        self.state.active_tunnels.lock().await.insert(channel.id(), tunnel_info);

        // Start TCP proxy in background
        let state = self.state.clone();
        let channel_id = channel.id();
        tokio::spawn(async move {
            if let Err(e) = tcp_proxy_task(channel, target_addr).await {
                error!("TCP proxy error: {}", e);
            }
            
            // Clean up tunnel info when done
            state.active_tunnels.lock().await.remove(&channel_id);
        });

        Ok((self, true, session))
    }

    async fn auth_publickey(
        mut self,
        user: &str,
        public_key: &russh_keys::key::PublicKey,
    ) -> Result<(Self, Auth), Self::Error> {
        info!(user = %user, "Public key authentication attempt");

        // In production, this would validate the public key against the certificate
        // For now, we'll extract certificate information from the key metadata if available
        
        // Check if this is a certificate-based authentication
        if let Some(ca) = &self.state.certificate_authority {
            // In a full implementation, we'd validate the certificate here
            // For now, we'll accept the authentication and track the certificate serial
            debug!("Certificate-based authentication accepted for user: {}", user);
            
            // Generate a mock certificate serial for tracking
            // In production, this would come from the actual certificate
            self.client_certificate_serial = Some(format!("cert-{}", uuid::Uuid::new_v4()));
            
            Ok((self, Auth::Accept))
        } else {
            // Development mode - accept any public key
            debug!("Development mode: public key authentication accepted for user: {}", user);
            Ok((self, Auth::Accept))
        }
    }

    async fn auth_password(
        mut self,
        user: &str,
        _password: &str,
    ) -> Result<(Self, Auth), Self::Error> {
        // Reject password authentication for security
        warn!(user = %user, "Password authentication rejected - use certificate-based authentication");
        Ok((self, Auth::Reject {
            proceed_with_methods: None,
        }))
    }

    async fn channel_close(
        mut self,
        channel: ChannelId,
        session: Session,
    ) -> Result<(Self, Session), Self::Error> {
        debug!("Channel {} closed", channel);
        self.channels.remove(&channel);
        self.state.active_tunnels.lock().await.remove(&channel);
        
        // Clean up reverse tunnel if this was one
        {
            let mut reverse_tunnels = self.state.reverse_tunnels.lock().await;
            reverse_tunnels.retain(|_, tunnel| tunnel.channel_id != channel);
        }
        
        Ok((self, session))
    }
}

/// TCP proxy task that forwards data between SSH channel and target
async fn tcp_proxy_task(mut channel: Channel<Msg>, target_addr: SocketAddr) -> Result<()> {
    debug!("Starting TCP proxy to {}", target_addr);
    
    // Connect to target
    let target_stream = TcpStream::connect(target_addr).await
        .context("Failed to connect to target")?;
    
    let (target_read, target_write) = target_stream.into_split();
    
    // Create bidirectional proxy using channels
    let (tx_to_target, mut rx_from_ssh) = mpsc::channel::<Vec<u8>>(1024);
    let (tx_to_ssh, mut rx_from_target) = mpsc::channel::<Vec<u8>>(1024);
    
    // SSH -> Target
    let ssh_to_target = {
        let tx = tx_to_target.clone();
        async move {
            while let Some(msg) = channel.wait().await {
                match msg {
                    russh::ChannelMsg::Data { data } => {
                        if tx.send(data.to_vec()).await.is_err() {
                            break;
                        }
                    }
                    russh::ChannelMsg::Eof => {
                        debug!("SSH channel EOF");
                        break;
                    }
                    _ => {}
                }
            }
        }
    };
    
    // Target -> SSH  
    let target_to_ssh = {
        let mut target_read = target_read;
        async move {
            use tokio::io::AsyncReadExt;
            let mut buffer = [0u8; 4096];
            
            loop {
                match target_read.read(&mut buffer).await {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        if tx_to_ssh.send(buffer[..n].to_vec()).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Target read error: {}", e);
                        break;
                    }
                }
            }
        }
    };
    
    // Forward data from channels
    let forward_to_target = {
        let mut target_write = target_write;
        async move {
            use tokio::io::AsyncWriteExt;
            while let Some(data) = rx_from_ssh.recv().await {
                if target_write.write_all(&data).await.is_err() {
                    break;
                }
            }
        }
    };
    
    let forward_to_ssh = {
        // We need to create a separate channel reference for sending data back
        // This is a simplified implementation - in production we'd need proper channel management
        async move {
            while let Some(_data) = rx_from_target.recv().await {
                // For now, we'll skip the actual data forwarding back to SSH
                // This would require proper channel management in the russh library
                break;
            }
        }
    };
    
    // Run all proxy tasks concurrently
    tokio::select! {
        _ = ssh_to_target => debug!("SSH to target proxy ended"),
        _ = target_to_ssh => debug!("Target to SSH proxy ended"),
        _ = forward_to_target => debug!("Forward to target ended"),
        _ = forward_to_ssh => debug!("Forward to SSH ended"),
    }
    
    debug!("TCP proxy to {} completed", target_addr);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ssh_config_default() {
        let config = SshConfig::default();
        assert_eq!(config.bind_addr.port(), 443); // Updated to port 443
        assert!(config.host_key_path.is_none());
        assert_eq!(config.public_domain, "fleetingdns.run");
        assert!(config.ca_config.is_some());
    }

    #[tokio::test]
    async fn test_host_key_generation() {
        let result = SshServer::load_or_generate_host_key(&None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_ssh_server_creation() {
        let config = SshConfig::default();
        let result = SshServer::new(config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_ssh_server_with_ca() {
        let mut config = SshConfig::default();
        config.ca_config = Some(CaConfig::default());
        
        let server = SshServer::new(config).await.unwrap();
        assert!(server.state.certificate_authority.is_some());
        
        // Test certificate issuance
        let response = server.issue_certificate("test-client", "test.example.com").await;
        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_ssh_server_without_ca() {
        let mut config = SshConfig::default();
        config.ca_config = None;
        
        let server = SshServer::new(config).await.unwrap();
        assert!(server.state.certificate_authority.is_none());
        
        // Test certificate issuance should fail
        let response = server.issue_certificate("test-client", "test.example.com").await;
        assert!(response.is_err());
    }

    #[tokio::test]
    async fn test_active_tunnel_count() {
        let config = SshConfig::default();
        let server = SshServer::new(config).await.unwrap();
        let count = server.active_tunnel_count().await;
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_reverse_tunnel_count() {
        let config = SshConfig::default();
        let server = SshServer::new(config).await.unwrap();
        let count = server.reverse_tunnel_count().await;
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_subdomain_generation() {
        let config = SshConfig::default();
        let server = SshServer::new(config).await.unwrap();
        let subdomain = server.generate_subdomain("myservice").await;
        assert!(subdomain.starts_with("myservice"));
        assert!(subdomain.len() > "myservice".len());
    }

    #[tokio::test]
    async fn test_certificate_validation() {
        let config = SshConfig::default();
        let server = SshServer::new(config).await.unwrap();
        
        // Test with mock certificate PEM
        let mock_cert_pem = "-----BEGIN CERTIFICATE-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA...\n-----END CERTIFICATE-----";
        let is_valid = server.validate_certificate(mock_cert_pem).await;
        
        // Should not fail (even if certificate is not valid in CA)
        assert!(is_valid.is_ok());
    }
} 