#![cfg(feature = "dot")]

use std::net::{TcpListener, UdpSocket as StdUdpSocket};
use std::process::Stdio;
use tokio::process::Command;
use tokio::time::{Duration, sleep};

use common::tls;
use dnsd::{Config, serve};

#[cfg(feature = "dot")]
#[tokio::test]
async fn kdig_dot_returns_loopback() {
    let udp_sock = StdUdpSocket::bind("127.0.0.1:0").unwrap();
    let udp_addr = udp_sock.local_addr().unwrap();
    drop(udp_sock);

    let tcp = TcpListener::bind("127.0.0.1:0").unwrap();
    let dot_addr = tcp.local_addr().unwrap();
    drop(tcp);

    let (tls_config, cert_pem) = tls::generate_tls_config(&["dot"]).unwrap();
    let cert_file = std::env::temp_dir().join("dot_cert.pem");
    std::fs::write(&cert_file, &cert_pem).unwrap();

    let cfg = Config {
        addr: udp_addr,
        dot_addr,
        tls_config,
    };

    let handle = tokio::spawn(async move { serve(cfg).await.unwrap() });
    sleep(Duration::from_millis(50)).await;

    let output = Command::new("kdig")
        .arg(format!("@{}", dot_addr.ip()))
        .arg("-p")
        .arg(dot_addr.port().to_string())
        .arg(format!("+tls-ca={}", cert_file.to_str().unwrap()))
        .arg("+tls-host=tls.local")
        .arg("test.fdns.run")
        .arg("+short")
        .stdout(Stdio::piped())
        .output()
        .await
        .expect("kdig executed");

    handle.abort();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("127.0.0.1"));
}
