//! FR-HUB-2 acceptance tests: slot listeners MUST be torn down — no zombies.
//!
//! 1. `cancel_tcpip_forward` closes the slot listener while the session lives.
//! 2. SSH disconnect (session end) closes every slot listener the session bound.
//!
//! Written before the implementation (TDD): with the pre-FR-HUB-2 hub these
//! fail because the accept-loop task is spawned and its JoinHandle leaked,
//! so the listener outlives both the forward request and the session.

use std::sync::Arc;
use std::time::Duration;

use edgehub::ssh_server::{SshConfig, SshServer};
use russh::client::{Config as ClientCfg, Handler};
use russh::keys::{Algorithm, PrivateKey, PrivateKeyWithHashAlg};
use tokio::net::TcpStream;

#[derive(Clone, Debug)]
struct NoopClientHandler;

impl Handler for NoopClientHandler {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

/// Spin up an SshServer on an ephemeral port; returns its address.
async fn start_hub() -> std::net::SocketAddr {
    let probe = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
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
        insecure_accept_all_keys: true,
    };

    let ssh_server = SshServer::new(ssh_config).await.expect("SshServer::new");

    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
    Box::leak(Box::new(shutdown_tx)); // keep alive for test duration
    tokio::spawn(async move {
        let _ = ssh_server.run(shutdown_rx).await;
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    server_addr
}

async fn connect_client(
    server_addr: std::net::SocketAddr,
) -> russh::client::Handle<NoopClientHandler> {
    let client_cfg = ClientCfg {
        inactivity_timeout: Some(Duration::from_secs(30)),
        ..Default::default()
    };
    let mut handle = russh::client::connect(
        Arc::new(client_cfg),
        ("127.0.0.1", server_addr.port()),
        NoopClientHandler,
    )
    .await
    .expect("russh connect");

    let kp = PrivateKey::random(&mut rand_key::rng(), Algorithm::Ed25519).unwrap();
    let authed = handle
        .authenticate_publickey(
            "teardown-test",
            PrivateKeyWithHashAlg::new(Arc::new(kp), None),
        )
        .await
        .expect("auth");
    assert!(authed.success());
    handle
}

/// Poll until connecting to `port` succeeds (listener up) or the deadline hits.
///
/// Needed because russh 0.40's client `tcpip_forward` is fire-and-forget for
/// a non-zero port (it only awaits the server reply when port == 0), so the
/// forward request races the server-side bind.
async fn wait_for_listener_up(port: u16) -> bool {
    for _ in 0..50 {
        if TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}

/// Poll until connecting to `port` fails (listener gone) or the deadline hits.
async fn wait_for_listener_gone(port: u16) -> bool {
    for _ in 0..50 {
        if TcpStream::connect(("127.0.0.1", port)).await.is_err() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}

#[tokio::test]
async fn cancel_tcpip_forward_tears_down_slot_listener() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .try_init();
    let server_addr = start_hub().await;
    let handle = connect_client(server_addr).await;

    let slot: u16 = 43511;
    handle
        .tcpip_forward("127.0.0.1", slot as u32)
        .await
        .expect("tcpip_forward send");
    assert!(
        wait_for_listener_up(slot).await,
        "slot listener must come up after tcpip_forward"
    );

    // russh 0.40's client cancel_tcpip_forward is fire-and-forget (always
    // returns true); the meaningful assertion is that the listener closes.
    handle
        .cancel_tcpip_forward("127.0.0.1", slot as u32)
        .await
        .expect("cancel_tcpip_forward send");

    assert!(
        wait_for_listener_gone(slot).await,
        "slot listener must be closed after cancel_tcpip_forward"
    );
}

#[tokio::test]
async fn ssh_disconnect_tears_down_slot_listener() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .try_init();
    let server_addr = start_hub().await;
    let handle = connect_client(server_addr).await;

    let slot: u16 = 43522;
    handle
        .tcpip_forward("127.0.0.1", slot as u32)
        .await
        .expect("tcpip_forward send");
    assert!(
        wait_for_listener_up(slot).await,
        "slot listener must come up after tcpip_forward"
    );

    // Graceful SSH disconnect; dropping the handle also severs the TCP link.
    let _ = handle
        .disconnect(russh::Disconnect::ByApplication, "test done", "en")
        .await;
    drop(handle);

    assert!(
        wait_for_listener_gone(slot).await,
        "slot listener must be closed after SSH disconnect (no zombie listeners)"
    );
}
