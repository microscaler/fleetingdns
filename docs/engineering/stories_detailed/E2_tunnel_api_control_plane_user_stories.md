# 📗 **E2 – Tunnel API & Control‑Plane**  
*Epic → User‑story breakdown (v0.1)*

This epic covers the Rust REST+gRPC service (`api-gateway` crate) that issues tunnels, signs short‑lived certs, stores slot metadata in Redis, enforces quotas, and feeds EdgeHub.

---

## Epic Goal
> “Expose a secure, multi‑tenant API that developers (CLI, SDK, CI) call to create, inspect, and revoke tunnels. It must mint 30‑minute mTLS/WireGuard creds, respect rate‑limits per plan, push events to Stripe webhooks, and run statelessly atop Redis + Postgres.”

---

## 🗂️ Story List
| ID        | Story                                                                                              | Outcome |
|-----------|----------------------------------------------------------------------------------------------------|---------|
| **E2‑S1** | As a *CLI user*, **POST /v1/tunnels** and receive `fqdn`, `expires_at`, creds JSON.                |
| **E2‑S2** | As an *SDK maintainer*, query **/v1/plans/{token}** to know plan limits (tunnel TTL, concurrency). |
| **E2‑S3** | As *Billing system*, receive **Webhook event** `tunnel.created`, `tunnel.closed`.                  |
| **E2‑S4** | As *EdgeHub*, fetch **slot metadata** (`GET /v1/slots/{id}`) for debugging & TXT record.           |
| **E2‑S5** | As *SecOps*, rate‑limit API per token & IP; 429 on abuse.                                          |
| **E2‑S6** | As *CLI user*, **DELETE /v1/tunnels/{id}** to tear down early.                                     |
| **E2‑S7** | As *Admin*, list active tunnels per plan via **/v1/admin/tunnels?plan=team** (JWT admin).          |

---

### Story template below
---

## E2‑S1 — Create Tunnel Endpoint
**Tasks**
1. Define OpenAPI schema (`openapi.yaml`).  
2. Implement `POST /tunnels` in axum handler.  
3. Generate HMAC label, slot row in Redis, short‑lived cert via `ca_service`.  
4. Unit tests + integration test with `edf-cli`.

**Functional Reqs**
* Accepts JSON `{mode, ttl, meta}`; returns 201 with `fqdn`, `expires_at`, `cert_pem`, `private_key` (base64).  
* Writes `slot:{id}` hash in Redis with TTL = `ttl+60s`.

**Non‑Functional**
* p95 latency < 60 ms (including cert sign).  
* Redis write failure returns 503; no partial state.

---

## E2‑S2 — Plan Lookup
**Tasks**
1. Add `/plans/{token}` route.  
2. Cache plan policy in Redis (24 h).  
3. Integrate with Stripe subscription lookup.

**Functional**
* Returns `max_ttl`, `daily_quota`, `rate_rps`.  
* 404 if token invalid or revoked.

**Non‑Functional**
* Cached read miss < 10 %.  
* External Stripe call timeout 500 ms.

---

## E2‑S3 — Webhook Events
**Tasks**
1. Fire event to Stripe `usage_record` API on tunnel create/close.  
2. Fan‑out internal Pub/Sub topic for analytics.

**Functional**
* At least‑once delivery; retries 5× with backoff.  
* Payload JSON includes `plan_id`, `bytes_tx`, `duration`.

**Non‑Functional**
* Event backlog queue depth < 1 000.  
* Lost event < 0.01 % month.

---

## E2‑S4 — Slot Metadata Endpoint
**Tasks**
1. Add `/slots/{id}` (GET) with JWT auth `aud=edgehub`.  
2. Returns JSON used for TXT record.

**Functional**
* Returns 200 if slot in Redis; 404 if expired.  
* Field `cluster_id` must match EdgeHub requesting.

**Non‑Functional**
* Latency < 5 ms (same region).  
* Auth failure < 0.1 % legitimate calls.

---

## E2‑S5 — Rate Limiting Middleware
**Tasks**
1. Integrate `tower::limit::ConcurrencyLimitLayer`.  
2. Key = token+IP; config from plan.  
3. Return `429` with `Retry‑After` header.

**Functional**
* Enforces burst+RPS per plan table.  
* `X‑RateLimit‑Remaining` header returned.

**Non‑Functional**
* Overhead < 2 µs per request.  
* Memory footprint < 10 MiB (dashmap buckets).

---

## E2‑S6 — Delete Tunnel
**Tasks**
1. Implement `DELETE /tunnels/{id}`; soft delete.  
2. EdgeHub listens Redis keyspace‑notifications to close socket.

**Functional**
* Returns 204 if successful; idempotent.  
* Redis TTL immediately set to 1s.

**Non‑Functional**
* Full teardown < 2 s observed.  
* No orphan sockets > 5 min.

---

## E2‑S7 — Admin‑List Endpoint
**Tasks**
1. JWT middleware for admin scope.  
2. `GET /admin/tunnels?plan=` query; stream JSON.  
3. Pagination via cursor.

**Functional**
* Supports `cursor` param; default page 100.  
* Response includes `plan_id`, `ip`, `created_at`.

**Non‑Functional**
* Query time < 100 ms for 5 k tunnels.  
* Authz error returns 403.

---

© 2025 FleetingDNS — Tunnel API stories

