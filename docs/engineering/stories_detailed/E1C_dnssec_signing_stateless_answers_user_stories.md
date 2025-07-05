# 📗 **E1c – DNSSEC Signing for Stateless Answers**  
*Sub‑Epic → User-story breakdown (v0.1)*

This covers on‑the‑fly RRSIG generation for every A/AAAA answer, daily ZSK rotation backed by CloudKMS, and DS record automation for the public zone.

---

## Epic Goal
> “Provide authenticated denial‑of‑modification for stateless DNS answers by attaching RRSIGs in <150µs, rotating keys safely, and automating DS publishing—all without adding persistent state.”

---

## 🗂️ Story List
| ID | Story | Outcome |
|----|-------|---------|
| **E1c-S1** | As a *DNS engineer*, sign A/AAAA answers with **ZSK‑RSA‑256** generated at pod start. |
| **E1c-S2** | As *Security*, store signing key material in **CloudKMS (CMEK)**; EdgeHub loads via K8sSecret. |
| **E1c-S3** | As *Zone admin*, publish/rotate **DS record** for new KSK without manual action. |
| **E1c-S4** | As *Perf engineer*, ensure RRSIG generation adds **<150µs** to query path. |
| **E1c-S5** | As *SRE*, monitor **dnssec_sign_failures_total** metric and alert if >0.01%. |

---

## E1c-S1 — On‑the‑Fly RRSIG
**Tasks**
1. Integrate `trust_dns::rr::dnssec::Signer` in `stateless_dns` resolver.  
2. Build signer from in‑memory ZSK at pod boot.  
3. Attach RRSIG for each answer; TTL = original.

**Functional Reqs**
* `dig +dnssec a <label>.fleetingdns.run` returns `ad` flag when upstream validates.  
* RRSIG `exp` = query TTL.

**Non‑Functional**
* Extra latency per query ≤150µs p95.  
* Signature cache 1000 entries, LRU.

---

## E1c-S2 — KMS‑Backed Key Storage
**Tasks**
1. Create KMS key‑ring `dnssec` and RSA‑2048 key.  
2. Job `scripts/push_zsk.sh` decrypts, writes Secret via SealedSecrets.  
3. EdgeHub loads key on startup; reload on Secret update.

**Functional**
* Private key never stored unencrypted on disk.  
* Decrypt call requires WorkloadIdentity SA.

**Non‑Functional**
* Decrypt latency ≤40ms at pod start.  
* Key material resident in memory only.

---

## E1c-S3 — Automated DS Update
**Tasks**
1. Weekly GitHub Action runs `dnssec-ksk-rotate`.  
2. Creates new KSK, generates DS, opens PR to `gcp-core/dns-zone-edf.yaml`.  
3. Flux applies; CloudDNS publishes DS.

**Functional**
* DS rollover uses double‑signature (RFC6781).  
* Old DS removed after 72h.

**Non‑Functional**
* Zero downtime validation (no SERVFAIL windows).  
* Manual approval step for DS push.

---

## E1c-S4 — Performance Guardrail
**Tasks**
1. Criterion benchmark before/after signing.  
2. Optimize with pre‑computed SHA256 digest of RRset.  
3. Add CI perf gate: fail if >150µs p95.

**Functional**
* Bench output archived artifact.  
* Optimisations documented.

**Non‑Functional**
* CPU overhead ≤20% at 10k QPS.  
* No unsafe code without audit comment.

---

## E1c-S5 — Sign Failure Metrics & Alert
**Tasks**
1. Increment `dnssec_sign_failures_total` on any signing error.  
2. Prometheus alert if >1/min for 5min.  
3. Grafana panel.

**Functional**
* Alert fires to Slack `#alerts-dns`.  
* Runbook link attached.

**Non‑Functional**
* False positives <1/quarter.  
* Alert latency <2min.

---

©2025FleetingDNS — DNSSEC Signing stories

