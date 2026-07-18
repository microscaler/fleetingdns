---
title: forwarded-tcpip channel
kind: concept
status: active
tags: [ssh, russh, reverse-tunnel, channel, rfc4254]
updated: 2026-04-20
sources:
  - sources/postmortem-reverse-tunnel.md
related:
  - concepts/ssh-reverse-tunnel-protocol.md
  - concepts/russh-handle-vs-channelid.md
  - entities/edgehub.md
  - entities/edf-cli.md
---

# forwarded-tcpip channel

RFC 4254 §7.2 channel type used **by the SSH server** to deliver each
externally-accepted TCP connection back to the client that previously
sent a `tcpip-forward` global request. This is the second half of the
reverse-tunnel primitive; see
[ssh-reverse-tunnel-protocol](./ssh-reverse-tunnel-protocol.md) for the
overall shape.

## Wire fields

`SSH_MSG_CHANNEL_OPEN` with channel type `"forwarded-tcpip"` carries:

- `connected_address: string` — the address bound on the server (matches
  the `"address to bind"` from the original `tcpip-forward` request).
- `connected_port: uint32` — the bound port.
- `originator_address: string` — IP of the external caller hitting the
  bound port.
- `originator_port: uint32` — the external caller's source port.

## russh APIs

- **Server side**: `russh::server::Handle::channel_open_forwarded_tcpip(
  connected_address, connected_port, originator_address,
  originator_port)`. Requires the server to hold an
  `Arc<russh::server::Handle>` cloned from `session.handle()` — see
  [russh-handle-vs-channelid](./russh-handle-vs-channelid.md).
- **Client side**: implement the trait method
  `server_channel_open_forwarded_tcpip` on `impl Handler for
  SshClientHandler`. The default impl just rejects the channel; you
  must override it and `copy_bidirectional` between the channel stream
  and a fresh `TcpStream::connect("127.0.0.1", local_port)`.

## What goes wrong if you skip it

- If the **server** never opens a `forwarded-tcpip` channel, external
  TCP connections to the bound port either hang or are immediately
  closed; the client side sees nothing.
- If the **client** doesn't implement the handler trait method, russh
  rejects the server's open with `OpenFailure`, the server logs the
  rejection, and the external caller's connection is reset.
- If the client implements the handler but **drops the `Channel<Msg>`**
  (the dropped-`_channel` bug in `start_tunnel_data_forwarding`), the
  channel closes and bytes never reach `localhost`.

## Tests to assert this works

- The harness in
  `crates/edgehub/tests/debug_reverse_tunnel.rs` exercises both the
  wrong (`direct-tcpip`) and the correct (`tcpip-forward` +
  `forwarded-tcpip`) primitives in-process against a real `SshServer`.
- The R9 acceptance test in the postmortem: `curl http://server:port` →
  bytes arrive at `localhost:<local_port>` echo server.
