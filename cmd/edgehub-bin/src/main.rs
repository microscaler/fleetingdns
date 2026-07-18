use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::LazyConfigAcceptor;
use tracing::{error, info, warn};

use common::{AppResult, init_metrics, init_tracing, shutdown::GracefulShutdown};
use edgehub::{self, SshConfig, SshServer};

/// Derive the tunnel subdomain from a TLS SNI hostname.
///
/// The SNI is the wildcard-record host, e.g. `abc123.fleetingdns.run` or
/// `<agent-uuid>.tilt.tiffany.microscaler.io`; the tunnel key is the
/// left-most label (`abc123` / `<agent-uuid>`). SNI carries a hostname
/// only (no port), unlike the HTTP `Host` header.
fn subdomain_from_sni(sni: &str) -> &str {
    sni.split('.').next().unwrap_or(sni)
}

/// Upper bound on the first request's header block for protected tunnels.
const MAX_HEADER_BYTES: usize = 16 * 1024;

/// How long we wait for the client to send its first request headers on a
/// protected tunnel before giving up.
const HEADER_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// Read from `stream` until the end of the first HTTP request's header
/// block (`\r\n\r\n`), appending everything read into `buf` so it can be
/// replayed to the slot after authorization. Bounded by
/// [`MAX_HEADER_BYTES`]; the caller wraps this in a timeout.
async fn read_request_head<S>(stream: &mut S, buf: &mut Vec<u8>) -> std::io::Result<()>
where
    S: tokio::io::AsyncRead + Unpin,
{
    let mut chunk = [0u8; 2048];
    loop {
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            return Ok(());
        }
        if buf.len() > MAX_HEADER_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "request headers exceed limit",
            ));
        }
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "connection closed before request headers completed",
            ));
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

/// Extract a cookie value by name from a raw HTTP request head.
fn cookie_from_request_head(head: &str, name: &str) -> Option<String> {
    for line in head.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if !key.trim().eq_ignore_ascii_case("cookie") {
            continue;
        }
        for pair in value.split(';') {
            if let Some((ck, cv)) = pair.trim().split_once('=')
                && ck.trim() == name
            {
                return Some(cv.trim().to_string());
            }
        }
    }
    None
}

