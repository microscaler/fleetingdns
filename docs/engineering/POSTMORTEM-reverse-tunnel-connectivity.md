# Postmortem — FleetingDNS reverse-tunnel connectivity failure


| Field          | Value                                                                                                           |
| -------------- | --------------------------------------------------------------------------------------------------------------- |
| Incident       | End-to-end reverse tunnels never complete; external HTTP requests never reach the developer's local service     |
| Component(s)   | `crates/edgehub` (SSH server, reverse proxy), `cmd/edf-cli` (ssh_client, tunnel), `cmd/edgehub-bin` (bootstrap) |
| Severity       | S1 — core product feature inoperative since Phase 0                                                             |
| First detected | Tunnel-lifecycle integration testing (PR #52–#61)                                                               |
| Status         | Diagnosed. Fix plan below. Runtime reproduction blocked locally by an unrelated toolchain regression (see §7).  |
| Debug session  | `c6eef8` — NDJSON evidence in `.cursor/debug-c6eef8.log`                                                        |


---

## 1. Summary (one paragraph)

The `FleetingDNS` reverse-tunnel feature does not work because both the CLI client and the EdgeHub server use the **wrong SSH channel primitive**. The design calls for SSH remote port forwarding (`ssh -R` / `tcpip-forward` global request + `forwarded-tcpip` channels), where the server binds a public listener on behalf of the client and opens new channels back to the client for each inbound connection. The current implementation uses SSH local port forwarding (`ssh -L` / `direct-tcpip` channel), where the client asks the server to dial **outbound** to a host:port and pipe that to the channel. Every secondary defect — an unbound allocated port, a hard-coded stub HTTP 200 reply, a `ChannelId` with no way to push bytes into an SSH channel, a dropped `Channel<Msg>` on the CLI, a port number fabricated from a UUID instead of read from the API response, and port-collision defaults in `edgehub-bin` — follows from or compounds this single wire-protocol mismatch.

---

## 2. Impact

- The reverse-tunnel data plane has **never worked** end-to-end. SSH handshake and auth succeed; everything after the `channel_open_direct_tcpip` call degrades to either a "Disconnected" error or a hard-coded stub response. External webhooks, OAuth callbacks, and multi-tenant routing — i.e. the entire product thesis — are blocked.
- The observed symptom "SSH handshake OK, tunnel port connection fails" has already forced a `tasks/connectivity.md` write-up but was misdiagnosed as "we just need to start a TCP listener on the allocated port". That fix would still not work, because the listener has no way to talk back into an SSH channel given only a `ChannelId`.
- CI E2E coverage in `crates/edgehub/tests/e2e_tunnel.rs` never exercises an actual inbound webhook through the tunnel; it only verifies SSH handshake + Redis registration. This is why the bug survived ~15 feature PRs.

---

## 3. Timeline (abridged, from git history)

- `ad50259` — comprehensive SSH tunnel API integration (PR #52)
- `57ccca1` — "CRITICAL-2 SSH tunnel bidirectional data forwarding" (PR #54) — introduced `tcp_proxy_task` with `copy_bidirectional`-style intent but wired to the wrong channel type
- `a6fe912` — certificate authority integration (PR #55)
- `b90c3b4` — Redis scalability (PR #57)
- `bf80aad` — authentication via GitHub OAuth (PR #68)
- `61e535e` — "Complete Phase 0 SSH tunnel implementation with reverse port forwarding" (current HEAD on `Enhanced-Tunnel-Health-Monitoring`)
- At no point did a PR add a `tcpip_forward` handler to the server's `russh::server::Handler`, and at no point did the CLI issue a `tcpip-forward` global request. Every PR kept extending the `direct-tcpip` path.

---

## 4. What actually happens at runtime

Reconstructed from `tasks/connectivity.md` (prior live runs) and static code analysis; all citations are NDJSON entries in `.cursor/debug-c6eef8.log`.

1. CLI → API: `POST /v1/tunnels` succeeds. API allocates a slot (observed value `54470`) and writes to Redis.
2. CLI → EdgeHub (SSH on 2222 or 8443 depending on env): TCP, TLS, russh handshake, `auth_publickey` all succeed (Phase-0 accepts any key).
3. CLI, in `ssh_client.rs:210`:
  ```rust
   handle.channel_open_direct_tcpip("127.0.0.1", allocated_port, "127.0.0.1", 0)
  ```
   This is RFC 4254 §7.2 *"direct-tcpip"* — semantics: *"Server, please open a TCP connection to `127.0.0.1:<allocated_port>` and splice it to this channel."*
4. EdgeHub, in `ssh_server.rs:1075` `channel_open_direct_tcpip` handler:
  - logs and accepts the channel
  - calls `handle_reverse_tunnel_registration` which only inserts metadata into `active_tunnels` and spawns an idle `tokio::sleep(30s)` log loop
  - **never** dials `host_to_connect:port_to_connect`, **never** binds a `TcpListener` on `allocated_port`, **never** calls `register_reverse_tunnel`
5. CLI, on the open channel, calls `start_tunnel_data_forwarding(_channel: Channel<Msg>)` which:
  - drops the channel immediately (parameter is `_channel`)
  - performs a single `TcpStream::connect("127.0.0.1:<local_port>")` probe with a 5-second timeout
  - returns `Ok(())` or the timeout error — which the CLI treats as "tunnel established"
6. External caller resolves `<subdomain>.fleetingdns.run` → EdgeHub IP, `POST` to `:443`. EdgeHub `serve_https_router` terminates TLS, parses the `Host:` header (not SNI — `extract_sni_from_tls` always returns `None`), looks up the tunnel in Redis, and returns either a **hard-coded placeholder** body `"Tunnel {id} is active for {sni}\n"` or `404 Not Found`. There is no code path that forwards the HTTP bytes to the developer's tunnel.
7. The developer's laptop sees nothing. Webhook times out.

---

## 5. Root cause

**Wire-protocol mismatch: `direct-tcpip` vs `tcpip-forward` + `forwarded-tcpip`.**

RFC 4254 defines two distinct forwarding primitives:


| Purpose                      | Primitive                                                  | Initiator                                                                              | Data direction                              |
| ---------------------------- | ---------------------------------------------------------- | -------------------------------------------------------------------------------------- | ------------------------------------------- |
| Local forwarding (`ssh -L`)  | `direct-tcpip` channel                                     | client opens channel                                                                   | client → server → target TCP host           |
| Remote forwarding (`ssh -R`) | `tcpip-forward` global request + `forwarded-tcpip` channel | client sends global request; *server* opens the channel on each inbound TCP connection | external → server:port → client → localhost |


FleetingDNS needs the second. The PRD is explicit:

> CLI spawns russh client — opens an SSH **reverse-port** `0.0.0.0:hub_slot` on **hub**.

The implementation is the first. This is not a bug that hides behind complexity; it is a category error at the protocol layer. All other defects are consequences.

---

## 6. Contributing factors

- `**channel_id: ChannelId` without a `Handle` in `start_tunnel_port_listener`.** The only way to push bytes into a russh channel from an external tokio task is with `russh::server::Handle` (from `session.handle()`). The function stored only the opaque `ChannelId`, forcing the author to write a stub HTTP 200 reply directly to the TCP socket. This choice made the code compile but baked the wrong mental model into the API.
- `**register_reverse_tunnel` defined on `SshServer`, not on `SshSession`.** The only place with access to the live SSH session (and therefore the handle needed for reverse forwarding) is the `Handler` impl on `SshSession`. Because the correct wiring function was on the wrong object, it became callable only from unit tests.
- **"Phase 0 / Phase 1 / Phase 2" comments documenting deferred work.** `start_tunnel_data_forwarding`: *"// The actual data forwarding will be implemented in Phase 2"*. These comments marked the defect but were never tied to a blocking task; the branch shipped and the PRD called it "complete".
- **E2E test shape.** `e2e_tunnel.rs` verifies *SSH session + Redis metadata*, not *inbound HTTP delivered to localhost*. The gap in test intent is exactly the gap in product behaviour.
- **Secondary port-allocation anti-pattern (H5).** The CLI fabricates a port client-side (`30000 + uuid_bytes[0] % 35535`) rather than using the slot returned by the API. Even with the right SSH primitive this would have prevented matching to Redis tunnel records.
- `**cmd/edgehub-bin/src/main.rs` defaults (H6).** `--addr` and `--ssh-addr` both default to `0.0.0.0:8443`, guaranteeing one of two `TcpListener::bind` calls fails at startup under defaults. `extract_sni_from_tls` is a hard-coded `None`.
- **Phase-0 "accept any pubkey" auth masks a related defect (auth).** The CLI generates a fresh Ed25519 keypair per run and uses it for SSH auth, ignoring the one issued and tracked by `SshKeyManager`. The server accepts it anyway because Phase-0 accepts anything, so there is no signal that the real CA-signed keypath is broken.
- **Workspace toolchain regression.** Locally, `cargo build -p edgehub` fails on `nightly 2025-06-28` because `generic-array 0.14.7` cannot resolve `crypto_common`/`hmac`/`rfc6979`. This prevented fresh runtime reproduction during this postmortem and is itself a P2 signal about dependency-pinning hygiene (Cargo.lock pins `generic-array = 0.14.7`, which is incompatible with recent nightlies).

---

## 7. Evidence

- `.cursor/debug-c6eef8.log` — 14 NDJSON entries, each mapping a hypothesis (H1…H6+, auth, env) to a source (`static`, `prior-runtime`, `environmental`, `synthesis`).
- `tasks/connectivity.md` — live-run observations captured by the team: *"SSH handshake completes", "EdgeHub doesn't listen on allocated ports", "No TCP listeners on allocated ports (e.g., 54470)", "Connection fails with 'Disconnected' error"*.
- Reproduction harness: `crates/edgehub/tests/debug_reverse_tunnel.rs` (added in this session). Exercises both the current (wrong) primitive and the correct `tcpip_forward` in-process against a real `SshServer`. Runs as `cargo test -p edgehub --test debug_reverse_tunnel -- --nocapture` once the workspace toolchain is unblocked.
- Source-level instrumentation (wrapped in `// #region agent log`) in:
  - `crates/edgehub/src/ssh_server.rs` — `channel_open_direct_tcpip`, `handle_reverse_tunnel_registration`, `start_tunnel_port_listener`, `forward_connection_to_ssh_channel`, and a new debug-only `tcpip_forward` Handler probe.
  - `cmd/edf-cli/src/ssh_client.rs` — `request_reverse_tunnel`, `start_tunnel_data_forwarding`.
  - `cmd/edf-cli/src/tunnel.rs` — fabricated-port site.

---

## 8. Remediation plan (surgical, minimum change set)

Ordered by dependency. Each step has an acceptance test.

**R1 — Unblock build.** Upgrade `generic-array` (or pin `rust-toolchain.toml` to a nightly known to work with the current Cargo.lock; simplest: `nightly-2024-06-01` or bump `generic-array` to `1.x` via a `sha2`/`digest` update). Acceptance: `cargo build --workspace` and `cargo nextest run --workspace` both green.

**R2 — Switch CLI to `tcpip-forward`.** In `cmd/edf-cli/src/ssh_client.rs`, replace `handle.channel_open_direct_tcpip(...)` with `handle.tcpip_forward("0.0.0.0", allocated_port).await?`. Implement `Handler::server_channel_open_forwarded_tcpip` on `SshClientHandler` to receive `forwarded-tcpip` channels opened by the server, and in that callback do:

```rust
let (mut ssh_stream, _) = channel.into_stream().split();
let mut local = TcpStream::connect(("127.0.0.1", local_port)).await?;
tokio::io::copy_bidirectional(&mut ssh_stream, &mut local).await?;
```

Acceptance: `request_reverse_tunnel` returns only after the server confirms the forward; opening a TCP connection to `hub:<allocated_port>` causes a real request to appear on `localhost:<local_port>`.

**R3 — Add `tcpip_forward` to the server Handler.** In `crates/edgehub/src/ssh_server.rs`, implement `tcpip_forward` on `impl Handler for SshSession`. The implementation should:

1. Bind `TcpListener::bind(("0.0.0.0", *port))` (0 → pick one, update `*port`).
2. Clone `session.handle()` into the listener task.
3. For each accepted TCP connection, call `handle.channel_open_forwarded_tcpip(connected_address, connected_port, originator_address, originator_port).await?` to open a `forwarded-tcpip` channel back to the CLI.
4. `tokio::spawn(copy_bidirectional(accepted_tcp, channel.into_stream()))`.
5. Store listener + channel-set in `SshServerState::reverse_proxy_state` so `cancel_tcpip_forward` and session-close can clean up.
6. Register the `subdomain → port` mapping in Redis via `ReverseProxyState::register_tunnel_route`.

Acceptance: external `curl http://localhost:<allocated_port>/ping` from the test harness reaches the fake local HTTP server and returns its body, not the stub `"Tunnel connection active!"`.

**R4 — Delete the orphan path.** Remove `channel_open_direct_tcpip` handler, `handle_reverse_tunnel_registration`, `start_tunnel_port_listener`, and `forward_connection_to_ssh_channel`. Keep `ReverseProxyState` but remove the orphan method; all wiring now lives in `tcpip_forward`. This is a net code-deletion.

**R5 — Port sourcing.** In `cmd/edf-cli/src/tunnel.rs`, use `session.slot` from the API response as the `tcpip-forward` port argument, not a fabricated UUID-derived value. The API already returns it.

**R6 — Edge router actually forwards.** In `cmd/edgehub-bin/src/main.rs::serve_https_router`, replace the hard-coded `"Tunnel {id} is active for {sni}\n"` body with a real forward: look up `(subdomain → allocated_port)` in `ReverseProxyState`, open a `TcpStream::connect(("127.0.0.1", allocated_port))`, and `copy_bidirectional` the decrypted HTTP stream into it. That goes through the R2/R3 listener into the SSH channel → to the CLI → to `localhost:<local_port>`.

Acceptance (end-to-end): hit `https://test.fleetingdns.run/anything` at the router with a real TLS client; the body of the response is produced by the dev laptop's local HTTP server, including streaming and chunked transfer.

**R7 — Fix edgehub-bin defaults.** `--addr` default `0.0.0.0:8444` (or separate role-explicit flags). Remove `extract_sni_from_tls` (dead) or replace with real SNI parsing via `rustls` `ClientHello` sniffing (`rustls::server::Acceptor::accept`).

**R8 — CLI uses SshKeyManager-issued keypair.** In `ssh_client.rs::establish_tunnel`, thread the `KeyPair` from `SshKeyManager::get_or_request_key_pair` through `TunnelClient::establish_tunnel`, remove the throwaway `generate_ed25519` call.

**R9 — Test shape.** Add a non-`--features e2e` integration test `crates/edgehub/tests/e2e_reverse_tunnel_http.rs` that:

1. Starts a real `SshServer` on 127.0.0.1:0.
2. Starts a fake "local app" HTTP server on 127.0.0.1:0 that echoes request bodies.
3. Runs the CLI's `TunnelClient` to register + forward.
4. `curl`s the server-side `allocated_port` and asserts the echoed body is received.

This is the test that would have caught the bug in PR #52.

---

## 9. Lessons (write-once, reuse in Tiffany)

1. **Protocol-layer choices before control-flow code.** The `direct-tcpip` vs `tcpip-forward` distinction is the kind of decision that must be made and reviewed on day 1; it cannot be papered over by "Phase N" comments.
2. **Tests must assert the product promise.** "SSH handshake works" and "Redis metadata stored" are not a reverse-tunnel feature. A reverse-tunnel test MUST send bytes from outside the server and observe them arrive at `localhost` on the client.
3. **Phase-0 "accept anything" auth masks downstream defects.** Short-circuit auth paths should be guarded by a single env flag and never used in test assertions.
4. **Listener tasks need handles, not IDs.** Any function that will write into a remote channel from an outside context must carry the async sink (`russh::server::Handle`), not a bare `ChannelId`.
5. **Pin the toolchain to something CI can actually build.** `nightly` + a stale `Cargo.lock` is not a reproducible build environment.
6. **Method placement matters.** Put the method on the object that has the state it needs. `register_reverse_tunnel` belonged on `SshSession`, not on `SshServer`.

---

## 10. Severity-weighted action items


| #   | Owner       | Priority | Description                                                            |
| --- | ----------- | -------- | ---------------------------------------------------------------------- |
| R1  | build       | P1       | Unblock workspace build                                                |
| R2  | edf-cli     | P0       | Client `tcpip-forward` + `server_channel_open_forwarded_tcpip`         |
| R3  | edgehub     | P0       | Server `Handler::tcpip_forward` with real listener + `forwarded-tcpip` |
| R4  | edgehub     | P1       | Delete orphaned direct-tcpip path                                      |
| R5  | edf-cli     | P1       | Use API-issued slot                                                    |
| R6  | edgehub-bin | P0       | Real edge-router forward (not stub)                                    |
| R7  | edgehub-bin | P2       | Fix `--addr`/`--ssh-addr` defaults; SNI sniffing                       |
| R8  | edf-cli     | P2       | Use issued keypair                                                     |
| R9  | edgehub     | P1       | E2E reverse-tunnel test (non-Docker)                                   |


End state: a 40-line net reduction in `ssh_server.rs` (delete orphan path), a small new `tcpip_forward` handler, and an actual working tunnel.