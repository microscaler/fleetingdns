# 📗 **E7b – Observability Data Lake (GrafanaMimir)**  
*Sub-Epic → User-story breakdown (v0.1)*

Switch from VictoriaMetrics to a **self‑hosted GrafanaMimir stack** in every workload project, storing metrics in regional GCS buckets and providing tenant‑scoped dashboards plus long‑term retention for billing.

---

## Epic Goal
> “Deploy a cost‑efficient, multi‑tenant Mimir cluster per billing account that ingests OpenTelemetry metrics via Remote Write, retains 180days in GCS, exposes RBAC‑guarded Grafana dashboards, and streams usage data into BigQuery for nightly billing jobs.”

---

## 🗂️ Story List
| ID | Story | Outcome |
|----|-------|---------|
| **E7b-S1** | As a *Platform engineer*, install **mimir-distributed Helm chart** with object‑storage backend in each workload cluster. |
| **E7b-S2** | As *Infra*, provision **regional GCS bucket** (`fdns‑metrics‑eu`) with CMEK and versioning. |
| **E7b-S3** | As *Otel Collector*, remote‑write to Mimir via **HTTP+tenant header**. |
| **E7b-S4** | As *Customer*, access **Grafana folder** scoped to my tenant and view latency/usage dashboards. |
| **E7b-S5** | As *Billing job*, query **BigQuery export** produced by Mimirruler every 5min. |
| **E7b-S6** | As *SRE*, auto‑scale ingesters 2→8 pods based on `ingester_memory_series` metric. |
| **E7b-S7** | As *Finance*, receive pager if **GCS cost** exceeds budgetalert (€50 / region). |

---

## E7b-S1 — Helm Deploy Mimir Distributed
**Tasks**
1. Add `deploy/observability/mimir/helmrelease.yaml` referencing grafana/mimir‑distributed chart v5+.  
2. Values: `replicas.distributor=2`, `ingester=2`, `querier=2`, `compactor=1`, `limits.memory`, `podAntiAffinity`.  
3. NetworkPolicy only allows ingress from Otel Collector namespace.

**Functional Reqs**
* `/_ready` endpoints green within 5min of install.  
* Tenant‑header auth enabled (`X-Scope-OrgID`).

**Non‑Functional**
* Total vCPU baseline≈4, RAM≈8GiB.  
* Control‑plane cost still €0 (same Autopilot cluster).

---

## E7b-S2 — GCS Bucket with CMEK & Versioning
**Tasks**
1. Crossplane `Bucket` XR `metrics‑eu` / `metrics‑us` / `metrics‑apac`.  
2. Enable uniform‑access, CMEK (`projects/fdns‑key/rings/metrics/keys/mimir`).  
3. Lifecycle rule deletes objects older>180days.

**Functional**
* Mimir chart `storage.tsdb.bucket` points to bucket.  
* Bucket location matches cluster region.

**Non‑Functional**
* Versioning keeps last3 versions.  
* Retention GC job hourly.

---

## E7b-S3 — Otel Remote Write Pipeline
**Tasks**
1. Update `otel-collector.yaml` exporter: `prometheusremotewrite { endpoint: http://mimir-distributor.observability.svc:8080/api/v1/push, headers { X-Scope-OrgID: default }}`  
2. Add tenant label `tenant_id` for custom metrics.  
3. Verify 200 status.

**Functional**
* Write success rate ≥99.9%.  
* Series arrive in Mimir within <10s.

**Non‑Functional**
* Collector CPU overhead <5%.  
* Tenant label cardinality = user count (10k) – test for memory.

---

## E7b-S4 — RBAC‑Scoped Grafana Dashboards
**Tasks**
1. Deploy Grafana Helm chart with OIDC login (Auth0).  
2. Datasource: Mimir, with `org_id` variable.  
3. Folder‑level permissions per tenant via `grafana.ini` auto‑provision.

**Functional**
* Customer sees only their own folder & dashboards.  
* Edge latency, bytes‑sent panels load <3s.

**Non‑Functional**
* Grafana memory 1GiB pod.  
* SSO login <2s round‑trip.

---

## E7b-S5 — BigQuery Export via MimirRuler
**Tasks**
1. Create ruler recording rule: sum(bytes_total) by (tenant_id,day).  
2. Configure ruler → Pub/Sub → Dataflow → BigQuery table `metrics_usage`.  
3. Nightly Cloud Run job queries last24h and posts Stripe usage.

**Functional**
* BQ table partition `day` auto‑populated.  
* Billing job accuracy ±1% against raw Mimir query.

**Non‑Functional**
* Dataflow cost <€5/mo.  
* Job runtime <5min.

---

## E7b-S6 — Ingester HPA
**Tasks**
1. Metric `cortex_ingester_memory_series`.  
2. HPA: target 3M series per pod, scale1→8.  
3. Test with `k6` load.

**Functional**
* Pods add within 2min on surge.  
* No 5xx during scale events.

**Non‑Functional**
* Max pod CPU 70%.  
* Scale‑down after 30min idle.

---

## E7b-S7 — Budget Alerts for GCS
**Tasks**
1. Create Cloud Budget for each workload project (€50).  
2. Alert channel Slack `#finance`.  
3. Runbook link.

**Functional**
* Alert at 80% spend.  
* Finance dashboard updated.

**Non‑Functional**
* False positives <1per quarter.  
* Alert latency <1h.

---

©2025 FleetingDNS — Observability Data Lake (Mimir) stories

