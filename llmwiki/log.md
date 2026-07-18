# llmwiki — log

Append-only chronological log of wiki activity. Each entry has a header
of the form `## [YYYY-MM-DD] <op> | <short slug>` so we can grep:
`grep "^## \[" log.md | tail -n 20`.

## [2026-04-20] init | wiki bootstrap

Created `llmwiki/` for the `fleetingdns` repo using the Karpathy
persistent-wiki pattern, modelled on the sibling
`../../cylon-local-infra/llmwiki/` schema. Wrote `AGENTS.md` (the
schema), `index.md` (catalog), and this `log.md`. Established the
directory layout (`entities/`, `concepts/`, `sources/`, `runs/`,
`assets/`) and the page-frontmatter convention.

## [2026-04-20] ingest | Karpathy LLM Wiki gist

Filed [`sources/karpathy-llm-wiki.md`](./sources/karpathy-llm-wiki.md).
This is the blueprint for the wiki and is referenced from `AGENTS.md`.

## [2026-04-20] ingest | README.md

Filed [`sources/readme-fleetingdns.md`](./sources/readme-fleetingdns.md)
summarising the product pitch, the four canonical user flows (Stripe
webhook, OAuth callback, multi-tenant routing, hotel-Wi-Fi NAT scenario)
and the SDK matrix. New entity stubs created for `edgehub`, `edf-cli`,
`dnsd`, `backendapi`, `edf-ca`, `redis-fleetingdns`.

## [2026-04-20] ingest | docs/prd/Ephemeral-DNS-Forwarder-PRD-v1.1

Filed [`sources/prd-ephemeral-dns-forwarder-v1.1.md`](./sources/prd-ephemeral-dns-forwarder-v1.1.md).
This is the product authority on the **reverse-port** semantic
(`tcpip-forward` + `forwarded-tcpip`) — important reference for the
postmortem.

## [2026-04-20] ingest | docs/engineering/POSTMORTEM-reverse-tunnel-connectivity

Filed [`sources/postmortem-reverse-tunnel.md`](./sources/postmortem-reverse-tunnel.md)
plus three concept pages distilled from it:
[`ssh-reverse-tunnel-protocol`](./concepts/ssh-reverse-tunnel-protocol.md)
(the protocol-layer category error),
[`russh-handle-vs-channelid`](./concepts/russh-handle-vs-channelid.md)
(the wrong-mental-model API), and
[`redis-slot-allocation`](./concepts/redis-slot-allocation.md) (the
fabricated-port anti-pattern, H5).

## [2026-04-20] ingest | tasks/connectivity.md

Filed [`sources/tasks-connectivity.md`](./sources/tasks-connectivity.md).
Adds runtime-evidence citations to the postmortem (matching the NDJSON
entries in `.cursor/debug-c6eef8.log`).

## [2026-04-20] ingest | KIND-TILT-SETUP.md

Filed [`sources/kind-tilt-setup.md`](./sources/kind-tilt-setup.md). Adds
operator guidance for the dev environment.

## [2026-04-20] ingest | E2-Tunnel_Server_&_CLI design

Filed [`sources/e2-tunnel-design.md`](./sources/e2-tunnel-design.md).

## [2026-04-20] run | reverse-tunnel-debug-instrumentation | partial

Filed [`runs/2026-04-19-reverse-tunnel-debug-instrumentation.md`](./runs/2026-04-19-reverse-tunnel-debug-instrumentation.md).
Static-analysis + prior-runtime evidence diagnosis; root cause confirmed.
Live reproduction blocked by `nightly-2025-06-28` + `generic-array
0.14.7` toolchain regression (cf. R1 of postmortem).

## [2026-04-20] run | shared-kind-on-ms02-migration | success

Filed [`runs/2026-04-20-shared-kind-on-ms02-migration.md`](./runs/2026-04-20-shared-kind-on-ms02-migration.md).
First iteration: Mac → ms02 via SSH apiserver+registry tunnels, Tilt
running locally on the Mac. Worked but was operationally heavy.

## [2026-04-20] update | root AGENTS.md rewrite

Replaced the stale "Tinkerbell coroutine-first agent runtime"
placeholder at [`../AGENTS.md`](../AGENTS.md) with a FleetingDNS-specific
repo-wide protocol. Key changes: (1) bootstrap order now starts at
`llmwiki/AGENTS.md` → `index.md` → `log.md`; (2) accurate crate + binary
topology table (edgehub, edf-ca, dnsd, backendapi, edf-cli, …) with
wiki cross-links; (3) hard workflow rules consolidated (no shell
scripts, no TS outside `/ui`, no Cursor co-author, TDD ≥65%, GitHub via
farm CLI); (4) dev-environment section points at the ms02 Tilt pattern;
(5) "Known gotchas" section surfaces the toolchain regression, the
reverse-tunnel protocol bug, and Phase-0 accept-any-pubkey auth with
direct links to the wiki pages that explain each. Filename is
`AGENTS.md` (plural) per Karpathy gist + Codex + Cursor conventions.

