---
title: edf-cli
kind: entity
status: active
tags: [cli, ssh, russh, tunnel, edf-cli]
updated: 2026-04-20
sources:
  - sources/postmortem-reverse-tunnel.md
  - sources/tasks-connectivity.md
related:
  - entities/edgehub.md
  - entities/backendapi.md
  - entities/edf-ca.md
  - concepts/ssh-reverse-tunnel-protocol.md
  - concepts/redis-slot-allocation.md
  - concepts/phase0-accept-any-pubkey.md
---

# edf-cli

Developer-facing CLI at `cmd/edf-cli`. Invoked as `edf forward --port
3000` from the laptop; opens the reverse tunnel that lets external
callers reach `localhost:<port>`.

## Today's flow (broken — see postmortem)

1. `POST /v1/tunnels` → [`backendapi`](./backendapi.md) returns slot
   metadata (allocated port, FQDN, signed cert).
2. SSH connect + russh handshake to [`edgehub-bin`](./edgehub-bin.md) on
   `:2222` (or `:8443` depending on env). **Auth succeeds** — Phase-0
   accepts any pubkey, see
   [phase0-accept-any-pubkey](../concepts/phase0-accept-any-pubkey.md).
3. `cmd/edf-cli/src/ssh_client.rs:210` →
   `handle.channel_open_direct_tcpip("127.0.0.1", allocated_port,
   "127.0.0.1", 0)`. **Wrong primitive** — see
   [ssh-reverse-tunnel-protocol](../concepts/ssh-reverse-tunnel-protocol.md).
4. `start_tunnel_data_forwarding(_channel: Channel<Msg>)` — the
   underscore in the parameter is fatal: the channel is dropped.
   Function performs a single 5-second `TcpStream::connect` *probe* of
   `localhost:<local_port>` and returns `Ok(())`, which the CLI
   interprets as "tunnel established".
5. `cmd/edf-cli/src/tunnel.rs` — `allocated_port` is **fabricated**
   client-side (`30000 + uuid_bytes[0] % 35535`) instead of being read
   from the API response. See
   [redis-slot-allocation](../concepts/redis-slot-allocation.md) (H5).

## Open work (from postmortem)

| ID | Action | Status |
|---|---|---|
| R2 | Replace `channel_open_direct_tcpip` with `handle.tcpip_forward(...)` + implement `server_channel_open_forwarded_tcpip` | **DONE** (2026-04-28) |
| R5 | Use `session.slot` from API response in `tunnel.rs` instead of UUID-derived port | **DONE** (2026-04-28) — SshKeyPair.slot extracted from API response, tunnel.rs uses key_pair.slot |
| R8 | Thread `KeyPair` from `SshKeyManager::get_or_request_key_pair` through `TunnelClient::establish_tunnel` | not started |
