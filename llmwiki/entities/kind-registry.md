---
title: kind-registry
kind: entity
status: active
tags: [registry, docker, kind, ms02]
updated: 2026-04-20
sources:
  - sources/kind-tilt-setup.md
related:
  - entities/shared-kind-cluster.md
  - entities/ms02.md
---

# kind-registry

Local Docker registry on [ms02](./ms02.md) that the kind cluster pulls
images from. Implements the standard Kind "local registry" pattern.

| Property | Value |
|---|---|
| Container name (on ms02) | `kind-registry` |
| Image | `registry:2` (Docker Hub) |
| Internal address (kind network) | `kind-registry:5000` |
| Host-published address (ms02) | `127.0.0.1:5001 → 5000/tcp` |
| Mac-side address (only when `just kubectl-tunnel-up` is active) | `localhost:5001` (SSH `-L`) |

## Wiring

`kind-config.yaml` includes `containerdConfigPatches` so the kind node's
containerd treats `localhost:5001` as a mirror of `kind-registry:5000`:

- Tilt builds images and pushes to `localhost:5001` on ms02 (since Tilt
  now runs ON ms02 — see
  [tilt-remote-host-pattern](../concepts/tilt-remote-host-pattern.md)).
- kubelet inside the kind node resolves the same logical name through
  the configured mirror to `kind-registry:5000` on the kind Docker
  network.

## Inspecting

From ms02:

```bash
docker ps --filter name=kind-registry
curl -s http://localhost:5001/v2/_catalog
```

From a Mac with `just kubectl-tunnel-up` running:

```bash
just registry-info   # runs the curl over the SSH tunnel
```