## [2026-04-20] run | tilt-on-ms02-pattern | success

Filed [`runs/2026-04-20-tilt-on-ms02-pattern.md`](./runs/2026-04-20-tilt-on-ms02-pattern.md).
Pivoted the dev pattern: Tilt now runs ON ms02 (where Docker + kind +
kind-registry live natively); only the Tilt UI port (10350) is forwarded
to the Mac via `ssh -L`. Default `MS02_SSH_USER` switched from `root` to
`casibbald` (the cluster owner). New concept page
[`tilt-remote-host-pattern`](./concepts/tilt-remote-host-pattern.md)
captures the pattern; the apiserver+registry tunnel is demoted to an
optional kubectl-from-Mac utility.

## [2026-04-27] run | r1-unblock-build | success

Fixed two blockers in `crates/edgehub/src/ssh_server.rs`: (1) missing
closing `}` brace for `impl ReverseProxyState` block (lines 72–129),
causing "unclosed delimiter" parse error; (2) three `Self::Error`
ambiguities on `auth_publickey` (line 976), `auth_password` (line
1143), and `channel_close` (line 1158) inside `impl SshSession` —
replaced with `anywarn::Error` matching the `type Error = anyhow::Error`
declared in the `Handler` impl. Build succeeds; 38 unit tests pass.
R1 status updated to "done".

## [2026-04-28] fix | reverse-tunnel data-plane (S1 + S2)

S1: edgehub ssh_server.rs — `Session` is not `Clone`, so `session.clone()`
doesn't compile. Fixed by using `session.handle()` to get a `Handle` (which
IS `Clone`), then spawning tasks with `handle.clone().channel_open_forwarded_tcpip()`.
`copy_bidirectional` used with `channel.into_stream()` (no `split()` on
`ChannelStream`). Added `#[instrument]` for tracing. Build: 0 errors, 38 tests pass.

S2: edf-cli ssh_client.rs — 5 compilation errors fixed:
- `ChannelStream::split()` doesn't exist → use `channel.into_stream()` directly
- `session.shutdown()` doesn't exist → removed `select!` arm
- `handle_forwarded_tcpip_channel` nested in `impl Handler` (not trait member)
  → moved to `impl SshClientHandler` as regular method
- `handle.tcpip_forward()` needs `&mut` but only had `&` → use `as_mut()`
- `Self::Error` ambiguous in non-trait impl → use `anyhow::Error`
Build: 0 errors, 9 warnings (unused code, expected).

Pre-existing test failures: `fleetingdns_integration_tests` import broken,
redis `MultiplexedConnection` trait issues — not related to this work.

## [2026-04-28] fix | R4: Delete orphaned direct-tcpip path

Removed `channel_open_direct_tcpip` handler from `SshSession` impl in
`crates/edgehub/src/ssh_server.rs`. This was the wrong SSH primitive for
reverse tunnels (local forwarding, not remote forwarding). Replaced with
a comment noting russh provides a default impl that rejects direct-tcpip.

Build: 0 errors, 9 warnings (all pre-existing dead code —
BruteForceProtection, AuthAttempt, tcp_proxy_task, etc. are never called).

Wiki updates:
- `llmwiki/sources/postmortem-reverse-tunnel.md` — R2 and R3 marked DONE
- `llmwiki/entities/edgehub.md` — R3 and R4 marked DONE, removed
  "blocked by R1 toolchain regression" note from tests section
- `llmwiki/entities/edf-cli.md` — R2 marked DONE
- `llmwiki/concepts/ssh-reverse-tunnel-protocol.md` — R2 marked DONE

## [2026-07-05] update | workspace-topology correction

Corrected all cross-repo references to the cylon-local-infra wiki. The
repos no longer share a filesystem: all microscaler code (including
fleetingdns) was moved to **ms02** (`~/Workspace/microscaler/…`) due to
Mac disk-space limits, and is reachable from the Mac over NFS at
`~/Workspace/remote/microscaler/…`. Only **cylon-local-infra** remains
local on the Mac at `~/Workspace/local/cylon-local-infra`. Relative
links like `../../cylon-local-infra/llmwiki/...` were therefore broken;
replaced with prose citations of the Mac-local path in `AGENTS.md`
(new "Workspace topology" section), `index.md`, `entities/ms02.md`,
and `sources/karpathy-llm-wiki.md`.

