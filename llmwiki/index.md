# llmwiki — index

Catalog of wiki pages. **Read [AGENTS.md](./AGENTS.md) first.**

## Entities

### Active

| Page | Summary |
|---|---|
| [edgehub](./entities/edgehub.md) | `crates/edgehub` — Rust crate hosting the SSH server, TLS terminator, and reverse-proxy state. |
| [edgehub-bin](./entities/edgehub-bin.md) | `cmd/edgehub-bin` — the EdgeHub binary; mTLS HTTPS router + SSH listener. |
| [edf-cli](./entities/edf-cli.md) | `cmd/edf-cli` — developer-facing CLI; opens the reverse tunnel from the laptop. |
| [dnsd](./entities/dnsd.md) | `crates/dnsd` + `cmd/dnsd-bin` — stateless DNS authority backed by Redis. |
| [backendapi](./entities/backendapi.md) | `crates/backendapi` + `cmd/api-bin` — REST control plane (`/v1/tunnels`, slot allocation). |
| [edf-ca](./entities/edf-ca.md) | `crates/edf-ca` — short-lived TLS certificate authority for the mTLS tunnels. |
| [redis-fleetingdns](./entities/redis-fleetingdns.md) | Redis instance that holds slot → tunnel routing + per-zone state. |
| [ms02](./entities/ms02.md) | Dev host on which the shared kind cluster + Docker daemon + kind-registry live (Mac SSHes here for Tilt). |
| [shared-kind-cluster](./entities/shared-kind-cluster.md) | Kind cluster `kind-kind` provisioned on `ms02` by cylon-local-infra `roles/kind`. |
| [kind-registry](./entities/kind-registry.md) | `kind-registry` Docker container on ms02 — local image registry mirrored into the kind node. |

## Concepts

### Active

| Page | Summary |
|---|---|
| [ssh-reverse-tunnel-protocol](./concepts/ssh-reverse-tunnel-protocol.md) | RFC 4254 §7: `tcpip-forward` global request + `forwarded-tcpip` channel — **the** primitive for reverse tunnels. |
| [forwarded-tcpip-channel](./concepts/forwarded-tcpip-channel.md) | Server-initiated channel that delivers each accepted external TCP connection back to the CLI. |
| [russh-handle-vs-channelid](./concepts/russh-handle-vs-channelid.md) | Why a long-lived listener task needs `russh::server::Handle`, not a bare `ChannelId`. |
| [tilt-remote-host-pattern](./concepts/tilt-remote-host-pattern.md) | Run Tilt ON ms02 (where Docker + kind + registry live) and forward the UI back to the Mac via `ssh -L`. |
| [redis-slot-allocation](./concepts/redis-slot-allocation.md) | Slot ports come from the API; the CLI MUST NOT fabricate them from UUID bytes. |
| [phase0-accept-any-pubkey](./concepts/phase0-accept-any-pubkey.md) | Short-circuit "accept any pubkey" auth in Phase 0 masks downstream defects (e.g. `SshKeyManager` not used). |
| [nodeport-mappings-ms02](./concepts/nodeport-mappings-ms02.md) | NodePorts the kind cluster exposes via host port-mappings (8880, 3000, 9090, 6379, 5433, …). |
| [mtls-edgehub-tunnel](./concepts/mtls-edgehub-tunnel.md) | CLI ↔ EdgeHub control channel = SSH-inside-mTLS; certs come from `edf-ca` (30-min default TTL). |
| [capability-url-access-model](./concepts/capability-url-access-model.md) | **Decision**: no user-auth gate by default — the unguessable short-lived subdomain (20 base-36 chars ≈ 103 bits) IS the credential. |
| [tunnel-launch-scenarios](./concepts/tunnel-launch-scenarios.md) | The three tunnel-launch contexts (desktop CLI, FAR VM automation, future k8s) and the one-protocol invariant across them. |

### Superseded

_(none yet)_

## Sources

| Page | Origin |
|---|---|
| [karpathy-llm-wiki](./sources/karpathy-llm-wiki.md) | Karpathy gist describing the persistent-wiki pattern (this wiki's blueprint). |
| [readme-fleetingdns](./sources/readme-fleetingdns.md) | In-repo `README.md` — product pitch + reference flows + SDKs. |
| [prd-ephemeral-dns-forwarder-v1.1](./sources/prd-ephemeral-dns-forwarder-v1.1.md) | `docs/prd/Ephemeral-DNS-Forwarder-Product_Requirements_Document_(v1.1).md` — product requirements. |
| [postmortem-reverse-tunnel](./sources/postmortem-reverse-tunnel.md) | `docs/engineering/POSTMORTEM-reverse-tunnel-connectivity.md` — root cause + remediation plan. |
| [tasks-connectivity](./sources/tasks-connectivity.md) | `tasks/connectivity.md` — runtime evidence captured during Phase-0 SSH testing. |
| [kind-tilt-setup](./sources/kind-tilt-setup.md) | `KIND-TILT-SETUP.md` — operator guide for the dev environment. |
| [e2-tunnel-design](./sources/e2-tunnel-design.md) | `docs/engineering/Epic_highlevel/E2-Tunnel_Server_&_CLI_(Design_v0.2).md` — design intent for the tunnel server + CLI. |

## Runs

| Page | Outcome |
|---|---|
| [2026-04-19 reverse-tunnel-debug-instrumentation](./runs/2026-04-19-reverse-tunnel-debug-instrumentation.md) | Postmortem session — instrumented `ssh_server.rs` / `ssh_client.rs` / `tunnel.rs`; H1–H6 evaluated; root cause = direct-tcpip vs tcpip-forward mismatch. |
| [2026-04-20 shared-kind-on-ms02-migration](./runs/2026-04-20-shared-kind-on-ms02-migration.md) | Migrated dev env from local kind to shared kind on ms02; built `kubeconfig_sync.py`, `kind-config.yaml`, updated `Justfile` + `Tiltfile`. |
| [2026-04-20 tilt-on-ms02-pattern](./runs/2026-04-20-tilt-on-ms02-pattern.md) | Pivoted from kubectl-tunnel-from-Mac to running Tilt directly on ms02 with only the UI port (10350) forwarded. Default `MS02_SSH_USER=casibbald`. |

## Categories at a glance

- **Open root causes (active)**: [ssh-reverse-tunnel-protocol](./concepts/ssh-reverse-tunnel-protocol.md) — fix R2/R3 from the postmortem still pending.
- **Active anti-patterns**: [redis-slot-allocation](./concepts/redis-slot-allocation.md) (H5), [phase0-accept-any-pubkey](./concepts/phase0-accept-any-pubkey.md), [russh-handle-vs-channelid](./concepts/russh-handle-vs-channelid.md).
- **Dev environment**: [tilt-remote-host-pattern](./concepts/tilt-remote-host-pattern.md), [shared-kind-cluster](./entities/shared-kind-cluster.md), [nodeport-mappings-ms02](./concepts/nodeport-mappings-ms02.md).
- **Cross-repo**: `ms02` is also documented in the cylon-local-infra wiki, which lives **on the operator's Mac** at `~/Workspace/local/cylon-local-infra/llmwiki/entities/ms02.md` (different machine — no relative link). Defer hardware/OS facts there; this wiki only records FleetingDNS-specific facts.
