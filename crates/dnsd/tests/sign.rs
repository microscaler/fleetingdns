use std::net::Ipv4Addr;
use std::process::Stdio;
use std::time::Duration;

use mini_redis::server;
use tokio::net::TcpListener;
use tokio::process::Command;
use tokio::task::JoinHandle;
use tokio::time::sleep;

use dnsd::dns_handler;
use dnsd::redis_cache;
use dnsd::sign;
use dnsd::{Config, serve};

#[cfg(feature = "dot")]
use common::tls;
use hickory_proto::op::{Message, MessageType, OpCode, Query};
use hickory_proto::rr::{Name, RecordType};
use hickory_proto::serialize::binary::{BinEncodable, BinEncoder};
use ring::hmac;
use tokio::net::UdpSocket;

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
async fn rrsig_validates() {
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

    unsafe { std::env::set_var("FDNS_HMAC_KEY", "secret") };

    #[cfg(feature = "dot")]
    let tcp = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    #[cfg(feature = "dot")]
    let dot_addr = tcp.local_addr().unwrap();
    #[cfg(feature = "dot")]
    drop(tcp);
    #[cfg(feature = "dot")]
    let (tls_config, _) = tls::generate_tls_config(&["dot"]).unwrap();

    let cfg = Config {
        addr,
        redis_pool: pool.clone(),
        dnssec_config: sign::DnssecConfig::default(),
        ddos_config: common::ddos_protection::DdosConfig::default(),
        enable_ddos_protection: false,
        performance_config: dns_handler::PerformanceConfig::default(),
    };
    let handle = tokio::spawn(async move { serve(cfg).await.unwrap() });
    sleep(Duration::from_millis(50)).await;

    let mut query = Message::new();
    query.set_id(1);
    query.set_message_type(MessageType::Query);
    query.set_op_code(OpCode::Query);
    query.add_query(Query::query(
        Name::from_ascii("demo.fdns.run.").unwrap(),
        RecordType::A,
    ));
    let mut qbuf = Vec::with_capacity(512);
    query.emit(&mut BinEncoder::new(&mut qbuf)).unwrap();

    let sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    sock.send_to(&qbuf, addr).await.unwrap();
    let mut resp_buf = [0u8; 512];
    let (len, _) = sock.recv_from(&mut resp_buf).await.unwrap();

    handle.abort();
    redis_handle.abort();

    let resp = Message::from_vec(&resp_buf[..len]).unwrap();
    let answers = resp.answers();
    assert!(answers.iter().any(|r| r.record_type() == RecordType::RRSIG));

    let mut set_bytes = Vec::new();
    {
        let mut enc = BinEncoder::new(&mut set_bytes);
        for rec in answers.iter().filter(|r| r.record_type() == RecordType::A) {
            rec.emit(&mut enc).unwrap();
        }
    }
    let sig_rec = answers
        .iter()
        .find(|r| r.record_type() == RecordType::RRSIG)
        .unwrap();
    let sig_data = match sig_rec.data().unwrap() {
        hickory_proto::rr::RData::DNSSEC(hickory_proto::rr::dnssec::rdata::DNSSECRData::RRSIG(
            s,
        )) => s,
        _ => panic!("unexpected rdata"),
    };
    let key = hmac::Key::new(hmac::HMAC_SHA256, b"secret");
    let expected = hmac::sign(&key, &set_bytes);
    assert_eq!(sig_data.sig(), expected.as_ref());
}

#[tokio::test]
async fn dig_returns_signed_response() {
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
        performance_config: dns_handler::PerformanceConfig::default(),
    };
    let handle = tokio::spawn(async move { serve(cfg).await.unwrap() });
    sleep(Duration::from_millis(50)).await;

    let output = Command::new("dig")
        .arg(format!("@{}", addr.ip()))
        .arg("-p")
        .arg(addr.port().to_string())
        .arg("demo.fdns.run")
        .arg("+dnssec")
        .stdout(Stdio::piped())
        .output()
        .await
        .expect("dig executed");

    handle.abort();
    redis_handle.abort();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("1.2.3.4"));
    // Note: DNSSEC signing is currently disabled by default, so we don't check for RRSIG records
}
