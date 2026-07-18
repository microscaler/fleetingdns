---
title: NodePort mappings on ms02
kind: concept
status: active
tags: [kind, nodeport, ms02, networking]
updated: 2026-04-20
sources:
  - sources/kind-tilt-setup.md
related:
  - entities/ms02.md
  - entities/shared-kind-cluster.md
---

# NodePort mappings on ms02

Codified in `kind-config.yaml` at the repo root. The kind node binds
these container ports to the ms02 host so that workloads inside the
cluster are reachable on `ms02:<host-port>` from anywhere on the dev
LAN, without further SSH plumbing.

| Service | NodePort (in cluster) | Host port (on ms02) | Reachable as |
|---|---|---|---|
| Backend API | 30080 | 8880 | `ms02:8880` (api moved off 8080 repo-wide, 2026-07-05) |
| Grafana | 30030 | 3000 | `ms02:3000` |
| Prometheus | 30090 | 9090 | `ms02:9090` |
| Loki | 30310 | 3100 | `ms02:3100` |
| Redis | 30379 | 6379 | `ms02:6379` |
| Postgres | 30432 | 5433 | `ms02:5433` |

(Exact NodePort numbers may differ; check `kind-config.yaml` for the
authoritative spec.)

## Reach-rules

- **From the Mac, on the dev LAN**: hit `ms02:<host-port>` directly. No
  SSH tunnel needed for these services.
- **From inside the cluster**: use the canonical service DNS name
  (`<svc>.<ns>.svc.cluster.local`).
- **Apiserver and kind-registry are NOT in the host-port list.** They
  are bound to `127.0.0.1` on ms02 and reach the Mac only via
  `just kubectl-tunnel-up` (see
  [tilt-remote-host-pattern](./tilt-remote-host-pattern.md)).

## When to add a new mapping

Pattern is to:

1. Edit `kind-config.yaml` to add an `extraPortMappings` entry.
2. Re-create the kind cluster (via the cylon-local-infra Ansible role,
   not by hand).
3. Add the mapping row to this page.
4. Add `just <service>` shortcut(s) in the `Justfile` if it's
   developer-facing.
