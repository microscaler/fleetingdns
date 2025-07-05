# 📗 **E1 – Stateless DNS Engine**

*Epic → User-story breakdown (v0.1)*

This epic covers the Rust DNS authority (`stateless_dns` crate) that encodes tunnel metadata into the label, answers without a persistent DB, signs with DNSSEC, and scales horizontally.

---

## Epic Goal

> “Deliver a Trust‑DNS–based authoritative server that resolves `*.fleetingdns.run` in <2ms, uses HMAC‑shell labels for verification, supports DNSSEC on the fly, and scales to >4b slot space with cluster sharding.”

---

## 🗂️ Story List

| ID        | Story (As a…)       | Outcome                                                                                |
|-----------|---------------------|----------------------------------------------------------------------------------------|
| **E1-S1** | *EdgeHub dev*       | Encode tunnel slot + timestamp into **HMAC label** (Base32) so no DB lookup is needed. |
| **E1-S2** | *DNS operator*      | Authoritative server **parses** label, validates HMAC, and replies with A/AAAA.        |
| **E1-S3** | *Security engineer* | **DNSSEC sign** each stateless answer on-the-fly, embed RRSIG using same HMAC key.     |
| **E1-S4** | *Developer user*    | Query `_edfmeta.<label>` TXT and get JSON‑encoded debug info (expiry, cluster).        |
| **E1-S5** | *Perf engineer*     | Use **Domain crate zero‑copy encode** to drop allocations per query below 5.           |
| **E1-S6** | *SRE*               | Shard slot space into `<cluster_id>` bits so each EdgeHub only owns its slice.         |

---

## E1-S1 — HMAC Label Encoder

**Tasks**

1. Implement `encode(label_data) -> String` in `stateless_dns::hmac_label`.
2. Unit-test 128‑bit combos.
3. Expose CLI `edf-cli hmac-encode` for debug.

**Functional Reqs**

* Output ≤ 52 chars, Base32 RFC4648, no padding.
* Verify(self\_decode(encode(x)) == x).

**Non‑Functional**

* Encode+decode < 50ns on e2-standard‑4.
* Secret key read from env only at startup.

---

## E1-S2 — Authoritative Resolver

**Tasks**

1. Build Trust‑DNS Server instance listening UDP/TCP 53.
2. Parse incoming label, validate HMAC, TTL = remaining seconds.
3. Return NXDOMAIN if invalid or expired.

**Functional**

* p99 response time < 2ms for hot cache.
* TTL never exceeds 30min.

**Non‑Functional**

* Memory footprint < 20MiB.
* 10k QPS sustained on e2-micro.

---

## E1-S3 — DNSSEC On‑the‑Fly

**Tasks**

1. Generate ZSK/DSK at boot, publish DS separately.
2. Sign A record into RRSIG set using rust‑dnssec.
3. Add `--dnssec=enabled` flag to EdgeHub args.

**Functional**

* `dig +dnssec +multi` returns `ad` flag.
* Sign fails closed (NXDOMAIN) if key missing.

**Non‑Functional**

* Sign add ≤ 150µs.
* Keys rotate daily, grace overlap 1 day.

---

## E1-S4 — TXT Debug Record

**Tasks**

1. Add pattern `_edfmeta.<label>` → returns JSON string.
2. Fields: slot, expires, cluster\_id, ip.

**Functional**

* `dig txt _edfmeta.<label>.fleetingdns.run` prints JSON.
* Fails with NXDOMAIN for invalid label.

**Non‑Functional**

* Response size ≤ 255B.

---

## E1-S5 — Zero‑Copy Encode (Domain Crate)

**Tasks**

1. Replace Trust‑DNS byte‐vec serialization with `domain::bits::Composer`.
2. Benchmark allocations via `criterion`.

**Functional**

* API unchanged.
* Allocations ≤ 5 per query.

**Non‑Functional**

* p50 latency improves ≥ 20%.
* No unsafe code without justification.

---

## E1-S6 — Slot Sharding (>4B slots)

**Tasks**

1. Reserve high‑order 6 bits of slot for `cluster_id`.
2. EdgeHub config `--cluster-id`.
3. Update HMAC encode/decode to pack/unpack id.

**Functional**

* Cluster with id=5 responds only for labels with id 5.
* Cross‑label query returns REFUSED.

**Non‑Functional**

* Collision probability < 1e‑12.
* Rollout plan: migrate from v0 label to v1 without downtime.

---

©2025 FleetingDNS — Stateless DNS Engine stories
