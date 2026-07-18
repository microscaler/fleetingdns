---
title: KIND-TILT-SETUP.md (operator guide)
kind: source
status: active
tags: [tilt, kind, ms02, operator, dev-environment]
updated: 2026-04-20
path: ../../KIND-TILT-SETUP.md
related:
  - concepts/tilt-remote-host-pattern.md
  - concepts/nodeport-mappings-ms02.md
  - entities/ms02.md
  - entities/shared-kind-cluster.md
  - entities/kind-registry.md
  - runs/2026-04-20-tilt-on-ms02-pattern.md
---

# KIND-TILT-SETUP.md (operator guide)

In-repo at `KIND-TILT-SETUP.md`. Covers the dev environment workflow
against the shared kind cluster on [ms02](../entities/ms02.md).

## Today's workflow (top of the doc)

`just up` from the Mac:

1. rsync the working copy to
   `casibbald@ms02:/home/casibbald/Workspace/microscaler/fleetingdns/`.
2. `ssh -L 10350:127.0.0.1:10350 casibbald@ms02 'cd <repo> && tilt up
   --context kind-kind --host 127.0.0.1 --port 10350 --stream'`.
3. Open `http://localhost:10350`.

The detailed mechanics are codified in `Justfile` +
`scripts/kubeconfig_sync.py`; see
[tilt-remote-host-pattern](../concepts/tilt-remote-host-pattern.md).

## NodePort mappings

The doc enumerates the host port-mappings exposed by `kind-config.yaml`.
Captured as the dedicated concept page
[nodeport-mappings-ms02](../concepts/nodeport-mappings-ms02.md).

## Optional kubectl-from-Mac

`just kubectl-tunnel-up` opens `ssh -L 38839:127.0.0.1:38839
-L 5001:127.0.0.1:5001 casibbald@ms02` so a local `kubectl` (with the
fetched kubeconfig) can talk to the apiserver and the kind-registry.
Only useful for local IDE plugins or `kubectl` commands you can't
delegate to `just remote-exec`.

## Legacy "laptop-local kind" recipes

Retained as `local-*` recipes in the `Justfile` for offline /
emergency use. They create a kind cluster on the developer's Mac and
do **not** interact with ms02. Prefer the shared cluster.
