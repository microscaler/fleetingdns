//! TDP-13: the hub authenticates SSH connections against the key the control
//! plane issued for the session. With Redis auth enabled and accept-all OFF,
//! the API-issued key must be accepted and any other key rejected.

use std::sync::Arc;
use std::time::Duration;

use edgehub::ssh_server::{SshConfig, SshServer};
use russh::client::{Config as ClientCfg, Handler};
use russh::keys::{Algorithm, PrivateKey, PrivateKeyWithHashAlg};
use testcontainers::{ImageExt, runners::AsyncRunner};
use testcontainers_modules::redis::Redis;
use tokio::net::TcpListener;

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

async fn connect(port: u16) -> russh::client::Handle<NoopClientHandler> {
    russh::client::connect(
        Arc::new(ClientCfg::default()),
        ("127.0.0.1", port),
        NoopClientHandler,
    )
    .await
    .expect("connect")
}

#[tokio::test]
async fn hub_accepts_issued_key_and_rejects_unknown_key() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .try_init();

    // Real Redis with an SSH session record for the issued key (as the API
    // would write via store_ssh_session).
    let container = Redis::default()
        .with_tag("7.2-alpine")
        .start()
        .await
        .expect("start redis");
    let redis_port = container.get_host_port_ipv4(6379).await.expect("port");
    let redis_url = format!("redis://localhost:{redis_port}");

    let session_id = "auth-test-1";
    let issued = common::ssh_keys::generate_ed25519_keypair().expect("keygen");

    let auth = common::redis::RedisAuthHandler::new(&redis_url, "session")
        .await
        .expect("redis auth handler");
    auth.add_authorized_key(&common::redis::SessionData {
        github_user_id: "u1".into(),
        public_key: issued.public_key_openssh.clone(),
        fingerprint: issued.fingerprint.clone(),
        expires_at: chrono::Utc::now() + chrono::Duration::seconds(600),
        session_id: session_id.into(),
    })
    .await
    .expect("store session");

    // Hub with Redis auth ENABLED and accept-all OFF (fail-closed).
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
        redis_auth_enabled: true,
        redis_key_prefix: "session".into(),
        insecure_accept_all_keys: false,
    };
    let ssh_server = SshServer::new(ssh_config).await.expect("SshServer::new");
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
    Box::leak(Box::new(shutdown_tx));
    tokio::spawn(async move {
        let _ = ssh_server.run(shutdown_rx).await;
    });
    tokio::time::sleep(Duration::from_millis(300)).await;

    let user = format!("tunnel-{session_id}");

    // 1) The issued key authenticates successfully.
    let issued_key = russh::keys::decode_secret_key(&issued.private_key_openssh, None)
        .expect("decode issued key");
    let mut ok_handle = connect(server_addr.port()).await;
    let ok = ok_handle
        .authenticate_publickey(
            &user,
            PrivateKeyWithHashAlg::new(Arc::new(issued_key), None),
        )
        .await
        .expect("auth call");
    assert!(ok.success(), "hub must accept the API-issued key");

    // 2) A different (unknown) key is rejected.
    let unknown = PrivateKey::random(&mut rand_key::rng(), Algorithm::Ed25519).unwrap();
    let mut bad_handle = connect(server_addr.port()).await;
    let bad = bad_handle
        .authenticate_publickey(&user, PrivateKeyWithHashAlg::new(Arc::new(unknown), None))
        .await
        .expect("auth call");
    assert!(!bad.success(), "hub must reject a key that was not issued");
}
