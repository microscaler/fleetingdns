#![cfg(feature = "dot")]

use std::net::{Ipv4Addr, TcpListener};
use std::process::Stdio;
use tokio::process::Command;
use tokio::time::{Duration, sleep};

use common::tls;
use mini_redis::server;
use tokio::net::TcpListener as TokioTcpListener;
use tokio::task::JoinHandle;

#[tokio::test]
async fn kdig_dot_returns_loopback() {
    if std::env::var("RUN_REDIS_TESTS").is_err() {
        eprintln!("skipping test: RUN_REDIS_TESTS not set");
        return;
    }
    async fn start_redis() -> Option<(String, JoinHandle<mini_redis::Result<()>>)> {
        let listener = TokioTcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle =
            tokio::spawn(async move { server::run(listener, tokio::signal::ctrl_c()).await });
        sleep(Duration::from_millis(50)).await;
        let url = format!("redis://{addr}/");
        if common::redis::new_pool(&url).await.is_err() {
            handle.abort();
            return None;
        }
        Some((url, handle))
    }

    let Some((redis_url, redis_handle)) = start_redis().await else {
        eprintln!("skipping test: redis not available");
        return;
    };
    let pool = common::redis::new_pool(&redis_url).await.unwrap();
    common::redis::set_slot(&pool, "demo", Ipv4Addr::new(1, 2, 3, 4), 60)
        .await
        .unwrap();

    let tcp = TcpListener::bind("127.0.0.1:0").unwrap();
    let dot_addr = tcp.local_addr().unwrap();
    drop(tcp);

    let (tls_config, cert_pem) = tls::generate_tls_config(&["dot"]).unwrap();
    let cert_file = std::env::temp_dir().join("dot_cert.pem");
    std::fs::write(&cert_file, &cert_pem).unwrap();

    // DoT is now served by the feature-gated `dnsd::dot::serve`, which takes the
    // bind address, a rustls ServerConfig, and the Redis pool directly (the old
    // Config-embedded `dot_addr`/`tls_config`/`cert_manager` fields were removed).
    let handle =
        tokio::spawn(async move { dnsd::dot::serve(dot_addr, tls_config, pool).await.unwrap() });
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
    redis_handle.abort();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("1.2.3.4"));
}
