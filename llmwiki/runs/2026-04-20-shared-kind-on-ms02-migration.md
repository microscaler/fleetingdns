---
title: 2026-04-20 — shared kind on ms02 migration
kind: run
status: superseded
outcome: success
tags: [run, kind, tilt, ms02, migration, ssh]
updated: 2026-04-20
sources:
  - sources/kind-tilt-setup.md
related:
  - entities/ms02.md
  - entities/shared-kind-cluster.md
  - entities/kind-registry.md
  - concepts/nodeport-mappings-ms02.md
  - runs/2026-04-20-tilt-on-ms02-pattern.md
---

# 2026-04-20 — shared kind on ms02 migration

Outcome: **success**, but **superseded** by the same-day pivot
documented in
[2026-04-20 tilt-on-ms02-pattern](./2026-04-20-tilt-on-ms02-pattern.md)
later that day.

## Context

The reverse-tunnel reproduction harness needs a working dev cluster.
Locally, `kind create cluster` on the Mac was slow, conflicted with
ms02's Docker setup, and failed under the toolchain regression. The
team already runs a kind cluster on
[ms02](../entities/ms02.md) provisioned by cylon-local-infra
`roles/kind`. Decision: don't run our own; reuse it.

## What we did (first iteration — kubectl-from-Mac)

- Wrote `kind-config.yaml` capturing the authoritative spec of the
  shared cluster: node image `kindest/node:v1.34.3`, NodePort
  port-mappings (see
  [nodeport-mappings-ms02](../concepts/nodeport-mappings-ms02.md)), and
  `containerdConfigPatches` mirroring `localhost:5001` →
  `kind-registry:5000` so workloads can pull images we build.
- Wrote `scripts/kubeconfig_sync.py` with three responsibilities:
  - `fetch`: SSH to ms02, run `kind get kubeconfig --name kind`, write
    to `.kube/fleetingdns.kubeconfig` (mode 0600).
  - `tunnel-up` / `tunnel-down`: detached `ssh -N -L
    38839:127.0.0.1:38839 -L 5001:127.0.0.1:5001` with
    `ExitOnForwardFailure=yes`, pid recorded in `.kube/tunnel.pid`.
  - `status`: print the health of the tunnel + kubeconfig.
- Added `Justfile` recipes:
  `setup`, `kubeconfig-sync`, `tunnel-up`, `tunnel-down`,
  `cluster-status`, plus the `up` / `down` / `dev` workflow.
- Added `KUBECONFIG := .kube/fleetingdns.kubeconfig` export so the
  Justfile never clobbers the developer's `~/.kube/config`.
- Updated `Tiltfile`: `allow_k8s_contexts('kind-kind')` +
  `default_registry('localhost:5001')`.
- Re-prefaced `KIND-TILT-SETUP.md` with the new shared-cluster section.
- Demoted the laptop-local kind recipes to `local-*` for offline use.

## What worked

| Outcome | Notes |
|---|---|
| `just kubeconfig-sync` | Pulls casibbald's kubeconfig (apiserver `127.0.0.1:38839`); writes 0600 file. |
| `just tunnel-up` | `ssh -N -L 38839 -L 5001` stays up; pidfile tracked. |
| `kubectl --context kind-kind get nodes` | `kind-control-plane Ready v1.34.3` returned over the tunnel. |
| `curl http://localhost:5001/v2/_catalog` | Returns the kind-registry catalog. |
| Tiltfile parses with `allow_k8s_contexts` | No more "context not allowed" rejection. |

## What was painful

- TLS SAN: the kubeconfig from `kind get kubeconfig` includes
  `127.0.0.1` in the apiserver SAN by default, which works for the
  SSH-`-L` loopback. Anything else would have required `--internal`
  variants and SAN patching.
- Image pushes from the Mac to `localhost:5001` traverse the SSH
  tunnel on every Tilt rebuild — slow and brittle if the SSH session
  flaps.
- Two long-lived SSH sessions per developer (`tilt up` and the
  apiserver/registry tunnel) is operationally heavy.
- The `up` recipe has a hidden ordering: must run `kubeconfig-sync`
  before `tunnel-up` before `tilt up`.

## Why this was superseded

The pain points above all collapse if Tilt itself runs on ms02 (where
Docker, kind, and kind-registry are local). See
[2026-04-20 tilt-on-ms02-pattern](./2026-04-20-tilt-on-ms02-pattern.md)
for the pivot.

## Files left in place

- `kind-config.yaml` — still the authoritative spec.
- `scripts/kubeconfig_sync.py` — kept; the `fetch` /
  `kubectl-tunnel-up` / `kubectl-tunnel-down` paths are now opt-in
  utilities for `kubectl`-from-Mac.
- `Tiltfile` allow-list / default-registry — still required.