## [2026-07-05] run | tilt-up-10654-no-rsync | success

Brought the FleetingDNS Tilt stack up on ms02 alongside the systemd Tilt
fleet. Changes: (1) Tilt UI port 10350 → **10654** (10348–10353 + 10450
held by tilt-shared-kind/rerp/sesame-idam/hauliage/brrtrouter/cylon
units); host-side port_forwards re-prefixed (api 18080, otel 14317/14318)
to avoid ports 3000/3100/4317/4318/8080/9090 held by other stacks.
(2) Removed the `just sync` rsync step — repo lives only on ms02, Mac
edits arrive via NFS. (3) Fixed a pre-existing unclosed-quote bug in the
Justfile `up` recipe. (4) Tiltfile: replaced stale k8s_resources
(prometheus/loki/mimir/grafana are Flux HelmReleases, absent in alocal)
with real ones; added dev-only redis:7-alpine + postgres:16-alpine
Deployments via `k8s_yaml(blob(...))` (the `k8s-tilt/infra/databases/`
manifests are Crossplane GCP Instances — unusable on kind); replaced
broken `experimental_analytics_report` call with `analytics_settings(False)`.
(5) otel-collector probes `/` → `/metrics` on :8888 (was 404 →
CrashLoop). Result: otel-collector/postgres/redis Running; dnsd/edgehub/
api building. Updated
[tilt-remote-host-pattern](./concepts/tilt-remote-host-pattern.md) with
the port-allocation table and no-rsync note.

## [2026-07-05] run | shared-data-services migration | success

Moved FleetingDNS off its dev-only in-cluster redis/postgres and onto
the shared-kind-cluster services, keeping the repo to core functionality
(api, dnsd, edgehub + fdns-db-init only). Changes:

1. **Tiltfile**: deleted the redis:7-alpine / postgres:16-alpine
   `k8s_yaml(blob(...))` block and the otel-collector kustomize include;
   added a one-shot `fdns-db-init` Job (postgres:16-alpine) that creates
   the `fdns` database in the shared postgres via the
   `SELECT 'CREATE DATABASE fdns' WHERE NOT EXISTS(...)\gexec` idiom.
   `api` now `resource_deps=['fdns-db-init']`.
2. **alocal overlay**: new `*-shared-services.patch.yaml` for
   api/dnsd/edgehub setting `REDIS_URL=redis://redis.data.svc.cluster.local:6379`,
   `DATABASE_URL=postgresql://postgres:postgres@postgres.data.svc.cluster.local:5432/fdns`,
   `OTEL_EXPORTER_OTLP_ENDPOINT=http://otel-collector.observability.svc.cluster.local:4317`.
   Gotcha: edgehub takes redis via `--redis` CLI arg which beats the env
   var, so the patch also replaces the args list wholesale.
3. **Flux leftovers purged from alocal**: delete-patches for the
   external-dns / external-secrets HelmRelease+HelmRepository and the four
   flux-system Kustomization CRs — the shared kind cluster has no Flux
   CRDs, these caused permanent `uncategorized` apply failures.
4. **edgehub probes**: tcpSocket 8080 → 2222 (edgehub binds 2222/443/8444,
   never 8080 for metrics); the resulting kubelet probes show up as
   harmless `tls handshake failed ... eof` INFO log lines.

Verified: all Tilt resources ok/ok; api `/health` 200 against shared
postgres (`fdns` db exists); dnsd serving on :53; shared redis PONG.
Updated [redis-fleetingdns](./entities/redis-fleetingdns.md) and
[shared-kind-cluster](./entities/shared-kind-cluster.md).

## [2026-07-05] update | api off port 8080 -> 8880 repo-wide

Port 8080 is chronically contested on ms02 (held by other tilt stacks)
and generally a conflict magnet, so the Backend API moved to **8880**
everywhere:

- Code defaults: `crates/backendapi/src/config.rs` (`API_BIND_ADDRESS`,
  `BASE_URL`) and `crates/common/src/config.rs` (`API_PORT`) now default
  to 8880; unit tests updated.
- k8s: api deployment containerPort/probes and service port 8880
  (NodePort stays 30080); Tiltfile port_forward now plain `8880:8880`
  (the 18080 workaround is gone).
- Vestigial "metrics 8080" containerPort/service-port entries removed
  from dnsd and edgehub (nothing ever listened there — metrics go via
  OTLP to the shared otel-collector).
- Also updated: kind-config.yaml hostPort (8080→8880), docker-compose,
  Justfile, edf-cli dev-overrides default api-url, integration test
  base URLs (tests/integration, tests/tunnel_lifecycle_tests.rs), docs
  (KIND-TILT-SETUP.md, DOCKER-COMPOSE-TO-KIND-MIGRATION.md),
  scripts/kind-setup.py, and the wiki port tables.
