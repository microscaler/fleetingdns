---
title: Redis slot allocation (don't fabricate ports)
kind: concept
status: active
tags: [redis, anti-pattern, edf-cli, backendapi, slot, port]
updated: 2026-04-20
sources:
  - sources/postmortem-reverse-tunnel.md
related:
  - entities/backendapi.md
  - entities/edf-cli.md
  - entities/redis-fleetingdns.md
---

# Redis slot allocation (don't fabricate ports)

Allocated tunnel ports are owned by [backendapi](../entities/backendapi.md).
The CLI MUST read the allocated port from the API response, not invent
one. This is hypothesis **H5** of the reverse-tunnel postmortem, and
remediation **R5**.

## The contract

`POST /v1/tunnels` returns a JSON body that includes:

- `slot` / `allocated_port` — the public TCP port the EdgeHub will (or
  should) listen on for inbound traffic for this tunnel.
- `fqdn` — `<subdomain>.fleetingdns.run` mapped to that slot.
- Signed ephemeral cert (PEM) for mTLS.

The slot port is what subsequent calls — DNS resolution, edge-router
routing, the SSH `tcpip-forward` request, and Redis `slot → tunnel`
lookups — must agree on.

## The bug

`cmd/edf-cli/src/tunnel.rs` fabricates the port client-side:

```rust
let allocated_port = 30000 + (uuid_bytes[0] as u32 % 35535);
```

This is wrong on two counts:

1. The number has no relationship to anything in Redis or the API
   response, so even if the SSH wire protocol were correct (it isn't,
   see [ssh-reverse-tunnel-protocol](./ssh-reverse-tunnel-protocol.md)),
   the EdgeHub side would never look up matching state.
2. UUID bytes can collide arbitrarily; there is no allocation
   bookkeeping.

## The fix (R5)

Read `session.slot` from the API response in `tunnel.rs` and pass it as
the `port` argument to `handle.tcpip_forward("0.0.0.0", slot)`. The API
already returns it; the CLI just needs to use it.

## Test

The R9 acceptance test in the postmortem will detect this regression
because the test server-side `curl http://hub:<allocated_port>` would
fail to find a registered route if the CLI told the server about a port
that doesn't match the API's slot.
