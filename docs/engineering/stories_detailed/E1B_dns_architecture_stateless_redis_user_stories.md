# 📗 **E1b – DNS Architecture (Stateless+ Redis)**  
*Epic → User‑story breakdown (v0.1)*

Foundation work that glues the Rust stateless resolver, the Redis slot metadata store, and external CoreDNS integration for non‑tunnel zones.

---

## Epic Goal
> “Deliver a production‑ready authoritative DNS service that resolves `*.fleetingdns.run` purely from HMAC‑encoded labels and Redis cache, falls back to CoreDNS for non‑EDF zones, supports health probes, and autosurvives Redis fail‑over.”

---

## 🗂️ Story List
| ID | Story | Outcome |
|----|-------|---------|
| **E1b-S1** | As a *DNS engineer*, run **`dnsd` binary** that listens on UDP/TCP53 and answers tunnel queries in <2ms. |
| **E1b-S2** | As *EdgeHub*, write slot metadata to **Redis** so `dnsd` can resolve without API DB. |
| **E1b-S3** | As *Ops*, configure **CoreDNS forward plugin** so `dnsd` forwards any non‑EDF query upstream. |
| **E1b-S4** | As *SRE*, expose **`/healthz`** HTTP handler & Prom metrics (`queries_total`, `redis_miss_total`). |
| **E1b-S5** | As *Dev*, reload **HMAC secret key** and Redis endpoint live via SIGHUP without downtime. |
| **E1b-S6** | As *Resilience team*, achieve >99.99% availability by failing open to secondary Redis / mock TTL when Redis unavailable <60s. |
| **E1b-S7** | As *Perf*, process ≥25000 QPS on e2‑micro with <65MiB RAM. |

---

## E1b-S1 — `dnsd` Authoritative Binary
**Tasks**
1. Create crate `cmd/dnsd`.  
2. Use `tokio` + `trust_dns_server` base; handler calls `stateless_dns::resolve()`.

**Functional Requirements**
* Listens UDP/TCP on 53; returns A/AAAA + (optional) RRSIG.  
* Responds NXDOMAIN for invalid/expired labels.

**Non‑Functional**
* p95 latency <2ms (hits Redis <100µs).  
* Single‑thread throughput ≥25k QPS.

---

## E1b-S2 — Redis Slot Write Path
**Tasks**
1. EdgeHub stores `HSET slot:{id} ip "1.2.3.4" exp 172800 cluster 3 plan team`.  
2. `dnsd` reads hash on miss and caches for 1s.  
3. Key TTL mirrors tunnel TTL +60s.

**Functional**
* Cache hit ratio ≥95%.  
* Stale key deleted via keyspace notification.

**Non‑Functional**
* Redis roundtrip <0.4ms.  
* Memory use Redis <256MB for 400k active slots.

---

## E1b-S3 — CoreDNS Fallback Chain
**Tasks**
1. Side‑car CoreDNS Deployment, configmap: `.:53 forward . 8.8.8.8`.  
2. `dnsd` passes unmatched query via TCP to CoreDNS; relays answer.  
3. Health‑check CoreDNS `/health`.

**Functional**
* `dig google.com @dnsd` returns expected answer.  
* No tunnel label ever forwarded upstream (regex guard).

**Non‑Functional**
* Forward overhead <0.3ms.  
* Loop detect if upstream ever points to self.

---

## E1b-S4 — Health & Metrics
**Tasks**
1. `/healthz` returns 200 if UDP listener alive & Redis PING ok.  
2. `/metrics` expose `queries_total{rcode}`, `redis_miss_total`, `resolve_latency_seconds_bucket`.  
3. Add K8s readiness probe.

**Functional**
* Liveness fails within 5s if Redis unreachable.  
* Prom scrape every 15s.

**Non‑Functional**
* /metrics alloc overhead <1µs.  
* Labels cardinality bounded (<100).

---

## E1b-S5 — Live Config Reload
**Tasks**
1. Watch K8s Secret mount for key/redis changes.  
2. On change, atomically swap Arc<Config>.  
3. Emit log `Reload complete`.

**Functional**
* No open UDP sockets reset.  
* In‑flight queries unaffected (<1 packet lost).

**Non‑Functional**
* Reload time ≤200ms.  
* 0 data race (loom test).

---

## E1b-S6 — Redis Fail‑Over Resilience
**Tasks**
1. Configure Redis Sentinel or MemoryStore Tier fallback IP.  
2. dnsd maintains two clients; on error <60s, switch.  
3. If both down, answer SERVFAIL with `Retry‑After`=5.

**Functional**
* Availability >99.99% measured over month.  
* Alert if fail‑over triggered >3/min.

**Non‑Functional**
* Fail‑over latency <1s.  
* No unbounded reconnect storm.

---

## E1b-S7 — Performance Budget
**Tasks**
1. Criterion micro‑bench of resolve path under load.  
2. Profiling (pprof) for hot spots.  
3. CI perf gate p50 <250µs.

**Functional**
* Sustains 25k QPS on e2‑micro in benchmark pod.  
* CPU util <90%.

**Non‑Functional**
* Heap≤65MiB RSS.  
* No latency spikes >5ms.

---

© 2025 FleetingDNS — DNS Architecture (Stateless + Redis) stories

