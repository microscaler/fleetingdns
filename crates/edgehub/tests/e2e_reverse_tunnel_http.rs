//! E2E test for reverse tunnel HTTP forwarding (R9 acceptance test)
//!
//! This test verifies that the reverse tunnel correctly forwards HTTP requests
//! from external clients through the SSH tunnel to a local service.
//!
//! This is the test that would have caught the original bug in PR #52.

use std::sync::Arc;
use std::time::Duration;

use edgehub::ssh_server::{SshConfig, SshServer};
use russh::client::{Config as ClientCfg, Handler};
use russh::keys::{Algorithm, PrivateKey, PrivateKeyWithHashAlg};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Clone, Debug)]
struct TestClientHandler {
    local_port: u16,
}

impl Handler for TestClientHandler {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }

    // Handle forwarded-tcpip channel opened by server (for reverse tunnel).
    //
    // CRITICAL: this Handler callback runs on the russh session event loop.
    // The bidirectional copy MUST be spawned onto its own task — awaiting it
    // inline blocks the session's message pump, so the channel data the copy
    // waits for can never arrive (deadlock).
    async fn server_channel_open_forwarded_tcpip(
        &mut self,
        channel: russh::Channel<russh::client::Msg>,
        connected_address: &str,
        connected_port: u32,
        originator_address: &str,
        originator_port: u32,
        _session: &mut russh::client::Session,
    ) -> Result<(), Self::Error> {
        tracing::info!(
            "Received forwarded-tcpip channel: connected={}:{}, originator={}:{}",
            connected_address,
            connected_port,
            originator_address,
            originator_port
        );

        let local_port = self.local_port;
        tokio::spawn(async move {
            let mut ssh_stream = channel.into_stream();

            let local_addr = format!("127.0.0.1:{}", local_port);
            let mut local_stream = match TcpStream::connect(&local_addr).await {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("Failed to connect to local service {}: {}", local_addr, e);
                    return;
                }
            };

            tracing::info!("Connected to local service at {}", local_addr);

            match tokio::io::copy_bidirectional(&mut ssh_stream, &mut local_stream).await {
                Ok(_) => tracing::info!("Tunnel data forwarding completed successfully"),
                Err(e) => tracing::error!("Tunnel data forwarding error: {}", e),
            }
        });

        Ok(())
    }
}

#[tokio::test]
async fn test_e2e_reverse_tunnel_http_forwarding() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    tracing::info!("Starting E2E reverse tunnel test");

    // Step 1: Start a fake local HTTP server (the "developer's local app")
    let local_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_port = local_listener.local_addr().unwrap().port();
    tracing::info!("Local server listening on port {}", local_port);

    let local_server_handle = tokio::spawn(async move {
        loop {
            match local_listener.accept().await {
                Ok((mut stream, _peer)) => {
                    tracing::info!("Local server: accepted connection");

                    // Read HTTP request
                    let mut buffer = [0u8; 4096];
                    if let Ok(n) = stream.read(&mut buffer).await {
                        let request = String::from_utf8_lossy(&buffer[..n]);
                        tracing::info!("Local server received request:\n{}", request);

                        // Send a simple HTTP response
                        let response = "HTTP/1.1 200 OK\r\n\
                                       Content-Type: text/plain\r\n\
                                       Content-Length: 25\r\n\
                                       Connection: close\r\n\
                                       \r\n\
                                       Hello from local server!";
                        let _ = stream.write_all(response.as_bytes()).await;
                        tracing::info!("Local server sent response");
                    }
                }
                Err(e) => {
                    tracing::error!("Local server accept error: {}", e);
                    break;
                }
            }
        }
    });

    // Step 2: Start the EdgeHub SSH server
    let probe = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = probe.local_addr().unwrap();
    drop(probe);

    tracing::info!("SSH server listening on {}", server_addr);

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
    Box::leak(Box::new(shutdown_tx)); // Keep alive for test duration

    let ssh_server_handle = tokio::spawn(async move {
        let _ = ssh_server.run(shutdown_rx).await;
    });

    // Give the server time to bind
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Step 3: Connect the SSH client and request reverse tunnel
    let client_cfg = ClientCfg {
        inactivity_timeout: Some(Duration::from_secs(30)),
        ..Default::default()
    };

    let mut handle = russh::client::connect(
        Arc::new(client_cfg),
        ("127.0.0.1", server_addr.port()),
        TestClientHandler { local_port },
    )
    .await
    .expect("russh connect");

    // Authenticate
    let kp = PrivateKey::random(&mut rand_key::rng(), Algorithm::Ed25519).unwrap();
    let authed = handle
        .authenticate_publickey(
            "tunnel-user",
            PrivateKeyWithHashAlg::new(Arc::new(kp), None),
        )
        .await
        .expect("auth");
    assert!(authed.success(), "SSH server accepts Ed25519 key");

    tracing::info!("SSH authentication successful");

    // Request reverse tunnel on a specific port
    let tunnel_port: u16 = 41234;
    tracing::info!("Requesting tcpip-forward for port {}", tunnel_port);

    match handle.tcpip_forward("0.0.0.0", tunnel_port as u32).await {
        Ok(bound) => {
            tracing::info!(bound, "tcpip-forward accepted by EdgeHub");
        }
        Err(e) => {
            panic!("Failed to send tcpip-forward request: {}", e);
        }
    }

    // Step 4: Wait for the tunnel port to be ready, then make an HTTP request through the tunnel
    tracing::info!("Waiting for tunnel port {} to be ready", tunnel_port);

    let mut client_stream = None;
    for attempt in 0..20 {
        if let Ok(stream) = TcpStream::connect(("127.0.0.1", tunnel_port)).await {
            tracing::debug!(attempt, "Connected to tunnel port");
            client_stream = Some(stream);
            break;
        }
        tracing::debug!(attempt, "Tunnel port not ready yet, retrying...");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let mut client_stream = client_stream.expect("Tunnel port was not ready after retries");

    // Send HTTP GET request
    let request = "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
    client_stream
        .write_all(request.as_bytes())
        .await
        .expect("Failed to send HTTP request");

    // Read response
    let mut buffer = [0u8; 4096];
    let n = client_stream
        .read(&mut buffer)
        .await
        .expect("Failed to read response");

    let response = String::from_utf8_lossy(&buffer[..n]);
    tracing::info!("Received response:\n{}", response);

    // Step 5: Verify the response
    assert!(
        response.contains("200 OK"),
        "Response should contain '200 OK', got: {}",
        response
    );
    assert!(
        response.contains("Hello from local server!"),
        "Response should contain 'Hello from local server!', got: {}",
        response
    );

    tracing::info!("E2E reverse tunnel test PASSED");

    // Cleanup: both tasks loop forever — abort them, don't await them.
    local_server_handle.abort();
    ssh_server_handle.abort();
}
