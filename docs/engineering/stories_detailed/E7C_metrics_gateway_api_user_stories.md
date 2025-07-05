# 📗 **E7c – Metrics Gateway API**  
*Sub-Epic → User-story breakdown (v0.1)*

A lightweight Rust service (`metrics-gateway`) that fronts Grafana Mimir, adding tenant-scoped authentication, query shaping, caching, and a JSON contract for the SolidJS customer portal.

---

## Epic Goal
> “Expose near‑real‑time latency, usage, and billing metrics to the customer portal through a secure, rate‑limited REST API while shielding Mimir from unbounded PromQL and enforcing tenant isolation.”

---

## 🗂️ Story List
| ID | Story | Outcome |
|----|-------|---------|
| **E7c-S1** | As a *portal*, call **GET /v1/metrics/latency** and receive p95 latency series JSON. |
| **E7c-S2** | As a *customer*, view **daily GB transferred** via `/v1/metrics/usage-by-day`. |
| **E7c-S3** | As *Security*, gateway injects **`X-Scope-OrgID`** header so queries stay tenant-scoped. |
| **E7c-S4** | As *Perf*, gateway caches recent 5‑min query ranges in Redis for <100 ms responses. |
| **E7c-S5** | As *Ops*, limit raw PromQL via POST `/v1/prom/query` to a safe subset. |
| **E7c-S6** | As *FinOps*, expose `/v1/internal/billing-rollup` to BigQuery job for nightly aggregation. |

---

## E7c-S1 — Latency Endpoint
**Tasks**
1. Add route `GET /v1/metrics/latency?range=1h&step=30s`.  
2. Build PromQL: `histogram_quantile(0.95, sum(rate(edge_request_duration_seconds_bucket{job="edgehub"}[5m])) by (le))`.  
3. Transform Mimir JSON to `{timestamps:[…], p95:[…]}`.

**Functional**
* Response 200 with array lengths = (range / step).  
* Accepts range 5m – 24h.

**Non-Functional**
* p95 endpoint latency <200 ms when uncached.  
* Cached hit ≤20 ms.

---

## E7c-S2 — Usage-by-Day Endpoint
**Tasks**
1. Aggregation query `sum_over_time(edge_bytes_total[24h])`.  
2. Group by day via `offset` or downsampled recording rules.  
3. Return last 30 days.

**Functional**
* JSON `{day:"2025-07-06", gb:1.24}` list 30 items.  
* 404 if tenant unknown.

**Non-Functional**
* Query cost <0.5 vCPU.  
* Response size <5 KB.

---

## E7c-S3 — Tenant Header Injection
**Tasks**
1. Extract `tenant_id` from JWT (`sub`).  
2. Add header `X-Scope-OrgID`.  
3. Block if caller scopes mismatch.

**Functional**
* Curl with token of tenant X cannot query tenant Y.  
* Audit log record.

**Non-Functional**
* Header inject cost negligible.  
* Penetration test passes.

---

## E7c-S4 — Redis Cache Layer
**Tasks**
1. Key = SHA1(query + range + tenant).  
2. TTL 60 s for ≤1 h ranges; 5 min for 24 h.  
3. Use `redis::aio` pipelined get/set.

**Functional**
* Cache hit ratio target 50 % for portal load.  
* Stale key eviction working.

**Non-Functional**
* Redis overhead <10 ops/s per 1 000 users.  
* Memory cap 200 MB.

---

## E7c-S5 — Safe Raw PromQL (optional)
**Tasks**
1. POST body `{query:"rate(edge_requests_total[5m])"}`.  
2. Validate regex (length<512, no `__name__`, no global selectors).  
3. Rate-limit to 5 req/min per user.

**Functional**
* 400 if query invalid.  
* 429 on exceed.

**Non-Functional**
* Rate limiter reuses E6A bucket.  
* Security review.

---

## E7c-S6 — Billing Roll-up Endpoint
**Tasks**
1. Auth internal SA only.  
2. Expose `/v1/internal/billing-rollup?day=YYYY-MM-DD`.  
3. Returns CSV/JSON lines per tenant: `{tenant, gb, minutes}`.

**Functional**
* Nightly job ingests output into BigQuery staging table.  
* Stripe usage-record job consumes it.

**Non-Functional**
* Endpoint heavy query can run 1–2 min; use async job pattern.  
* CPU pod limit 2 vCPU.

---

© 2025 FleetingDNS — Metrics Gateway stories

