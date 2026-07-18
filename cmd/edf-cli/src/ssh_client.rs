use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tracing::{error, info, warn};

// russh 0.60: Handler uses native async fn in traits (no async_trait).
use russh::client::{Config as SshConfig, Handle, Handler};
use russh::{client::Msg, Channel};

/// SSH client configuration for Phase 1
#[derive(Debug, Clone)]
pub struct TunnelClientConfig {
    pub hub_url: String,
    pub hub_port: u16,
    pub local_port: u16,
    pub subdomain: String,
    pub connection_timeout: Duration,
    pub keep_alive_interval: Duration,
}

impl Default for TunnelClientConfig {
    fn default() -> Self {
        Self {
            hub_url: "localhost".to_string(),
            hub_port: 2222, // Plain SSH port for Phase 1
            local_port: 8080,
            subdomain: "test".to_string(),
            connection_timeout: Duration::from_secs(30),
            keep_alive_interval: Duration::from_secs(60),
        }
    }
}

/// SSH client handler for tunnel establishment
#[derive(Clone, Debug)]
pub struct SshClientHandler {
    session_id: String,
    subdomain: String,
    local_port: u16,
}

impl SshClientHandler {
    pub fn new(session_id: String, subdomain: String, local_port: u16) -> Self {
        Self {
            session_id,
            subdomain,
            local_port,
        }
    }

    /// Handle forwarded-tcpip channel opened by server for reverse tunnel connections.
    ///
    /// CRITICAL: this is invoked from a russh Handler callback, which runs on
    /// the SSH session's event loop. The bidirectional copy MUST be spawned
    /// onto its own task — awaiting it inline blocks the session's message
    /// pump, so the channel data the copy waits for can never arrive
    /// (deadlock; found via the R9 e2e test hanging forever).
    fn handle_forwarded_tcpip_channel(&self, channel: Channel<Msg>) -> Result<(), anyhow::Error> {
        info!(
            session_id = %self.session_id,
            subdomain = %self.subdomain,
            "Forwarded-tcpip channel opened for reverse tunnel"
        );

        let local_port = self.local_port;
        tokio::spawn(async move {
            // Convert SSH channel into an AsyncRead/AsyncWrite stream.
            // ChannelStream<Msg> implements both traits directly — no split() needed.
            let mut ssh_stream = channel.into_stream();

            // Connect to local service
            let local_addr = format!("127.0.0.1:{}", local_port);
            let mut local_stream = match TcpStream::connect(&local_addr).await {
                Ok(s) => s,
                Err(e) => {
                    error!("Failed to connect to local service {}: {}", local_addr, e);
                    return;
                }
            };

            info!("Phase 1: Connected to local service at {}", local_addr);

            // Bidirectionally copy data between SSH channel and local service.
            // copy_bidirectional handles both directions concurrently and completes
            // when either side closes.
            match tokio::io::copy_bidirectional(&mut ssh_stream, &mut local_stream).await {
                Ok((from_ssh, from_conn)) => {
                    info!(
                        from_ssh,
                        from_conn, "Phase 1: Tunnel data forwarding completed successfully"
                    );
                }
                Err(e) => {
                    error!("Phase 1: Tunnel data forwarding error: {}", e);
                }
            }
        });

        Ok(())
    }
}

impl Handler for SshClientHandler {
    type Error = anyhow::Error;

