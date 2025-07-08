use std::net::SocketAddr;

use common::AppResult;

mod udp;
use tokio::net::UdpSocket;
use tracing::{info, instrument};

/// Configuration for the DNS server.
#[derive(Debug, Clone)]
pub struct Config {
    /// Address to bind the UDP socket to.
    pub addr: SocketAddr,
}

/// Run the UDP server.
///
/// This binds a UDP socket and logs the number of bytes received for each
/// packet. The function runs until cancelled.
#[instrument]
pub async fn serve(cfg: Config) -> AppResult<()> {
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
        drop(std_sock);

        let cfg = Config { addr };
        let handle = tokio::spawn(async move { serve(cfg).await.unwrap() });

        sleep(Duration::from_millis(50)).await;

        let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        client.send_to(&[0u8; 12], addr).await.unwrap();

        sleep(Duration::from_millis(50)).await;
        handle.abort();

        assert!(logs_contain("received 12 bytes"));
    }
}
