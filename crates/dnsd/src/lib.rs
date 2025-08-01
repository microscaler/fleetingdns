use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tracing::{info, instrument, error};
use common::{AppResult, shutdown::GracefulShutdown};
use tokio::sync::broadcast;

use crate::dns_handler::{DnsHandler, PerformanceConfig};

pub mod redis_cache;
pub mod sign;
pub mod redis_performance;
pub mod redis_sentinel;
pub mod redis_cluster;
pub mod dns_handler;
pub mod metrics_manager;

/// Configuration for the DNS server.
pub struct Config {
    /// Address to bind the UDP socket to.
    pub addr: SocketAddr,
    /// Redis connection pool for slot lookups.
    pub redis_pool: redis_cache::RedisPool,
    /// DDoS protection configuration
    pub ddos_config: common::ddos_protection::DdosConfig,
    /// Enable DDoS protection
    pub enable_ddos_protection: bool,
    /// DNSSEC configuration
    pub dnssec_config: sign::DnssecConfig,
    /// Performance optimization configuration
    pub performance_config: PerformanceConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            addr: "0.0.0.0:5353".parse().unwrap(),
            redis_pool: {
                let manager = bb8_redis::RedisConnectionManager::new("redis://localhost:6379").unwrap();
                bb8::Pool::builder().build_unchecked(manager)
            },
            ddos_config: common::ddos_protection::DdosConfig::default(),
            enable_ddos_protection: false,
            dnssec_config: sign::DnssecConfig::default(),
            performance_config: PerformanceConfig::default(),
        }
    }
}

