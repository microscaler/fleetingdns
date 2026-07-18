---
title: shared-kind-cluster
kind: entity
status: active
tags: [kind, kubernetes, ms02, dev-cluster]
updated: 2026-07-05
sources:
  - sources/kind-tilt-setup.md
related:
  - entities/ms02.md
  - entities/kind-registry.md
  - concepts/nodeport-mappings-ms02.md
  - concepts/tilt-remote-host-pattern.md
---

# shared-kind-cluster

The Kind cluster that lives on [ms02](./ms02.md). All FleetingDNS
development against Kubernetes targets this cluster.

| Property | Value |
|---|---|
| Kind cluster name | `kind` |
| Kubernetes context | `kind-kind` |
| Owner user (on ms02) | `casibbald` (cluster ownership; also has Docker group) |
| Node image | `kindest/node:v1.34.3` |
| API server bind | `127.0.0.1:38839` (loopback on ms02; reach from Mac via `just kubectl-tunnel-up`) |
| Provisioned by | cylon-local-infra `roles/kind` (`playbooks/dev_hosts.yml`) |

## Cluster spec source of truth

`kind-config.yaml` at the repo root. Captures:

- `extraPortMappings` for the NodePorts in
  [nodeport-mappings-ms02](../concepts/nodeport-mappings-ms02.md).
- `containerdConfigPatches` mirroring `localhost:5001` →
  `kind-registry:5000` so workloads can pull images that Tilt builds and
  pushes to the host-published kind-registry.

## Bringing it up / re-creating it

The cluster is provisioned by the cylon-local-infra Ansible role; do not
run `kind create cluster` directly on ms02 unless the role has changed.
For a clean re-create, drive it via the cylon-local-infra playbook.

## Shared services consumed by FleetingDNS (2026-07-05)

The tilt-shared-kind stack (UI :10348) owns cluster-wide infra that
FleetingDNS consumes instead of deploying its own:

| Service | In-cluster address | Host (ms02) |
|---|---|---|
| Redis | `redis.data.svc.cluster.local:6379` | :6379 (NodePort 30379) |
| PostgreSQL (primary) | `postgres.data.svc.cluster.local:5432` (postgres/postgres) | :5433 (NodePort 30432) |
| OTel collector | `otel-collector.observability.svc.cluster.local:4317` | — |
| Grafana / Prometheus / Loki / Jaeger | `observability` namespace | Grafana NodePort 31300 |

FleetingDNS's `fdns` database is created by the one-shot `fdns-db-init`
Job (Tiltfile blob) against the shared postgres. The alocal overlay's
`*-shared-services.patch.yaml` files point api/dnsd/edgehub at these
addresses.

## Day-to-day from a Mac

- `just remote-status` → `kind get clusters; kubectl config current-context;
  kubectl get nodes; kubectl -n fleetingdns get pods; tilt get resources`.
- `just up` → ssh + tilt up (see
  [tilt-remote-host-pattern](../concepts/tilt-remote-host-pattern.md)).
