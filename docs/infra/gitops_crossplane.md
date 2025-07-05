### 📋 Crossplane / Upbound Manifest Inventory

*(“What we will declare - before we write the YAML”)*

| #                                             | Kind / CRD                             | Logical Name                   | Provider                 | Scope (Infra / Workload) | Purpose / Notes                                                               |
| --------------------------------------------- | -------------------------------------- | ------------------------------ | ------------------------ | ------------------------ | ----------------------------------------------------------------------------- |
| **Provider plumbing**                         |                                        |                                |                          |                          |                                                                               |
| 1                                             | `ProviderConfig`                       | `gcp-org`                      | `provider-gcp`           | Infra                    | Auth via Workload-Identity; used by all GCP resources.                        |
| 2                                             | `ProviderConfig`                       | `hetzner-hcloud`               | `provider-hcloud`        | Infra                    | Token secret for CX11 CI runner.                                              |
| **Org-/project-level guard rails**            |                                        |                                |                          |                          |                                                                               |
| 3                                             | `OrganizationPolicy`                   | `no-sa-keys`                   | GCP                      | Infra                    | Enforce **no service-account keys** (`iam.disableServiceAccountKeyCreation`). |
| 4                                             | `IAMServiceAccount`                    | `flux-deployer`                | GCP                      | Infra                    | SA bound to infra-cluster Flux via WI.                                        |
| 5                                             | `IAMPolicyMember`                      | `flux-deployer-wi`             | GCP                      | Infra                    | `roles/container.developer` on workload cluster project.                      |
| **Networking / VPC**                          |                                        |                                |                          |                          |                                                                               |
| 6                                             | `GlobalAddress`                        | `glb-ip`                       | GCP                      | Infra                    | Reserved anycast IP for TCP LB.                                               |
| 7                                             | `Network` / `Subnetwork`               | `workload-vpc`                 | GCP                      | Infra                    | Autopilot VPC; peered to infra VPC.                                           |
| 8                                             | `NetworkPeering`                       | `infra<->workload`             | GCP                      | Infra                    | Enable Flux-to-workload API reachability (if push model).                     |
| **DNS**                                       |                                        |                                |                          |                          |                                                                               |
| 9                                             | `DNSManagedZone`                       | `edf-zone` (`edf.run`)         | GCP                      | Infra                    | Authoritative zone for long-lived records.                                    |
| 10                                            | `DNSRecordSet`                         | `wildcard-app` (`*.edf.run`)   | GCP                      | Infra                    | A → global LB IP (managed).                                                   |
| **Artifact & Secret management**              |                                        |                                |                          |                          |                                                                               |
| 11                                            | `ArtifactRegistryRepository`           | `edf-containers`               | GCP                      | Infra                    | gcr.io replacement for Rust & UI images.                                      |
| 12                                            | `SecretManagerSecret`                  | `stripe-webhook-key`           | GCP                      | Infra                    | Injected into workload via CSI.                                               |
| **Data layer**                                |                                        |                                |                          |                          |                                                                               |
| 13                                            | `CloudSQLInstance`                     | `edf-pg` (db-f1-micro)         | GCP                      | Infra                    | User/billing metadata.                                                        |
| 14                                            | `CloudSQLDatabase`                     | `edf-meta`                     | GCP                      | Infra                    | Default DB.                                                                   |
| 15                                            | `CloudSQLUser`                         | `edf-app`                      | GCP                      | Infra                    | Password stored in SM.                                                        |
| 16                                            | `MemcacheInstance` (MemoryStore Redis) | `edf-redis`                    | GCP                      | Infra                    | Slot cache + rate-limit store.                                                |
| **Infra-cluster (management)**                |                                        |                                |                          |                          |                                                                               |
| 17                                            | `GKECluster`                           | `infra-cluster` (Standard)     | GCP                      | Infra                    | Single e2-micro node.                                                         |
| 18                                            | `NodePool`                             | `infra-default`                | GCP                      | Infra                    | Runs Flux + Crossplane controllers.                                           |
| 19                                            | `GitRepository`                        | `infra-flux-sys`               | Kubernetes (in infra)    | Infra                    | Points to `main` branch.                                                      |
| 20                                            | `Kustomization`                        | `crossplane-stack`             | Kubernetes (in infra)    | Infra                    | Installs provider-gcp, provider-hcloud.                                       |
| **Bootstrapped workload-cluster (Autopilot)** |                                        |                                |                          |                          |                                                                               |
| 21                                            | `GKECluster`                           | `workload-cluster` (Autopilot) | GCP                      | Managed by Crossplane    | API/Edge workloads.                                                           |
| 22                                            | `NodePool`                             | `edge-large`                   | GCP                      | Workload                 | Spot e2-standard-4 pool.                                                      |
| 23                                            | `NodePool`                             | `api-small`                    | GCP                      | Workload                 | Autopilot default e2-micro.                                                   |
| 24                                            | `GitRepository`                        | `workload-flux-sys`            | Kubernetes (in workload) | Workload                 | Secondary Flux (pull-in-place).                                               |
| 25                                            | `Kustomization`                        | `edge-stack`                   | Kubernetes (in workload) | Workload                 | Deploy EdgeHub, API, Flagger, Otel.                                           |
| **CI Runner**                                 |                                        |                                |                          |                          |                                                                               |
| 26                                            | `Server` (hcloud.crossplane.io)        | `ci-runner` (CX11)             | Hetzner                  | Infra                    | Boots GitHub Actions runner via cloud-init; managed key pair.                 |
| **Progressive delivery**                      |                                        |                                |                          |                          |                                                                               |
| 27                                            | `HelmRelease`                          | `flagger`                      | Flux-helm                | Workload                 | Installs Flagger & Contour.                                                   |
| 28                                            | `Canary` CR (flagger.app)              | `edgehub-canary`               | K8s                      | Workload                 | 1 %→50 % rollout with latency/5xx checks.                                     |
| **Observability & budgets**                   |                                        |                                |                          |                          |                                                                               |
| 29                                            | `ConfigConnectorMonitoringDashboard`   | `edgehub-latency`              | GCP                      | Infra                    | Publishes p95 latency graph.                                                  |
| 30                                            | `MonitoringAlertPolicy`                | `budget-50pct`                 | GCP                      | Infra                    | Email/Webhook at 50 % of €200 budget.                                         |
| **Bootstrap VM (optional)**                   |                                        |                                |                          |                          |                                                                               |
| 31                                            | `ComputeInstance`                      | `flux-bootstrap-vm` (spot)     | GCP                      | One-shot                 | Day-0 k3s + Flux; `DeletionPolicy: Delete` after success.                     |

