---

## title: tasks/connectivity.md (runtime evidence)
kind: source
status: active
tags: [runtime, evidence, ssh, edgehub, edf-cli]
updated: 2026-04-20
path: ../../tasks/connectivity.md
related:
  - sources/postmortem-reverse-tunnel.md
  - concepts/ssh-reverse-tunnel-protocol.md

# tasks/connectivity.md (runtime evidence)

In-repo at `tasks/connectivity.md`. Captures live observations from
Phase-0 SSH testing; quoted directly in the
[reverse-tunnel postmortem](./postmortem-reverse-tunnel.md) §7.

## Key live observations

- ✅ API tunnel registration: CLI hits `/v1/tunnels`, gets back e.g.
port `54470`, metadata stored in Redis.
- ✅ SSH handshake: russh handshake completes against EdgeHub on
`:2222`.
- ✅ DNS resolution: `dnsd` answers `<slot>.fdns.run` correctly.
- ❌ SSH reverse port forwarding: CLI calls
`channel_open_direct_tcpip` to allocated port → EdgeHub doesn't
listen on the allocated port → "Disconnected" error.
- ❌ Dynamic port listening: EdgeHub allocates ports but never starts
TCP listeners on them. **No TCP listeners on allocated ports (e.g.,
54470).**
- ❌ Tunnel data flow: no bidirectional data forwarding; SSH channels
not connected to allocated ports; local service not reachable.

## Why this misdiagnosed the bug

The doc concluded "we just need to start a TCP listener on the
allocated port." That alone won't work: the listener has no way to push
bytes back into the SSH channel given only a `ChannelId` (see
[russh-handle-vs-channelid](../concepts/russh-handle-vs-channelid.md)).
The actual fix requires switching to the `tcpip-forward` /
`forwarded-tcpip` primitives — see
[ssh-reverse-tunnel-protocol](../concepts/ssh-reverse-tunnel-protocol.md).

## Diagrams

The doc contains two mermaid diagrams: a `graph TB` of the current
broken state and a `sequenceDiagram` of the expected (still-not-working)
flow. Useful to read alongside the postmortem's call-site citations.