---
title: edgehub-bin
kind: entity
status: active
tags: [edgehub, binary, mtls, https, ssh, sni]
updated: 2026-04-20
sources:
  - sources/postmortem-reverse-tunnel.md
related:
  - entities/edgehub.md
  - concepts/ssh-reverse-tunnel-protocol.md
  - concepts/mtls-edgehub-tunnel.md
---

# edgehub-bin

The EdgeHub binary at `cmd/edgehub-bin`. Glues
[`edgehub`](./edgehub.md)'s SSH server and `proxy` modules into a single
process: terminates inbound TLS on the public address, runs the SSH
listener for CLI tunnels, and (today) returns a stub HTTP body for any
forwarded request instead of actually piping it through the tunnel.

## Bind addresses

- `--addr` and `--ssh-addr` both default to `0.0.0.0:8443`
  (`cmd/edgehub-bin/src/main.rs`). Under the defaults one of the two
  `TcpListener::bind` calls fails at startup (postmortem H6). Recovery
  plan = R7.

## Edge router (placeholder)

`serve_https_router` calls `extract_sni_from_tls(...)` which is
hard-coded to return `None`, then returns either:

- `200` with body `"Tunnel {id} is active for {sni}\n"` if a slot exists
  in Redis, or
- `404 Not Found`.

There is **no** code path that opens a TCP connection to the allocated
slot port and `copy_bidirectional`s the decrypted HTTP into it — see
[ssh-reverse-tunnel-protocol](../concepts/ssh-reverse-tunnel-protocol.md)
+ postmortem R6.

## Open work

| ID | Action |
|---|---|
| R6 | Real forward: lookup `(subdomain → allocated_port)` via `ReverseProxyState`, `TcpStream::connect("127.0.0.1", port)`, `copy_bidirectional` decrypted HTTP into it. |
| R7 | Split `--addr`/`--ssh-addr` defaults; replace `extract_sni_from_tls` with proper `rustls::server::Acceptor::accept` SNI sniffing. |
