//! Debug-mode reproduction of the reverse-tunnel connectivity bug.
//!
//! Session: c6eef8. This test drives a real in-process `SshServer` with a real
//! russh client over a loopback TCP connection, exercising BOTH:
//!
//!   1. the WRONG primitive the CLI currently uses
//!      (`channel_open_direct_tcpip` — SSH `-L` / local forwarding)
//!   2. the CORRECT primitive needed for reverse tunnels
//!      (`tcpip_forward` global request — SSH `-R` / remote forwarding)
//!
//! It writes NDJSON evidence to the debug session log so each hypothesis can
//! be confirmed or rejected from runtime behaviour alone.

use std::io::Write as _;
use std::sync::Arc;
use std::time::Duration;

use edgehub::ssh_server::{SshConfig, SshServer};
use russh::client::{Config as ClientCfg, Handle, Handler};
use russh::keys::{Algorithm, PrivateKey, PrivateKeyWithHashAlg, PublicKey};
use tokio::net::TcpListener;

const LOG_PATH: &str =
    "/Users/casibbald/Workspace/microscaler/cylon-local-infra/.cursor/debug-c6eef8.log";

// #region agent log
fn log_evt(hypothesis: &str, location: &str, message: &str, data: serde_json::Value) {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    let line = serde_json::json!({
        "sessionId": "c6eef8",
        "runId": std::env::var("DEBUG_RUN_ID").unwrap_or_else(|_| "pre-fix".to_string()),
        "hypothesisId": hypothesis,
        "location": location,
        "message": message,
        "data": data,
        "timestamp": ts,
    });
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_PATH)
    {
        let _ = writeln!(f, "{}", line);
    }
}
// #endregion

#[derive(Clone, Debug)]
struct AcceptAllClient;

impl Handler for AcceptAllClient {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

async fn spawn_server() -> u16 {
    // Bind an ephemeral port first so we know what port the SSH server is on.
    let probe = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = probe.local_addr().unwrap();
    drop(probe);

    let cfg = SshConfig {
        bind_addr: addr,
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

    let server = SshServer::new(cfg).await.expect("SshServer::new");

    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
    // keep tx alive for the lifetime of the test
    Box::leak(Box::new(shutdown_tx));

    tokio::spawn(async move {
        let _ = server.run(shutdown_rx).await;
    });

    // give the listener a moment to bind
    tokio::time::sleep(Duration::from_millis(200)).await;
    addr.port()
}

async fn connect_and_auth(server_port: u16) -> Handle<AcceptAllClient> {
    let cfg = ClientCfg {
        inactivity_timeout: Some(Duration::from_secs(30)),
        ..Default::default()
    };
    let mut handle =
        russh::client::connect(Arc::new(cfg), ("127.0.0.1", server_port), AcceptAllClient)
            .await
            .expect("russh connect");

    let kp = PrivateKey::random(&mut rand_key::rng(), Algorithm::Ed25519).unwrap();
    let authed = handle
        .authenticate_publickey(
            "tunnel-user",
            PrivateKeyWithHashAlg::new(Arc::new(kp), None),
        )
        .await
        .expect("auth");
    assert!(authed.success(), "SSH server accepts Phase-0 ed25519 key");
    handle
}

#[tokio::test]
async fn reproduce_reverse_tunnel_bug() {
    log_evt(
        "setup",
        "debug_reverse_tunnel.rs",
        "test start",
        serde_json::json!({}),
    );

    let server_port = spawn_server().await;
    log_evt(
        "setup",
        "debug_reverse_tunnel.rs:spawn_server",
        "SshServer listening on loopback",
        serde_json::json!({ "server_port": server_port }),
    );

    let handle = connect_and_auth(server_port).await;

    // ---- Attempt #1: what the CLI does today (WRONG primitive) ----
    let allocated_port: u16 = 41234;
    log_evt(
        "H1",
        "debug_reverse_tunnel.rs",
        "attempting channel_open_direct_tcpip (ssh -L); this is what the CLI does today",
        serde_json::json!({ "allocated_port": allocated_port }),
    );
    let chan_result = handle
        .channel_open_direct_tcpip(
            "127.0.0.1".to_string(),
            allocated_port as u32,
            "127.0.0.1".to_string(),
            0,
        )
        .await;
    log_evt(
        "H1",
        "debug_reverse_tunnel.rs",
        "direct-tcpip channel_open returned",
        serde_json::json!({ "ok": chan_result.is_ok() }),
    );

    // give server handler time to run
    tokio::time::sleep(Duration::from_millis(300)).await;

    // H2 probe: if the server had started a TCP listener on allocated_port,
    // this bind would fail with AddrInUse. If bind SUCCEEDS, the server never
    // listened on it — confirming H2.
    let bind_probe = TcpListener::bind(("0.0.0.0", allocated_port)).await;
    let bind_ok = bind_probe.is_ok();
    log_evt(
        "H2",
        "debug_reverse_tunnel.rs:bind_probe",
        "if bind succeeds on allocated_port, server never bound a listener on it (confirms H2)",
        serde_json::json!({
            "allocated_port": allocated_port,
            "test_bind_succeeded": bind_ok,
            "interpretation": if bind_ok { "H2 CONFIRMED: no server listener" } else { "H2 REJECTED: server is listening" }
        }),
    );
    drop(bind_probe);

    // ---- Attempt #2: the CORRECT primitive (ssh -R) ----
    // russh's client Handle has `tcpip_forward` — this sends the `tcpip-forward`
    // global request. The server-side default `Handler::tcpip_forward` returns
    // `false`. We've temporarily overridden it to log and return false so we
    // can observe whether the current CLI code path would even request this.
    log_evt(
        "H1",
        "debug_reverse_tunnel.rs",
        "attempting tcpip_forward (ssh -R); this is what a reverse tunnel REQUIRES",
        serde_json::json!({ "allocated_port": allocated_port }),
    );
    let forward_accepted = handle.tcpip_forward("0.0.0.0", allocated_port as u32).await;
    log_evt(
        "H1",
        "debug_reverse_tunnel.rs",
        "tcpip_forward global request returned",
        serde_json::json!({
            "ok": forward_accepted.is_ok(),
            "accepted": forward_accepted.as_ref().ok().copied(),
            "note": "server's current Handler only logs and returns false; no binding happens",
        }),
    );

    // Give instrumentation a moment to flush.
    tokio::time::sleep(Duration::from_millis(200)).await;

    log_evt(
        "done",
        "debug_reverse_tunnel.rs",
        "test end",
        serde_json::json!({}),
    );
}
