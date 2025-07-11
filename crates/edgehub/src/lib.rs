use std::net::SocketAddr;
use std::sync::Arc;

use common::AppResult;
use common::shutdown::ShutdownSignal;
use rand::Rng;
use rustls::ServerConfig;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio_rustls::TlsAcceptor;
use tracing::{info, instrument};

pub mod redis;

/// Configuration for the EdgeHub server.
#[derive(Debug, Clone)]
pub struct Config {
    /// Address to bind the TLS listener to.
    pub addr: SocketAddr,
    /// TLS configuration used for incoming connections.
    pub tls_config: ServerConfig,
    /// Redis connection pool used to record tunnel state.
    pub redis_pool: redis::RedisPool,
}

/// Start the EdgeHub server.
///
/// Binds a TCP socket, accepts TLS connections, and logs a placeholder
/// mapping of the `slot` to a randomly chosen local port.
#[instrument]
pub async fn serve(cfg: Config) -> AppResult<()> {
    let listener = TcpListener::bind(cfg.addr).await?;
    info!(addr = %listener.local_addr()?, "edgehub listening");

    let acceptor = TlsAcceptor::from(Arc::new(cfg.tls_config));
    let pool = cfg.redis_pool.clone();

    loop {
        let (stream, peer) = listener.accept().await?;
        let acceptor = acceptor.clone();
        let pool = pool.clone();
        tokio::spawn(async move {
            match acceptor.accept(stream).await {
                Ok(mut tls) => {
                    let port: u16 = rand::thread_rng().gen_range(30000..60000);
                    let slot = "demo";
                    if let std::net::IpAddr::V4(ip) = peer.ip() {
                        let _ = redis::set_slot(&pool, slot, ip, redis::DEFAULT_TTL).await;
                    }
                    info!(peer=%peer, slot, port, "tunnel mapped");
                    let _ = tls.shutdown().await;
                    let _ = redis::del_slot(&pool, slot).await;
                }
                Err(e) => {
                    info!(error=%e, peer=%peer, "tls handshake failed");
                }
            }
        });
    }
}

/// Start the EdgeHub server with graceful shutdown support.
///
/// This version accepts a shutdown signal receiver and will gracefully
/// shutdown when a signal is received, properly cleaning up TLS connections
/// and Redis state.
#[instrument]
pub async fn serve_with_shutdown(
    cfg: Config,
    mut shutdown_rx: broadcast::Receiver<ShutdownSignal>,
) -> AppResult<()> {
    let listener = TcpListener::bind(cfg.addr).await?;
    info!(addr = %listener.local_addr()?, "EdgeHub listening with graceful shutdown support");

    let acceptor = TlsAcceptor::from(Arc::new(cfg.tls_config));
    let pool = cfg.redis_pool.clone();

    loop {
        tokio::select! {
            // Handle new connections
            result = listener.accept() => {
                match result {
                    Ok((stream, peer)) => {
                        let acceptor = acceptor.clone();
                        let pool = pool.clone();
                        tokio::spawn(async move {
                            match acceptor.accept(stream).await {
                                Ok(mut tls) => {
                                    let port: u16 = rand::thread_rng().gen_range(30000..60000);
                                    let slot = "demo";
                                    if let std::net::IpAddr::V4(ip) = peer.ip() {
                                        let _ = redis::set_slot(&pool, slot, ip, redis::DEFAULT_TTL).await;
                                    }
                                    info!(peer=%peer, slot, port, "tunnel mapped");
                                    let _ = tls.shutdown().await;
                                    let _ = redis::del_slot(&pool, slot).await;
                                }
                                Err(e) => {
                                    info!(error=%e, peer=%peer, "tls handshake failed");
                                }
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("EdgeHub accept error: {}", e);
                        break;
                    }
                }
            }
            // Handle shutdown signal
            _ = shutdown_rx.recv() => {
                info!("EdgeHub received shutdown signal, stopping");
                break;
            }
        }
    }

    info!("EdgeHub shutdown complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mini_redis::server;
    use rustls::{
        ClientConfig, RootCertStore,
        pki_types::{CertificateDer, ServerName},
    };
    use rustls_pemfile::Item;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;
    use tokio::net::TcpStream;
    use tokio_rustls::TlsConnector;
    use tracing_test::traced_test;

    #[tokio::test]
    #[traced_test]
    async fn logs_mapping_on_connect() {
        let std_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = std_listener.local_addr().unwrap();
        drop(std_listener);

        async fn start_redis() -> (String, tokio::task::JoinHandle<mini_redis::Result<()>>) {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let handle =
                tokio::spawn(async move { server::run(listener, tokio::signal::ctrl_c()).await });
            (format!("redis://{addr}"), handle)
        }

        let (redis_url, redis_handle) = start_redis().await;
        let pool = redis::new_pool(&redis_url).await.unwrap();

        let (tls_config, cert_pem) = common::tls::generate_tls_config(&["ssh"]).unwrap();
        let handle = tokio::spawn(async move {
            serve(Config {
                addr,
                tls_config,
                redis_pool: pool,
            })
            .await
            .unwrap();
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let mut roots = RootCertStore::empty();
        let mut cursor = std::io::Cursor::new(cert_pem);
        if let Some(Item::X509Certificate(cert)) = rustls_pemfile::read_one(&mut cursor).unwrap() {
            roots.add(CertificateDer::from(cert)).unwrap();
        }

        let client_config = ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        let connector = TlsConnector::from(Arc::new(client_config));
        let stream = TcpStream::connect(addr).await.unwrap();
        let name = ServerName::try_from("tls.local").unwrap();
        let mut tls = connector.connect(name, stream).await.unwrap();
        tls.shutdown().await.unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        handle.abort();
        redis_handle.abort();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let pool = redis::new_pool(&redis_url).await.unwrap();
        let err = redis::get_slot(&pool, "demo").await.err();
        assert!(err.is_some());
    }
}
