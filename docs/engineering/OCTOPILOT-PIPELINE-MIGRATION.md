# Octopilot pipeline migration (FleetingDNS)

Reworks the FleetingDNS CI/CD onto the Octopilot composite actions
(`octopilot/actions`), modelled on the reference Rust consumer
`octopilot/cronjob-log-monitor`. Full adoption: detect → lint/test → build →
Helm-OCI deploy, with SOPS secret decryption.

## 1. How Octopilot detects application "shape"

`detect-contexts` parses **`skaffold.yaml`** as the source of truth. For each
`build.artifacts[]` entry it detects the language from a marker file in the
artifact's `context` (`Cargo.toml` → **rust**, version from `rust-toolchain`),
and `build_method` = `docker` iff a file literally named `Dockerfile` exists in
that context, else Paketo `pack`. Helm charts are found separately by walking
for `Chart.yaml`. It emits a `pipeline-context` JSON (`matrix`, `languages`,
`versions`, `chart_paths`, `integration_matrix`) that lint/test/build/deploy
jobs key off. **No `skaffold.yaml` ⇒ Octopilot detects nothing and builds
nothing.**

## 2. Assessment: container-packaged binaries vs CLI release binaries

**Gap (confirmed in `octopilot/actions`):** `detect-contexts` and the build
actions treat *every* `skaffold.yaml` artifact as a container image (or a
`-chart` OCI artifact). There is **no first-class notion of a "release binary"**
— a CLI that should be cross-compiled and attached to a GitHub Release rather
than shipped as a container. The reference consumer (`cronjob-log-monitor`)
handles its single binary as a container and bolts on a plain `build-release`
job (`cargo build --release` + `softprops/action-gh-release`) for the artifact;
there is no shared action, no cross-compilation matrix, and no link between
`detect-contexts` output and the release path.

FleetingDNS makes this gap concrete — its workspace is a **mix**:

| cmd crate | Kind | Pipeline treatment |
|-----------|------|--------------------|
| `api-bin` | service | **container** (`docker/Dockerfile.api`) |
| `dnsd-bin` | service | **container** (`docker/Dockerfile.dnsd`) |
| `edgehub-bin` | service | **container** (`docker/Dockerfile.edgehub`) |
| `feed_grpc-bin`, `feed_webhook-bin`, `intake_collector-bin`, `ml_scorer-bin` | future services (empty stubs) | container later; excluded for now |
| `edf-cli` | **CLI** (developer `fleetingdns forward …`) | **release binary** (cross-compiled, attached to release) |
| `fleetingdns-ctl` | **CLI** (admin/control) | **release binary** |
| `slot-setter` | **CLI** (dev/CI utility) | **release binary** (or CI-only) |

### Proposed convention (consumer-side now, Octopilot-side later)

**Now (this repo):** keep the two paths cleanly separated:
- `skaffold.yaml` lists **only container artifacts** (api, dnsd, edgehub) + the
  `-chart` artifact. CLI crates are deliberately **absent** from skaffold, so
  `detect-contexts` never containerises them.
- A dedicated **`release-binaries` job** (tags only) cross-compiles the CLI
  crates for `x86_64`/`aarch64` × linux/darwin and attaches the archives to the
  GitHub Release created by the Octopilot `release` action. This mirrors, but
  generalises, `cronjob-log-monitor`'s `build-release`.

**Later (propose upstream to `octopilot/actions`):** teach Octopilot the
distinction so consumers don't hand-roll it. Two viable designs:
1. **skaffold annotation** — read an artifact label such as
   `artifact.metadata.octopilot/kind: release-binary` (or a top-level
   `release.binaries: []` block) in `detect-contexts`, and emit a
   `release_matrix` alongside `integration_matrix`. Add a `build-release`
   composite action consuming it (toolchain from the same rust-version
   detection, `cross`/`cargo` build, upload).
2. **Cargo metadata** — read `[package.metadata.octopilot] release-binary =
   true` from each crate. Keeps the signal next to the code, no skaffold
   coupling.

Recommendation: (1) — skaffold is already Octopilot's shape source of truth, so
keeping the container/release split in one file is the least surprising. A short
RFC to `octopilot/actions` (new `detect-contexts` output `release_matrix` + a
`build-release` action) is filed as a follow-up; until it lands, the
consumer-side `release-binaries` job below is the bridge.

## 3. Target pipeline (this branch)

Mirrors `cronjob-log-monitor/.github/workflows/ci.yml`:

```
detect (detect-contexts)
 ├─ lint            (octopilot/actions/lint)         if languages != []
 ├─ test  [matrix]  (octopilot/actions/test)         per pipeline-context.matrix
 ├─ integration-validate                              smoke the primary service
 ├─ integration-artifacts [matrix over integration_matrix]  build images + -chart to ttl.sh
 ├─ integration-deploy                                merge-build-results → Kind + setup-flux + HelmRelease OCI
 ├─ release-binaries (tags)                           cross-compile CLIs → attach to release   ← the gap fix
 └─ release-notes (tags)                              previous-tag + release (Anthropic)
```

Secrets: an early **`sops-decrypt`** step (or per-job) decrypts
`deployment-configuration/profiles/dev/fleetingdns/core/**/application.secrets.env`
using `age_key: ${{ secrets.SOPS_AGE_KEY }}` (already set on the repo; matches
the `.sops.yaml` recipient). This is what unblocks the currently-failing
integration tests (services couldn't get DB credentials).

## 4. Deliverables

1. **`skaffold.yaml`** — 3 container artifacts (api/dnsd/edgehub, each a Docker
   build) + 1 `-chart` artifact; contexts carry `Cargo.toml` (workspace root) so
   Rust is detected. CLIs excluded (see §2).
2. **Helm chart** (`chart/`) — the k8s-tilt Kustomize deployments/services for
   api/dnsd/edgehub converted to a single chart, OCI-packageable, deployable via
   Flux HelmRelease (matching cronjob's `chart/` + `k8s/deployment` OCIRepository
   + HelmRelease, with `ci`/`kind` overlays).
3. **`.github/workflows/octopilot-ci.yml`** — the pipeline above.
4. **`SOPS_AGE_KEY`** repo secret — **done** (flux-shared-gitops age private key).
5. **Upstream RFC** to `octopilot/actions` for the release-binary distinction —
   follow-up.

## 5. Open decisions / notes

- The existing `fleetingdns-ci.yml` stays until the Octopilot workflow is green,
  then is retired.
- The ML stub services are excluded from skaffold until they do something.
- Helm chart vs the existing `deployment-configuration` (RERP Kustomize
  generators): the chart carries the Deployments/Services; the SOPS-generated
  Secrets/ConfigMaps continue to come from `deployment-configuration`
  (Kustomize `secretGenerator`), referenced by the chart via `secretKeyRef`.