/// Write a minimal HTTP error response over the (already-terminated) TLS
/// stream. This is the ONLY place the edge originates HTTP bytes — the
/// forward path is a raw byte splice (FR-EDGE-5).
async fn write_http_status<S>(stream: &mut S, status_line: &str, body: &str)
where
    S: tokio::io::AsyncWrite + Unpin,
{
    let resp = format!(
        "HTTP/1.1 {status_line}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(resp.as_bytes()).await;
    let _ = stream.shutdown().await;
}

/// Handle one inbound HTTPS connection: sniff SNI from the TLS ClientHello
/// (FR-EDGE-2 — before completing the handshake, never HTTP Host sniffing),
/// look up the slot in Redis, and raw-splice bytes to the hub slot listener.
async fn handle_router_connection(
    tcp: TcpStream,
    peer: SocketAddr,
    tls_config: Arc<rustls::ServerConfig>,
    pool: common::redis::RedisPool,
) {
    // Peek the ClientHello without completing the handshake.
    let start = match LazyConfigAcceptor::new(rustls::server::Acceptor::default(), tcp).await {
        Ok(s) => s,
        Err(e) => {
            info!(error = %e, peer = %peer, "TLS ClientHello read failed");
            return;
        }
    };

    // Extract SNI from the ClientHello (server_name extension, RFC 6066).
    let sni = start
        .client_hello()
        .server_name()
        .map(std::string::ToString::to_string);

    // Complete the TLS handshake so we can either splice or return a real
    // HTTP error to the client.
    let mut tls = match start.into_stream(tls_config).await {
        Ok(s) => s,
        Err(e) => {
            info!(error = %e, peer = %peer, "TLS handshake failed");
            return;
        }
    };

    let sni = if let Some(s) = sni {
        s
    } else {
        info!(peer = %peer, "no SNI in ClientHello");
        write_http_status(
            &mut tls,
            "400 Bad Request",
            "400 Bad Request: missing SNI\n",
        )
        .await;
        return;
    };

    let subdomain = subdomain_from_sni(&sni).to_string();
    info!(sni = %sni, subdomain = %subdomain, "routing by SNI");

    let tunnel_info = match common::redis::get_tunnel_by_subdomain(&pool, &subdomain).await {
        Ok(Some(info)) => info,
        Ok(None) => {
            info!(sni = %sni, "no tunnel for SNI");
            write_http_status(&mut tls, "404 Not Found", "404 Not Found\n").await;
            return;
        }
        Err(e) => {
            error!(sni = %sni, error = %e, "tunnel lookup failed");
            write_http_status(
                &mut tls,
                "500 Internal Server Error",
                "500 Internal Server Error\n",
            )
            .await;
            return;
        }
    };

    info!(sni = %sni, tunnel_id = %tunnel_info.id, slot = tunnel_info.slot, "routing to tunnel slot");

    // FR-EDGE-3: protected tunnels require a live session grant before any
    // bytes are forwarded. Read the first request's header block off the
    // decrypted stream (bounded + timed), check the grant cookie against
    // Redis, and replay the buffered bytes to the slot only on success.
    // Each fresh TCP connection is verified once, then raw-spliced — the
    // WebSocket upgrade request carries the cookie too, so /ws/view works.
    let mut replay_buf: Vec<u8> = Vec::new();
    if tunnel_info.protected {
        if let Err(e) = tokio::time::timeout(
            HEADER_READ_TIMEOUT,
            read_request_head(&mut tls, &mut replay_buf),
        )
        .await
        .unwrap_or_else(|_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "timed out reading request headers",
            ))
        }) {
            info!(sni = %sni, error = %e, "failed to read request head on protected tunnel");
            write_http_status(&mut tls, "400 Bad Request", "400 Bad Request\n").await;
            return;
        }

        let head = String::from_utf8_lossy(&replay_buf);
        let grant_token = cookie_from_request_head(&head, common::redis::SESSION_COOKIE_NAME);

        let authorized = match grant_token {
            Some(token) => {
                match common::redis::check_session_grant(&pool, &subdomain, &token).await {
                    Ok(ok) => ok,
                    Err(e) => {
                        error!(sni = %sni, error = %e, "session grant lookup failed");
                        write_http_status(
                            &mut tls,
                            "500 Internal Server Error",
                            "500 Internal Server Error\n",
                        )
                        .await;
                        return;
                    }
                }
            }
            None => false,
        };

        if !authorized {
            info!(sni = %sni, "rejecting protected tunnel connection without valid session grant");
            write_http_status(&mut tls, "403 Forbidden", "403 Forbidden\n").await;
            return;
        }
        info!(sni = %sni, "session grant verified");
    }

    // Second hop: dial the hub-side slot listener bound by the SSH server's
    // tcpip_forward handler; it relays through the forwarded-tcpip channel to
    // the CLI, which dials the developer's local service.
    let slot_addr = format!("127.0.0.1:{}", tunnel_info.slot);
    let mut slot_stream = match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        TcpStream::connect(&slot_addr),
    )
    .await
    {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            error!(slot = tunnel_info.slot, error = %e, "failed to connect to hub slot listener");
            write_http_status(&mut tls, "502 Bad Gateway", "502 Bad Gateway\n").await;
            return;
        }
        Err(_) => {
            error!(
                slot = tunnel_info.slot,
                "timeout connecting to hub slot listener"
            );
            write_http_status(&mut tls, "504 Gateway Timeout", "504 Gateway Timeout\n").await;
            return;
        }
    };

    // Replay any bytes consumed during grant verification before splicing.
    if !replay_buf.is_empty()
        && let Err(e) = slot_stream.write_all(&replay_buf).await
    {
        error!(slot = tunnel_info.slot, error = %e, "failed to replay request head to slot");
        write_http_status(&mut tls, "502 Bad Gateway", "502 Bad Gateway\n").await;
        return;
    }

    // Raw byte splice of the decrypted stream (FR-EDGE-4: WebSocket-safe —
    // copy_bidirectional never interprets HTTP request boundaries, so Tilt's
    // /ws/view upgrade passes straight through).
    match tokio::io::copy_bidirectional(&mut tls, &mut slot_stream).await {
        Ok((from_client, from_slot)) => {
            info!(
                subdomain = %tunnel_info.subdomain,
                from_client, from_slot,
                "tunnel connection completed"
            );
        }
        Err(e) => {
            warn!(subdomain = %tunnel_info.subdomain, error = %e, "tunnel splice error");
        }
    }
}

