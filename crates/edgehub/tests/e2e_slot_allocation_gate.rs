//! Hub slot-allocation gate: when the hub has Redis, `tcpip-forward` is only
//! honoured for ports that belong to an API-allocated tunnel record. Anyone
//! can SSH in during Phase 0 (cert auth is deferred), but they cannot bind
//! arbitrary ports on the hub pod — only slots the control plane handed out.

use std::sync::Arc;
use std::time::Duration;

use edgehub::ssh_server::{SshConfig, SshServer};
use russh::client::{Config as ClientCfg, Handler};
use russh::keys::{Algorithm, PrivateKey, PrivateKeyWithHashAlg};
use testcontainers::{ImageExt, runners::AsyncRunner};
use testcontainers_modules::redis::Redis;
use tokio::net::{TcpListener, TcpStream};

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

const ALLOCATED_SLOT: u16 = 43555;
const UNALLOCATED_SLOT: u16 = 43556;

#[tokio::test]
async fn hub_only_binds_api_allocated_slots() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .try_init();

    // Real Redis with one allocated tunnel record for ALLOCATED_SLOT.
    let container = Redis::default()
        .with_tag("7.2-alpine")
        .start()
        .await
        .expect("start redis");
    let redis_port = container.get_host_port_ipv4(6379).await.expect("port");
    let redis_url = format!("redis://localhost:{redis_port}");

    let pool = common::redis::new_pool(&redis_url).await.expect("pool");
    let expires = (chrono::Utc::now() + chrono::Duration::seconds(600)).to_rfc3339();
    let record = common::redis::tunnel::TunnelInfo {
        id: "gate-test-1".into(),
        github_user_id: "u1".into(),
        github_username: "user1".into(),
        subdomain: "gate-test".into(),
        fqdn: "gate-test.fleetingdns.run".into(),
        local_port: 3000,
        slot: ALLOCATED_SLOT,
        certificate_serial: "none".into(),
        created_at: chrono::Utc::now().to_rfc3339(),
        expires_at: expires,
        status: "active".into(),
        protected: false,
        teardown_policy: common::redis::TeardownPolicy::TtlOnly,
    };
    common::redis::store_tunnel_data(&pool, &record)
        .await
        .expect("store record");
    common::redis::store_user_tunnel_lookup(&pool, "u1", "user1", "gate-test-1")
        .await
        .expect("store lookup");

    // Hub wired to that Redis.
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
        redis_url: Some(redis_url),
        redis_auth_enabled: false,
        redis_key_prefix: "session".into(),
        insecure_accept_all_keys: true,
    };
    let ssh_server = SshServer::new(ssh_config).await.expect("SshServer::new");
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
    Box::leak(Box::new(shutdown_tx));
    tokio::spawn(async move {
        let _ = ssh_server.run(shutdown_rx).await;
    });
    tokio::time::sleep(Duration::from_millis(300)).await;

    let mut handle = russh::client::connect(
        Arc::new(ClientCfg::default()),
        ("127.0.0.1", server_addr.port()),
        NoopClientHandler,
    )
    .await
    .expect("connect");
    let kp = PrivateKey::random(&mut rand_key::rng(), Algorithm::Ed25519).unwrap();
    assert!(
        handle
            .authenticate_publickey("gate", PrivateKeyWithHashAlg::new(Arc::new(kp), None))
            .await
            .expect("auth")
            .success()
    );

    // Allocated slot: listener must come up.
    handle
        .tcpip_forward("127.0.0.1", ALLOCATED_SLOT as u32)
        .await
        .expect("send forward");
    let mut up = false;
    for _ in 0..50 {
        if TcpStream::connect(("127.0.0.1", ALLOCATED_SLOT))
            .await
            .is_ok()
        {
            up = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(up, "API-allocated slot must be bindable");

    // Unallocated slot: the hub must refuse. russh 0.60 surfaces the SSH
    // request-failure reply as an Err (RequestDenied) from `tcpip_forward`
    // (russh 0.40 returned a silent `Ok(false)`); either way the hub-side
    // gate declines to bind the listener.
    let denied = handle
        .tcpip_forward("127.0.0.1", UNALLOCATED_SLOT as u32)
        .await;
    assert!(
        denied.is_err(),
        "slot with no tunnel record must be denied (arbitrary-port squatting); got {denied:?}"
    );

    // Belt-and-suspenders: no listener ever came up for the denied slot.
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert!(
        TcpStream::connect(("127.0.0.1", UNALLOCATED_SLOT))
            .await
            .is_err(),
        "denied slot must NOT be bindable"
    );
}
