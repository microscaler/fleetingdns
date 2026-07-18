---
title: backendapi
kind: entity
status: active
tags: [api, axum, sea-orm, oauth, github, rate-limiting, quota]
updated: 2026-04-20
sources:
  - sources/readme-fleetingdns.md
  - sources/postmortem-reverse-tunnel.md
related:
  - entities/edf-cli.md
  - entities/edf-ca.md
  - entities/redis-fleetingdns.md
  - concepts/redis-slot-allocation.md
---

# backendapi

REST control plane. Crate `crates/backendapi` + binary `cmd/api-bin`.
Built on `axum` + `sea-orm`. Issues tunnel slots, signs ephemeral
certificates via [edf-ca](./edf-ca.md), persists tunnel metadata in
[redis-fleetingdns](./redis-fleetingdns.md), and holds the
rate-limit/quota machinery.

## Public surface

`ApiState { config, ca, storage, github_client, rate_limiter, db,
quota_enforcer }` (`crates/backendapi/src/lib.rs:33-42`):

- `ca: Arc<edf_ca::CertificateAuthority>` — issues 30-min PEM certs.
- `storage: Arc<TunnelStorage>` — Redis-backed tunnel metadata.
- `github_client: reqwest::Client` — for OAuth login (PR #68 added GitHub
  OAuth).
- `rate_limiter: Arc<RateLimitState>` — generic per-user/per-IP buckets.
- `quota_enforcer: Arc<ServicePlanRateLimiter>` — Stripe-tier quotas.
- `db: DatabaseConnection` — sea-orm to Postgres.

Re-exported helpers: `ApiError`, `ApiResult`, `ApiConfig`,
`error_handler_middleware`, `error_recovery_middleware`,
`request_size_middleware`, `timeout_middleware`, `CircuitBreaker`,
`RateLimitConfig`, `RateLimitState`.

## Lifecycle

`run()` → `ApiConfig::from_env()` → `run_with_config(config)` → init CA →
init `TunnelStorage` from `config.redis_url` → spin up axum router on
`config.bind_address`.

## Slot allocation contract

`POST /v1/tunnels` returns the allocated slot (port) in its response.
The CLI MUST use that value when issuing the SSH `tcpip-forward` request
— see [redis-slot-allocation](../concepts/redis-slot-allocation.md). The
fact that `edf-cli` currently fabricates the port from UUID bytes
(postmortem H5 / R5) is **a CLI bug**, not an API gap.
