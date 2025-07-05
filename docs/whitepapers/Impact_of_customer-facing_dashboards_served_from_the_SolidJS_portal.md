### Impact of **customer-facing dashboards served from the SolidJS portal**

| Aspect                  | Current plan (Grafana folders) | **New requirement** (SolidJS portal)                                   |
| ----------------------- | ------------------------------ | ---------------------------------------------------------------------- |
| **User authentication** | Grafana’s built-in OIDC        | Portal already handles OIDC / session cookies                          |
| **Dashboard rendering** | Grafana UI or iframes          | React/Solid components calling an API that returns JSON/Time-series    |
| **Tenant isolation**    | Grafana folder RBAC            | Must be enforced by the API we expose                                  |
| **Query language**      | PromQL                         | Probably keep PromQL under the hood but hide complexity from front-end |

**Conclusion:** we should insert a **metrics-gateway micro-service** between the portal and Mimir.
It will act as a **facade API** that:

1. Authenticates the caller using the same JWT/API-token you already issue.
2. Injects the caller’s **`tenant_id`** into the `X-Scope-OrgID` header for Mimir.
3. Accepts a *limited* set of pre-canned query names (or PromQL if you want flexibility).
4. Shapes the Mimir response into compact JSON the SolidJS charts can consume.
5. Caches short query ranges (e.g., last 15 min) in Redis to keep load low.

---

## Proposed service outline

```
cmd/
 └── metrics-gateway/            # new Rust binary (axum)
crates/
 └── metrics_client/             # thin async client to Mimir HTTP API
```

| Endpoint                    | Method | Description                                                          |
| --------------------------- | ------ | -------------------------------------------------------------------- |
| `/v1/metrics/latency`       | GET    | Returns `{timestamps[], p95[]}` for caller’s tenant over given range |
| `/v1/metrics/usage-by-day`  | GET    | Returns daily GB transferred, used by billing page                   |
| `/v1/prom/query` (optional) | POST   | Accept raw PromQL; validate length & allowed label match             |

**Auth flow**

* Portal includes session JWT (same as Tunnel API).
* Gateway validates → extracts `tenant_id`.
* Adds header `X-Scope-OrgID: <tenant_id>` when proxying to Mimir.

**Rate-limit**

* Re-use E6A token buckets so one customer can’t DoS Mimir.

---

## Does this change earlier epics?

* **E7b** – Mimir deployment itself is unchanged (still multi-tenant).
* Adds a small **new sub-epic** (call it **E7c – Metrics Gateway API**) with stories such as:

    * S1: Implement `/metrics/latency` endpoint.
    * S2: Redis cache layer.
    * S3: RBAC & scope enforcement tests.
    * S4: Portal hooks (`react-query` calls, charts).
* Remove Grafana folder provisioning for customers; keep **internal Grafana** for ops.

We still store long-term data in BigQuery for billing; the nightly aggregation can now read **directly from BigQuery** or via the same gateway running in batch mode.

---

## Complexity & footprint

* Metrics-gateway ≈ 50 MiB container, few hundred mCPU under load.
* One instance per workload cluster (behind internal Service); portal calls via internal DNS.
* No extra control-plane cost.

---

### Next steps

1. Create **E7c – Metrics Gateway API** user-story document.
2. Add HelmRelease `metrics-gateway` in `deploy/observability/`.
3. Update portal backlog to call the new endpoints.

Let me know if you’d like me to draft E7c now.
