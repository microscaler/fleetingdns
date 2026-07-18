---
title: Postmortem — reverse-tunnel connectivity
kind: source
status: active
tags: [postmortem, ssh, russh, reverse-tunnel, root-cause]
updated: 2026-04-20
path: ../../docs/engineering/POSTMORTEM-reverse-tunnel-connectivity.md
related:
  - concepts/ssh-reverse-tunnel-protocol.md
  - concepts/forwarded-tcpip-channel.md
  - concepts/russh-handle-vs-channelid.md
  - concepts/redis-slot-allocation.md
  - concepts/phase0-accept-any-pubkey.md
  - entities/edgehub.md
  - entities/edgehub-bin.md
  - entities/edf-cli.md
  - runs/2026-04-19-reverse-tunnel-debug-instrumentation.md
---

# Postmortem — reverse-tunnel connectivity

In-repo at `docs/engineering/POSTMORTEM-reverse-tunnel-connectivity.md`.
S1 incident: end-to-end reverse tunnels never complete; external HTTP
requests never reach the developer's local service.

## Root cause (one sentence)

**Wire-protocol mismatch**: both CLI and EdgeHub use SSH local
forwarding (`direct-tcpip` channel) where the design requires SSH
remote forwarding (`tcpip-forward` global request + `forwarded-tcpip`
channel). See
[ssh-reverse-tunnel-protocol](../concepts/ssh-reverse-tunnel-protocol.md).

## Contributing factors (summarized)

| # | Factor | Captured as |
|---|---|---|
| H1 | Server `channel_open_direct_tcpip` accepts but never binds a listener | concept: ssh-reverse-tunnel-protocol |
| H2 | CLI drops the `Channel<Msg>` (`_channel` parameter) and only probes localhost | concept: forwarded-tcpip-channel |
| H3 | `start_tunnel_port_listener` has only a `ChannelId` so falls back to a stub HTTP 200 | concept: russh-handle-vs-channelid |
| H4 | CLI never implements `server_channel_open_forwarded_tcpip` | concept: forwarded-tcpip-channel |
| H5 | CLI fabricates `allocated_port` from UUID bytes | concept: redis-slot-allocation |
| H6 | `--addr` and `--ssh-addr` defaults collide on `:8443`; `extract_sni_from_tls` hard-coded `None` | entity: edgehub-bin |
| H7 | CLI generates fresh Ed25519 keypair per run; EdgeHub Phase-0 accepts any key | concept: phase0-accept-any-pubkey |
| H8 | E2E tests assert SSH handshake + Redis registration, **not** HTTP-bytes-to-localhost | entity: edgehub (R9) |

## Remediation plan (postmortem §8)

|| ID | Description | Status |
||---|---|---|
|| R1 | Unblock build (`generic-array 0.14.7` vs nightly-2025-06-28) | **DONE** (2026-04-27) |
|| R2 | Client `tcpip_forward` + `server_channel_open_forwarded_tcpip` handler | **DONE** (2026-04-28) — see run page |
|| R3 | Server `Handler::tcpip_forward` with real listener + `forwarded-tcpip` | **DONE** (2026-04-28) — real TcpListener + copy_bidirectional wired |
|| R4 | Delete orphaned `direct-tcpip` path | **DONE** (2026-04-28) — removed handler, russh defaults to rejecting |
|| R5 | CLI uses API-issued slot | **DONE** (2026-04-28) — SshKeyPair.slot extracted from POST /v1/tunnels, tunnel.rs uses key_pair.slot |
|| R6 | Real edge-router forward in `serve_https_router` | not started |
|| R7 | Fix `--addr` / `--ssh-addr` defaults; SNI sniffing | not started |
|| R8 | CLI uses `SshKeyManager`-issued keypair | not started |
|| R9 | Non-Docker E2E reverse-tunnel test that asserts bytes reach localhost | not started |

## Debug session

`.cursor/debug-c6eef8.log` — 14 NDJSON entries. Source markers:
`static`, `prior-runtime`, `environmental`, `synthesis`. The
[2026-04-19 reverse-tunnel-debug-instrumentation](../runs/2026-04-19-reverse-tunnel-debug-instrumentation.md)
run page recapitulates the hypotheses → evidence mapping.

## Acceptance for "fixed"

`crates/edgehub/tests/e2e_reverse_tunnel_http.rs` (R9) green:

1. Real `SshServer` on `127.0.0.1:0`.
2. Fake echo HTTP server on `127.0.0.1:0` (the dev "local app").
3. CLI `TunnelClient` registers + forwards.
4. `curl http://127.0.0.1:<allocated_port>/anything` reaches the echo
   server and echoed bytes return up the stack.
