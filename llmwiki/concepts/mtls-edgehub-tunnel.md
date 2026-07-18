---
title: mTLS EdgeHub tunnel
kind: concept
status: active
tags: [mtls, tls, ssh, edgehub, edf-ca, security]
updated: 2026-04-20
sources:
  - sources/readme-fleetingdns.md
  - sources/prd-ephemeral-dns-forwarder-v1.1.md
related:
  - entities/edgehub.md
  - entities/edgehub-bin.md
  - entities/edf-ca.md
  - concepts/phase0-accept-any-pubkey.md
---

# mTLS EdgeHub tunnel

The CLI ↔ EdgeHub control channel is **SSH wrapped in mTLS**. Both ends
present X.509 certificates from [`edf-ca`](../entities/edf-ca.md); the
SSH session runs inside the established TLS tunnel.

## Why mTLS first, then SSH

- TLS gives the **edge** a way to identify the tunnel client at the L7
  router boundary without parsing SSH packets. The same listener can
  serve HTTPS traffic for end users and tunnel control traffic for
  developers, distinguished by ALPN/SNI.
- SSH gives the **session** russh-native primitives for `tcpip-forward`
  + `forwarded-tcpip` (see
  [ssh-reverse-tunnel-protocol](./ssh-reverse-tunnel-protocol.md)),
  including built-in framing, multiplexing, and per-channel flow control.

## Cert provenance

- CLI receives a 30-min ephemeral cert (PEM, in-memory) from
  `POST /v1/tunnels` (issued by [`edf-ca`](../entities/edf-ca.md) via
  [`backendapi`](../entities/backendapi.md)).
- EdgeHub server presents its own server cert chain (also rooted at
  `edf-ca` for dev; production may pin a public CA).
- TTL = `DEFAULT_CERT_TTL` (30 min) by default; max
  `MAX_CERT_TTL` (2 h).

## Current shortcuts that mask defects

- Phase-0 accepts any SSH public key after TLS — see
  [phase0-accept-any-pubkey](./phase0-accept-any-pubkey.md). This means
  a cert from `edf-ca` is checked at the TLS layer but **not at the SSH
  auth layer**, so the SSH key can be anything.
- The HTTPS edge router never opens a tunnel data path (returns a stub
  body). Even with a perfect mTLS handshake, end-to-end forwarding does
  not happen — see
  [ssh-reverse-tunnel-protocol](./ssh-reverse-tunnel-protocol.md).

## What still needs verification

- That `edf_ca::CertificateAuthority::issue` paths in `backendapi`
  produce certs whose SAN includes the assigned FQDN
  (`<slot>.fleetingdns.run` or `<wildcard>.<subdomain>.fleetingdns.run`
  for tenant routing).
- That the EdgeHub `rustls::ServerConfig` enforces
  `with_client_cert_verifier` against the same CA. (Today the codebase
  has both `tls_router` (disabled) and the bare proxy module — verify
  which is actually consulted.)
