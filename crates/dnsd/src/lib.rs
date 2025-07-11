use std::net::SocketAddr;

use common::AppResult;
use common::shutdown::ShutdownSignal;
#[cfg(feature = "dot")]
use rustls::ServerConfig;
use tokio::sync::broadcast;

pub mod redis_cache;
pub mod sign;
mod udp;
use tokio::net::UdpSocket;
use tracing::{info, instrument};

/// Configuration for the DNS server.
#[derive(Debug, Clone)]
pub struct Config {
    /// Address to bind the UDP socket to.
    pub addr: SocketAddr,
    /// Redis connection pool for slot lookups.
    pub redis_pool: redis_cache::RedisPool,
    #[cfg(feature = "dot")]
    /// Address for DNS-over-TLS.
    pub dot_addr: SocketAddr,
    #[cfg(feature = "dot")]
    /// TLS configuration for DoT.
    pub tls_config: ServerConfig,
}

/// Run the UDP server.
///
/// This binds a UDP socket and logs the number of bytes received for each
/// packet. The function runs until cancelled.
#[instrument]
pub async fn serve(cfg: Config) -> AppResult<()> {
    let pool = cfg.redis_pool.clone();
    #[cfg(feature = "dot")]
    tokio::spawn(dot::serve(
        cfg.dot_addr,
        cfg.tls_config.clone(),
        pool.clone(),
    ));

    let socket = UdpSocket::bind(cfg.addr).await?;
    info!(addr = %socket.local_addr()?, "listening");
    let mut buf = [0u8; 512];
    loop {
        let (len, peer) = socket.recv_from(&mut buf).await?;
        info!("received {} bytes", len);
        if let Ok(resp) = udp::handle_packet(&buf[..len], &pool).await {
            let _ = socket.send_to(&resp, peer).await?;
        }
    }
}

/// Run the DNS server with graceful shutdown support.
///
/// This version accepts a shutdown signal receiver and will gracefully
/// shutdown when a signal is received.
#[instrument]
pub async fn serve_with_shutdown(
    cfg: Config,
    mut shutdown_rx: broadcast::Receiver<ShutdownSignal>,
) -> AppResult<()> {
    let pool = cfg.redis_pool.clone();

    // Start DoT server with shutdown support
    #[cfg(feature = "dot")]
    let dot_handle = tokio::spawn(dot::serve_with_shutdown(
        cfg.dot_addr,
        cfg.tls_config.clone(),
        pool.clone(),
        shutdown_rx.resubscribe(),
    ));

    let socket = UdpSocket::bind(cfg.addr).await?;
    info!(addr = %socket.local_addr()?, "DNS server listening with graceful shutdown support");

    let mut buf = [0u8; 512];
    loop {
        tokio::select! {
            // Handle DNS requests
            result = socket.recv_from(&mut buf) => {
                match result {
                    Ok((len, peer)) => {
                        info!("received {} bytes", len);
                        if let Ok(resp) = udp::handle_packet(&buf[..len], &pool).await {
                            let _ = socket.send_to(&resp, peer).await;
                        }
                    }
                    Err(e) => {
                        tracing::error!("UDP socket error: {}", e);
                        break;
                    }
                }
            }
            // Handle shutdown signal
            _ = shutdown_rx.recv() => {
                info!("DNS server received shutdown signal, stopping");
                break;
            }
        }
    }

    // Cleanup: DoT server will shutdown via its own signal
    #[cfg(feature = "dot")]
    {
        dot_handle.abort();
        let _ = dot_handle.await;
    }

    info!("DNS server shutdown complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mini_redis::server;
    use std::net::UdpSocket as StdUdpSocket;
    use tokio::net::{TcpListener, UdpSocket};
    use tokio::task::JoinHandle;
    use tokio::time::{Duration, sleep};
    use tracing_test::traced_test;

    #[tokio::test]
    #[traced_test]
    async fn logs_received_bytes() {
        async fn start_redis() -> (String, JoinHandle<mini_redis::Result<()>>) {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let handle =
                tokio::spawn(async move { server::run(listener, tokio::signal::ctrl_c()).await });
            (format!("redis://{addr}"), handle)
        }

        let (redis_url, redis_handle) = start_redis().await;
        let pool = redis_cache::new_pool(&redis_url).await.unwrap();

        let std_sock = StdUdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = std_sock.local_addr().unwrap();
        #[cfg(feature = "dot")]
        let dot_addr = addr;
        drop(std_sock);

        #[cfg(feature = "dot")]
        let (tls_config, _) = common::tls::generate_tls_config(&["dot"]).unwrap();
        let cfg = Config {
            addr,
            redis_pool: pool.clone(),
            #[cfg(feature = "dot")]
            dot_addr,
            #[cfg(feature = "dot")]
            tls_config,
        };
        let handle = tokio::spawn(async move { serve(cfg).await.unwrap() });

        sleep(Duration::from_millis(50)).await;

        let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        client.send_to(&[0u8; 12], addr).await.unwrap();

        sleep(Duration::from_millis(50)).await;
        handle.abort();
        redis_handle.abort();

        assert!(logs_contain("received 12 bytes"));
    }
}

#[cfg(feature = "dot")]
mod dot {
    use super::redis_cache;
    use super::udp;
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
    /// length-prefixed DNS message. Queries are processed by [`udp::handle_packet`] and
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
        loop {
            let (stream, peer) = listener.accept().await?;
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
                        if let Ok(resp) = udp::handle_packet(&buf, &pool).await {
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
