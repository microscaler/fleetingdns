# 📗 **E1e – Zero‑Copy DNS Encode via `Domain` Crate**  
*Sub‑Epic → User‑story breakdown (v0.1)*

Objective: swap Trust‑DNS byte‑vector serialization with the `domain` crate’s zero‑copy composer to cut allocations and latency for each DNS answer.

---

## Epic Goal
> “Reduce memory allocations per DNS query from ~25 to ≤5 and shave ≥20% off p50 latency by using the `domain` crate’s `Composer` and `MessageBuilder`, while keeping identical wire output and full DNSSEC signing support.”

---

## 🗂️ Story List
| ID | Story | Outcome |
|----|-------|---------|
| **E1e‑S1** | As a *Perf engineer*, benchmark current Trust‑DNS encode path and capture baseline alloc & time. |
| **E1e‑S2** | As a *Rust dev*, implement **MessageBuilder wrapper** that writes A/AAAA + RRSIG via `domain` crate without heap copy. |
| **E1e‑S3** | As *QA*, verify byte‑for‑byte equality (wire format) between old and new encoders for 100 label samples. |
| **E1e‑S4** | As *Security*, guarantee no unsafe code is introduced or it is fully audited. |
| **E1e‑S5** | As *SRE*, expose `dns_encode_latency_seconds` histogram to ensure perf regression alerting. |

---

## E1e‑S1 — Establish Baseline
**Tasks**
1. Add Criterion bench `encode_a_record_trustdns`.  
2. Capture: allocations via `cargo instruments` or `valgrind --tool=massif`.  
3. Store baseline numbers (allocs, p50, p99) in `docs/perf/baseline.md`.

**Functional Reqs**
* Baseline file committed and referenced by CI.

**Non‑Functional**
* Benchmark variance ≤5%.  
* Script runnable in <2min.

---

## E1e‑S2 — Implement Domain Composer
**Tasks**
1. Add new module `stateless_dns::encode::domain_impl`.  
2. Use `domain::bits::Composer::with_capacity(512)` to write header, question echo, answer RRset, RRSIG.  
3. Abstract behind trait `DnsEncoder` with `fn encode(&self, rrset) -> BytesMut`.

**Functional**
* Encodes A + optional RRSIG identical to Trust‑DNS order.  
* Returns `BytesMut` ready for UDP send.

**Non‑Functional**
* Allocations per encode ≤5.  
* No dynamic re‑allocation (reserve exact length if possible).

---

## E1e‑S3 — Wire‑Format Parity Tests
**Tasks**
1. Property test 1000 random labels with `proptest`.  
2. Compare bytes equality: old vs new.  
3. Ensure DNSSEC bit flag identical.

**Functional**
* Test suite passes; any diff fails CI.  
* Output size identical except compression offset byte differences (still RFC‑compliant).

**Non‑Functional**
* Test runtime <30s.  
* Coverage ≥90% encode paths.

---

## E1e‑S4 — Unsafe Code Audit
**Tasks**
1. Run `cargo geiger` to detect unsafe blocks.  
2. If any unsafe needed (slice cast), wrap in small module + comment link to audit doc.  
3. Security lead reviews PR.

**Functional**
* CI fails if new unsafe appears un‑audited.

**Non‑Functional**
* Unsafe LOC ≤10.  
* Audit file `AUDIT.md` stored.

---

## E1e‑S5 — Runtime Metrics & Alert
**Tasks**
1. Histogram metric `dns_encode_latency_seconds{impl=domain}` via `metrics` crate.  
2. Grafana panel comparing `trustdns` vs `domain` for rollout canary.  
3. Alert if p95 > baseline *1.25.

**Functional**
* Exposed on `/metrics`; scraped every 15s.  
* Canary deploy toggles encoder via ENV `ENCODER_IMPL`.

**Non‑Functional**
* Metrics overhead <1µs.  
* Alert false positive <1/month.

---

©2025FleetingDNS — Zero‑Copy DNS Encode stories

