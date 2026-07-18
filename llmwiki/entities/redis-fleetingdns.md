---
title: redis-fleetingdns
kind: entity
status: active
tags: [redis, slots, tunnel-state, dns, bb8]
updated: 2026-07-05
sources:
  - sources/readme-fleetingdns.md
  - sources/postmortem-reverse-tunnel.md
related:
  - entities/backendapi.md
  - entities/dnsd.md
  - entities/edgehub.md
  - concepts/redis-slot-allocation.md
---

# redis-fleetingdns

The Redis instance that holds:

- **Slot map**: `slot → tunnel metadata` (FQDN, allocated port,
  expiration, originator). Written by [backendapi](./backendapi.md) on
  `POST /v1/tunnels`.
- **DNS records** (stateless authority): looked up by
  [dnsd](./dnsd.md) for `<slot>.fleetingdns.run` resolution.
- **Subdomain → port routing**: should be written by
  [edgehub](./edgehub.md) once the `tcpip_forward` listener binds (R3 of
  the postmortem); today this never happens.

## Where it's wired

- `crates/edgehub/src/lib.rs:35` — `Config.redis_pool:
  common::redis::RedisPool` (bb8 + bb8-redis).
- `crates/dnsd/src/lib.rs:38` — `bb8_redis::RedisConnectionManager` from
  `config.redis.url`.
- `crates/backendapi/src/lib.rs:59` — `TunnelStorage::new(&config.redis_url)`.

## Dev environment

Since 2026-07-05 FleetingDNS **does not deploy its own Redis** in the
local kind workflow. It uses the shared-kind-cluster Redis at
`redis.data.svc.cluster.local:6379` (deployed by the tilt-shared-kind
stack, `shared-kind-cluster/k8s/platform-data/data/cache/`; NodePort
30379 → host ms02:6379). The alocal overlay patches
(`k8s-tilt/clusters/workload/alocal/patches/*-shared-services.patch.yaml`)
rewrite `REDIS_URL` for all three services and edgehub's `--redis` CLI
arg (the flag takes precedence over the env var).
The legacy docker-compose config pinned `redis:6379`; the Crossplane
manifests in `k8s-tilt/infra/databases/` are GCP-only.

## Caveats

- The `slot-setter` helper (`README.md`) lets you populate the slot map
  manually during dev: `cargo run -p slot-setter demo 1.2.3.4 --ttl 600`.
- There is a tracked `dump.rdb` in the repo root; should be `.gitignore`d
  but currently isn't.
