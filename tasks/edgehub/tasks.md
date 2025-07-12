### edgehub Crate Tasks - Reverse Tunnel Prototype

| # | Title | Path / crate | Detailed description & acceptance criteria |
|---|-------|--------------|--------------------------------------------|
| - [ ] **T-26a** | Embed Rust SSH server in EdgeHub | `crates/edgehub/src/ssh.rs` | Integrate **`russh`**. Accept client mTLS cert, implement `server::Server`; on `tcpip-forward` allocate a listener port and signal success.<br>**AC**: Running `ssh -p2222 -R 8080:localhost:80 user@edgehub` prints the allocated port and logs `tunnel open`. |
| - [ ] **T-26b** | TCP proxy to developer channel | `edgehub::proxy` | For each allocated port, connect back to the developer via `Channel` and relay traffic with `tokio::io::copy_bidirectional`.<br>**AC**: Netcat echo on dev machine, curl through EdgeHub returns payload. |
| - [ ] **T-26c** | Redis lifecycle hooks | `edgehub::redis` | Write `SETEX slot VALUE ttl` when a tunnel opens and `DEL` on close.<br>**AC**: Integration test using a Redis mock verifies key presence only during active tunnel. |
| - [ ] **T-28** | **e2e_tunnel.rs** full flow | `crates/edgehub/tests/e2e_tunnel.rs` | Start dnsd (UDP+DoT) and edgehub (TLS+SSH). Map slot via `slot-setter` or EdgeHub’s Redis write. Spawn `python -m http.server 8000`; run `ssh -i dev_key -R 8080:localhost:8000 certuser@127.0.0.1 -p2222 -oStrictHostKeyChecking=no -oUserKnownHostsFile=/dev/null -F none`. Curl `http://demo.<slot>.fdns.run:8080` returns HTML index.<br>**AC**: Test passes locally and in CI. |
