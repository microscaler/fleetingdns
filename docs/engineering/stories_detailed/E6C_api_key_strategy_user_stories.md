# 📗 **E6C – API Key Strategy**  
*Sub-Epic → User-story breakdown (v0.1)*

Defines issuance, scope, rotation, audit, and governance model for API keys used by CLI, SDKs, and CI systems.

---

## Epic Goal
> “Provide fine-grained, revocable API tokens that support per-scope permissions, expiry, audit logging, and team/organization ownership—enabling secure automation without over-privileged credentials.”

---

## 🗂️ Story List
| ID | Story | Outcome |
|----|-------|---------|
| **E6C-S1** | As a *user*, create a **personal API key** with default scopes via portal. |
| **E6C-S2** | As a *Team admin*, issue **team-scoped key** limited to `endpoints:create` & `endpoints:delete`. |
| **E6C-S3** | As *DevOps*, set **expiry date** (e.g., 30 days) when creating a key. |
| **E6C-S4** | As *Security*, rotate keys with zero downtime using **overlapping validity window**. |
| **E6C-S5** | As *Auditor*, view **API key usage logs** (timestamp, route, IP) via portal. |
| **E6C-S6** | As *API*, validate key prefix quickly (Bloom filter) before Redis lookup. |
| **E6C-S7** | As *Admin*, revoke key instantly and ensure subsequent requests get 401 ≤5 s. |

---

## E6C-S1 — Personal Key Creation
**Tasks**
1. Portal endpoint `/api-keys` POST with scopes [].  
2. Generate token `epk_live_<base62(32)>` + HMAC prefix.  
3. Save hash (bcrypt) in Postgres table `api_keys`.

**Functional**
* UI shows `token_id`, `scopes`, `created_at`.  
* Copy button one-time reveal.

**Non-Functional**
* Token entropy ≥ 256 bits.  
* Key creation latency < 200 ms.

---

## E6C-S2 — Team-Scoped Keys
**Tasks**
1. Add `owner_type` enum (user/team) in DB.  
2. Portal restrict UI to team admins only.  
3. SDK includes `X-EDF-Team` header to disambiguate (optional).

**Functional**
* Plan quotas enforced at team level.  
* Key lookup filters by owner.

**Non-Functional**
* Role-based guard checked server-side.  
* UI lists keys grouped by owner.

---

## E6C-S3 — Expiry & Automatic Disable
**Tasks**
1. `expires_at` column; nullable (∞).  
2. Redis TTL mirror for cache.  
3. Portal default 90 days.

**Functional**
* 401 if key expired.  
* Email notification 7 days before expiry.

**Non-Functional**
* Expiry check overhead none (epoch compare).  
* Email CRON cost <€1/mo.

---

## E6C-S4 — Rotation Workflow
**Tasks**
1. Portal `rotate` button duplicates scopes, sets overlap window 24 h.  
2. Old key `status=grace`.  
3. CLI `edf key rotate` helper.

**Functional**
* Both keys valid during overlap.  
* Old key auto-revoked after window.

**Non-Functional**
* Overlap adjustable 5–168 h.  
* No double billing entries.

---

## E6C-S5 — Usage Audit Log
**Tasks**
1. API middleware logs `token_id`, route, status, IP to BigQuery.  
2. Portal UI filter by date range.  
3. CSV export.

**Functional**
* Audit retention 1 year.  
* Download completes <5 s (100 K rows).

**Non-Functional**
* Ingest cost <€5/mo.  
* PII—IP hashed last octet.

---

## E6C-S6 — Fast Validation Bloom
**Tasks**
1. Build Bloom filter of active key prefixes (`epk_live_<first6>`).  
2. Load into memory at API start, refresh every 5 min.  
3. Reject obviously invalid keys before hitting Redis.

**Functional**
* False‑positive rate <0.1 %.  
* Drop invalid attempts early (saves 70 µs).

**Non-Functional**
* Bloom size <1 MB for 100 K keys.  
* Update latency <2 s.

---

## E6C-S7 — Instant Revocation
**Tasks**
1. Portal `revoke` sets `revoked_at` timestamp.  
2. Publish Redis pub/sub `key:revoked`.  
3. API removes from in-memory Bloom filter immediately.

**Functional**
* Requests with revoked key get 401 within 5 s.  
* Audit log entry created.

**Non-Functional**
* Pub/sub message delivery latency <1 s.  
* Memory leak none.

---

© 2025 FleetingDNS — API Key Strategy stories

