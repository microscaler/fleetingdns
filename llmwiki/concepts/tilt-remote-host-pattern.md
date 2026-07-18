---
title: Tilt-on-remote-host pattern
kind: concept
status: active
tags: [tilt, ms02, ssh, dev-environment, kind]
updated: 2026-07-05
sources:
  - sources/kind-tilt-setup.md
related:
  - entities/ms02.md
  - entities/shared-kind-cluster.md
  - entities/kind-registry.md
  - runs/2026-04-20-tilt-on-ms02-pattern.md
  - runs/2026-04-20-shared-kind-on-ms02-migration.md
---

# Tilt-on-remote-host pattern

Run Tilt on the host where Docker, the kind cluster, and the
kind-registry already live. From the developer's Mac, only the Tilt UI
port is forwarded back via SSH `-L`. This avoids the much heavier
"kubectl-from-Mac" pattern that requires SSH-tunneling the Kubernetes
API server and the registry.

## Why

The first iteration of the shared-cluster migration ran Tilt on the Mac
and forwarded the apiserver (`:38839`) and kind-registry (`:5001`) from
ms02 via two `ssh -L` channels. That worked but was operationally
heavy: it required (a) a separate kubeconfig file pointing at
`127.0.0.1:38839` with a TLS SAN for that loopback address, (b)
keeping the SSH session alive between dev sessions, (c) image pushes
crossing the SSH connection on every `tilt up` rebuild.

Running Tilt on ms02 collapses all that:

- Tilt's `docker_build` talks to ms02's local Docker socket → fast,
  no SSH framing on every build.
- Image pushes go over the kind Docker network on ms02 → no copying
  back to the Mac.
- kubectl uses `casibbald`'s native kubeconfig on ms02 → no
  apiserver tunnel, no SAN headache.
- Mac side only needs SSH and `localhost:10654`.

## No rsync (2026-07-05+)

The repo lives **only on ms02** (`~/Workspace/microscaler/fleetingdns`);
the Mac sees it over NFS at `~/Workspace/remote/microscaler/fleetingdns`.
Edits made on the Mac land directly on ms02's disk, so the old `just
sync` rsync step was removed entirely — `just up` goes straight to SSH.

## Port allocation (2026-07-05+)

ms02 hosts a fleet of Tilt environments as systemd user services
(`~/.config/systemd/user/tilt-*.service`); the UI ports 10348–10353 and
10450 are taken (shared-kind 10348, rerp 10350, sesame-idam 10351,
hauliage 10352, brrtrouter 10353, cylon 10450), and their stacks hold
host ports 3000/3100/4317/4318/8080/9090. FleetingDNS therefore uses:

| Purpose | Port |
|---|---|
| Tilt UI | **10654** |
| api port_forward | 8880 → 8880 (api moved off 8080 repo-wide, 2026-07-05) |
| otel-collector port_forwards | 14317 → 4317, 14318 → 4318 |
| edgehub / redis / postgres port_forwards | 2222 / 6379 / 5432 (free on ms02) |

Check before picking a new port: `ssh casibbald@ms02 'ss -tln'` and
`grep ExecStart ~/.config/systemd/user/tilt-*.service`.

## Mechanics

`scripts/kubeconfig_sync.py remote-tilt-up` (driven by `just up`):

```text
# detached SSH with Tilt UI port-forward (no rsync — repo lives on ms02)
ssh -T -o ExitOnForwardFailure=yes \
    -o ServerAliveInterval=30 -o ServerAliveCountMax=3 \
    -L 10654:127.0.0.1:10654 casibbald@ms02 \
    'cd /home/casibbald/Workspace/microscaler/fleetingdns && \
     exec tilt up --context kind-kind --host 0.0.0.0 --port 10654 --stream'
```

Process lifecycle:

- `ssh` is launched via `subprocess.Popen(..., start_new_session=True)`
  with stdout/stderr redirected to `.kube/tilt.log` and pid recorded in
  `.kube/tilt.pid`.
- `just down` SSHes to ms02 to issue `tilt down --context kind-kind`,
  then SIGTERMs the local SSH session.

## Tiltfile prerequisites

- `allow_k8s_contexts('kind-kind')` — Tilt rejects non-`kind-` contexts
  by default for safety. Adding the context name to the allow-list is
  essential.
- `default_registry('localhost:5001')` — points all `docker_build` pushes
  at the host-published kind-registry port. The kind node's
  `containerdConfigPatches` (in `kind-config.yaml`) mirrors that to
  `kind-registry:5000`.

## Hostname auto-detection

The `Justfile` reads `HOST := \`hostname -s 2>/dev/null || hostname\``.
When `HOST == MS02_HOST` (default `ms02`), `just up` runs Tilt directly
in-process; otherwise it shells out to `scripts/kubeconfig_sync.py
remote-tilt-up`. Same Justfile, both contexts.

## Cluster owner = SSH user

`MS02_SSH_USER` defaults to `casibbald` (not `root`) because the kind
cluster is owned by that user on ms02. SSHing as root yields a stale
kubeconfig pointing at a port that the apiserver never bound (one
manifestation: `connection refused: 127.0.0.1:39891`). See the
[shared-kind-on-ms02 migration](../runs/2026-04-20-shared-kind-on-ms02-migration.md)
run page for the original symptom.

## When to use the kubectl-from-Mac alternative

The apiserver+registry SSH tunnel is now an **opt-in** utility:

```
just kubeconfig-sync       # pull the casibbald kubeconfig
just kubectl-tunnel-up     # ssh -L 38839 + 5001
kubectl get pods -n fleetingdns
just kubectl-tunnel-down
```

Use it when you genuinely need `kubectl` invocations from the Mac (e.g.
local IDE plugins). For everything else, prefer `just remote-exec`.
