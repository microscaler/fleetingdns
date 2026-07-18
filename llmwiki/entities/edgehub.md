---
title: edgehub (crate)
kind: entity
status: active
tags: [edgehub, ssh, russh, tls, redis, reverse-tunnel]
updated: 2026-04-20
sources:
  - sources/postmortem-reverse-tunnel.md
  - sources/readme-fleetingdns.md
  - sources/e2-tunnel-design.md
related:
  - entities/edgehub-bin.md
  - entities/edf-cli.md
  - entities/redis-fleetingdns.md
  - concepts/ssh-reverse-tunnel-protocol.md
  - concepts/forwarded-tcpip-channel.md
  - concepts/russh-handle-vs-channelid.md
---

# edgehub

Rust crate at `crates/edgehub`. Hosts the SSH server, the TLS terminator,
the reverse-proxy state, and the certificate manager that the EdgeHub
binary glues together.

## Public surface

- `Config { addr, tls_config, redis_pool }` (`crates/edgehub/src/lib.rs:28`)
- `pub mod ssh_server` — russh-based SSH server. Exports `SshServer` /
  `SshServerState` / `SshSession` / `ReverseProxyState`.
- `pub mod proxy` — TLS/HTTPS edge router (currently shell-only; see
  postmortem R6).
- `pub mod certificate_manager` — runtime certificate provisioning glue.
- `pub mod tls_router` — disabled in tree (`// pub use tls_router::*; //
  Disabled due to compilation issues`, `lib.rs:24`).

## SSH server (the hot spot)

`crates/edgehub/src/ssh_server.rs` implements
`russh::server::Handler for SshSession`. Today it accepts
`channel_open_direct_tcpip` (the wrong primitive for reverse forwarding —
see [ssh-reverse-tunnel-protocol](../concepts/ssh-reverse-tunnel-protocol.md))
and never implements `tcpip_forward`. Key call-sites:

- `channel_open_direct_tcpip` handler at `ssh_server.rs:1075` —
  accepts the channel, calls `handle_reverse_tunnel_registration`.
- `handle_reverse_tunnel_registration` — only inserts metadata into
  `active_tunnels` and spawns an idle `tokio::sleep(30s)` log loop. No
  `TcpListener::bind`, no `register_reverse_tunnel`.
- `start_tunnel_port_listener` — receives a bare `ChannelId`, so it
  cannot push bytes back into the SSH channel and falls back to a stub
  HTTP 200 reply (see
  [russh-handle-vs-channelid](../concepts/russh-handle-vs-channelid.md)).
- `forward_connection_to_ssh_channel` — orphaned helper; never wired.
- Debug-only `tcpip_forward` Handler probe (added during postmortem,
  wrapped in `// #region agent log`).

## Tests

- `crates/edgehub/tests/e2e_tunnel.rs` — verifies SSH handshake +
  Redis registration, **does not** assert HTTP delivery to localhost.
  This shape gap is exactly the bug the postmortem flags (R9).
- `crates/edgehub/tests/debug_reverse_tunnel.rs` — postmortem-era
  reproduction harness. Exercises both the wrong (current) and the
  correct (`tcpip-forward`) primitive in-process.

## Open work (from postmortem)

| ID | Action | Status |
|---|---|---|
| R3 | Implement `Handler::tcpip_forward` on `SshSession` with real `TcpListener` + `forwarded-tcpip` channels + `copy_bidirectional` | **DONE** (2026-04-28) — real TcpListener on allocated port, `forwarded-tcpip` channels + `copy_bidirectional` wired |
| R4 | Delete `direct-tcpip` orphan path | **DONE** (2026-04-28) — removed `channel_open_direct_tcpip` handler |
| R6 | Real edge-router forward in `serve_https_router` (not the stub `"Tunnel {id} is active for {sni}"` body) | not started |
| R9 | E2E reverse-tunnel test that proves bytes reach `localhost` | not started |

## Cross-references

- Sister binary: [edgehub-bin](./edgehub-bin.md).
- Client side: [edf-cli](./edf-cli.md) → currently issues
  `direct-tcpip`; switching to `tcpip-forward` per R2 is the matching
  fix.
- Persistence: [redis-fleetingdns](./redis-fleetingdns.md) holds slot →
  tunnel routing.
