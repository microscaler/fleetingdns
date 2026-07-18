### Why T-28 fails now

* **EdgeHub** currently just terminates the TLS “wrapper”, validates the client cert, and writes
  `slot → 127.0.0.1:port` into Redis.
* It does **not** run an SSH server nor honour `tcpip-forward` / reverse-tunnel requests, so no bytes
  flow from `curl` → dnsd → edgehub → developer listener.
* Therefore `crates/edgehub/tests/e2e_tunnel.rs` cannot assert end-to-end traffic.

---

## What needs to be built next

| Missing piece                                                      | Proposed tech                                                      | Minimal scope                                                                                                                        |
| ------------------------------------------------------------------ | ------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------ |
| **SSH server that accepts reverse-port-forward (`tcpip-forward`)** | `russh` (pure-Rust fork of thrussh) or `openssh-sftp-server` crate | *auth* with our 30-min cert, *channel* type `direct-tcpip`, honour `tcpip-forward`, allocate random port.                            |
| **TCP proxy loop** (edge side)                                     | plain `tokio::net::TcpStream`                                      | Accept connections on allocated port, proxy to developer’s SSH channel.                                                              |
| **Lifecycle hooks**                                                | Redis pool                                                         | `SETEX slot -> "127.0.0.1:<port>" 1800` on open; `DEL` on close.                                                                     |
| **E2E test harness**                                               | integration test spawns echo listener, `ssh -R`, curl              | Use `Command` to start edgehub, dnsd, spawn `ssh -R 7000:127.0.0.1:8080` (from local `openssh` binary) against edgehub TLS endpoint. |

---

## New task breakdown (replace T-26 / T-28)

| ID                   | Title                                              | Path / crate                                             | Details & AC                                                                                                                                                                                                                                                                                                                                                                                            |
| -------------------- | -------------------------------------------------- | -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **T-26a**            | Embed Rust SSH server in EdgeHub                   | `crates/edgehub/src/ssh.rs`                              | *Desc*  Integrate **`russh`**; accept mTLS cert (no password), implement `server::Server` trait; on `tcpip-forward` allocate local listener port, return success.  \n*AC*  `ssh -p2222 -R 8080:localhost:80 user@edgehub` prints “Allocated port …” and EdgeHub logs “tunnel open”.                                                                                                                     |
| **T-26b**            | TCP proxy to developer channel                     | `edgehub::proxy`                                         | For each accepted connection on allocated port, open `Channel` to client (`channel_open_session` + `open_direct_tcpip`) and pipe data (`tokio::io::copy_bidirectional`).  \n*AC*  Netcat echo on dev box → curl via EdgeHub returns payload.                                                                                                                                                            |
| **T-26c**            | Redis lifecycle hooks                              | `edgehub::redis`                                         | Write `SETEX slot VALUE ttl` on open, `DEL` on close.  \n*AC*  Integration unit test mocks Redis, asserts key exists while tunnel active.                                                                                                                                                                                                                                                               |
| **T-28 (re-scoped)** | **e2e\_tunnel.rs** full flow                       | `crates/edgehub/tests/e2e_tunnel.rs`                     | Start dnsd (UDP+DoT), edgehub (TLS+SSH). Use helper `slot-setter` OR rely on EdgeHub’s Redis write. Spawn `python -m http.server 8000`; run `ssh -i dev_key -R 8080:localhost:8000 certuser@127.0.0.1 -p2222 -oStrictHostKeyChecking=no -oUserKnownHostsFile=/dev/null -F none`. Curl `http://demo.<slot>.fdns.run:8080` → returns HTML index.  \n*AC*  Test passes locally and in CI (docker-compose). |
| **T-26d**            | Docker, Compose & Tilt updates                     | `docker/docker-compose.yml`, `tilt/k8s/app/edgehub.yaml` | Expose port 2222 TCP, mount certs, set `EDGE_TLS_CERT`, `EDGE_TLS_KEY`.                                                                                                                                                                                                                                                                                                                                 |
| **T-26e**            | Grafana dashboard panel: `edge_tunnels_open` gauge | `docker/grafana/dashboards/fdns-unified.json`            | Increment metric on `tcpip-forward` open/close.                                                                                                                                                                                                                                                                                                                                                         |

---

### Implementation tips

1. **rustls + russh** – russh lets you supply your own socket (already TLS-wrapped). After TLS handshake succeed, pass the decrypted TcpStream into russh’s `run_server()`.
2. **Resource leaks** – keep a `HashMap<Port, (TaskHandle, slot)>`; drop handle closes tunnel and triggers Redis cleanup.
3. **CI stability** – run SSH client inside container (`panubo/sshd`) instead of host binary to avoid OpenSSH version issues on GitHub runners.
Once these tasks merge, **T-29 metrics → T-31 CI DoT/Redis smoke** will pass with real traffic, unlocking the ML-pipeline tasks (T-39 +).
