use std::net::SocketAddr;

use clap::Parser;
use tracing::info;

use common::{init_tracing, AppResult};
use dnsd::{self, redis_cache, Config};
#[cfg(feature = "dot")]
use common::tls;

/// DNS daemon command line arguments.
#[derive(Parser, Debug, Clone)]
struct Args {
    /// Address to listen on.
    #[arg(long, default_value = "0.0.0.0:5353")]
    addr: SocketAddr,
}

async fn run(args: Args) -> AppResult<()> {
    init_tracing();
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
    let pool = redis_cache::new_pool(&redis_url)
        .await
        .map_err(|e| common::AppError::Message(e.to_string()))?;
    info!(addr = %args.addr, "dnsd listening");
    #[cfg(feature = "dot")]
    let (tls_config, _) = tls::generate_tls_config(&["dot"])?;
    let cfg = Config {
        addr: args.addr,
        redis_pool: pool,
        #[cfg(feature = "dot")]
        dot_addr: SocketAddr::new(args.addr.ip(), 853),
        #[cfg(feature = "dot")]
        tls_config,
    };
    dnsd::serve(cfg).await
}

#[tokio::main]
async fn main() -> AppResult<()> {
    let args = Args::parse();
    run(args).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::UdpSocket as StdUdpSocket;
    use tokio::net::{TcpListener, UdpSocket};
    use tokio::task::JoinHandle;
    use tokio::time::{sleep, Duration};
    use tracing_test::traced_test;
    use mini_redis::server;

    async fn start_redis() -> (String, JoinHandle<mini_redis::Result<()>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move { server::run(listener, tokio::signal::ctrl_c()).await });
        (format!("redis://{}", addr), handle)
    }

    #[tokio::test]
    #[traced_test]
    async fn logs_startup_message() {
        let std_sock = StdUdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = std_sock.local_addr().unwrap();
        drop(std_sock);

        let (redis_url, redis_handle) = start_redis().await;
        std::env::set_var("REDIS_URL", &redis_url);
        let handle = tokio::spawn(async move { run(Args { addr }).await.unwrap() });

        sleep(Duration::from_millis(50)).await;

        let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        client.send_to(&[0u8; 12], addr).await.unwrap();

        sleep(Duration::from_millis(50)).await;
        handle.abort();
        redis_handle.abort();

        assert!(tracing_test::logs_contain("dnsd listening"));
    }
}
