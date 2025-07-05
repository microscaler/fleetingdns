# 📗 **E1h – Per‑Zone Rate‑Limit Buckets & Vanity ACME Automation**  
*Sub-Epic → User-story breakdown (v0.1)*

Introduces fine‑grained QPS buckets per custom domain (zone) to prevent abuse, plus automated ACME issuance for vanity domains (`*.my‑product.dev`) that are CNAME’d to FleetingDNS rather than NS‑delegated.

---

## Epic Goal
> “Protect shared DNS/edge resources by capping QPS per zone according to plan tier and automatically mint wildcard TLS certificates for vanity domains pointed at our anycast IP via CNAME records.”

---

## 🗂️ Story List
| ID | Story | Outcome |
|----|-------|---------|
| **E1h-S1** | As a *Team admin*, add **vanity CNAME domain** (`preview.acme.dev`) in portal and get HTTPS working automatically. |
| **E1h-S2** | As *Platform*, detect CNAME ownership via **HTTP‑01 challenge** and issue ACME cert (Let’s Encrypt). |
| **E1h-S3** | As *Security*, enforce **rate‑limit bucket** of 100 QPS for Free plan domains, 500 for Org, with 429 on excess. |
| **E1h-S4** | As *SRE*, view **prom metric** `zone_qps_limit_exceeded_total{zone}` and alert if sustained >1%. |
| **E1h-S5** | As *Customer*, receive email warning if my domain exceeds 80% of limit for 3 consecutive hours. |
| **E1h-S6** | As *Billing*, upsell **additional QPS add‑on** purchasable in portal, reflected instantly in bucket size. |

---

## E1h-S1 — Vanity Domain Registration Flow
**Tasks**
1. Portal form “Add CNAME domain”; validate apex not already used.  
2. Show instruction: `CNAME preview.acme.dev → cname.fleetingdns.run`.  
3. Poll DNS every 2min until CNAME detected.

**Functional**
* Domain status transitions `pending_cname` → `pending_tls` → `active`.  
* Limit: 5 vanity domains per plan (configurable).

**Non‑Functional**
* Polling job cost <€0.05/mo per 100 domains.  
* False positives <0.5%.

---

## E1h-S2 — ACME HTTP‑01 Automation
**Tasks**
1. On `pending_tls`, allocate random token, store in Redis `acme:token`.  
2. EdgeHub serves `/.well-known/acme-challenge/{token}` returning key‑auth.  
3. Call Let’s Encrypt API; store cert in SecretManager.

**Functional**
* Cert issued in ≤2min after CNAME appears.  
* Auto‑renew 20days before expiry.  

**Non‑Functional**
* Retry back‑off up to 5 times.  
* Rate‑limit ≤300 orders / 3h (LE burst quota).

---

## E1h-S3 — Per‑Zone QPS Bucket
**Tasks**
1. Extend EdgeHub rate‑limit middleware: key=`zone` → token‑bucket (dashmap).  
2. Limits pulled from Redis `zone:limit:{zone}` with fallback plan default.  
3. 429 response includes `Retry‑After` & `X‑RateLimit‑Remaining`.

**Functional**
* Free plan default 100 qps; Supporter300; Team1000; Org5000.  
* Burst = limit ×2 for 30s.

**Non‑Functional**
* Overhead <3µs per query.  
* Memory per zone ≤256B.

---

## E1h-S4 — Metrics & Alert
**Tasks**
1. Counter `zone_qps_limit_exceeded_total{zone}`.  
2. Gauge `zone_qps_current{zone}` updated every second.  
3. Alert: if limit_exceeded rate >1/s for 5min.

**Functional**
* Alert routes to Abuse team Slack + customer email (E1h‑S5).  
* Grafana dashboard top 20 noisy zones.

**Non‑Functional**
* Metrics cardinality bounded (#custom domains).  
* Alert false positive <1/mo.

---

## E1h-S5 — Customer Warning Email
**Tasks**
1. Cloud Task enqueues email when `zone` flagged.  
2. Template includes upgrade CTA & docs link.  
3. Do not resend within 24h.

**Functional**
* Email send within 10min of breach.  
* Contains current QPS & limit.

**Non‑Functional**
* Bounce rate <1%.  
* Opt‑out link respected.

---

## E1h-S6 — QPS Add‑On Purchase
**Tasks**
1. Stripe price `addon-zone-qps-1000`.  
2. Portal “Upgrade limit” button; creates Checkout session.  
3. Webhook `invoice.paid` updates Redis `zone:limit` and Plan table.

**Functional**
* New limit takes effect ≤60s after payment.  
* Billing reflects prorated amount.

**Non‑Functional**
* Stripe webhook retries handled.  
* UI shows new limit immediately.

---

© 2025 FleetingDNS — Per‑Zone Rate‑Limit & Vanity ACME stories