    // Required method: Check server key (accept all for development)
    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        info!("Checking server key: {:?}", server_public_key);
        // For development, accept all server keys.
        // TODO: validate against known hosts (client-side host-key pinning).
        Ok(true)
    }

    // Handle data received from server
    async fn data(
        &mut self,
        channel: russh::ChannelId,
        data: &[u8],
        _session: &mut russh::client::Session,
    ) -> Result<(), Self::Error> {
        info!(
            "Received data on channel {:?}: {:?}",
            channel,
            String::from_utf8_lossy(data)
        );
        Ok(())
    }

    // Handle channel open confirmation
    async fn channel_open_confirmation(
        &mut self,
        id: russh::ChannelId,
        max_packet_size: u32,
        window_size: u32,
        _session: &mut russh::client::Session,
    ) -> Result<(), Self::Error> {
        info!(
            "Channel opened: id={}, max_packet={}, window={}",
            id, max_packet_size, window_size
        );
        Ok(())
    }

    // Handle channel open failure
    async fn channel_open_failure(
        &mut self,
        channel: russh::ChannelId,
        reason: russh::ChannelOpenFailure,
        description: &str,
        _language: &str,
        _session: &mut russh::client::Session,
    ) -> Result<(), Self::Error> {
        warn!(
            "Channel open failed: channel={}, reason={:?}, description={}",
            channel, reason, description
        );
        Ok(())
    }

    // Handle forwarded-tcpip channel opened by server (for reverse tunnel)
    async fn server_channel_open_forwarded_tcpip(
        &mut self,
        channel: Channel<Msg>,
        connected_address: &str,
        connected_port: u32,
        originator_address: &str,
        originator_port: u32,
        _session: &mut russh::client::Session,
    ) -> Result<(), Self::Error> {
        info!(
            connected_address = %connected_address,
            connected_port,
            originator_address = %originator_address,
            originator_port,
            "Received forwarded-tcpip channel from server"
        );

        // Handle the forwarded channel by connecting to local service
        self.handle_forwarded_tcpip_channel(channel)
    }
}

/// SSH tunnel client for Phase 1
pub struct TunnelClient<H: Handler + std::fmt::Debug + 'static>
where
    H::Error: std::fmt::Debug,
{
    config: TunnelClientConfig,
    session_id: String,
    handle: Option<Handle<H>>,
}

