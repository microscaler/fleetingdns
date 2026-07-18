---
title: PRD — Ephemeral DNS Forwarder v1.1
kind: source
status: active
tags: [prd, product, ssh, reverse-port, mtls]
updated: 2026-04-20
path: ../../docs/prd/Ephemeral-DNS-Forwarder-Product_Requirements_Document_(v1.1).md
related:
  - sources/postmortem-reverse-tunnel.md
  - concepts/ssh-reverse-tunnel-protocol.md
  - concepts/mtls-edgehub-tunnel.md
---

# PRD — Ephemeral DNS Forwarder v1.1

In-repo product requirements document at
`docs/prd/Ephemeral-DNS-Forwarder-Product_Requirements_Document_(v1.1).md`.
The authoritative product description; cited in the
[reverse-tunnel postmortem](./postmortem-reverse-tunnel.md) for the
explicit reverse-port semantics requirement.

## Key claims relevant to current work

- "CLI spawns russh client — opens an SSH **reverse-port**
  `0.0.0.0:hub_slot` on **hub**." — i.e. the `tcpip-forward` global
  request semantic, not `direct-tcpip`. This single sentence is the
  unambiguous spec that the current codebase contradicts.
- mTLS tunnel: TLS-wrapped SSH between CLI and EdgeHub.
- API issues 30-minute ephemeral certs.
- Stateless DNS authority (Redis-backed).
- Per-zone rate limits + brute-force protection (per E1h design).
- DoT / certificate pinning (per E1k / E1l designs).

## Companion design docs (in repo)

The PRD is the umbrella; granular designs live under
`docs/engineering/Epic_highlevel/`:

- `E1-Core_DNS_Service_(Design_v0.1).md`
- `E1b_DNS_Architecture_(Stateless_+_Redis)_Design_(v0.1).md`
- `E1c-DNSSEC_for_Stateless_Authority_(Design_v0.1).md`
- `E2-Tunnel_Server_&_CLI_(Design_v0.2).md` — see
  [e2-tunnel-design](./e2-tunnel-design.md).
- `E3-Edge_Proxy_(Design_v0.1).md`
- `E4-Basic_Auth_Redirect_and_Auth_Modes_(Design_v0.2).md`
- `E5-SDK_Integration_(Design_v0.2).md`
- `E6-CI_Integration_GitHub_Action_(Design_v0.1).md`
- `E7-Rate_Limiting_Design(v0.1).md`
- `E8-Security_&_Hardening_(Design_v0.1).md`
- `E9-Api_Key_Strategy_Design_(V0.1).md`
- … and E10–E19 for honeypot intel, ML scoring, billing, scalability.

These are the source-of-truth for product behaviour. The wiki should
**link** to them, not paraphrase them.
