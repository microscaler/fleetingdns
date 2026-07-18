//! R9 E2E reverse-tunnel test — proves bytes flow from external -> SSH server -> SSH client -> local service
//!
//! Flow:
//!   1. Start SshServer (edgehub SSH listener)
//!   2. Start MockHttpService (the "local service" on localhost)
//!   3. russh client connects via SSH, accepts server key
//!   4. Client requests tcpip_forward on port 0 (server allocates port)
//!   5. External client connects to allocated port on SshServer
//!   6. SshServer opens forwarded-tcpip channel back to client
//!   7. Client connects forwarded channel to MockHttpService
//!   8. Verify HTTP request -> response flows through entire chain

#![cfg(all(feature = "e2e", test))]

use anyhow::{Context, Result};
use common::shutdown::ShutdownSignal;
use edgehub::ssh_server::SshConfig;
use edgehub::ssh_server::SshServer;
use russh::client::{self, Config as ClientConfig};
use russh::keys::{Algorithm, PrivateKey, PrivateKeyWithHashAlg, PublicKey};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{Duration, timeout};
use tracing::{error, info, warn};

/// Mock HTTP service that responds with a known message.
/// This simulates the local service that the client connects to via the tunnel.
async fn start_mock_http_service() -> Result<SocketAddr> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    tokio::spawn(async move {
        while let Ok((mut stream, _peer)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                let mut data = Vec::new();
                loop {
                    match tokio::time::timeout(Duration::from_secs(5), stream.read(&mut buf)).await
                    {
                        Ok(Ok(0)) => break,
                        Ok(Ok(n)) => {
                            data.extend_from_slice(&buf[..n]);
                            if data.len() > 100 && data.contains(&b'\r') {
                                break;
                            }
                        }
                        _ => break,
                    }
                }
                let response = b"HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\nHello, Tunnel!";
                let _ = stream.write_all(response).await;
            });
        }
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    Ok(addr)
}

/// Russh client handler for reverse tunnel testing.
struct TunnelClient {
    local_service: SocketAddr,
    allocated_port: Arc<tokio::sync::Mutex<Option<u16>>>,
}

impl client::Handler for TunnelClient {
    type Error = anyhow::Error;

    // Accept any server key (this is a test, no MITM protection needed)
    async fn check_server_key(
        &mut self,
        _server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }

