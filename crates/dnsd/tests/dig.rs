#![cfg(not(feature = "dot"))]

use std::net::{Ipv4Addr, SocketAddr};
use std::process::Stdio;
use std::time::Duration;

use mini_redis::server;
use tokio::net::TcpListener;
use tokio::process::Command;
use tokio::task::JoinHandle;
use tokio::time::sleep;

use dnsd::{serve, Config};
use dnsd::redis_cache;
use dnsd::sign;
use dnsd::performance;

async fn start_redis() -> Option<(String, JoinHandle<mini_redis::Result<()>>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await.ok()?;
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move { server::run(listener, tokio::signal::ctrl_c()).await });
    sleep(Duration::from_millis(50)).await;
    let url = format!("redis://{addr}/");
    if redis_cache::new_pool(&url).await.is_err() {
        handle.abort();
        return None;
    }
    Some((url, handle))
}

#[tokio::test]
async fn dig_returns_cached_ip() {
    if std::env::var("RUN_REDIS_TESTS").is_err() {
        eprintln!("skipping test: RUN_REDIS_TESTS not set");
        return;
    }
    let Some((redis_url, redis_handle)) = start_redis().await else {
        eprintln!("skipping test: redis not available");
        return;
    };
    let pool = redis_cache::new_pool(&redis_url).await.unwrap();
    redis_cache::set_slot(&pool, "demo", Ipv4Addr::new(1, 2, 3, 4), 60)
        .await
        .unwrap();

    let std_sock = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let addr = std_sock.local_addr().unwrap();
    drop(std_sock);

    let cfg = Config {
        addr,
        redis_pool: pool.clone(),
        dnssec_config: sign::DnssecConfig::default(),
        ddos_config: common::ddos_protection::DdosConfig::default(),
        enable_ddos_protection: false,
        performance_config: performance::PerformanceConfig::default(),
    };
    let handle = tokio::spawn(async move { serve(cfg).await.unwrap() });
    sleep(Duration::from_millis(50)).await;

    let output = Command::new("dig")
        .arg(format!("@{}", addr.ip()))
        .arg("-p")
        .arg(addr.port().to_string())
        .arg("demo.fdns.run")
        .arg("+short")
        .stdout(Stdio::piped())
        .output()
        .await
        .expect("dig executed");

    handle.abort();
    redis_handle.abort();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("1.2.3.4"));
}

#[tokio::test]
async fn dig_returns_nxdomain_on_miss() {
    if std::env::var("RUN_REDIS_TESTS").is_err() {
        eprintln!("skipping test: RUN_REDIS_TESTS not set");
        return;
    }
    let Some((redis_url, redis_handle)) = start_redis().await else {
        eprintln!("skipping test: redis not available");
        return;
    };
    let pool = redis_cache::new_pool(&redis_url).await.unwrap();

    let std_sock = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let addr = std_sock.local_addr().unwrap();
    drop(std_sock);

    let cfg = Config {
        addr,
        redis_pool: pool.clone(),
        dnssec_config: sign::DnssecConfig::default(),
        ddos_config: common::ddos_protection::DdosConfig::default(),
        enable_ddos_protection: false,
        performance_config: performance::PerformanceConfig::default(),
    };
    let handle = tokio::spawn(async move { serve(cfg).await.unwrap() });
    sleep(Duration::from_millis(50)).await;

    let output = Command::new("dig")
        .arg(format!("@{}", addr.ip()))
        .arg("-p")
        .arg(addr.port().to_string())
        .arg("missing.fdns.run")
        .stdout(Stdio::piped())
        .output()
        .await
        .expect("dig executed");

    handle.abort();
    redis_handle.abort();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.trim().is_empty());
}