---

*Totals:* **31 Crossplane objects** (plus Gatekeeper/OPA ConstraintTemplates and Flux `Kustomizations`).
This list covers every moving part: bootstrap, clusters, data stores, DNS, IAM, CI runner, delivery, and cost-governance, all managed declaratively.


---

### Can Google Cloud Build *be* a GitHub Actions runner?

**Not directly.**
Google Cloud Build is its own CI/CD service; it pulls code (including from GitHub) and runs Cloud Build steps, but it **can’t execute a GitHub Actions workflow file (`.github/workflows/*.yml`)** or register itself as a “self-hosted runner” in the GitHub API.

---

## What you *can* do instead

| Option                                   | How it works                                                                                                                  | Pros                                                                                | Caveats / Cost                                                                                   |
| ---------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| **Switch to Cloud Build triggers**       | Migrate pipelines to `cloudbuild.yaml` (or Cloud Build’s declarative Buildpacks).                                             | Uses pay-per-build seconds; no always-on VM. Free tier gives 120 build-minutes/day. | You’d abandon GitHub Actions features (Pact, CodeQL, matrix, etc.).                              |
| **Ephemeral self-hosted runners on GCP** | Use the *actions-runner-controller* helm chart: each job spins a short-lived pod/node in GKE Autopilot or Cloud Run Jobs.     | Still GitHub Actions; pod lives only for job minutes → spot pricing; no Hetzner VM. | vCPU/GB billed per job (Autopilot \~€0.00024/vCPU-min). Needs controller pod (\~100 m CPU idle). |
| **Cloud Run job wrapper**                | A Cloud Run Job starts the `gh-runner` container with a registration token, runs one workflow, then shuts down.               | No cluster; only pay “vCPU-seconds” + egress.                                       | You implement token retrieval + log upload logic yourself.                                       |
| **Artifact-only approach**               | Leave tests in GitHub Actions (using free minutes), but offload Docker builds to Cloud Build via `gcloud builds submit` step. | Big compilations billed per build second, but still one CI UI.                      | You still need an Action runner, but builds scale out in Cloud Build.                            |

---

### Ball-park cost comparison (per 1 000 CI minutes)

| Scheme                                   | €/1 000 min\* | Notes                                              |
| ---------------------------------------- | ------------- | -------------------------------------------------- |
| **Hetzner CX11 VM**                      | \~€49 flat    | Unlimited mins; fixed even when idle.              |
| Autopilot ephemeral pods (e2-micro spot) | **≈ €5–7**    | 500 vCPU-mins @ €0.00024/vCPU-min.                 |
| Cloud Run Jobs (2 vCPU)                  | **≈ €10–12**  | Pay while container active; includes 360 MB-s RAM. |
| Cloud Build free tier                    | **0**         | First 120 min/day free, then €0.003/vCPU-min.      |

\*excludes image pulls & egress.

---

## Recommended path to “no long-lived VM”

1. **Install `actions-runner-controller` in the Autopilot cluster**

    * A small controller Deployment idles at \~100 m CPU.
    * Each job spins a `RunnerDeployment` pod that deregisters after completion.
    * Spot node-pool scales from 0 → 1 for job duration, then back to 0 h.
    * All managed by Flux; no monthly base cost.

2. **Use Workload Identity** so runner pods pull images & push to Artifact Registry without keys.

3. **Delete the Hetzner CX11 manifest** (and its €49/mo line).

    * OPEX now driven by actual CI usage. Heavy test nights still cheap vs. 24 × 7 VM.

If you’re comfortable migrating *all* workflows to Cloud Build, that’s even cheaper, but most teams keep GitHub Actions for ecosystem (Pact, CodeQL, Renovate) and simply replace the fixed runner with **ephemeral GKE/Cloud Run runners**.

---

### Next steps if you choose runner-controller

* Add provider-gcp `NodePool` with `minNodeCount: 0` and `spot: true`.
* HelmRelease: `flux-system/arc` referencing [https://github.com/actions-runner-controller/chaos-mesh](https://github.com/actions-runner-controller/chaos-mesh).
* Secret: GitHub PAT with “Admin\:repo\_hook” and “repo” scopes.
* Update OPEX: remove €49 Hetzner, add \~€5 buffer for 1 000 runner minutes.

That removes the long-lived bill while keeping the GitHub Actions developer experience intact.
