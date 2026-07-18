---
title: dnsd
kind: entity
status: active
tags: [dns, dnssec, redis, ddos, stateless]
updated: 2026-04-20
sources:
  - sources/readme-fleetingdns.md
related:
  - entities/redis-fleetingdns.md
---

# dnsd

Crate `crates/dnsd` + binary `cmd/dnsd-bin`. Stateless DNS authority that
serves `<slot>.fleetingdns.run` answers by looking up slot → IP from
[redis-fleetingdns](./redis-fleetingdns.md).

## Public surface

- `Config { addr, redis_pool, ddos_config, enable_ddos_protection,
  dnssec_config, performance_config }`
  (`crates/dnsd/src/lib.rs:14-28`).
- `pub async fn serve(cfg: Config) -> AppResult<()>`
  (`crates/dnsd/src/lib.rs:50`).
- Submodules: `dns_handler`, `metrics_manager`, `response_compression`,
  `sign` (DNSSEC).

## Defaults

- Bind address: `0.0.0.0:6353` (per `README.md` Prototype 0.1; production
  uses 53).
- Backed by `common::config::FleetingDnsConfig::from_env()`.
- DDoS protection via `common::ddos_protection::DdosConfig` — opt-in via
  `enable_ddos_protection`.
- DNSSEC signer initialised at start when `enable_signature_cache=true`
  (`crates/dnsd/src/lib.rs:54`).

## Smoke test (from README)

```bash
cargo run -p dnsd-bin
dig @127.0.0.1 -p6353 test.fdns.run +short
# → 127.0.0.1
```

## Cross-references

- Slot data is owned by [backendapi](./backendapi.md) +
  [redis-fleetingdns](./redis-fleetingdns.md). `dnsd` is read-only on the
  data plane.
