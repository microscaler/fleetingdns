---
title: 2026-04-19 — reverse-tunnel debug instrumentation
kind: run
status: active
outcome: partial
tags: [run, debug, ssh, russh, reverse-tunnel]
updated: 2026-04-20
sources:
  - sources/postmortem-reverse-tunnel.md
  - sources/tasks-connectivity.md
related:
  - concepts/ssh-reverse-tunnel-protocol.md
  - concepts/forwarded-tcpip-channel.md
  - concepts/russh-handle-vs-channelid.md
  - concepts/redis-slot-allocation.md
  - concepts/phase0-accept-any-pubkey.md
  - entities/edgehub.md
  - entities/edf-cli.md
---

# 2026-04-19 — reverse-tunnel debug instrumentation

Outcome: **partial**. Diagnosis confirmed by static analysis + prior
runtime evidence; live in-process reproduction blocked by toolchain.

## Context

Continuation of the FleetingDNS reverse-tunnel investigation. Symptom:
SSH handshakes succeed but inbound HTTP never reaches `localhost:3000`
on the developer's laptop. Goal: get from "it doesn't work" to a
mechanically grounded postmortem.

## What we did

- Read the relevant code: `crates/edgehub/src/ssh_server.rs`,
  `crates/edgehub/src/proxy.rs`, `cmd/edgehub-bin/src/main.rs`,
  `cmd/edf-cli/src/ssh_client.rs`, `cmd/edf-cli/src/tunnel.rs`.
- Added NDJSON instrumentation `// #region agent log` blocks at the key
  call-sites listed in the postmortem §7.
- Built `crates/edgehub/tests/debug_reverse_tunnel.rs` — a reproduction
  harness that exercises both the wrong (`direct-tcpip`) and the
  correct (`tcpip-forward`) primitives in-process against a real
  `SshServer`.
- Catalogued evidence into `.cursor/debug-c6eef8.log` (NDJSON, 14
  entries, source markers `static` / `prior-runtime` /
  `environmental` / `synthesis`).
- Wrote `docs/engineering/POSTMORTEM-reverse-tunnel-connectivity.md`
  with the H1–H8 hypothesis table and R1–R9 remediation plan.

## Hypotheses (status)

| ID | Statement | Status | Evidence |
|---|---|---|---|
| H1 | Server `channel_open_direct_tcpip` accepts but never binds | CONFIRMED | static analysis of `ssh_server.rs:1075`; prior live observation in `tasks/connectivity.md` |
| H2 | CLI drops `Channel<Msg>` (`_channel` parameter) | CONFIRMED | static analysis of `ssh_client.rs::start_tunnel_data_forwarding` |
| H3 | `start_tunnel_port_listener` writes stub HTTP 200, has no SSH-channel sink | CONFIRMED | static analysis + `russh::ChannelId` API surface |
| H4 | CLI never overrides `server_channel_open_forwarded_tcpip` | CONFIRMED | static analysis of `SshClientHandler` impl |
| H5 | `allocated_port` fabricated from UUID bytes in CLI | CONFIRMED | static analysis of `tunnel.rs` |
| H6 | `--addr` and `--ssh-addr` defaults collide on `:8443` | CONFIRMED | static analysis of `cmd/edgehub-bin/src/main.rs` |
| H7 | CLI generates fresh Ed25519 per run; EdgeHub Phase-0 accepts any | CONFIRMED | static analysis of `establish_tunnel` + `auth_publickey` callback |
| H8 | E2E tests don't assert HTTP delivery to localhost | CONFIRMED | inspection of `crates/edgehub/tests/e2e_tunnel.rs` |

All eight hypotheses CONFIRMED; the root cause is a single
protocol-layer category error (H1+H4) with the rest as consequences.

## Why "partial"

`cargo build -p edgehub` fails locally on `nightly-2025-06-28` with
`generic-array 0.14.7` not resolving `crypto_common`/`hmac`/`rfc6979`.
`Cargo.lock` pins the broken version. Live in-process reproduction via
`crates/edgehub/tests/debug_reverse_tunnel.rs` is therefore deferred
until R1 of the postmortem lands.

## Concept pages distilled

- [ssh-reverse-tunnel-protocol](../concepts/ssh-reverse-tunnel-protocol.md)
- [forwarded-tcpip-channel](../concepts/forwarded-tcpip-channel.md)
- [russh-handle-vs-channelid](../concepts/russh-handle-vs-channelid.md)
- [redis-slot-allocation](../concepts/redis-slot-allocation.md)
- [phase0-accept-any-pubkey](../concepts/phase0-accept-any-pubkey.md)

## Outputs

- `docs/engineering/POSTMORTEM-reverse-tunnel-connectivity.md`
- `crates/edgehub/tests/debug_reverse_tunnel.rs`
- `.cursor/debug-c6eef8.log`
- Inline `// #region agent log` blocks in:
  - `crates/edgehub/src/ssh_server.rs`
  - `cmd/edf-cli/src/ssh_client.rs`
  - `cmd/edf-cli/src/tunnel.rs`

## Next

R1 (toolchain) → R2/R3 (the real fix) → R4–R9 in postmortem order.
