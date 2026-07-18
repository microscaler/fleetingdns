---
title: edf-ca
kind: entity
status: active
tags: [ca, tls, mtls, x509, ephemeral, rcgen]
updated: 2026-04-20
sources:
  - sources/readme-fleetingdns.md
related:
  - entities/backendapi.md
  - concepts/mtls-edgehub-tunnel.md
---

# edf-ca

FleetingDNS Certificate Authority. Crate at `crates/edf-ca`. Issues
short-lived (default 30-minute) X.509 certificates used for client
authentication in the SSH-over-TLS tunnel.

## Public surface

- `CaConfig { ca_name, organization, organizational_unit, default_ttl,
  max_ttl, ca_key_path, ca_cert_path }`
  (`crates/edf-ca/src/lib.rs:32-47`).
- `CertificateAuthority` (re-exported from `ca` module).
- `EphemeralCertificate`, `CertificateRequest`, `CertificateMetadata`
  (re-exported from `certificate`).
- `CaError`.
- Constants: `DEFAULT_CERT_TTL = 30 min`, `MAX_CERT_TTL = 2 h`.
- Submodules: `batch_operations`, `ca`, `certificate`, `errors`.

## Defaults (`CaConfig::default`)

| Field | Value |
|---|---|
| `ca_name` | `"FleetingDNS-CA"` |
| `organization` | `"FleetingDNS"` |
| `organizational_unit` | `"Tunnel Services"` |
| `default_ttl` | 30 min |
| `max_ttl` | 2 h |
| `ca_key_path` / `ca_cert_path` | `None` → CA generated on first run |

## Where it's used

- [`backendapi`](./backendapi.md) creates one as
  `Arc<edf_ca::CertificateAuthority>` at startup.
- The signed PEM is returned from `POST /v1/tunnels` and held in-memory
  by the CLI.
