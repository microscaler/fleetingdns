# FleetingDNS — agent protocol

> **Desktop dev environment** — before doing anything in this repo, read the
> Microscaler-wide topology brief. It explains that you are on a Mac but the
> code lives on `ms02` (NFS), where commands execute for this environment, how
> the Kind cluster and vLLM fit in, and the network constraints behind the SSH
> tunneling. Do not duplicate its contents here — link to it. If reality drifts,
> fix the canonical doc, not this copy.
>
> - GitHub: [`cylon-local-infra/docs/desktop-dev-environment.md`](https://github.com/microscaler/cylon-local-infra/blob/main/docs/desktop-dev-environment.md)
> - On ms02 NFS: `~/Workspace/microscaler/cylon-local-infra/docs/desktop-dev-environment.md`

---

Welcome, autonomous contributor. This file is the repo-wide protocol for
anything touching `fleetingdns`. The **detailed** agent schema for our
persistent knowledge base lives at [`llmwiki/AGENTS.md`](./llmwiki/AGENTS.md);
read that file before doing non-trivial work.

## Bootstrap order (every new session)

1. Read this file.
2. Read [`llmwiki/AGENTS.md`](./llmwiki/AGENTS.md) (wiki schema + workflows).
3. Skim [`llmwiki/index.md`](./llmwiki/index.md).
4. `tail -30 llmwiki/log.md` to see what happened recently.
5. Read the entity/concept pages closest to the task before touching code.
6. If your task touches the SSH / reverse-tunnel data plane, read
   [`llmwiki/sources/postmortem-reverse-tunnel.md`](./llmwiki/sources/postmortem-reverse-tunnel.md)
   and
   [`llmwiki/concepts/ssh-reverse-tunnel-protocol.md`](./llmwiki/concepts/ssh-reverse-tunnel-protocol.md)
   **first**. Do not re-introduce `direct-tcpip` for reverse forwarding.

## What this repo is

FleetingDNS (FDF / EDF) — ephemeral public DNS endpoints + reverse
tunnels for dev/CI. Product pitch + canonical flows in
[`README.md`](./README.md); product authority in
[`docs/prd/Ephemeral-DNS-Forwarder-Product_Requirements_Document_(v1.1).md`](./docs/prd/Ephemeral-DNS-Forwarder-Product_Requirements_Document_(v1.1).md).

## Repo topology

Rust workspace (`Cargo.toml`). Crates and binaries with dedicated wiki
entity pages are marked `→`.

### Crates (`crates/`)

| Crate | Role | Wiki |
|---|---|---|
| `edgehub` | SSH server + TLS terminator + reverse-proxy state | → [`llmwiki/entities/edgehub.md`](./llmwiki/entities/edgehub.md) |
| `edf-ca` | Ephemeral certificate authority (30-min TTL default) | → [`llmwiki/entities/edf-ca.md`](./llmwiki/entities/edf-ca.md) |
| `dnsd`, `dnsd_backend` | Stateless DNS authority (Redis-backed), DNSSEC signer | → [`llmwiki/entities/dnsd.md`](./llmwiki/entities/dnsd.md) |
| `backendapi` | axum + sea-orm REST control plane (tunnels, quotas, GitHub OAuth) | → [`llmwiki/entities/backendapi.md`](./llmwiki/entities/backendapi.md) |
| `auth` | Auth primitives used by backendapi + edgehub |  |
| `common`, `models` | Shared config, error types, Redis pool, shutdown signal, DDoS config |  |
| `intake_collector`, `feature_pipe`, `ml_scorer`, `feed_grpc`, `feed_webhook` | Honeypot intel pipeline (E11–E14 epics) |  |
| `metrics_client`, `migration`, `test-service`, `bin` | Internal utilities |  |

### Binaries (`cmd/`)

| Binary | Role | Wiki |
|---|---|---|
| `api-bin` | Runs `backendapi` | — |
| `edgehub-bin` | Runs the EdgeHub process (mTLS HTTPS router + SSH listener) | → [`llmwiki/entities/edgehub-bin.md`](./llmwiki/entities/edgehub-bin.md) |
| `edf-cli` | Developer-facing CLI (`edf forward --port 3000`) | → [`llmwiki/entities/edf-cli.md`](./llmwiki/entities/edf-cli.md) |
| `dnsd-bin` | Runs `dnsd` | — |
| `fleetingdns-ctl` | Operator control-plane CLI | — |
| `intake_collector-bin`, `feed_grpc-bin`, `feed_webhook-bin`, `ml_scorer-bin` | Honeypot intel pipeline binaries | — |

### Design docs

- Umbrella PRD: `docs/prd/Ephemeral-DNS-Forwarder-Product_Requirements_Document_(v1.1).md`.
- Epics: `docs/engineering/Epic_highlevel/E{0..19}-*.md` (E2 = Tunnel
  Server & CLI — the design the current code contradicts).
- Stories: `docs/engineering/stories_detailed/`.
- Active postmortems: `docs/engineering/POSTMORTEM-*.md`.
- Deployment / ops: `docs/infra/gitops_crossplane.md`,
  [`KIND-TILT-SETUP.md`](./KIND-TILT-SETUP.md),
  `DOCKER-COMPOSE-TO-KIND-MIGRATION.md`.

### Task / progress trackers

`tasks/*.md` — live task state, connectivity status, production
readiness PRD, e2e broken integration status. New agents should **not**
rewrite these files; instead file an entry in
[`llmwiki/log.md`](./llmwiki/log.md) and a `runs/<YYYY-MM-DD>-<slug>.md`
page describing what was attempted.

## Workflow rules (hard)

1. **All changes tied to a task or a wiki `runs/` page.** If the work is
   a one-off debug or bring-up, record it in
   `llmwiki/runs/YYYY-MM-DD-<slug>.md` per
   [`llmwiki/AGENTS.md`](./llmwiki/AGENTS.md) § "Run".
2. **Commit checklist** (agent must pass all of these before `git
   commit`):
   - `cargo fmt --all`
   - `cargo clippy --all-targets --all-features -- -D warnings`
   - `cargo nextest run --workspace` (or `cargo test --workspace` if
     nextest is not installed)
   - README / design-doc update **only if** a public API, CLI flag, or
     user-visible behaviour changed.
3. **Rust code only in `crates/` and `cmd/`.** Experimental code goes in
   a new `crates/experiments/<name>` module and is accessed via the farm
   CLI. Delete experiments when no longer needed.
4. **TypeScript only in `/ui`** (the portal). Do not write TS anywhere
   else.
5. **No shell scripts.** Any automation that isn't `cargo`, `just`, or a
   standard external tool goes into `scripts/*.py` and is invoked from
   the `Justfile`. Same rule as the sibling cylon-local-infra repo.
6. **Do not touch crates outside the task scope** — use other crates'
   public interfaces. Need a change in another crate? Create a
   dependency task.
7. **Tests must be deterministic.** Anything flaky is a bug.
8. **TDD.** Target ≥ 65% coverage (hard floor) and 80% (goal). New
   public surface ships with tests.
9. **GitHub access via the farm CLI** (see `.cursor/rules/` user-rules).
   Never `--no-verify` / `--no-verify-commit`. Never commit with
   `Co-authored-by: Cursor …` — clients prohibit it.
10. **No secrets in the repo.** Environment via `.env` (decrypted from
    sops by `just decrypt-dev` / `farm env decrypt`). Redact tokens
    before filing anything into `llmwiki/`.

## Dev environment

- **Shared kind cluster on ms02** (context `kind-kind`). Do not run a
  local kind cluster unless you're offline.
- **Tilt runs on ms02**; the Mac forwards only the Tilt UI (`:10350`)
  via `ssh -L`. See
  [`llmwiki/concepts/tilt-remote-host-pattern.md`](./llmwiki/concepts/tilt-remote-host-pattern.md).
- **Default SSH user**: `casibbald@ms02` (cluster owner). Root's
  kubeconfig on ms02 is stale — do not use it.
- **Just recipes** (`just --list`): `up`, `down`, `sync`, `status`,
  `logs`, `remote-exec`, `remote-status`, plus the opt-in
  `kubectl-tunnel-*` / `kubeconfig-sync` flow for local `kubectl` on the
  Mac.

## Known gotchas (captured in llmwiki)

- Local `cargo build -p edgehub` fails on `nightly-2025-06-28` because
  `generic-array 0.14.7` (pinned in `Cargo.lock`) can't resolve
  `crypto_common`/`hmac`/`rfc6979`. Tracked as R1 in the
  [reverse-tunnel postmortem](./docs/engineering/POSTMORTEM-reverse-tunnel-connectivity.md).
- Reverse tunnels don't actually forward bytes end-to-end. This is a
  protocol-layer bug, not a wiring bug — see
  [`llmwiki/concepts/ssh-reverse-tunnel-protocol.md`](./llmwiki/concepts/ssh-reverse-tunnel-protocol.md)
  and plan R1–R9 of the postmortem.
- Phase-0 SSH `auth_publickey` accepts any key. Do not assert identity
  from SSH auth until R8 lands — see
  [`llmwiki/concepts/phase0-accept-any-pubkey.md`](./llmwiki/concepts/phase0-accept-any-pubkey.md).

## Output guidelines

- All public structs, enums, traits documented.
- `tracing::instrument` on runtime-meaningful async fns.
- Log levels: `info` for lifecycle, `warn` for recoverable issues,
  `error` for failures that need operator attention, `debug` for
  development breadcrumbs (compiled out of release if appropriate).
- Prefer `tracing` over `println!`/`eprintln!`.