/// HTTPS router: terminates TLS and routes by ClientHello SNI to the hub slot.
async fn serve_https_router(
    addr: SocketAddr,
    tls_config: Arc<rustls::ServerConfig>,
    redis_pool: common::redis::RedisPool,
    mut shutdown_rx: tokio::sync::broadcast::Receiver<common::shutdown::ShutdownSignal>,
) -> AppResult<()> {
    let listener = TcpListener::bind(addr).await?;
    info!(addr = %listener.local_addr()?, "HTTPS router listening");

    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, peer)) => {
                        info!(peer = %peer, "New HTTPS connection");
                        let config = tls_config.clone();
                        let pool = redis_pool.clone();
                        tokio::spawn(handle_router_connection(stream, peer, config, pool));
                    }
                    Err(e) => {
                        info!(error = %e, "Failed to accept connection");
                    }
                }
            }
            _ = shutdown_rx.recv() => {
                info!("HTTPS router received shutdown signal");
                break;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod router_tests {
    use super::subdomain_from_sni;

    #[test]
    fn subdomain_is_leftmost_label() {
        assert_eq!(subdomain_from_sni("abc123.fleetingdns.run"), "abc123");
        assert_eq!(
            subdomain_from_sni("6f3a-agent.tilt.tiffany.microscaler.io"),
            "6f3a-agent"
        );
    }

    #[test]
    fn bare_hostname_returned_as_is() {
        assert_eq!(subdomain_from_sni("localhost"), "localhost");
    }
}

#[cfg(test)]
mod grant_tests {
    use super::{cookie_from_request_head, read_request_head};

    const HEAD: &str = "GET / HTTP/1.1\r\nHost: abc.fleetingdns.run\r\nCookie: theme=dark; fdns_session=tok123; other=1\r\n\r\n";

    #[test]
    fn cookie_extracted_among_others() {
        assert_eq!(
            cookie_from_request_head(HEAD, "fdns_session"),
            Some("tok123".to_string())
        );
    }

    #[test]
    fn missing_cookie_is_none() {
        let head = "GET / HTTP/1.1\r\nHost: x\r\n\r\n";
        assert_eq!(cookie_from_request_head(head, "fdns_session"), None);
        // A cookie header without our cookie
        let head = "GET / HTTP/1.1\r\nCookie: a=b\r\n\r\n";
        assert_eq!(cookie_from_request_head(head, "fdns_session"), None);
    }

    #[test]
    fn cookie_header_name_is_case_insensitive() {
        let head = "GET / HTTP/1.1\r\ncOoKiE: fdns_session=t\r\n\r\n";
        assert_eq!(
            cookie_from_request_head(head, "fdns_session"),
            Some("t".to_string())
        );
    }

    #[tokio::test]
    async fn write_http_status_emits_well_formed_response() {
        let (mut client, server) = tokio::io::duplex(1024);
        let mut server = server;
        super::write_http_status(&mut server, "404 Not Found", "404 Not Found\n").await;
        drop(server);
        let mut buf = Vec::new();
        tokio::io::AsyncReadExt::read_to_end(&mut client, &mut buf)
            .await
            .unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.starts_with("HTTP/1.1 404 Not Found\r\n"));
        assert!(text.contains("Content-Length: 14"));
        assert!(text.contains("Connection: close"));
        assert!(text.ends_with("404 Not Found\n"));
    }

    #[tokio::test]
    async fn read_head_rejects_oversized_headers() {
        // A header block that never terminates and exceeds MAX_HEADER_BYTES.
        let huge = vec![b'a'; super::MAX_HEADER_BYTES + 4096];
        let mut reader = std::io::Cursor::new(huge);
        let mut buf = Vec::new();
        let err = read_request_head(&mut reader, &mut buf).await.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }

    #[tokio::test]
    async fn read_head_stops_at_header_end() {
        let payload = format!("{HEAD}BODYBYTES");
        let mut reader = std::io::Cursor::new(payload.into_bytes());
        let mut buf = Vec::new();
        read_request_head(&mut reader, &mut buf).await.unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("\r\n\r\n"));
        assert!(text.starts_with("GET / HTTP/1.1"));
    }

    #[tokio::test]
    async fn read_head_errors_on_eof_before_headers_end() {
        let mut reader = std::io::Cursor::new(b"GET / HTTP/1.1\r\nHost: x\r\n".to_vec());
        let mut buf = Vec::new();
        let err = read_request_head(&mut reader, &mut buf).await.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof);
    }
}

