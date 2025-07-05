# 📗 **E6A – Request & Tunnel Rate Limiting**  
*Epic → User-story breakdown (v0.1)*

This public portion of E6 focuses on runtime enforcement of API call‑rate, tunnel concurrency, and DNS TTL quotas by plan or token.

---

## Epic Goal
> “Protect the control‑plane and edge from abuse by applying hierarchical token buckets (per‑token, per‑plan) and concurrency guards, while surfacing friendly 429 hints to clients.”

---

## 🗂️ Story List
| ID | Story | Outcome |
|----|-------|---------|
| **E6A-S1** | As a *Free-tier user*, I can make up to **60 API calls/min**, otherwise get 429 + Retry‑After. |
| **E6A-S2** | As a *Team plan*, spin up to **10 concurrent tunnels**; 11th call is rejected. |
| **E6A-S3** | As *EdgeHub*, cap DNS provisions to **5/s per IP** to mitigate brute‑force. |
| **E6A-S4** | As *DevOps*, dynamically adjust burst limits via **Redis plan config** without redeploy. |
| **E6A-S5** | As *SRE*, emit **Prom metrics** (`rate_limit_hits_total`) for alerting. |

---

## E6A-S1 — API RPS Limiter
**Tasks**
1. Implement `RateLayer` in `api-gateway` using `governor` crate (leaky bucket).  
2. Key = token hash; fallback IP bucket if missing.

**Functional**
* 429 response contains `X-RateLimit-Limit` + `Retry-After`.  
* Limit table from Redis plan config hash.

**Non-Functional**
* rps limit check adds < 3 µs.  
* Memory < 1 MiB for 10 k active tokens.

---

## E6A-S2 — Concurrent Tunnel Guard
**Tasks**
1. Before `POST /tunnels`, read `concurrent_active` counter in Redis.  
2. If > plan limit, return 429 with JSON error.  
3. Increment on success; decrement via keyspace event.

**Functional**
* Plan limits: Free=1, Supporter=3, Team=10, Org=50+.  
* Counter reset on tunnel expiry.

**Non-Functional**
* Race condition window ≤ 1 (Lua script atomic inc/dec).  
* False reject rate <0.1%.

---

## E6A-S3 — DNS Provision Throttle
**Tasks**
1. EdgeHub counts parse-success per remote IP per second.  
2. Exceeding 5/s returns NXDOMAIN + 429 Retry header.  
3. Counter resets sliding window 1 s.

**Functional**
* Legit client making ≤2/s never throttled.

**Non-Functional**
* Overhead <1 µs per DNS query.  
* Protects from 100 k qps spray.

---

## E6A-S4 — Hot Config Reload
**Tasks**
1. Plan table (hash) in Redis watched via Pub/Sub `plan:update`.  
2. Limiter caches update within 5 s.  
3. CLI `edf admin plan set team.rate_rps=120` helper.

**Functional**
* No pod restart needed.  
* Version stamped for audit.

**Non-Functional**
* Live update latency <5 s.  
* Redis bandwidth minimal (<1 KB/s idle).

---

## E6A-S5 — Metrics & Alerts
**Tasks**
1. Expose `/metrics` Prom endpoint with `rate_limit_hits_total{plan}`.  
2. Grafana dashboard + alert if hit_ratio >10 % for 5 m.

**Functional**
* Metric increments on every 429.  
* Dashboard shows per-plan hit %.

**Non-Functional**
* Metrics scrape overhead <0.1 %.  
* Alert false positives <1 / month.

---

© 2025 FleetingDNS — Rate Limiting stories

