# FleetingDNS CI/CD Workflows

FleetingDNS runs a single, Octopilot-based pipeline. It is the first microscaler
repo migrated to the Octopilot composite actions; the goal is to converge every
repo on this same shape.

## `octopilot-ci.yml` — the pipeline

The application "shape" is auto-detected from [`skaffold.yaml`](../../skaffold.yaml)
(the source of truth). `octopilot/actions/detect-contexts` reads it and derives
the build matrix, languages, chart paths, and integration matrix — there is no
hand-maintained job list per service.

### Detected shape

Container services (built with the `octopilot/rust` buildpack via
`BP_RUST_PACKAGE`, no Dockerfile):

- `fleetingdns-api` (api-bin)
- `fleetingdns-dnsd` (dnsd-bin)
- `fleetingdns-edgehub` (edgehub-bin)

Plus the `fleetingdns` Helm chart artifact (`chart/fleetingdns`).

CLI tools (`edf-cli`, `fleetingdns-ctl`, `slot-setter`) are deliberately **not**
in `skaffold.yaml`. Octopilot models every skaffold artifact as a container, so
the CLIs are shipped as GitHub Release binaries instead (see `release-binaries`).

### Jobs

| Job | Action | Purpose |
| --- | --- | --- |
| `detect` | `detect-contexts` | Parse `skaffold.yaml` → `pipeline-context` JSON. |
| `lint` | `lint` | Language-aware lint (fmt + clippy). |
| `test` | `test` (matrix) | Per-context tests. |
| `integration-validate` | `integration-validate` | Release build gate + ttl.sh UUID. |
| `integration-artifacts` | `integration-build-artifact` (matrix) | Build + push service images and the OCI chart. |
| `integration-deploy` | `merge-build-results`, `sops-decrypt`, `setup-flux` | Kind cluster; decrypt DB secret; apply ephemeral redis+postgres; reconcile the chart via Flux OCIRepository + HelmRelease. |
| `release-binaries` (tags) | — | Cross-compile the CLIs and attach tarballs + `SHA256SUMS.txt` to the GitHub Release. |
| `release-notes` (tags) | `previous-tag`, `release` | Generate and publish release notes. |

### Triggers

Push to `main`, tags `v*`, pull requests to `main`, and manual dispatch.

## Secrets & config

- `SOPS_AGE_KEY` — shared Flux age private key; used by `sops-decrypt` to
  materialise the DB credentials Secret from
  `deployment-configuration/profiles/dev/fleetingdns/core/runtime/application.secrets.env`.
- `ANTHROPIC_API_KEY` — used by the release-notes generator (tags only).

The age **public** recipient lives in [`.sops.yaml`](../../.sops.yaml); secrets are
encrypted only on ms02 with the shared identity.

## Docker Compose (local + smoke)

`docker-compose.yml` reads DB config and passwords via `${VAR:-default}`
interpolation from the same env contract, not hardcoded values:

- Secrets: `FDNS_DB_PASSWORD`, `DATABASE_URL` (SOPS-decrypted).
- Non-secret config: `FDNS_DB_USER`, `FDNS_DB_NAME`, `REDIS_URL`.

Every var has a dev default, so plain `docker compose up` needs no `.env`. To run
against the real decrypted values, generate a gitignored `.env`
(`just compose-env`, ms02 only) or let the octopilot `sops-decrypt` action export
them into the job environment before `docker compose` runs. See `.env.example`.

## Deploy manifests

- `k8s/deployment/` — Flux `OCIRepository` + `HelmRelease` base (namespace,
  ocirepository, helmrelease).
- `k8s/env/ci` / `k8s/env/kind` — overlays toggling `spec.insecure` and pull policy.
- `k8s/ci-deps/` — ephemeral redis + postgres for the Kind smoke deploy only
  (production redis/postgres are provisioned externally).

## Local equivalents

```bash
# Lint + unit tests (what lint/test run)
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Render the chart the pipeline ships
helm template fleetingdns chart/fleetingdns

# Render the Flux deploy overlay
kubectl kustomize k8s/env/ci
```

## Design notes

See [`docs/engineering/OCTOPILOT-PIPELINE-MIGRATION.md`](../../docs/engineering/OCTOPILOT-PIPELINE-MIGRATION.md)
for the full migration rationale, the container-vs-CLI assessment, and the
proposed upstream extension to `octopilot/actions`.

## Migration history

The previous consolidated `fleetingdns-ci.yml` (bespoke rust-tests /
integration-tests / dns-integration / compose-smoke jobs) has been retired in
favour of this Octopilot pipeline. That workflow itself had consolidated the
older `rust_ci.yml`, `testcontainers.yml`, `compose-ci.yml`, and
`dns-integration.yml`.