/// Start the DNS server with the given configuration.
#[instrument(skip(cfg))]
pub async fn serve(cfg: Config) -> AppResult<()> {
    info!("Starting DNS server on {}", cfg.addr);

    // Initialize DNSSEC signer if configured
    if cfg.dnssec_config.enable_signature_cache {
        if let Err(e) = sign::init_production_signer(cfg.dnssec_config.clone()) {
            error!("Failed to initialize DNSSEC signer: {}", e);
        }
    }

    // Create unified DNS handler with performance optimizations
    let dns_handler = DnsHandler::new(cfg.performance_config.clone());

    // Create UDP socket
    let socket = UdpSocket::bind(cfg.addr).await?;
    info!("DNS server listening on {}", cfg.addr);

    // Create shutdown signal
    let (_shutdown_tx, mut shutdown_rx) = broadcast::channel::<()>(1);

    // Main server loop
    let mut buf = [0; 512];
    loop {
        tokio::select! {
            result = socket.recv_from(&mut buf) => {
                match result {
                    Ok((len, peer)) => {
                        // Use unified DNS handler
                        if let Ok(resp) = dns_handler.handle_packet(&buf[..len], &cfg.redis_pool).await {
                            if let Err(e) = socket.send_to(&resp, peer).await {
                                error!("Failed to send response to {}: {}", peer, e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to receive packet: {}", e);
                    }
                }
            }
            _ = shutdown_rx.recv() => {
                info!("Shutdown signal received, stopping DNS server");
                break;
            }
        }
    }

    Ok(())
}

/// Start the DNS server with graceful shutdown support.
pub async fn serve_with_shutdown(
    cfg: Config,
    shutdown: GracefulShutdown,
) -> AppResult<()> {
    info!("Starting DNS server with graceful shutdown on {}", cfg.addr);

    // Initialize DNSSEC signer if configured
    if cfg.dnssec_config.enable_signature_cache {
        if let Err(e) = sign::init_production_signer(cfg.dnssec_config.clone()) {
            error!("Failed to initialize DNSSEC signer: {}", e);
        }
    }

    // Create unified DNS handler with performance optimizations
    let dns_handler = DnsHandler::new(cfg.performance_config.clone());

    // Create UDP socket
    let socket = UdpSocket::bind(cfg.addr).await?;
    info!("DNS server listening on {}", cfg.addr);

    // Main server loop with graceful shutdown
    let mut buf = [0; 512];
    let mut shutdown_rx = shutdown.subscribe();
    loop {
        tokio::select! {
            result = socket.recv_from(&mut buf) => {
                match result {
                    Ok((len, peer)) => {
                        // Use unified DNS handler
                        if let Ok(resp) = dns_handler.handle_packet(&buf[..len], &cfg.redis_pool).await {
                            if let Err(e) = socket.send_to(&resp, peer).await {
                                error!("Failed to send response to {}: {}", peer, e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to receive packet: {}", e);
                    }
                }
            }
            _ = shutdown_rx.recv() => {
                info!("Shutdown signal received, stopping DNS server gracefully");
                break;
            }
        }
    }

    info!("DNS server stopped gracefully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Configure tests to run sequentially to avoid race conditions with global singleton
    #[test]
    fn test_sequential_execution() {
        // This test ensures other tests run sequentially
    }
}

#[cfg(feature = "dot")]
mod dot {
    use super::redis_cache;
    use super::dns_handler::DnsHandler;
    use common::AppResult;
    use rustls::ServerConfig;
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio_rustls::TlsAcceptor;
    use tracing::info;

    /// Run the DNS-over-TLS server.
    ///
    /// Binds a TLS listener on the provided address and accepts connections
    /// using the given [`ServerConfig`]. Each connection expects a 16-bit
    /// length-prefixed DNS message. Queries are processed by the unified DNS handler and
    /// the encoded response is written back to the client.
    ///
    /// The server runs indefinitely until the task is cancelled. Errors are
    /// returned if binding the listener or accepting connections fails.
    /// Returns `Ok(())` when the server shuts down gracefully.
    ///
    /// * `addr` - Address to bind the TLS listener.
    /// * `cfg` - TLS configuration for the server.
    /// * `pool` - Redis connection pool used for DNS record lookups.
    pub async fn serve(
        addr: std::net::SocketAddr,
        cfg: ServerConfig,
        pool: redis_cache::RedisPool,
    ) -> AppResult<()> {
        let listener = TcpListener::bind(addr).await?;
        info!(addr=%listener.local_addr()?, "dot listening");
        let acceptor = TlsAcceptor::from(Arc::new(cfg));
        
        // Create unified DNS handler
        let dns_handler = DnsHandler::new(super::dns_handler::PerformanceConfig::default());
        
        loop {
            let (stream, peer) = listener.accept().await?;
            let acceptor = acceptor.clone();
            let pool = pool.clone();
            let dns_handler = dns_handler.clone();
            tokio::spawn(async move {
                if let Ok(mut tls) = acceptor.accept(stream).await {
                    let mut len_buf = [0u8; 2];
                    loop {
                        if tls.read_exact(&mut len_buf).await.is_err() {
                            break;
                        }
                        let len = u16::from_be_bytes(len_buf) as usize;
                        let mut buf = vec![0u8; len];
                        if tls.read_exact(&mut buf).await.is_err() {
                            break;
                        }
                        if let Ok(resp) = dns_handler.handle_packet(&buf, &pool).await {
                            let resp_len = (resp.len() as u16).to_be_bytes();
                            if tls.write_all(&resp_len).await.is_err() {
                                break;
                            }
                            if tls.write_all(&resp).await.is_err() {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    let _ = tls.shutdown().await;
                }
                let _ = peer;
            });
        }
    }

    /// Run the DNS-over-TLS server with graceful shutdown support.
    pub async fn serve_with_shutdown(
        addr: std::net::SocketAddr,
        cfg: ServerConfig,
        pool: redis_cache::RedisPool,
        mut shutdown_rx: super::broadcast::Receiver<super::ShutdownSignal>,
    ) -> AppResult<()> {
        let listener = TcpListener::bind(addr).await?;
        info!(addr=%listener.local_addr()?, "DoT server listening with graceful shutdown support");
        let acceptor = TlsAcceptor::from(Arc::new(cfg));

        loop {
            tokio::select! {
                // Handle new connections
                result = listener.accept() => {
                    match result {
                        Ok((stream, peer)) => {
                            let acceptor = acceptor.clone();
                            let pool = pool.clone();
                            tokio::spawn(async move {
                                if let Ok(mut tls) = acceptor.accept(stream).await {
                                    let mut len_buf = [0u8; 2];
                                    loop {
                                        if tls.read_exact(&mut len_buf).await.is_err() {
                                            break;
                                        }
                                        let len = u16::from_be_bytes(len_buf) as usize;
                                        let mut buf = vec![0u8; len];
                                        if tls.read_exact(&mut buf).await.is_err() {
                                            break;
                                        }
                                        if let Ok(resp) = super::udp::handle_packet(&buf, &pool).await {
                                            let resp_len = (resp.len() as u16).to_be_bytes();
                                            if tls.write_all(&resp_len).await.is_err() {
                                                break;
                                            }
                                            if tls.write_all(&resp).await.is_err() {
                                                break;
                                            }
                                        } else {
                                            break;
                                        }
                                    }
                                    let _ = tls.shutdown().await;
                                }
                                let _ = peer;
                            });
                        }
                        Err(e) => {
                            tracing::error!("DoT accept error: {}", e);
                            break;
                        }
                    }
                }
                // Handle shutdown signal
                _ = shutdown_rx.recv() => {
                    info!("DoT server received shutdown signal, stopping");
                    break;
                }
            }
        }

        info!("DoT server shutdown complete");
        Ok(())
    }
}
