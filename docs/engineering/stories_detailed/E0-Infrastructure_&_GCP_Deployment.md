# 📗 **E0 – Infrastructure & GCP Deployment**

*Epic → User‑story breakdown (v0.1)*

---

## Epic Goal

> “Stand up a secure, multi‑project GCP landing zone with dual clusters (infra & workload EU) plus automated bootstrap, IAM federation, budget guard rails, and Flux‑managed Crossplane providers — all codified in GitOps.”

---

## 🗂️ Story List

| ID         | Story (As a…)       | Outcome (So that…)                                                                        |
|------------|---------------------|-------------------------------------------------------------------------------------------|
| **E0‑S1**  | *Org admin*         | Create **two GCP projects** (`infra`, `workload-eu`) under org & attach billing.          |
| **E0‑S2**  | *Platform engineer* | Have **VPCs + peering** between infra/workload projects to enable Flux push/pull.         |
| **E0‑S3**  | *Platform engineer* | Provision **Standard GKE infra-cluster** with single e2‑micro via Crossplane.             |
| **E0‑S4**  | *Dev lead*          | Bootstrap **Flux + Crossplane** on infra cluster auto‑matically (Day‑0 VM self‑destruct). |
| **E0‑S5**  | *Platform engineer* | Provision **Autopilot workload cluster** + spot node‑pool.                                |
| **E0‑S6**  | *SecOps*            | Enforce **Workload Identity Federation** (GitHub OIDC → GCP) — no SA keys.                |
| **E0‑S7**  | *Finance*           | **Budget alert** at €200, 50/90 % e‑mail + Slack webhook.                                 |
| **E0‑S8**  | *Infra Dev*         | Reserve **global anycast IP & DNS zone** (`*.fleetingdns.run`).                           |
| **E0‑S9**  | *DB admin*          | Stand up **Cloud SQL (db‑f1‑micro)** + initial DB & user.                                 |
| **E0‑S10** | *Ops*               | Deploy **MemoryStore Redis 1GB** instance.                                                |
| **E0‑S11** | *NetOps*            | Create **forwarding rules** (TCP 80/443 & UDP 51820) on the LB.                           |

---

### Story template below

---

## E0‑S1 — Org & Projects

**Tasks**

1. Authorize CLI account with org‑level owner.
2. Run Day‑0 shell script (`scripts/day0_bootstrap.sh`).
3. Commit project IDs to `infra/providers/providerconfig-gcp.yaml`.

**Functional Req**

* Two distinct projects exist: `fleetingdns-infra`, `fleetingdns-workload-eu`.
* Billing account linked; APIs enabled.

**Non‑Functional Req**

* Idempotent script rerun ≤ 60 s.
* CI lint job ensures project IDs are globally unique.

---

## E0‑S2 — VPC & Peering

**Tasks**

1. Author VPC manifests (`gcp-core/vpc-*.yaml`).
2. Apply via Flux; verify with `gcloud compute networks peerings list`.

**Functional**

* `infra-vpc` & `workload-vpc` auto‑peered.
* Private RFC1918 ranges do not overlap.

**Non‑Functional**

* RTT across peering ≤ 5ms.
* Terraform / manual edits blocked by policy label `managed-by=flux`.

---

## E0‑S3 — Infra Cluster

**Tasks**

1. Add `clusters/infra-cluster.yaml` XR.
2. NodePool spec (`infra-nodepool-default.yaml`).
3. Kustomize overlay.

**Functional**

* GKE Standard cluster up with node count =1.
* Private nodes enabled.

**Non‑Functional**

* Control‑plane SLA 99.5%.
* Monthly cost ≤ €70 credit.

---

## E0‑S4 — Flux + Crossplane Bootstrap

**Tasks**

1. Spot VM YAML in `bootstrap/` applied manually.
2. VM installs k3s + Flux; commits provider installs.
3. VM auto deletes after `BootstrapSucceeded` CR.

**Functional**

* Flux `Ready=True` within 10min.
* Provider‑gcp `Healthy=True`.

**Non‑Functional**

* Bootstrap log archived to GCS bucket.
* Spot VM cost ≈ €0.02.

---

## E0‑S5 — Workload Autopilot Cluster

**Tasks**

1. XR `workload-cluster-eu.yaml`.
2. Spot node‑pool YAML.
3. Verify Workload Identity enabled.

**Functional**

* Autopilot cluster ready; default namespace cost = free.
* Node pool scales 0→4.

**Non‑Functional**

* p95 API latency < 100ms from infra‑cluster.
* Control‑plane cost zero (credit covers).

---

## E0‑S6 — OIDC Workload Identity

**Tasks**

1. Create `iam-oidc/wi-pool-github.yaml`, provider, IAM bindings.
2. Annotate Flux SA + ARC SA.
3. Remove SA keys from secrets.

**Functional**

* GitHub workflow can `gcloud` into infra cluster w/ no key file.
* ARC runner pods pull/push images w/ GCR creds.

**Non‑Functional**

* Token lifetime 60min; rotation auto.
* IAM Policy Analyzer shows 0 keys.

---

## E0‑S7 — Budget Alerts

**Tasks**

1. Create `observability/alertpolicy-budget50.yaml`.
2. Sink to Slack webhook.

**Functional**

* Alert at 50% of €200.

**Non‑Functional**

* False‑positive rate < 1/mo.

---

## E0‑S8 — Anycast IP & DNS Zone

**Tasks**

1. `global-address-glb.yaml` reserves IP.
2. `dns-zone-edf.yaml` + wildcard record.

**Functional**

* `nslookup *.fleetingdns.run` returns reserved IP.
* Propagation within 5min via Cloud DNS.

**Non‑Functional**

* No manual edits in Cloud Console; IAM deny‑policy enforced.

---

## E0‑S9 — Cloud SQL

**Tasks**

1. Instance, DB, user manifests.
2. Secret Manager entry for password.

**Functional**

* db‑f1‑micro instance Ready.
* SSL required.

**Non‑Functional**

* Idle cost €0 (free tier).
* Connections max 50.

---

## E0‑S10 — MemoryStore Redis

**Tasks**

1. 1‑GB basic tier manifest.
2. IAM network config.

**Functional**

* Redis endpoint reachable from workload cluster.
* Redis AUTH enabled.

**Non‑Functional**

* Latency < 3ms from edge‑hub pod.

---

## E0‑S11 — LB Forwarding Rules

**Tasks**

1. TCP 80/443, UDP 51820 rules YAML.
2. Attach to global IP.

**Functional**

* healthChecks pass from three Google POPs.
* Tunnel handshake works via GLB.

**Non‑Functional**

* Cold‑start < 60s new back‑end.

---

© 2025 FleetingDNS – *E0 user stories & tasks*
