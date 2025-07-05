# 📗 **E1g – Hosted Sub‑Domain NS for Customers**  
*Sub‑Epic → User‑story breakdown (v0.1)*

Enables enterprise & Team plans to delegate a sub‑domain (e.g., `ci.acme.com`) via NS records to FleetingDNS so tunnels resolve beneath customer‑controlled zones while preserving stateless label scheme, DNSSEC, and billing.

---

## Epic Goal
> “Provide turnkey name‑server hosting for delegated customer sub‑domains that automatically provisions DNSSEC, ACME TLS certificates, and plan‑aware quotas—letting teams expose tunnels under their brand with zero manual operations.”

---

## 🗂️ Story List
| ID | Story | Outcome |
|----|-------|---------|
| **E1g‑S1** | As a *Team admin*, create **Custom Domain** in portal and receive NS targets to add at registrar. |
| **E1g‑S2** | As *Platform*, provision **per‑tenant Trust‑DNS authority** that answers `*.ci.acme.com` using same stateless label logic. |
| **E1g‑S3** | As *Security*, issue **Wildcard ACME cert** via DNS‑01 using customer NS delegation. |
| **E1g‑S4** | As *DNSSEC operator*, sign sub‑zone with our KSK and provide DS record for customer to publish. |
| **E1g‑S5** | As *Billing*, enforce plan limit `max_custom_domains` and extra €99/yr add‑on. |
| **E1g‑S6** | As *SRE*, expose metrics (`custom_domain_queries_total`) and alert if QPS > plan quota. |

---

## E1g‑S1 — Portal Domain Registration Flow
**Tasks**
1. UI form `Add custom domain` → validate FQDN regex.  
2. Generate four NS targets `<hash0>.ns.fdns.run` … `<hash3>.ns`.  
3. Display DNSSEC DS & ACME TXT hints.

**Functional Reqs**
* Domain status = `pending_ns`.  
* Poll every 5 min using `dig +ns +trace` until delegation detected.

**Non‑Functional**
* Max setup wizard < 2 steps.  
* Support up to 50 domains per org.

---

## E1g‑S2 — Per‑Tenant Authority Instance
**Tasks**
1. `dnsd` supports multi‑zone map at runtime (hashmap <domain, label_parser>).  
2. On domain activation, configmap hot‑reloaded via SIGHUP.  
3. Zone apex SOA served with `hostmaster@fleetingdns.run`.

**Functional**
* Query `abc.ci.acme.com` resolves to tunnel IP.  
* TTL mirrors main zone rules.

**Non‑Functional**
* Added heap per domain ≤64 KiB.  
* 1000 custom domains per cluster OK.

---

## E1g‑S3 — ACME Wildcard Cert Automation
**Tasks**
1. On activation, create ACME order `*.ci.acme.com`.  
2. Present TXT `_acme‑challenge.ci.acme.com` via same authority.  
3. Use RFC8555; store cert in Secret Manager.

**Functional**
* Cert issued within 2 min (DV).  
* Cert auto‑renews 30 days before expiry.

**Non‑Functional**
* Failure retries exponential back‑off.  
* No more than 50 failed orders/day (Let’s Encrypt rate‑limit).

---

## E1g‑S4 — DNSSEC Chain
**Tasks**
1. Generate per‑domain KSK & ZSK; sign zone.  
2. Provide DS SHA‑256 digests in portal.  
3. Remind admin weekly until DS inserted at parent.

**Functional**
* `dig +dnssec abc.ci.acme.com` returns `ad` once DS live.  
* Portal shows status `secure`.

**Non‑Functional**
* Key rotation 90‑day cycle.  
* DS reminder emails max 4.

---

## E1g‑S5 — Plan Enforcement & Billing
**Tasks**
1. Column `custom_domains_used` in users/org table.  
2. Free/Supporter = 0, Team = 3, Org = 20, add‑on increments.  
3. Stripe webhook `invoice.paid` increases entitlement.

**Functional**
* API returns 402 if limit exceeded.  
* Add‑on price €99/yr per domain.

**Non‑Functional**
* Over‑usage block latency <10 s.  
* Audit trail records domain count changes.

---

## E1g‑S6 — Metrics & Alerts
**Tasks**
1. Counter `custom_domain_queries_total{domain}`.  
2. Alert if QPS > 100 for Free domains (abuse).  
3. Dashboard per‑domain query/latency.

**Functional**
* Metric scraped by Mimir; shown in customer portal if plan≥Team.  
* Alert routes to Abuse response team.

**Non‑Functional**
* Label cardinality bounded (#domains).  
* Alert false positive <1/mo.

---

© 2025 FleetingDNS — Hosted Sub‑Domain NS stories

