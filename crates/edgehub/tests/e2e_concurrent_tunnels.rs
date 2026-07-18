//! Multi-tunnel concurrency + isolation acceptance test.
//!
//! One hub, three independent SSH clients, each reverse-forwarding its own
//! slot to its own local HTTP server with a distinct payload. Proves:
//!
//! 1. Concurrent tunnels coexist on one hub (each with its own endpoint).
//! 2. NO cross-communication: parallel interleaved requests to every slot
//!    always return that slot's payload, never a neighbour's.
//! 3. Teardown isolation: disconnecting one client kills only its slot;
//!    the others keep serving.

use std::sync::Arc;
use std::time::Duration;

use edgehub::ssh_server::{SshConfig, SshServer};
use russh::client::{Config as ClientCfg, Handler};
use russh::keys::{Algorithm, PrivateKey, PrivateKeyWithHashAlg};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Client handler that forwards forwarded-tcpip channels to its local port.
#[derive(Clone, Debug)]
struct ForwardingClientHandler {
    local_port: u16,
}

impl Handler for ForwardingClientHandler {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }

    async fn server_channel_open_forwarded_tcpip(
        &mut self,
        channel: russh::Channel<russh::client::Msg>,
        _connected_address: &str,
        _connected_port: u32,
        _originator_address: &str,
        _originator_port: u32,
        _session: &mut russh::client::Session,
    ) -> Result<(), Self::Error> {
        // Spawn — never await a copy inline in a Handler callback (deadlock).
        let local_port = self.local_port;
        tokio::spawn(async move {
            let mut ssh_stream = channel.into_stream();
            let mut local_stream = match TcpStream::connect(("127.0.0.1", local_port)).await {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("connect to local {local_port} failed: {e}");
                    return;
                }
            };
            let _ = tokio::io::copy_bidirectional(&mut ssh_stream, &mut local_stream).await;
        });
        Ok(())
    }
}

/// Minimal HTTP server that always answers with `payload` and closes.
async fn spawn_payload_server(payload: &'static str) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf).await;
                let body = payload;
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(resp.as_bytes()).await;
                let _ = stream.shutdown().await;
            });
        }
    });
    port
}

async fn start_hub() -> std::net::SocketAddr {
    let probe = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = probe.local_addr().unwrap();
    drop(probe);

    let ssh_config = SshConfig {
        bind_addr: server_addr,
        host_key_path: None,
        public_domain: "fleetingdns.run".into(),
        ca_config: None,
        require_client_certificates: false,
        certificate_pinning_enabled: false,
        max_auth_attempts: 3,
        auth_lockout_duration: Duration::from_secs(60),
        redis_url: None,
        redis_auth_enabled: false,
        redis_key_prefix: "session".into(),
    };
    let ssh_server = SshServer::new(ssh_config).await.expect("SshServer::new");

    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
    Box::leak(Box::new(shutdown_tx));
    tokio::spawn(async move {
        let _ = ssh_server.run(shutdown_rx).await;
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    server_addr
}

/// Open one tunnel: SSH client + tcpip_forward of `slot` → local `port`.
async fn open_tunnel(
    server_addr: std::net::SocketAddr,
    slot: u16,
    local_port: u16,
) -> russh::client::Handle<ForwardingClientHandler> {
    let mut handle = russh::client::connect(
        Arc::new(ClientCfg {
            inactivity_timeout: Some(Duration::from_secs(60)),
            ..Default::default()
        }),
        ("127.0.0.1", server_addr.port()),
        ForwardingClientHandler { local_port },
    )
    .await
    .expect("russh connect");

    let kp = PrivateKey::random(&mut rand_key::rng(), Algorithm::Ed25519).unwrap();
    assert!(
        handle
            .authenticate_publickey(
                "concurrent-test",
                PrivateKeyWithHashAlg::new(Arc::new(kp), None)
            )
            .await
            .expect("auth")
            .success()
    );
    handle
        .tcpip_forward("127.0.0.1", slot as u32)
        .await
        .expect("tcpip_forward send");

    // Wait for the hub-side listener to come up (forward is fire-and-forget).
    for _ in 0..50 {
        if TcpStream::connect(("127.0.0.1", slot)).await.is_ok() {
            return handle;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("slot {slot} listener never came up");
}

/// GET through a hub slot and return the response body.
async fn fetch_via_slot(slot: u16) -> anyhow::Result<String> {
    let mut stream = TcpStream::connect(("127.0.0.1", slot)).await?;
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n")
        .await?;
    let mut response = Vec::new();
    // Read until EOF with an overall timeout.
    tokio::time::timeout(Duration::from_secs(5), stream.read_to_end(&mut response)).await??;
    let text = String::from_utf8_lossy(&response);
    let body = text
        .split("\r\n\r\n")
        .nth(1)
        .unwrap_or("")
        .trim()
        .to_string();
    Ok(body)
}

#[tokio::test]
async fn concurrent_tunnels_are_isolated() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .try_init();

    let server_addr = start_hub().await;

    const SLOTS: [u16; 3] = [43541, 43542, 43543];
    const PAYLOADS: [&str; 3] = ["payload-alpha", "payload-bravo", "payload-charlie"];

    // Stand up three tunnels, each to its own payload server.
    let mut handles = Vec::new();
    for i in 0..3 {
        let local_port = spawn_payload_server(PAYLOADS[i]).await;
        handles.push(open_tunnel(server_addr, SLOTS[i], local_port).await);
    }

    // Fire 10 requests per tunnel, all 30 in parallel, interleaved.
    let mut tasks = Vec::new();
    for i in 0..3 {
        for _ in 0..10 {
            let slot = SLOTS[i];
            let expected = PAYLOADS[i];
            tasks.push(tokio::spawn(async move {
                let body = fetch_via_slot(slot).await.expect("fetch via slot");
                (slot, expected, body)
            }));
        }
    }
    for task in tasks {
        let (slot, expected, body) = task.await.unwrap();
        assert_eq!(
            body, expected,
            "slot {slot} must return ONLY its own tunnel's payload (cross-communication!)"
        );
    }

    // Teardown isolation: drop tunnel 1 (bravo); alpha + charlie must survive.
    let bravo = handles.remove(1);
    let _ = bravo
        .disconnect(russh::Disconnect::ByApplication, "done", "en")
        .await;
    drop(bravo);

    // bravo's slot must close...
    let mut gone = false;
    for _ in 0..50 {
        if TcpStream::connect(("127.0.0.1", SLOTS[1])).await.is_err() {
            gone = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(gone, "disconnected tunnel's slot listener must close");

    // ...while the others still answer correctly.
    assert_eq!(fetch_via_slot(SLOTS[0]).await.unwrap(), PAYLOADS[0]);
    assert_eq!(fetch_via_slot(SLOTS[2]).await.unwrap(), PAYLOADS[2]);
}
