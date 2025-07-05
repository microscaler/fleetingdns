# 📗 **E1f – Slot‑Space Sharding (>4B Slots)**  
*Sub‑Epic → User‑story breakdown (v0.1)*

Implements high‑order `cluster_id` bits in the HMAC label so each EdgeHub is authoritative for only its slice, enabling horizontal scaling to billions of concurrent tunnels.

---

## Epic Goal
> “Horizontally scale the stateless label system by reserving 6 high‑order bits as `cluster_id`, allowing up to 64 independent clusters worldwide while keeping backward compatibility and supporting a live migration path.”

---

## 🗂️ Story List
| ID | Story | Outcome |
|----|-------|---------|
| **E1f‑S1** | As an *Architect*, define **label v1 format**`<cluster_id><slot_bits>` and publish spec. |
| **E1f‑S2** | As *EdgeHub*, respond only to labels with my **`--cluster-id`** else REFUSED. |
| **E1f‑S3** | As *API*, encode labels with correct `cluster_id` based on workload region. |
| **E1f‑S4** | As *SRE*, run **dual‑stack migration**: accept both v0 & v1 labels during cut‑over. |
| **E1f‑S5** | As *Analytics*, expose **metric** `labels_by_cluster_total` for capacity planning. |
| **E1f‑S6** | As *Security*, ensure HMAC collision probability unchanged after bit shuffle. |

---

## E1f‑S1 — Label v1 Spec
**Tasks**
1. Reserve top 6 bits of 32‑bit slot.  
2. Update `docs/label_spec_v1.md` with bit diagram.  
3. Add version byte prefix for future.

**Functional Reqs**
* Supports 64 clusters, 4294967296 slots total.  
* Encoded Base32 length ≤52 chars.

**Non‑Functional**
* Backward comp: v0 labels still decode.  
* Spec reviewed by Edge, API teams.

---

## E1f‑S2 — EdgeHub Cluster Filter
**Tasks**
1. Add CLI flag `--cluster-id 5`.  
2. On query, decode label, compare id; REFUSED if mismatch.  
3. Unit tests for IDs 0..63.

**Functional**
* Correct cluster answers; wrong cluster returns `REFUSED`.  
* Metric `refused_wrong_cluster_total` increments.

**Non‑Functional**
* ID comparison cost negligible (<0.5 µs).  
* Memory overhead none.

---

## E1f‑S3 — API Encoder Update
**Tasks**
1. Map GKE region → cluster_id table in config.  
2. `encode_label` packs id bits.  
3. Return new label in `fqdn`.

**Functional**
* EU cluster encodes id 3, US id 7 etc.  
* SDKs unchanged (label opaque).

**Non‑Functional**
* Mis‑mapping alert if EdgeHub 404 rate >1 %.  
* Config hot‑reload.

---

## E1f‑S4 — Dual‑Stack Migration
**Tasks**
1. EdgeHub temp flag `--legacy-labels true`.  
2. Serve both v0 & v1; respond to TXT meta accordingly.  
3. After 30 days, disable legacy.

**Functional**
* No downtime; clients auto upgrade via new API label.  
* Migration progress dashboard.

**Non‑Functional**
* Legacy path removed within 60 days.  
* Alert if v0 share >5 % after 4 weeks.

---

## E1f‑S5 — Metrics
**Tasks**
1. Counter `labels_by_cluster_total{id}` increment on successful answer.  
2. Grafana heatmap.  
3. Alert if any id >80 % capacity (slots used >3.4B).

**Functional**
* Metric scrape every 30 s.  
* Dashboard shows regional load.

**Non‑Functional**
* Metric overhead <1 %.  
* Heatmap resolution hourly.

---

## E1f‑S6 — Security Analysis
**Tasks**
1. Confirm HMAC preimage space unchanged (bits merely re‑purposed).  
2. Update cryptography ADR.  
3. Fuzz test collisions 1e8 trials.

**Functional**
* Collision rate matches theoretical 1/2³².  
* ADR merged.

**Non‑Functional**
* Fuzz test runtime <2 h CI nightly.  
* No new unsafe code.

---

©2025 FleetingDNS — Slot‑Space Sharding stories

