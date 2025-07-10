use std::net::SocketAddr;

use common::AppResult;
#[cfg(feature = "dot")]
use rustls::ServerConfig;

pub mod redis_cache;
mod udp;
use tokio::net::UdpSocket;
use tracing::{info, instrument};

/// Configuration for the DNS server.
#[derive(Debug, Clone)]
pub struct Config {
    /// Address to bind the UDP socket to.
    pub addr: SocketAddr,
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
    #[cfg(feature = "dot")]
    tokio::spawn(dot::serve(cfg.dot_addr, cfg.tls_config.clone()));

    let socket = UdpSocket::bind(cfg.addr).await?;
    info!(addr = %socket.local_addr()?, "listening");
    let mut buf = [0u8; 512];
    loop {
        let (len, peer) = socket.recv_from(&mut buf).await?;
        info!("received {} bytes", len);
        if let Ok(resp) = udp::handle_packet(&buf[..len]) {
            let _ = socket.send_to(&resp, peer).await?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::UdpSocket as StdUdpSocket;
    use tokio::net::UdpSocket;
    use tokio::time::{Duration, sleep};
    use tracing_test::traced_test;

    #[tokio::test]
    #[traced_test]
    async fn logs_received_bytes() {
        let std_sock = StdUdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = std_sock.local_addr().unwrap();
        #[cfg(feature = "dot")]
        let dot_addr = addr;
        drop(std_sock);

        #[cfg(feature = "dot")]
        let (tls_config, _) = common::tls::generate_tls_config(&["dot"]).unwrap();
        let cfg = Config {
            addr,
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

        assert!(logs_contain("received 12 bytes"));
    }
}

#[cfg(feature = "dot")]
mod dot {
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
    pub async fn serve(addr: std::net::SocketAddr, cfg: ServerConfig) -> AppResult<()> {
        let listener = TcpListener::bind(addr).await?;
        info!(addr=%listener.local_addr()?, "dot listening");
        let acceptor = TlsAcceptor::from(Arc::new(cfg));
        loop {
            let (stream, peer) = listener.accept().await?;
            let acceptor = acceptor.clone();
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
                        if let Ok(resp) = udp::handle_packet(&buf) {
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
}
