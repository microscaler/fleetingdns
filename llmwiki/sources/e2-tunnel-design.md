---
title: E2 — Tunnel Server & CLI (Design v0.2)
kind: source
status: active
tags: [design, ssh, russh, edgehub, edf-cli, e2]
updated: 2026-04-20
path: ../../docs/engineering/Epic_highlevel/E2-Tunnel_Server_&_CLI_(Design_v0.2).md
related:
  - sources/prd-ephemeral-dns-forwarder-v1.1.md
  - concepts/ssh-reverse-tunnel-protocol.md
  - entities/edgehub.md
  - entities/edf-cli.md
---

# E2 — Tunnel Server & CLI (Design v0.2)

In-repo design doc at
`docs/engineering/Epic_highlevel/E2-Tunnel_Server_&_CLI_(Design_v0.2).md`.
The detailed design that fleshes out the PRD's "reverse-port" semantic.

## Why it lives in the wiki

This design doc is the **primary reference** for any change to the SSH
data plane. Whenever the wiki captures a fact about how `edgehub` or
`edf-cli` *should* behave, it should cite this doc (or the PRD) rather
than re-deriving the spec from code reading.

## Key contracts (per design)

- CLI uses `russh::client` with a server-host-key verifier rooted at
  [`edf-ca`](../entities/edf-ca.md).
- Reverse-port allocation is API-driven; CLI receives the slot from the
  REST control plane (see
  [redis-slot-allocation](../concepts/redis-slot-allocation.md)).
- EdgeHub binds the slot port on accepting the `tcpip-forward` request.
- Inbound TCP at `slot` → `forwarded-tcpip` channel back to CLI →
  `localhost:<local_port>`. See
  [ssh-reverse-tunnel-protocol](../concepts/ssh-reverse-tunnel-protocol.md)
  and [forwarded-tcpip-channel](../concepts/forwarded-tcpip-channel.md).
- TTL + cleanup: when CLI disconnects (or sends `cancel-tcpip-forward`),
  EdgeHub releases the slot listener and the Redis routing entry.

## Gaps vs current code

The design specifies behaviour that the implementation does not yet
satisfy. The
[reverse-tunnel postmortem](./postmortem-reverse-tunnel.md) is the
authoritative gap analysis; that file's R1–R9 enumeration is the
remediation plan.
