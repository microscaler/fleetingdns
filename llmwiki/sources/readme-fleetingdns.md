---
title: README.md (FleetingDNS product overview)
kind: source
status: active
tags: [readme, product, dns, tunnel, sdk, oauth, webhook]
updated: 2026-04-20
path: ../../README.md
related:
  - entities/edgehub.md
  - entities/edf-cli.md
  - entities/dnsd.md
  - entities/backendapi.md
  - entities/edf-ca.md
  - sources/prd-ephemeral-dns-forwarder-v1.1.md
---

# README.md (FleetingDNS product overview)

In-repo `README.md`. The product pitch + four canonical user-flow
mermaid diagrams + SDK matrix + dev smoke tests.

## Summary

- **Product**: instant, secure, temporary DNS endpoints + reverse
  tunnels for dev/CI. Developer runs `fleetingdns forward --port 3000
  --ttl 1800`; gets back a public `https://abc123.fleetingdns.run`
  endpoint that closes automatically after TTL expires.
- **Solves**: OAuth callback testing, webhook integration testing
  (Stripe, GitHub, Twilio), multi-tenant subdomain routing tests,
  realistic public DNS scenarios for integration tests.
- **Architecture**: developer laptop opens an outbound mTLS tunnel
  (TLS-wrapped SSH) to an EdgeHub PoP; EdgeHub stores `slot → tcp` in
  Redis; the API publishes a stateless DNS label (TTL 30s); inbound
  webhook resolves to EdgeHub anycast IP, EdgeHub forwards over the
  tunnel back to laptop.
- **Auth modes**: Basic, HMAC, OAuth/OIDC. PR #68 added GitHub OAuth
  to [backendapi](../entities/backendapi.md).
- **CA**: ephemeral 30-min TLS certs from [edf-ca](../entities/edf-ca.md);
  see [mtls-edgehub-tunnel](../concepts/mtls-edgehub-tunnel.md).
- **SDKs claimed**: Python, JavaScript, Go, Java, Kotlin, C#,
  TypeScript, Ruby, Swift, Rust. (Most are aspirational — the only ones
  with public package URLs in the README are
  `pypi.org/project/fleetingdns-client`,
  `npmjs.com/package/@fleetingdns/client`,
  `github.com/fleetingdns/sdk-go`.)
- **CI**: `fleetingdns/fleetingdns-action@v1` GitHub Action.
- **Smoke tests** (Prototype 0.1):
  - `cargo run -p dnsd-bin` then `dig @127.0.0.1 -p6353 test.fdns.run
    +short` should return `127.0.0.1`.
  - `cargo run -p edgehub-bin` listens on `0.0.0.0:2222` and logs
    `edgehub listening`.
  - `cargo run -p slot-setter demo 1.2.3.4 --ttl 600` populates Redis.
- **Mermaid flow diagrams** for: hotel-Wi-Fi+GHA+Stripe scenario, dev
  webhook testing, OAuth login flow, multi-tenant subdomain matching.

## Reality check (vs README)

The README describes the product as if the data plane works
end-to-end. As of 2026-04-20 it does not — see
[postmortem-reverse-tunnel](./postmortem-reverse-tunnel.md). The DNS
authority and the API/CA paths do work; the SSH reverse-forwarding
path does not.