- NOT changed: `local_port: 8080` in test payloads / tunnel examples
  (that's the *user's* app port, not ours) and the GCP firewall rules
  for crossplane/flux controllers' own 8080.

Verified: `cargo test -p common --lib config` and
`-p backendapi --lib config` pass on ms02; Tilt rebuilt api and the pod
is 1/1 with `/health` 200 on :8880; old :18080 forward no longer
answers.

## [2026-07-05] run | audit: why tunnel traffic never reaches the hub

Re-audited the end-to-end path after R1-R5. The SSH plumbing (client
`tcpip_forward` + `server_channel_open_forwarded_tcpip`, server listener
+ `forwarded-tcpip`) is now correct, but the surrounding hops are broken.
Findings, in packet-lifecycle order:

1. **DNS answers 127.0.0.1** — `backendapi/src/storage.rs:113` writes
   `slot:{fqdn} = "127.0.0.1"` (hard-coded) so dnsd sends external
   callers to themselves, never to the edge.
2. **The exposed port 2222 is a dead stub** — `edgehub-bin --addr
   0.0.0.0:2222` binds `edgehub::serve_with_shutdown` (lib.rs:86): a
   placeholder that TLS-handshakes, writes slot "demo" to Redis, and
   closes. The real SSH reverse-tunnel server is on `--ssh-addr` :8444
   and the HTTPS router on `--https-addr` :443 — NEITHER is in the k8s
   Service (only 2222 is). The CLI defaults to hub:2222 "plain SSH",
   russh speaks SSH to a TLS listener → handshake fails → no tunnel.
   Port-role confusion between manifest (2222 = "tunnel") and binary
   (2222 = legacy TLS stub).
3. **Router skips the second hop (R6 half-done)** — `serve_https_router`
   (edgehub-bin/main.rs:81) on SNI match dials
   `127.0.0.1:{tunnel_info.local_port}` — the DEVELOPER's port on the
   DEVELOPER's machine. Correct target is the hub-side slot listener
   `127.0.0.1:{tunnel_info.slot}` bound by tcpip_forward, which relays
   through the SSH channel to the CLI. The intended design is a two-hop
   relay (external → edge router → hub slot port → forwarded-tcpip →
   CLI → localhost); the code short-circuits hop 2, which only works
   when hub and dev machine are the same host (i.e. in tests).
4. **Session↔tunnel binding missing** — server `tcpip_forward`
   registers hard-coded subdomain "tunnel" into in-memory
   ReverseProxyState that nothing reads; Phase-0 auth accepts any key
   (R8 pending) so the hub cannot map an SSH session to its tunnel
   record/subdomain/slot, and never validates the requested forward
   port against the API-allocated slot.
5. Minor: duplicate `auth_publickey` (trait impl at ssh_server.rs:989
   used; inherent fn at :1028 with brute-force logic is dead),
   `get_tunnel_by_subdomain` does KEYS + N+1 GETs per request,
   R9 e2e (bytes-through-tunnel assertion) still missing.

Conclusion: fix order = expose/bind SSH on 2222 (or expose 8444+443),
router dials slot not local_port, API writes edge IP not 127.0.0.1,
bind SSH session to tunnel record. Postmortem R6/R7/R8/R9 remain the
right remediation list; R6 needs "dial the slot port" spelled out.

## [2026-07-05] fix | forwarded-tcpip inline-await deadlock (R9 unblocked)

The R9 e2e test (`crates/edgehub/tests/e2e_reverse_tunnel_http.rs`) hung
for 60s and was SIGTERM'd — not a flaky test, a real data-plane bug.