impl<H: Handler + std::fmt::Debug + 'static> TunnelClient<H>
where
    H::Error: std::fmt::Debug,
{
    /// Create a new tunnel client
    pub fn new(config: TunnelClientConfig, session_id: String) -> Self {
        Self {
            config,
            session_id,
            handle: None,
        }
    }

    /// Establish SSH tunnel connection to EdgeHub, authenticating with the
    /// API-issued keypair (TDP-12).
    pub async fn establish_tunnel(
        &mut self,
        handler: H,
        key_pair: russh::keys::PrivateKey,
    ) -> Result<()> {
        info!(
            local_port = self.config.local_port,
            "Establishing SSH tunnel connection to EdgeHub at {}:{}",
            self.config.hub_url,
            self.config.hub_port
        );

        // TDP-11: NEVER set inactivity_timeout from connection_timeout. The
        // previous code set it to 30 s, so russh dropped every idle tunnel
        // after 30 s while the CLI kept reporting it active. The tunnel's
        // lifetime is governed by the API-issued TTL; transport liveness is
        // maintained by the keepalive loop (see `send_keepalive`).
        let ssh_config = SshConfig {
            inactivity_timeout: None,
            ..Default::default()
        };

        // Connect to EdgeHub via SSH using the correct russh API
        let target_addr = format!("{}:{}", self.config.hub_url, self.config.hub_port);

        info!(
            "Phase 1: Connecting to EdgeHub SSH server at {}",
            target_addr
        );

        // Use russh::client::connect which is the correct API
        let mut handle = match tokio::time::timeout(
            self.config.connection_timeout,
            russh::client::connect(Arc::new(ssh_config), target_addr, handler),
        )
        .await
        {
            Ok(Ok(handle)) => {
                info!("Phase 1: SSH connection established successfully!");
                handle
            }
            Ok(Err(e)) => {
                error!("Phase 1: Failed to establish SSH connection: {:?}", e);
                return Err(anyhow::anyhow!("SSH connection failed: {:?}", e));
            }
            Err(_) => {
                error!(
                    "Phase 1: SSH connection timeout after {:?}",
                    self.config.connection_timeout
                );
                return Err(anyhow::anyhow!("SSH connection timeout"));
            }
        };

        info!("SSH handshake completed; authenticating with API-issued key");

        // TDP-12: authenticate with the keypair minted by POST /v1/tunnels —
        // not a throwaway generated key. The session_id is carried in the
        // username so the hub can validate key↔session in Redis (TDP-13).
        // russh 0.60 wraps the key in PrivateKeyWithHashAlg (None → default
        // hash for the key's algorithm) and returns an AuthResult enum.
        let key = russh::keys::PrivateKeyWithHashAlg::new(Arc::new(key_pair), None);
        let auth_result = handle
            .authenticate_publickey(format!("tunnel-{}", self.session_id), key)
            .await;
        let authenticated_handle = match auth_result {
            Ok(russh::client::AuthResult::Success) => {
                info!("SSH public key authentication successful");
                handle
            }
            Ok(russh::client::AuthResult::Failure { .. }) => {
                warn!("SSH public key authentication failed - server rejected key");
                return Err(anyhow::anyhow!("SSH public key authentication failed"));
            }
            Err(e) => {
                error!("SSH public key authentication error: {:?}", e);
                return Err(anyhow::anyhow!(
                    "SSH public key authentication error: {:?}",
                    e
                ));
            }
        };

        // Store the authenticated handle for later use
        self.handle = Some(authenticated_handle);

        info!("SSH tunnel connection established and ready for tunnel operations");

        Ok(())
    }

    /// Request reverse tunnel from EdgeHub using tcpip-forward (SSH -R)
    pub async fn request_reverse_tunnel(&mut self, allocated_port: u16) -> Result<()> {
        info!(
            "Phase 1: Requesting reverse tunnel for subdomain: {}",
            self.config.subdomain
        );
        info!(
            "Phase 1: Sending tcpip-forward request to EdgeHub for port: {}",
            allocated_port
        );

        if let Some(handle) = self.handle.as_mut() {
            // Send tcpip-forward global request (SSH -R semantics for reverse
            // tunnel). russh 0.60 returns the port the server bound (u32).
            match handle.tcpip_forward("0.0.0.0", allocated_port as u32).await {
                Ok(bound_port) => {
                    info!(
                        requested = allocated_port,
                        bound = bound_port,
                        "tcpip-forward accepted by EdgeHub"
                    );
                    // The server will open forwarded-tcpip channels when external connections arrive
                }
                Err(e) => {
                    error!("Phase 1: Failed to send tcpip-forward request: {}", e);
                    return Err(anyhow::anyhow!(
                        "Failed to send tcpip-forward request: {}",
                        e
                    ));
                }
            }
        } else {
            warn!("Phase 1: No SSH handle available for tunnel request");
            return Err(anyhow::anyhow!("No SSH handle available"));
        }

        info!("Phase 1: Reverse tunnel request sent, waiting for external connections");
        Ok(())
    }

    /// Whether the SSH session is still up (TDP-11: the CLI must observe the
    /// real transport, not its own bookkeeping).
    pub fn is_connected(&self) -> bool {
        self.handle.as_ref().is_some_and(|h| !h.is_closed())
    }

    /// TDP-11: generate transport traffic so neither side's inactivity timer
    /// fires on an idle tunnel. russh 0.40 exposes no
    /// `keepalive@openssh.com` global request, so we open and immediately
    /// close a session channel — a rejected open is still traffic, so only
    /// transport-level errors count as failure. NOTE: when russh is upgraded
    /// (≥0.44), replace this with the built-in keepalive configuration.
    pub async fn send_keepalive(&mut self) -> Result<()> {
        let handle = self
            .handle
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("No active SSH handle"))?;
        if handle.is_closed() {
            return Err(anyhow::anyhow!("SSH session closed"));
        }
        match handle.channel_open_session().await {
            Ok(channel) => {
                let _ = channel.close().await;
                Ok(())
            }
            // A channel-open rejection proves the transport is alive.
            Err(russh::Error::ChannelOpenFailure(_)) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("keepalive failed: {e}")),
        }
    }

    /// TDP-11: hold the tunnel open until `expires_at`, sending keepalives on
    /// `keep_alive_interval` and returning an error the moment the session
    /// actually drops (instead of ticking a stale HashMap forever).
    pub async fn run_until_expiry(
        &mut self,
        expires_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<()> {
        let interval = self.config.keep_alive_interval;
        loop {
            if chrono::Utc::now() >= expires_at {
                info!("Tunnel TTL reached; closing");
                return self.close_tunnel().await;
            }
            tokio::time::sleep(interval).await;
            if !self.is_connected() {
                return Err(anyhow::anyhow!(
                    "SSH session to EdgeHub dropped (detected within one keepalive interval)"
                ));
            }
            if let Err(e) = self.send_keepalive().await {
                return Err(anyhow::anyhow!("SSH session lost: {e}"));
            }
        }
    }

    /// Close the tunnel, sending an SSH disconnect so the hub tears down the
    /// slot listener immediately rather than waiting for TCP timeout.
    pub async fn close_tunnel(&mut self) -> Result<()> {
        info!("Closing SSH tunnel for session: {}", self.session_id);

        if let Some(handle) = self.handle.take() {
            let _ = handle
                .disconnect(russh::Disconnect::ByApplication, "tunnel closed", "en")
                .await;
            info!("SSH connection closed");
        }
        Ok(())
    }
}

