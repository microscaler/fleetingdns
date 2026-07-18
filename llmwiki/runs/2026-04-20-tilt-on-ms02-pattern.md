---
title: 2026-04-20 — Tilt-on-ms02 pattern
kind: run
status: active
outcome: success
tags: [run, tilt, ms02, ssh, dev-environment]
updated: 2026-04-20
sources:
  - sources/kind-tilt-setup.md
related:
  - entities/ms02.md
  - entities/shared-kind-cluster.md
  - entities/kind-registry.md
  - concepts/tilt-remote-host-pattern.md
  - runs/2026-04-20-shared-kind-on-ms02-migration.md
---

# 2026-04-20 — Tilt-on-ms02 pattern

Outcome: **success**. Replaces the kubectl-from-Mac iteration
([2026-04-20 shared-kind-on-ms02 migration](./2026-04-20-shared-kind-on-ms02-migration.md)).

## Context

The kubectl-from-Mac pattern (apiserver+registry SSH tunnels, Tilt on
the Mac) works but is operationally heavy. Tilt only really needs a
docker socket, a kubeconfig, and a registry — all three live natively
on [ms02](../entities/ms02.md). Run Tilt there; forward only the UI.

## What we did

- Rewrote `scripts/kubeconfig_sync.py`:
  - New `remote-tilt-up`: detached `ssh -L 10350:127.0.0.1:10350
    casibbald@ms02 'cd <repo> && exec tilt up --context kind-kind
    --host 127.0.0.1 --port 10350 --stream'`. Captures stdout/stderr to
    `.kube/tilt.log`, pid to `.kube/tilt.pid`. Early-exit detection
    looks for `Tilt started` / `Api server listening` markers in the
    log within 7.5s.
  - New `remote-tilt-down`: SSHes to ms02 to `tilt down --context
    kind-kind`, then SIGTERMs the local SSH process group.
  - New `remote-exec`, `remote-status` helpers (run kubectl/tilt
    commands on ms02).
  - Demoted `tunnel-up` / `tunnel-down` / `fetch` to opt-in
    `kubectl-tunnel-*` / `kubeconfig-sync` recipes — only needed for
    `kubectl` from the Mac.
- Rewrote `Justfile`:
  - `just up` → `sync` (rsync) + `remote-tilt-up`.
  - `just down` → `remote-tilt-down`.
  - Auto-detects `HOST = $(hostname -s)`; if running on ms02, runs
    `tilt up`/`down` directly.
  - `just remote-exec "<cmd>"` and `just remote-status` for
    delegation.
  - `MS02_SSH_USER` default switched from `root` → `casibbald` (the
    cluster owner; root's kubeconfig is stale).
- Updated `Tiltfile` comment header to reflect "this Tiltfile runs on
  ms02".
- Existing `kind-config.yaml` + `allow_k8s_contexts('kind-kind')` +
  `default_registry('localhost:5001')` remain unchanged.

## Verification

| Check | Result |
|---|---|
| `cd /Users/casibbald/Workspace/remote/microscaler/fleetingdns && python3 -m py_compile scripts/kubeconfig_sync.py` | OK |
| `just --list` | parses, lists new recipes |
| `just sync` | rsync to `casibbald@ms02:/home/casibbald/Workspace/microscaler/fleetingdns/` succeeds |
| `just remote-status` | `kind cluster: kind`, `current-context: kind-kind`, `kind-control-plane Ready v1.34.3`, `fleetingdns ns exists, no pods` |
| `ssh casibbald@ms02 'which tilt just python3'` | All three present (`tilt v0.36.3`) |
| Default SSH as `root@ms02` | rejected `kubectl get nodes` (`127.0.0.1:39891 connection refused`) — confirmed root's kubeconfig is stale |
| Default SSH as `casibbald@ms02` | `kubectl get nodes` returns `kind-control-plane Ready v1.34.3` |

## What it costs

- One persistent SSH session per developer (the `tilt up` one), plus a
  short rsync at every `just up`.
- The Mac side has no docker / kubectl / tilt requirement except for
  the optional `kubectl-tunnel-up` flow.

## What it gains

- `docker_build` runs against ms02's local Docker daemon → fast.
- Image pushes go over the local kind Docker network → no SSH framing
  on rebuilds.
- No apiserver tunnel + no SAN headache + no kubeconfig juggling.
- Same `Justfile` works whether you're on the Mac or SSH'd into ms02.

## New / updated concept

[tilt-remote-host-pattern](../concepts/tilt-remote-host-pattern.md) —
captures the rationale + mechanics so future agents don't re-invent the
kubectl-from-Mac iteration.

## Open follow-ups

- `KIND-TILT-SETUP.md` should drop the kubectl-from-Mac section to a
  small "Optional" appendix; today it still leads with that flow.
- `urls` recipe should drop the misleading `(via kubectl port-forward)`
  comments now that NodePorts are first-class.
- `health` / `status` should default to `just remote-exec ...` instead
  of assuming a local kubectl.