Root cause: `server_channel_open_forwarded_tcpip` is a russh Handler
callback that runs ON the SSH session event loop. Both the test client
handler AND the real CLI (`cmd/edf-cli/src/ssh_client.rs`
`handle_forwarded_tcpip_channel`) called
`copy_bidirectional(ssh_stream, local).await` INLINE. That await blocks
until the SSH channel yields bytes, but the loop that pumps bytes into
the channel is the same loop the callback is blocking → deadlock. The
inbound TCP reached `127.0.0.1:local_port` (log shows "Local server:
accepted connection") but the response could never flow back.

Fix: `tokio::spawn` the copy so the callback returns immediately and the
session pump keeps running. Applied in both the test handler and the CLI.
Also switched the test cleanup from `.await` (would hang on the
forever-looping server/local tasks) to `.abort()`. Result: test passes
in 0.66s.

Impact on cylon FAR-TILT-TUNNEL-PRD: this is exactly FR-VM-2/FR-VM-3 —
the agent's `far-tunneld` must spawn per-channel or Tilt's long-lived
`/ws/view` WebSocket (AT-2) deadlocks the agent SSH session on the first
connection. spawn-per-forwarded-channel is now the established pattern.

Remaining audit fixes still queued: router dials slot not local_port
(R6), port-layout/dead-stub on 2222 (R7), hard-coded 127.0.0.1 DNS
record, session↔tunnel binding (R8), expose SSH+router in k8s Service.

## [2026-07-05] fix | tunnel data-plane: R6/R7 + DNS + port layout (live on ms02)

Second pass on the tunnel audit — made the hub actually routable.

R6 (router dials the slot, not the dev port): `serve_https_router`
(cmd/edgehub-bin/src/main.rs) on SNI match now dials
`127.0.0.1:{tunnel_info.slot}` — the hub-side listener bound by
tcpip_forward — instead of `tunnel_info.local_port` (which is the dev's
port and only resolves inside the hub pod in same-host tests). This is
the missing second hop of the double-hop relay. Deleted the
always-None `extract_sni_from_tls` placeholder (prohibition #8).

R7 (port layout): removed the dead `edgehub::serve_with_shutdown` TLS
stub and its `--addr` flag from edgehub-bin. The k8s Service exposed
ONLY 2222, which the stub was squatting — so the CLI's russh handshake
hit a TLS listener and failed. Now `--ssh-addr` defaults to 2222 (the
real SSH reverse-tunnel server) and `--https-addr` to 8443 (router).
Added `arg_tests::socket_addr_defaults_are_disjoint` (prohibition #7).
Verified live: `nc edgehub:2222` returns `SSH-2.0-russh_0.40.2`;
router logs `HTTPS router listening addr=0.0.0.0:8443`.

Privileged-port gotcha: edgehub runs as non-root user `app` (dnsd runs
as root, which is why :53 worked). 443 can't bind as `app`, so the
router listens on 8443 in-container and the Service maps port 443 →
targetPort 8443. edgehub Service now exposes ssh(2222) + https(443→8443).

DNS: `backendapi/src/storage.rs` no longer hard-codes the slot record to
`127.0.0.1`; it reads `EDGE_PUBLIC_IP` (default 127.0.0.1 for single-host
dev). Set this to the edge's reachable IP in real deployments so dnsd
sends callers to the edge, not themselves.

Session↔tunnel binding: routing is keyed by SLOT NUMBER end to end (API
allocates slot → stores in Redis tunnel record → CLI requests
tcpip_forward on that exact slot → hub binds it → router resolves SNI →
subdomain → Redis → slot → dials it). Removed the misleading hard-coded
in-memory "tunnel" route registration in tcpip_forward (never read on
the edge path). Real per-session cert auth (AT-6 / PRD T3) is still
future work.

Maps to cylon FAR-TILT-TUNNEL-PRD: FR-HUB-1/3 (real listener +
copy_bidirectional), FR-EDGE-5 (no placeholder responses), §6.7
(disjoint clap defaults), §6.8 (no always-None extract helpers).
Still open from that PRD: SNI-from-ClientHello (FR-EDGE-2, today we
sniff Host header), Clerk auth (FR-EDGE-3), cancel_tcpip_forward
lifecycle (FR-HUB-2), cert propagation (FR-API-1).

## [2026-07-05] feat | FR-EDGE-2: SNI from ClientHello (no Host-header sniffing)

Rewrote the edge HTTPS router (cmd/edgehub-bin/src/main.rs) to route on
the TLS SNI extracted from the ClientHello instead of the decrypted HTTP
Host header (cylon FAR-TILT-TUNNEL-PRD FR-EDGE-2; prohibition against
Host fallback).

How: `tokio_rustls::LazyConfigAcceptor` +
`rustls::server::Acceptor::default()` peeks the ClientHello before the
handshake; `start.client_hello().server_name()` yields the SNI; then
`start.into_stream(config)` completes the handshake. Subdomain =
left-most SNI label (`subdomain_from_sni`, unit-tested for
`abc123.fleetingdns.run` and `<uuid>.tilt.tiffany.microscaler.io`).

Bonus correctness: the forward path is now a single
`tokio::io::copy_bidirectional(tls, slot)` raw splice instead of the old
"read first request to find Host, buffer it, then two manual copy loops".
This is WebSocket-safe by construction (FR-EDGE-4) — no HTTP request
boundary is ever parsed, so Tilt's /ws/view upgrade passes straight
through. The only HTTP bytes the edge originates are 4xx/5xx errors via
`write_http_status` (FR-EDGE-5).

Removed the now-unused Host-buffer plumbing, `TlsAcceptor` import, and
`AsyncReadExt`. Tests: router_tests (2) + arg_tests (1) green;
edgehub-bin builds clean.

Still open on the edge: Clerk cookie + agent-grant check (FR-EDGE-3),
wildcard cert via cert-manager (FR-EDGE-1 / NFR), audience-bound cookie.

## [2026-07-05] feat | FR-EDGE-3: session-grant auth gate on protected tunnels

Implemented the edge authorization gate from the cylon
FAR-TILT-TUNNEL-PRD (FR-EDGE-3, adapted: fleetingdns has no Clerk, so the
grant is an opaque Redis-backed token minted by the control API — the
Clerk-cookie-specific part lands in cylon's far-tunnel-edge, but the
mechanism/keyspace is identical).

Data model: `TunnelInfo`/`Tunnel` gained `protected: bool` with
`#[serde(default)]` (old Redis records stay public/deserializable).
`CreateTunnelRequest.protected` opts a tunnel in.

Control API: `POST /v1/tunnels/{id}/session` (owner-only, same auth as
get/delete) mints a 32-char opaque token via
`TunnelStorage::store_session_grant` → Redis
`session_grant:{subdomain}:{token}`, TTL 900 s (FR-API-2's 15 min).
Response includes a ready-to-set `fdns_session` cookie string. Key format
and cookie name are shared contracts in `common::redis`
(`session_grant_key`, `SESSION_COOKIE_NAME`).

Edge (cmd/edgehub-bin): for `protected` tunnels, after SNI match + Redis
lookup but BEFORE dialing the slot, read the first request's header block
(bounded 16 KiB, 10 s timeout), extract the `fdns_session` cookie, check
grant EXISTS in Redis. No/wrong token → 403 before any bytes are
forwarded. On success, replay the buffered head to the slot and raw-splice
as before — each fresh TCP connection is verified once, then spliced, and
the WS upgrade request carries the cookie so /ws/view still works
(FR-EDGE-4 preserved). Unprotected tunnels skip the peek entirely (zero
change to that path).

Tests: grant_tests (cookie parse incl. case-insensitive header + multi
cookie, read_request_head stop/EOF) — edgehub-bin 8/8; backendapi lib
89/89; common lib 93/93. Live on ms02 (seeded protected record in shared
redis): no cookie → 403, wrong token → 403, valid grant → gate passes and
slot dial 502s (no listener bound — correct ordering proven), logs show
"rejecting protected tunnel connection" / "session grant verified".

Still open: real per-session SSH cert auth on the hub (AT-6/T3),
cancel_tcpip_forward lifecycle (FR-HUB-2), wildcard cert (FR-EDGE-1).

## [2026-07-05] decision | capability-URL access model; subdomain entropy 8→20 chars

Owner decision: no Clerk / no default user-auth gate. Tunnels are
short-lived and unique per inception — whoever holds the link may access
it while it lives. Client-side cert validation is explicitly deferred.
Recorded in [capability-url-access-model](./concepts/capability-url-access-model.md).

Consequence: the random subdomain becomes the load-bearing credential,
and 8 base-36 chars (≈41 bits) was too guessable for that role.
`generate_random_subdomain` now emits `tunnel-` + 20 base-36 chars
(≈103 bits, CSPRNG), guarded by the
`random_subdomain_is_high_entropy_capability` test (TDD: test written
first against the 8-char version, then the generator widened).
`custom_subdomain` remains the user's explicit opt-out.

The FR-EDGE-3 session-grant machinery (commit 5dd8b08) stays but is
dormant: `protected` defaults to false everywhere and is NOT the plan of
record — kept only in case per-user binding is revived for cylon.

backendapi lib tests: 90/90.

## [2026-07-05] design | FR-HUB-2 teardown: per-tunnel policy flag, not header (open)

Question raised by owner: agents driving Playwright open/close browsers
repeatedly, so a "viewer idle >60s" teardown would kill their tunnel
between iterations. Header from the client vs flag at tunnel creation?

Recommendation (not yet implemented): creation-time flag
(`teardown_policy: viewer_idle | ttl_only` on CreateTunnelRequest +
tunnel record, serde default like `protected`). Header rejected:
per-connection, unauthenticated/spoofable (capability URL grants access,
not lifecycle control), conflicting signals when human tab + agent share
a tunnel, and would force HTTP parsing onto the raw-splice path
(FR-EDGE-4). Key split: SSH-side disconnect ALWAYS tears down the slot
listener (with reconnect grace window, FR-VM-5); only the viewer-idle GC
is policy-gated. ttl_only tunnels die on TTL, explicit DELETE, or SSH
disconnect — deterministic for automation. Suggested defaults:
fleetingdns=ttl_only, cylon portal tunnels=viewer_idle.
Awaiting owner confirmation before implementing FR-HUB-2 this way.

## [2026-07-05] feat | FR-HUB-2: slot teardown + per-tunnel teardown policy

Implemented the confirmed design (creation-time flag, never a header).

Data model: `TeardownPolicy { ttl_only (default) | viewer_idle }` lives in
`common::redis` (shared contract); `#[serde(default)]` field on all three
tunnel structs; `CreateTunnelRequest.teardown_policy` opts in. ttl_only =
deterministic lifecycle for automation (agents driving Playwright): dies
on TTL, explicit DELETE, or SSH disconnect only.

Hub (crates/edgehub/src/ssh_server.rs):
1. **Teardown on SSH disconnect** — `SshSession.forward_listeners`
   (port → JoinHandles) + `impl Drop` aborts every accept-loop task when
   the session ends (graceful or crash). Aborting drops the future, which
   drops the TcpListener → port closed. Previously the JoinHandle was
   LEAKED — every listener was a zombie after disconnect.
2. **cancel_tcpip_forward** — implemented (was default-reject): removes
   the port's handles, aborts, replies true; false for unknown ports.
   In-flight forwarded-tcpip copies drain naturally (RFC 4254 semantics).
3. **Viewer-idle reaper** — only when the tunnel record (looked up by
   slot via new `common::redis::get_tunnel_by_slot`; SshServerState gained
   an optional redis_pool from SshConfig.redis_url, wired in edgehub-bin)
   says viewer_idle: SlotActivity (AtomicUsize active + last-event
   Instant) counts OPEN connections, so a quiet long-lived WebSocket is
   NOT idle; reaper polls 10s, fires at 60s idle (VIEWER_IDLE_TEARDOWN).
   Decision logic is the pure `should_teardown_idle` fn (unit-tested
   decision table).

TDD: `tests/e2e_slot_teardown.rs` written FIRST — both tests failed
against the old hub (cancel unacknowledged, zombie listener after
disconnect), pass after. russh 0.40 gotcha: client `tcpip_forward` with
port != 0 is fire-and-forget (only awaits reply for port 0) and client
`cancel_tcpip_forward` always returns true — so the tests poll for
listener up/gone rather than trusting the client-side return values.

Tests: edgehub lib 40/40, teardown e2e 2/2, R9 e2e ok, common 93/93,
backendapi 90/90. Live on ms02: real OpenSSH client (`ssh -N -R
43533:...`) against deployed hub → "listener ready policy=TtlOnly";
kill ssh → "SSH session ended: slot listener torn down port=43533".
OpenSSH ↔ russh interop confirmed in passing.

Still open from the PRD: real per-session cert auth (AT-6/T3, Phase-0
accepts any key), FR-HUB-7 random-slot allocation with reply-port
propagation, wildcard cert (FR-EDGE-1).

## [2026-07-05] feat | multi-tunnel concurrency + isolation (prioritised over cert auth)

Owner reprioritised: concurrent tunnels, each with its own endpoint,
secure with NO cross-communication > client-side cert auth (deferred).
Three fixes + three test layers.

Fixes:
1. **Subdomain uniqueness (was a silent no-op).** create_tunnel now calls
   `is_subdomain_available` and 400s on collision. Root cause: the
   `subdomain:{name}` index that get_tunnel_by_subdomain reads was NEVER
   written (only deleted). storage.rs::store_tunnel now writes it with
   SET NX EX; on NX-miss it 400s unless the holder is the same tunnel id
   (re-store refreshes TTL). Subdomain is the SNI routing key, so a dup
   would route one tunnel's viewers into another — this is load-bearing
   for isolation.
2. **Atomic slot allocation.** allocate_port GET-then-SET was a TOCTOU
   race (two concurrent creates could grab one port → two tunnels one
   slot). Now SET NX EX; exactly one caller wins.
3. **Hub slot-allocation gate.** SshSession.tcpip_forward now looks up the
   tunnel record by slot (get_tunnel_by_slot) BEFORE binding and DENIES
   (reply false) any port with no live record, when Redis is present.
   Phase-0 accepts any SSH key, so without this any client could squat
   arbitrary ports on the hub pod. Fails closed on lookup error.

Also: slot listeners now bind 127.0.0.1 explicitly (not the client's
requested addr). Slots are hub-internal (only the in-pod router dials
127.0.0.1:slot); loopback-only is safer AND fixes OpenSSH `-R` interop —
OpenSSH's default "localhost" bind resolves to ::1, so the listener bound
::1 while the router dialed 127.0.0.1 → connection refused (the 502s in
the first live run).

Tests (TDD, all green): edgehub `e2e_concurrent_tunnels` (3 tunnels, 30
interleaved parallel requests, each slot returns ONLY its payload, +
teardown-isolation: kill one, others survive); `e2e_slot_allocation_gate`
(real Redis: allocated slot binds, unallocated denied); backendapi
`storage_isolation` (real Redis: subdomain uniqueness, 32-way concurrent
allocation no dupes). Full sweep: edgehub lib 40, edgehub-bin 8, common
93, backendapi 90+2. Live on ms02: two OpenSSH reverse tunnels (slots
40001/40002) to two python http servers, SNI-routed through the edge
(curl --resolve) — livea→TUNNEL-A-SECRET, liveb→TUNNEL-B-SECRET across
interleaved requests, and after killing tunnel A: A→502, B still
TUNNEL-B-SECRET. No cross-talk.

Deferred (owner): client-side cert auth (AT-6/T3). Still open: FR-HUB-7
random-slot alloc with reply-port propagation, FR-EDGE-1 wildcard cert.

## [2026-07-05] feat | FR-EDGE-1: wildcard TLS on the edge router

The capability-URL model leaks without this: a per-subdomain cert would
publish every "unguessable" tunnel FQDN to CT logs, and the old hardcoded
`tls.local` self-signed cert broke SNI-based trust entirely.

common/src/tls.rs: `generate_wildcard_tls_config(alpn, base_domain)`
(self-signed SAN set `*.{base}`, apex, localhost — dev path) and
`load_tls_config_from_files(cert, key, alpn)` (PEM chain + PKCS8/RSA/SEC1
key — prod path, e.g. cert-manager mount). edgehub-bin grew `--tls-cert`/
`--tls-key` (must come together; else dev fallback generates from
`--public-domain`). Unit tests parse the generated cert with x509-parser
and assert the SANs; file-loading round-trips and errors on garbage.

Live on ms02: `openssl s_client` against the edge NodePort shows
`CN=*.fleetingdns.run` with matching SANs for any tunnel SNI. Gotcha
found on the way: Tilt live_update stanzas were broken by design (runtime
images are non-root debian-slim with no toolchain → tar Permission denied
→ UpdateFailed → stale binary kept serving old cert); removed live_update
from all Rust services, full docker rebuilds only.

## [2026-07-06] fix | integration suite green: dev-bypass hardening + real API contract + DELETE bug

The legacy tests/integration suite was written against an imagined API
(localhost:8081, /v1/statistics, /v1/slots, POST /v1/auth, 201-on-create)
and ran unauthenticated — 21-23 of 34 failing. Rewrote it against the
real contract (port 8880; create → 200 with id/fqdn/slot; malformed body
→ 422 from axum's Json extractor; non-UUID id → 400; /v1/stats;
POST /v1/certificates {common_name,ttl}; dnsd via kind-node NodePort
172.19.0.2:30053, override FDNS_TEST_DNS_ADDR). test_common now sends
`x-development-bypass: true` + `Authorization: Bearer dev-bypass-token`
and parses non-JSON bodies as Null.

Server-side fixes the suite forced out:
1. **SECURITY: dev-bypass token gating.** validate_jwt_token accepts the
   literal `dev-bypass-token` unconditionally — in production a client
   could authenticate as dev-user by just sending it. Both extractors
   (crates/auth + backendapi::auth) now reject that literal as a Bearer
   credential unless development_mode is on (unit-tested both modes).
2. **DELETE /v1/tunnels/{id} 500 (WRONGTYPE).** store_tunnel writes the
   user index `tunnel_lookup:{uid}` as a JSON string, but delete_tunnel
   SREM'd it as a set → every delete 500'd. Now read-modify-write of the
   JSON lookup; also deletes `slot:{fqdn}` so dnsd stops resolving
   immediately. Regression test in backendapi storage_isolation
   (delete_tunnel_cleans_all_indexes, real Redis).
3. **Dev-mode rate limits.** dev-bypass-token registered as a rate-limit
   bypass token when development_mode (suite shares one identity; Free
   tier 60/min starved it). edf-ca per-client cert cap made configurable
   (CaConfig.certs_per_hour_per_client, default 10 unchanged); api-bin
   raises it to 10k in dev mode only — every tunnel create issues a cert
   for the same dev user, so ten tunnels exhausted the production cap.
4. **api k8s Service/Deployment**: dropped the phantom 8081 "metrics"
   port — nothing in api-bin ever listened there; /health and /metrics
   are on 8880. DEVELOPMENT_MODE=true set via alocal patch (dev cluster
   only).

Result: 34/34 integration tests green (single-threaded AND parallel),
full workspace sweep green. Deleted tunnels now free their subdomain for
reuse — previously every delete leaked the subdomain+slot keys until TTL.
