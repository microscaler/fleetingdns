### edgehub Crate Tasks - Reverse Tunnel Prototype

| # | Title | Path / crate | Detailed description & acceptance criteria |
|---|-------|--------------|--------------------------------------------|
| - [ ] **T-26a** | Embed Rust SSH server in EdgeHub | `crates/edgehub/src/ssh.rs` | Integrate **`russh`**. Accept client mTLS cert, implement `server::Server`; on `tcpip-forward` allocate a listener port and signal success.<br>**AC**: Running `ssh -p2222 -R 8080:localhost:80 user@edgehub` prints the allocated port and logs `tunnel open`. |
| - [ ] **T-26b** | TCP proxy to developer channel | `edgehub::proxy` | For each allocated port, connect back to the developer via `Channel` and relay traffic with `tokio::io::copy_bidirectional`.<br>**AC**: Netcat echo on dev machine, curl through EdgeHub returns payload. |
| - [x] **T-26c** | Redis lifecycle hooks | `edgehub::redis` | Write `SETEX slot VALUE ttl` when a tunnel opens and `DEL` on close.<br>**AC**: Integration test using a Redis mock verifies key presence only during active tunnel. |