/// Create a tunnel client with default configuration
#[allow(dead_code)] // exercised in tests; public convenience constructor
pub fn create_tunnel_client(
    hub_url: String,
    hub_port: u16,
    local_port: u16,
    subdomain: String,
) -> TunnelClient<SshClientHandler> {
    let config = TunnelClientConfig {
        hub_url,
        hub_port,
        local_port,
        subdomain,
        connection_timeout: Duration::from_secs(30),
        keep_alive_interval: Duration::from_secs(60),
    };

    let session_id = uuid::Uuid::new_v4().to_string();
    TunnelClient::new(config, session_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_sane() {
        let c = TunnelClientConfig::default();
        assert_eq!(c.hub_port, 2222);
        assert_eq!(c.local_port, 8080);
        assert_eq!(c.hub_url, "localhost");
        // TDP-11 regression guard: keepalive interval must exist and the
        // connection timeout must never be reused as an inactivity timeout.
        assert!(c.keep_alive_interval >= Duration::from_secs(10));
        assert!(c.connection_timeout <= Duration::from_secs(60));
    }

    #[test]
    fn handler_construction() {
        let h = SshClientHandler::new("sess-1".to_string(), "sub".to_string(), 3000);
        assert_eq!(h.local_port, 3000);
        assert_eq!(h.session_id, "sess-1");
        assert_eq!(h.subdomain, "sub");
    }

    #[test]
    fn create_tunnel_client_builds_config() {
        let client = create_tunnel_client("hub.example".to_string(), 2222, 3000, "sub".to_string());
        assert!(!client.is_connected(), "no session yet");
        assert_eq!(client.config.hub_url, "hub.example");
        assert_eq!(client.config.local_port, 3000);
    }

    #[tokio::test]
    async fn keepalive_without_session_errors() {
        let mut client: TunnelClient<SshClientHandler> =
            TunnelClient::new(TunnelClientConfig::default(), "sess".to_string());
        assert!(client.send_keepalive().await.is_err());
        assert!(client.request_reverse_tunnel(40000).await.is_err());
    }

    #[tokio::test]
    async fn close_without_session_is_ok() {
        let mut client: TunnelClient<SshClientHandler> =
            TunnelClient::new(TunnelClientConfig::default(), "sess".to_string());
        assert!(client.close_tunnel().await.is_ok());
    }

    #[tokio::test]
    async fn run_until_expiry_returns_at_ttl() {
        // Already-expired tunnel: loop exits immediately via close_tunnel.
        let mut client: TunnelClient<SshClientHandler> =
            TunnelClient::new(TunnelClientConfig::default(), "sess".to_string());
        let past = chrono::Utc::now() - chrono::Duration::seconds(1);
        assert!(client.run_until_expiry(past).await.is_ok());
    }

    #[tokio::test]
    async fn establish_tunnel_fails_fast_when_hub_unreachable() {
        let config = TunnelClientConfig {
            hub_url: "127.0.0.1".to_string(),
            hub_port: 1, // nothing listens here
            connection_timeout: Duration::from_secs(2),
            ..TunnelClientConfig::default()
        };
        let mut client = TunnelClient::new(config, "sess".to_string());
        let handler = SshClientHandler::new("sess".to_string(), "sub".to_string(), 3000);
        let key =
            russh::keys::PrivateKey::random(&mut rand_key::rng(), russh::keys::Algorithm::Ed25519)
                .unwrap();
        assert!(client.establish_tunnel(handler, key).await.is_err());
        assert!(!client.is_connected());
    }
}
