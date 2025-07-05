# 📗 **E1i – Private Delegated Zones & Split‑Horizon DNS**  
*Sub-Epic → User‑story breakdown (v0.1)*

Adds private RFC1918 views and split‑horizon capabilities so customers can delegate an internal zone (e.g., `*.dev.acme.local`) that resolves differently inside their VPC / VPN than on the public internet, while still using FleetingDNS tunnel labels.

---

## Epic Goal
> “Deliver dual‑view DNS where private queries (over VPC‑peering, VPN, or WireGuard) resolve to 10.x ‘internal ingress’ addresses and public queries resolve to the anycast edge, allowing secure on‑prem integration tests and zero‑trust networking.”

---

## 🗂️ Story List
| ID | Story | Outcome |
|----|-------|---------|
| **E1i‑S1** | As a *Corp‑Net admin*, delegate **`dev.acme.internal`** via Private DNS Peering to FleetingDNS and have queries resolve over my VPC only. |
| **E1i‑S2** | As *EdgeHub*, detect **`view=private`** query and answer with reserved 10.10.0.0/16 addresses (tunnel’s WireGuard IP). |
| **E1i‑S3** | As *Platform*, ensure **split‑horizon configs** reload without pod restarts when new customer VPCs peer. |
| **E1i‑S4** | As *Security*, block leakage: private zone never served on public interface and vice‑versa. |
| **E1i‑S5** | As *SRE*, expose metrics `private_view_queries_total` & alert if ratio >30% (indicates mis‑routing). |
| **E1i‑S6** | As *Customer*, tunnel over **WireGuard** instead of VPC‑peering and still resolve internal view via DNS64 if needed. |

---

## E1i‑S1 — Private Zone Delegation Flow
**Tasks**
1. Portal wizard “Add private zone” → collect CIDR(s) that will query the zone.  
2. Create Cloud DNS **Private Managed Zone** `dev-acme-internal` inside customer workload project with `peering` to customer VPC network or Hub‑Spoke VPN.  
3. Return instructions to add **VPC Peering** (`gcloud compute networks peerings create`).

**Functional Reqs**
* Query from 10.50.0.5 returns 10.x‎ address; query from internet gets NXDOMAIN.  
* Supports up to 5 CIDR ranges per zone.

**Non‑Functional**
* Setup automation completes in <10min.  
* All configs managed by Crossplane CRs.

---

## E1i‑S2 — Private‑View Answer Logic
**Tasks**
1. In `dnsd`, inspect **client IP**; if src in `private_ranges` list AND qname matches zone, switch to *private* view.  
2. Pull slot metadata WireGuard IP (`10.10.x.x`) and answer A record with that.  
3. TXT / RRSIG signed by separate ZSK for private view.

**Functional**
* Same label resolves to **public anycast IP** externally, **WireGuard IP** internally.  
* TTL equal both views.

**Non‑Functional**
* View lookup adds ≤5µs.  
* Memory per range list ≤32KiB.

---

## E1i‑S3 — Dynamic View Reload
**Tasks**
1. Store `private_ranges` JSON in ConfigMap; watch for change.  
2. Atomically swap in‑memory RangeTrie.  
3. Rollout new private zone without dropping queries.

**Functional**
* New CIDR active in ≤60s after portal save.  
* No UDP packet loss observed.

**Non‑Functional**
* Reload CPU spike <5%.  
* Race‑condition tests with loom.

---

## E1i‑S4 — Leakage Prevention
**Tasks**
1. Add integration test: query private zone from public IP → expect REFUSED.  
2. IPtables / eBPF rule on Edge public interface denies `*.internal.`.  
3. Audit logs if attempted leak.

**Functional**
* `dig dev.acme.internal @8.8.8.8` returns NXDOMAIN.  
* Security scorecard pass.

**Non‑Functional**
* False leak alarm <1 / year.  
* eBPF rule latency <0.1µs.

---

## E1i‑S5 — Metrics & Alert
**Tasks**
1. Counter `private_view_queries_total` and `public_view_queries_total`.  
2. Alert if private/public ratio >0.3 or <0.01 (anomaly by tenant).  
3. Grafana dashboard heatmap by view.

**Functional**
* Alert route to SRE within 5min.  
* Metric scraped by Mimir.

**Non‑Functional**
* Metric label cardinality ≤ tenants.  
* Alert false positive <1/mo.

---

## E1i‑S6 — WireGuard‑Only Private View
**Tasks**
1. EdgeHub label includes `wg_ip=10.10.x.x`.  
2. If query via WG tunnel IP (source 172.16.0.2), treat as private view without peering.  
3. DNS64 plugin optional to map AAAA.  

**Functional**
* Developer on laptop ↔ WireGuard to Edge can resolve `*.dev.acme.internal` to 10.x IP.  
* Works even without GCP VPC peering.

**Non‑Functional**
* Added handshake latency none (DNS over WG).  
* WG path QPS limited to 5k per peer.

---

© 2025 FleetingDNS — Private Delegated Zones & Split‑Horizon stories

