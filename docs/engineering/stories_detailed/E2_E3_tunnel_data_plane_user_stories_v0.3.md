# 📗 E2/E3 — Tunnel Data Plane User Stories (v0.3)

**Status: AUTHORITATIVE.** This document supersedes the story lists in
`Epic_highlevel/E2-Tunnel_Server_&_CLI_(Design_v0.2).md` §"E3 story equivalents" and
`Epic_highlevel/E3-Edge_Proxy_(Design_v0.1).md` for everything touching the tunnel data plane.
It was produced after the reverse-tunnel connectivity postmortem
(`docs/engineering/POSTMORTEM-reverse-tunnel-connectivity.md`) and a July 2026 code review of
the as-built implementation. Story statuses reflect **verified code**, not PR claims.

---

## 1. As-built architecture (the source of truth for these stories)

One live forwarding path exists. Everything else that looks like a forwarding path in the
tree is dead scaffolding scheduled for deletion (TDP-10).

```
external client
  → edgehub-bin HTTPS router (SNI sniffed from ClientHello via LazyConfigAcceptor,
     FR-EDGE-2; session-grant cookie check for protected tunnels, FR-EDGE-3)
  → Redis: subdomain → tunnel record → slot
  → TcpStream to 127.0.0.1:<slot>            (raw byte splice, FR-EDGE-4/5)
  → SshServer tcpip_forward slot listener     (slot-allocation gate against Redis)
  → forwarded-tcpip channel → copy_bidirectional
  → CLI Handler::server_channel_open_forwarded_tcpip (spawned, NOT awaited inline)
  → copy_bidirectional → developer's localhost:<port>
```

Key files: `cmd/edgehub-bin/src/main.rs` (router), `crates/edgehub/src/ssh_server.rs`
(`tcpip_forward`, slot listeners, teardown), `cmd/edf-cli/src/ssh_client.rs` (client side).

---

## 2. Story index

