### edgehub Crate Tasks - Reverse Tunnel Prototype

> **⚠️ CORRECTION (2026-07-17):** the T-26b "✅ COMPLETE" claim below was **false** — the
> `edgehub::proxy` "dynamic reverse proxy" allocated ports nothing listened on and was never
> on the live path (see postmortem D-4). The working data plane shipped later via
> `tcpip_forward` + the SNI router splice. Authoritative stories:
> `docs/engineering/stories_detailed/E2_E3_tunnel_data_plane_user_stories_v0.3.md`
> (TDP-1/TDP-2 done; TDP-10 deletes the dead T-26b code; T-29 absorbed into TDP-15).

| # | Title | Path / crate | Detailed description & acceptance criteria |
|---|-------|--------------|--------------------------------------------|
| - [x] **T-26a** | Embed Rust SSH server in EdgeHub | `crates/edgehub/src/ssh.rs` | Integrate **`russh`**. Accept client mTLS cert, implement `server::Server`; on `tcpip-forward` allocate a listener port and signal success.<br>**AC**: Running `ssh -p2222 -R 8080:localhost:80 user@edgehub` prints the allocated port and logs `tunnel open`. ⚠️ Note: "TLS-wrapped SSH" claim inaccurate — hub accepts plain SSH on :2222 (E2 D-7). |
| - [x] ~~**T-26b**~~ | TCP proxy to developer channel | `edgehub::proxy` | **DEPRECATED (D-4)** — implementation described here is dead code, removed by story TDP-10. The *goal* (curl through EdgeHub returns payload from dev machine) is delivered by TDP-1 (`tcpip_forward` + `forwarded-tcpip` + `copy_bidirectional`) and TDP-2 (SNI router splice). |
| - [x] **T-26c** | Redis lifecycle hooks | `edgehub::redis` | Write `SETEX slot VALUE ttl` when a tunnel opens and `DEL` on close.<br>**AC**: Integration test using a Redis mock verifies key presence only during active tunnel. |
| - [ ] **T-29** | `edge_tunnels_open` gauge | `edgehub::metrics` | *Desc* Maintain a gauge of active SSH tunnels. Increment on `tcpip-forward` open and decrement on close using `metrics::gauge!`.<br><br>*AC* Unit test opens and closes a mock tunnel and asserts the gauge value. |