/// EdgeHub command line arguments.
#[derive(Parser, Debug, Clone)]
struct Args {
    /// Address to bind the SSH reverse-tunnel server. This is the port the
    /// CLI / far-tunneld connects out to and the port exposed by the k8s
    /// Service; default 2222.
    #[arg(long, default_value = "0.0.0.0:2222")]
    ssh_addr: SocketAddr,
    /// Address to bind the public HTTPS router (SNI-based routing → hub slot).
    /// Binds a non-privileged port (8443) so the container can run as non-root;
    /// the k8s Service exposes it publicly on 443 → targetPort 8443.
    #[arg(long, default_value = "0.0.0.0:8443")]
    https_addr: SocketAddr,
    /// Path to SSH host key file.
    #[arg(long)]
    ssh_host_key: Option<String>,
    /// Public domain for tunnel URLs (e.g., fleetingdns.run).
    #[arg(long, default_value = "fleetingdns.run")]
    public_domain: String,
    /// Redis connection URL.
    #[arg(long, env = "REDIS_URL", default_value = "redis://127.0.0.1:6379")]
    redis: String,
    /// PEM cert-chain for the public HTTPS router (FR-EDGE-1). Should be a
    /// wildcard cert for `*.{public_domain}`. When omitted, a self-signed
    /// wildcard cert is generated for dev. Pair with `--tls-key`.
    #[arg(long, env = "TLS_CERT_PATH")]
    tls_cert: Option<PathBuf>,
    /// PEM private key matching `--tls-cert`.
    #[arg(long, env = "TLS_KEY_PATH")]
    tls_key: Option<PathBuf>,
    /// Path to control socket for graceful shutdown
    #[arg(long)]
    control_socket: Option<PathBuf>,
    /// Timeout for graceful shutdown in seconds
    #[arg(long, default_value = "30")]
    shutdown_timeout: u64,
}