| ID | Story | Status |
|----|-------|--------|
| TDP-1 | Reverse tunnel via `tcpip-forward` + `forwarded-tcpip` | ✅ Done |
| TDP-2 | Edge routes by SNI → Redis → slot splice | ✅ Done |
| TDP-3 | Slot-allocation gate on `tcpip_forward` (fail closed) | ✅ Done |
| TDP-4 | Tunnel teardown: session drop, cancel, viewer-idle reaper (FR-HUB-2) | ✅ Done |
| TDP-5 | Session-grant gating for protected tunnels (FR-EDGE-3) | ✅ Done |
| TDP-10 | Delete dead forwarding implementations | ✅ Done (2026-07-17) — `BruteForceProtection` retained under `#[allow(dead_code)]` pending TDP-13 |
| TDP-11 | CLI stops killing its own tunnel (inactivity timeout + keepalives) | ✅ Code landed (2026-07-17) — 10-min-idle AC test still owed under TDP-16 |
| TDP-12 | CLI authenticates with the API-issued keypair (ex-R8) | ✅ Code landed (2026-07-17) — rejection AC blocked on TDP-13 |
| TDP-13 | Real SSH auth wired into the live handler | 🆕 P1 |
| TDP-14 | One public domain, sourced from the API | ✅ Done (2026-07-17) |
| TDP-15 | Hub robustness: accept backoff + `edge_tunnels_open` gauge (absorbs T-29) | ◕ Partial (2026-07-17) — backoff + gauge done; connection caps still open |
| TDP-16 | E2E asserts the product promise, both directions, over time | 🆕 P1 (now also owns TDP-11's idle AC test) |
| — | Deprecated stories | see §5 |

---

## 3. Completed stories (as-built, verified)

### TDP-1 — Reverse tunnel uses the correct SSH primitive
As a CLI user, when I run `fleetingdns forward --port N`, the CLI issues a `tcpip-forward`
global request for the API-allocated slot, and the hub opens `forwarded-tcpip` channels back
to me for each inbound connection (RFC 4254 remote forwarding, `ssh -R` semantics).

*Implementation notes that must not regress:*
- The client-side bidirectional copy is **spawned** from the russh handler callback, never
  awaited inline — awaiting blocks the session message pump and deadlocks (R9 lesson,
  `ssh_client.rs:57-61`).
- The hub binds slot listeners on **127.0.0.1 only** regardless of the requested address;
  slots are hub-internal.

Replaces the deprecated `direct-tcpip` design (see §5, D-1).

### TDP-2 — Edge routes by SNI, before completing the handshake
As an external caller, my request to `https://<subdomain>.<domain>` is routed by sniffing SNI
from the TLS ClientHello (`rustls` `LazyConfigAcceptor`), looking up the subdomain's tunnel
record in Redis, and raw-splicing decrypted bytes to `127.0.0.1:<slot>` with
`copy_bidirectional`. The edge never interprets HTTP framing on the forward path (WebSocket
upgrades pass through), and originates HTTP bytes only for error responses (400/403/404/502/504).

Corrects the deprecated design claims: no Host-header routing, no HTTP2 multiplexing hub-link,
no Unix sockets, no etcd (see §5, D-2/D-3).

### TDP-3 — Only allocated slots can be bound
As the platform, a `tcpip-forward` request is honored only if a live API-allocated tunnel
record exists in Redis for that slot; lookup failures **fail closed**. Without Redis
(bare test setups) all slots are allowed with `ttl_only` policy.

### TDP-4 — Slots are torn down, never leaked
As the platform: `cancel-tcpip-forward` aborts the slot's accept loop (dropping the listener);
SSH session end (graceful or crash) tears down every listener the session bound (`Drop` on
`SshSession`); `viewer_idle` tunnels with zero open connections and ≥60 s quiet are reaped.
`ttl_only` tunnels are never idle-reaped (agents driving Playwright must survive idle).

### TDP-5 — Protected tunnels require a session grant
As a tunnel owner with `protected: true`, the edge reads the first request head (bounded
16 KiB / 10 s), validates the `fdns_session` cookie against Redis, replays the buffered bytes
to the slot on success, and returns 403 otherwise. One check per TCP connection, then splice.

---

## 4. New stories (remediation backlog — July 2026 code review)

### TDP-10 — Delete the dead forwarding implementations **(P0)**
As a maintainer, I can trust that every forwarding code path in the tree is live, so I stop
losing debugging days to confident-looking scaffolding.

**Delete:**
1. `crates/edgehub/src/tls_router.rs` — disabled module ("compilation issues"), placeholder
   cert `vec![1,2,3]`, old rustls API. Its job is done by the router in `edgehub-bin`.
2. `crates/edgehub/src/proxy.rs` (`TcpProxy`) — unused; superseded by `copy_bidirectional`.
3. `tcp_proxy_task` (`ssh_server.rs:1441`) — uncalled, and half-duplex: its handler drains
   SSH→target to EOF **before** forwarding target→SSH; any request/response protocol hangs.
4. The T-26b "dynamic reverse proxy": `ReverseProxyState`, `allocate_tunnel_port`,
   `register_tunnel_route`, `handle_reverse_tunnel_request`, `forward_to_tunnel_port`,
   `forward_to_developer_service`. It allocates random ports **nothing ever listens on**
   (guaranteed connection-refused), uses `read_to_end` (incompatible with keep-alive,
   streaming, WebSockets), and returns a fake 200 with a wrong Content-Length. Routing is by
   slot number; this map was never on the live path.
5. `edgehub::lib.rs` `serve`/`serve_with_shutdown` — accepts TLS, writes slot `"demo"` to
   Redis, closes the connection. Demo theater.
6. The dead **inherent** `auth_publickey`/`auth_password` on `SshSession`
   (`ssh_server.rs:1233+`) — see TDP-13 before deleting; the logic moves, the dead copy goes.

**AC:** workspace builds green with `-D warnings` and no `#[allow(dead_code)]` in
`crates/edgehub`; no log line claims a listener/route that doesn't exist; net LOC in
`ssh_server.rs` drops by ≥500.

### TDP-11 — The CLI must not kill its own tunnel **(P0)**
As a CLI user, an idle tunnel stays alive for its full TTL.

**Defects:** `ssh_client.rs:211` reuses `connection_timeout` (30 s) as russh
`inactivity_timeout`, so an idle session is dropped after 30 s — the hub then tears down the
slot listener and the next request 502s. `keep_tunnel_alive` (`tunnel.rs:163`) sends nothing
and polls a local HashMap no one updates, so the CLI reports "active" on a dead session.

**Fix:** separate inactivity timeout (≥ TTL or disabled); send SSH keepalives on
`keep_alive_interval`; monitor the actual session (surface disconnect immediately, exit
non-zero or reconnect).

**AC:** e2e test — establish tunnel, wait 10 min idle, send request: 200. Kill hub: CLI
reports disconnect within one keepalive interval.

### TDP-12 — Authenticate with the key the API issued (ex-R8) **(P1)**
As the platform, the keypair minted by `POST /v1/tunnels` and fetched via `SshKeyManager` is
the one used for SSH auth. Today `establish_tunnel` generates a throwaway Ed25519 key
(`ssh_client.rs:246`); issuance is theater and only works because the server accepts all keys.
**AC:** with TDP-13 enabled, a CLI using a non-issued key is rejected; the issued key succeeds.

### TDP-13 — Wire real auth into the live handler **(P1)**
As SecOps, the auth logic that exists actually runs. The elaborate Redis-auth +
brute-force-protection `auth_publickey` (`ssh_server.rs:1233`) is an inherent method — the
live trait impl (`:1194`) accepts every key ("Phase 0"). The brute-force tracker also keys on
a hard-coded `0.0.0.0:0`, which — if it were live — would let one attacker lock out everyone.

**Fix:** move Redis key validation into the trait `Handler` impl; plumb the real peer address;
gate accept-all behind an explicit `FDNS_INSECURE_ACCEPT_ALL_KEYS=1` dev flag that logs loudly.
**AC:** default build rejects unknown keys; lockout is per-peer; postmortem lesson #3 honored.

### TDP-14 — One public domain, sourced from the API **(P2)**
As a CLI user, the FQDN I'm shown is the FQDN that resolves. `tunnel.rs:59` hard-codes
`.edf.run` while the hub's domain is `fleetingdns.run` (and deployments use others). The FQDN
must come from the API response, never be assembled client-side.
**AC:** printed URL == the URL that serves traffic, under any configured domain.

### TDP-15 — Hub robustness and the tunnels gauge **(P2, absorbs T-29)**
As SRE: slot accept-loops back off on accept errors instead of spinning hot
(`ssh_server.rs:1118`); per-slot and global connection caps exist; `edge_tunnels_open` gauge
increments on `tcpip-forward` accept and decrements on teardown; splice errors are counted,
not just logged.
**AC:** gauge visible in Grafana matches active slot listeners; EMFILE storm does not peg CPU.

### TDP-16 — E2E asserts the product promise **(P1)**
As the team, CI proves bytes flow **both directions, from outside, over time** — the gap that
let the original bug survive ~15 PRs. Extend the R9 test with: response bodies (not just
request delivery), a long-lived connection with server-push after idle (WebSocket shape),
HTTP keep-alive across multiple requests on one connection, concurrent tunnels with distinct
subdomains, and the TDP-11 idle-then-request case.
**AC:** all in CI without Docker; failure output names the direction that stalled.

---

## 5. Deprecated stories

| ID | Was | Why deprecated |
|----|-----|----------------|
| **D-1** | E3-S1 "TLS-SSH Handshake … upgrades to OpenSSH `direct-tcpip` reverse tunnel" | `direct-tcpip` is local forwarding — the exact protocol category error that caused the S1 outage (postmortem §5). Replaced by TDP-1. The TLS-wrapped-SSH outer layer is **not implemented** (plain SSH on :2222); if still wanted, write a new story — do not resurrect this one. |
| **D-2** | E3-S2 "route traffic to correct **Unix socket**", etcd fallback lookup | No Unix sockets, no etcd. Live path is Redis → TCP `127.0.0.1:<slot>`. Replaced by TDP-2. |
| **D-3** | E2 design "edf-edge speaks to hub over **HTTP2 multiplexed stream**" | No HTTP2 hub link exists; the edge raw-splices TCP. Replaced by TDP-2. |
| **D-4** | T-26b "TCP proxy to developer channel" via `edgehub::proxy` — and its "✅ COMPLETE — dynamic reverse proxy with port allocation, route registration, instant traffic routing" claim | The claim was false: allocated ports had no listeners; routing map was never read on the live path. The *goal* shipped via TDP-1/TDP-2; the *implementation* it describes is dead code deleted by TDP-10. |
| **D-5** | E3-S3 Basic Auth / redirect mode | Superseded by session-grant cookie gating (TDP-5, FR-EDGE-3). Basic-auth/redirect modes are unimplemented; write fresh stories if product wants them. |
| **D-6** | E2 keep-alive design "CLI sends `SSH_MSG_GLOBAL_REQUEST keepalive@edf`" | Never implemented; current code does the opposite (self-disconnect at 30 s). Replaced by TDP-11. |
| **D-7** | E2 "GitHub OAuth gating, ephemeral cert used for TLS handshake, compression `zlib@openssh.com`" | None wired into the live tunnel path. Auth reality: accept-all (fixed by TDP-12/13). OAuth, mTLS-wrapped transport, and compression are **backlog**, not shipped — the E2 doc's checked-off security guarantees do not hold today. |

Still-valid but **not started** (kept, unmodified): E3-S4 byte accounting, E3-S5 zstd
compression, E3-S6 autoscaling/health probes.

---

*v0.3 — 2026-07-17. Derived from code review of `ssh_server.rs`, `edgehub-bin/main.rs`,
`ssh_client.rs`, `tunnel.rs`, `proxy.rs`, `tls_router.rs`, and the connectivity postmortem.*