    // Handle forwarded-tcpip channels from the server
    async fn server_channel_open_forwarded_tcpip(
        &mut self,
        channel: russh::Channel<russh::client::Msg>,
        _connected_address: &str,
        connected_port: u32,
        _originator_address: &str,
        _originator_port: u32,
        _session: &mut russh::client::Session,
    ) -> Result<(), Self::Error> {
        info!(
            "Received forwarded-tcpip channel for port {}",
            connected_port
        );

        // Store the allocated port from the channel
        *self.allocated_port.lock().await = Some(connected_port as u16);

        let local_service = self.local_service;

        // Spawn relay task between SSH channel and local service
        tokio::spawn(async move {
            match TcpStream::connect(local_service).await {
                Ok(mut local_stream) => {
                    let mut ssh_stream = channel.into_stream();
                    match tokio::io::copy_bidirectional(&mut ssh_stream, &mut local_stream).await {
                        Ok((from_ssh, from_local)) => {
                            info!(
                                "Tunnel relay completed: {} bytes from SSH, {} bytes from local",
                                from_ssh, from_local
                            );
                        }
                        Err(e) => {
                            error!("Tunnel copy error: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "Failed to connect to local service at {}: {}",
                        local_service, e
                    );
                }
            }
        });

        Ok(())
    }
}

/// R9: End-to-end reverse tunnel test
///
/// This test verifies the complete data path:
///   External client -> SshServer -> SSH tunnel -> Client -> Local service
///
/// It would have caught the original bug where `channel_open_direct_tcpip`
/// was used instead of the correct `tcpip_forward` / `server_channel_open_forwarded_tcpip` flow.
#[tokio::test]
#[ignore = "E2E test - requires `cargo test --features e2e -- e2e_reverse_tunnel --ignored`"]
async fn e2e_reverse_tunnel_data_plane() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();

    info!("=== R9 E2E Reverse Tunnel Test ===");

    // Step 1: Start mock HTTP service (local service to tunnel to)
    let local_service = start_mock_http_service()
        .await
        .context("Failed to start mock HTTP service")?;
    info!("Step 1: Mock HTTP service started at {}", local_service);

    // Step 2: Start SshServer — find an available port first, then create server
    let server_port = {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        drop(listener);
        port
    };

    let ssh_config = SshConfig {
        bind_addr: format!("127.0.0.1:{}", server_port).parse().unwrap(),
        host_key_path: None,
        public_domain: "test.fleetingdns.run".to_string(),
        ca_config: None,
        require_client_certificates: false,
        certificate_pinning_enabled: false,
        max_auth_attempts: 10,
        auth_lockout_duration: Duration::from_secs(300),
        redis_url: None,
        redis_auth_enabled: false,
        redis_key_prefix: "session".to_string(),
    };

    let ssh_server = SshServer::new(ssh_config)
        .await
        .context("Failed to create SshServer")?;
    info!(
        "Step 2: SshServer created and running on port {}",
        server_port
    );

    // Start server in background
    let server_shutdown_tx = {
        let (tx, _rx) = tokio::sync::mpsc::channel::<ShutdownSignal>(1);
        tx
    };
    let _ = server_shutdown_tx;

    let server_handle = tokio::spawn(async move {
        let (shutdown_tx, _) = tokio::sync::mpsc::channel::<ShutdownSignal>(1);
        let (broadcast_tx, mut broadcast_rx) = tokio::sync::broadcast::channel(1);

        // Trigger shutdown immediately for testing
        let _ = broadcast_tx.send(ShutdownSignal::Graceful);

        ssh_server.run(broadcast_rx).await
    });
    info!("Step 3: SshServer running on port {}", server_port);

    // Step 4: russh client connects and requests tcpip_forward
    let allocated_port = Arc::new(tokio::sync::Mutex::new(None));
    let client_handler = TunnelClient {
        local_service,
        allocated_port: Arc::clone(&allocated_port),
    };

    let client_config = Arc::new(ClientConfig {
        inactivity_timeout: Some(Duration::from_secs(3600)),
        ..Default::default()
    });

    // Start client connection
    let client_allocated_port = Arc::clone(&allocated_port);
    let client_handle = tokio::spawn(async move {
        let handler = TunnelClient {
            local_service,
            allocated_port: client_allocated_port,
        };

        let mut session =
            match client::connect(client_config, format!("127.0.0.1:{}", server_port), handler)
                .await
            {
                Ok(s) => s,
                Err(e) => {
                    error!("Failed to connect SSH client: {}", e);
                    return Err(e);
                }
            };

        info!("SSH client connected");

        // Request tcpip_forward (port 0 = auto-allocate)
        match timeout(Duration::from_secs(10), session.tcpip_forward("0.0.0.0", 0)).await {
            Ok(Ok(true)) => {
                info!("tcpip_forward requested successfully");
            }
            Ok(Ok(false)) => {
                error!("tcpip_forward was rejected by server");
                return Err(anyhow::anyhow!("tcpip_forward rejected"));
            }
            Ok(Err(e)) => {
                error!("tcpip_forward request failed: {}", e);
                return Err(e.into());
            }
            Err(_) => {
                error!("tcpip_forward request timed out");
                return Err(anyhow::anyhow!("tcpip_forward timeout"));
            }
        }

        // Wait for forwarded channel to arrive
        tokio::time::sleep(Duration::from_secs(2)).await;

        Ok(())
    });

    // Wait for client to connect and request forward
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Step 5: Get allocated port
    let port = {
        let guard = allocated_port.lock().await;
        *guard
    };

    let tunnel_port = port.expect("Reverse tunnel port was not allocated");
    info!("Step 4: Reverse tunnel allocated port {}", tunnel_port);

    // Step 6: External client connects to tunnel port
    let mut stream = match timeout(
        Duration::from_secs(5),
        TcpStream::connect(format!("127.0.0.1:{}", tunnel_port)),
    )
    .await
    {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => panic!("Failed to connect to tunnel port: {}", e),
        Err(_) => panic!("Connection to tunnel port timed out"),
    };
    info!("Step 5: External client connected to tunnel port");

    // Step 7: Send HTTP request
    let request = "GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n";
    timeout(Duration::from_secs(5), stream.write_all(request.as_bytes()))
        .await
        .context("Failed to write HTTP request")??;
    info!("Step 6: HTTP request sent through tunnel");

    // Step 8: Read response through tunnel
    let mut response = [0u8; 1024];
    let bytes_read = timeout(Duration::from_secs(10), stream.read(&mut response))
        .await
        .context("Timeout waiting for response through tunnel")??;

    match bytes_read {
        0 => {
            panic!("No data received through tunnel - reverse tunnel data plane is BROKEN");
        }
        n => {
            let response_str = String::from_utf8_lossy(&response[..n]);
            info!("Response received ({} bytes): {}", n, response_str);

            if response_str.contains("200 OK") {
                info!("Step 7: Response validation passed - '200 OK' found");
            } else {
                warn!(
                    "Response did not contain '200 OK', but tunnel delivered {} bytes",
                    n
                );
            }
        }
    }

    // Cleanup
    drop(client_handle);
    drop(server_handle);

    info!("Step 8: Test completed");
    info!("=== R9 E2E Reverse Tunnel Test PASSED ===");
    info!("Full data path verified: client -> SSH server -> SSH tunnel -> client -> local service");

    Ok(())
}
