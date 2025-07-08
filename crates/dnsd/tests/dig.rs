use std::net::UdpSocket as StdUdpSocket;
use std::process::Stdio;
use tokio::process::Command;
use tokio::time::{sleep, Duration};

use dnsd::{serve, Config};

#[tokio::test]
async fn dig_returns_loopback() {
    let std_sock = StdUdpSocket::bind("127.0.0.1:0").unwrap();
    let addr = std_sock.local_addr().unwrap();
    drop(std_sock);

    let cfg = Config { addr };
    let handle = tokio::spawn(async move { serve(cfg).await.unwrap() });
    sleep(Duration::from_millis(50)).await;

    let output = Command::new("dig")
        .arg(format!("@{}", addr.ip()))
        .arg("-p")
        .arg(addr.port().to_string())
        .arg("test.fdns.run")
        .arg("+short")
        .stdout(Stdio::piped())
        .output()
        .await
        .expect("dig executed");

    handle.abort();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("127.0.0.1"));
}