async fn run(args: Args) -> AppResult<()> {
    let _ = init_tracing("edgehub-bin");
    init_metrics();

    // Initialize graceful shutdown framework
    let mut shutdown = if let Some(socket_path) = args.control_socket {
        let config = common::shutdown::ShutdownConfig {
            control_socket_path: socket_path,
            component_name: "edgehub".to_string(),
            graceful_timeout: std::time::Duration::from_secs(args.shutdown_timeout),
            ..Default::default()
        };
        GracefulShutdown::with_config(config)?
    } else {
        GracefulShutdown::new("edgehub")?
    };

    // Start shutdown framework
    shutdown.start().await?;

    info!(
        https_addr = %args.https_addr,
        ssh_addr = %args.ssh_addr,
        control_socket = %shutdown.config.control_socket_path.display(),
        "edgehub starting with HTTPS router and SSH reverse-tunnel server"
    );

    // FR-EDGE-1: the router must present a wildcard cert for
    // `*.{public_domain}` so every ephemeral tunnel subdomain validates
    // against one cert (per-subdomain certs would leak tunnel FQDNs to CT
    // logs). Load a real cert from disk when mounted; otherwise generate a
    // self-signed wildcard for dev.
    let https_config = match (&args.tls_cert, &args.tls_key) {
        (Some(cert), Some(key)) => {
            info!(cert = %cert.display(), "loading TLS cert for HTTPS router");
            common::tls::load_tls_config_from_files(cert, key, &["http/1.1", "h2"])?
        }
        (None, None) => {
            info!(public_domain = %args.public_domain, "generating self-signed wildcard TLS cert (dev)");
            common::tls::generate_wildcard_tls_config(&["http/1.1", "h2"], &args.public_domain)?.0
        }
        _ => {
            return Err(common::AppError::Message(
                "--tls-cert and --tls-key must be provided together".to_string(),
            ));
        }
    };
    let pool = common::redis::new_pool(&args.redis)
        .await
        .map_err(|e| common::AppError::Message(e.to_string()))?;

    // Create SSH server with development-friendly defaults
    let ssh_config = SshConfig {
        bind_addr: args.ssh_addr,
        host_key_path: args.ssh_host_key,
        public_domain: args.public_domain,
        ca_config: None, // No CA configuration for development mode
        // CRITICAL-3 ENHANCEMENT: Disable strict certificate validation for development
        require_client_certificates: false,
        certificate_pinning_enabled: false,
        // Lets the hub resolve per-tunnel teardown policy by slot (FR-HUB-2).
        redis_url: Some(args.redis.clone()),
        ..Default::default()
    };
    let ssh_server = SshServer::new(ssh_config)
        .await
        .map_err(|e| common::AppError::Message(e.to_string()))?;

    // Get shutdown signals for both servers
    let https_shutdown_rx = shutdown.subscribe();
    let ssh_shutdown_rx = shutdown.subscribe();

    // The public HTTPS router (443) terminates TLS, sniffs SNI → subdomain,
    // looks up the slot in Redis, and splices to the hub slot listener.
    let https_server = serve_https_router(
        args.https_addr,
        Arc::new(https_config),
        pool,
        https_shutdown_rx,
    );

    // The SSH server (2222) accepts outbound tunnel sessions and binds slot
    // listeners via Handler::tcpip_forward.
    let ssh_server_task = ssh_server.run(ssh_shutdown_rx);

    // Run both servers concurrently
    let (https_result, ssh_result) = tokio::join!(https_server, ssh_server_task);

    // Wait for graceful shutdown to complete
    shutdown.wait_for_shutdown().await?;

    // Check results
    https_result?;
    ssh_result.map_err(|e| common::AppError::Message(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod arg_tests {
    use super::*;

    /// Prohibition #7 (FAR-TILT-TUNNEL-PRD §6.7): no two SocketAddr-typed
    /// clap defaults may collide, or two listeners fight for one port.
    #[test]
    fn socket_addr_defaults_are_disjoint() {
        let args = Args::parse_from(["edgehub-bin"]);
        assert_ne!(
            args.ssh_addr, args.https_addr,
            "ssh_addr and https_addr defaults must not collide"
        );
    }
}

#[tokio::main]
async fn main() -> AppResult<()> {
    // Initialize the crypto provider for Rustls
    rustls::crypto::CryptoProvider::install_default(rustls::crypto::ring::default_provider())
        .expect("Failed to install crypto provider");

    let args = Args::parse();
    run(args).await
}
