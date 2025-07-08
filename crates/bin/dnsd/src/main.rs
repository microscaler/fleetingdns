use std::net::SocketAddr;

use clap::Parser;
use tracing::info;

use common::{init_tracing, AppResult};
use dnsd::{self, Config};
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
    info!(addr = %args.addr, "dnsd listening");
    #[cfg(feature = "dot")]
    let (tls_config, _) = tls::generate_tls_config(&["dot"])?;
    let cfg = Config {
        addr: args.addr,
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
    use tokio::net::UdpSocket;
    use tokio::time::{sleep, Duration};
    use tracing_test::traced_test;

    #[tokio::test]
    #[traced_test]
    async fn logs_startup_message() {
        let std_sock = StdUdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = std_sock.local_addr().unwrap();
        drop(std_sock);

        let handle = tokio::spawn(async move { run(Args { addr }).await.unwrap() });

        sleep(Duration::from_millis(50)).await;

        let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        client.send_to(&[0u8; 12], addr).await.unwrap();

        sleep(Duration::from_millis(50)).await;
        handle.abort();

        assert!(tracing_test::logs_contain("dnsd listening"));
    }
}
