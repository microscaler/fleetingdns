# 📘 E1e – Zero‑Copy DNS Encoding with `domain` Crate (Design v0.1)

## 1 ▪ WHY

Our current Trust‑DNS implementation for stateless responses executes:

* `rr::Record::to_bytes()` → allocates a new `Vec<u8>`
* Serialises labels & RDATA per request (even though label bytes are already present in the query)

At 200k QPS the allocations become non‑trivial (\~70MB/s) and pressure the allocator.  The [`domain`](https://crates.io/crates/domain) crate offers **zero‑copy parsing & composable builders** built on `bytes::BytesMut`, enabling us to:

1. Re‑use the caller’s `Bytes` for owner name
2. Reserve exactly RDATA length (no re‑alloc)
3. Encode directly into the UDP buffer passed by Tokio, skipping the `Vec→Cursor→Bytes` hop that Trust‑DNS does.

Goal: **shave \~15µs off encode path** at p99 and drop heap alloc count by 80% for the stateless hot loop.

---

## 2 ▪ WHAT (evaluation scope)

* Prototype replacement **`StatelessEncoder`** using `domain::base::octets` builder.
* Bench vs current Trust‑DNS pipeline under:

    * A‑record (28B) response
    * A+RRSIG (≈ 340B) response
    * TXT (≤ 255B) response
* Metrics: allocations/op, p50/p99 encode ns, max throughput (Mreq/s).
* Decide: **full migration**, **hybrid (only stateless path)**, or **no‑go**.

---

## 3 ▪ HOW – prototype sketch

```rust
use domain::base::{MessageBuilder, iana::Class, iana::Rtype};
use domain::rdata::{A, Txt};

fn build_a(buf: &mut [u8], name: &str, ip: Ipv4Addr, ttl: u32,
           signer: &Signer) -> usize {
    let mut mb = MessageBuilder::from_target(buf).unwrap();
    let (header, mut msg) = mb.answer_message(0.into()).unwrap();
    header.set_recursion_available(true);
    let owner = msg.push_qname(name).unwrap();
    msg.push((owner, Class::In, ttl, &A::from(ip))).unwrap();
    // sign rrset
    let sig = signer.sign_a(owner, ip, ttl, ts());
    msg.push(('name, Class::In, ttl, &sig)).unwrap();
    msg.len()
}
```

*Zero alloc path*: buffer provided by Tokio Udp socket, we fill in‑place.

### Integration plan

1. Feature‑gate: `--fast-encode`.
2. Inside `StatelessAuthority::lookup` choose encoder based on flag.
3. Keep Trust‑DNS builder for Redis authority (lower QPS, complexity).

---

## 4 ▪ Bench harness

```rust
criterion_group!(benches, bench_trustdns, bench_domain);
criterion_main!(benches);
```

Target 1M iterations w/ 512‑byte buffer.

Expected: `domain` path shows **\~3× speed‑up** encode and **0 alloc**.

---

## 5 ▪ Risks & mitigations

| Risk                                                          | Mitigation                                                                                         |
| ------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| `domain` crate maintenance status (last release 0.7, 2023‑12) | Fork if upstream stalls; code is `#![forbid(unsafe_code)]` and audited by NLnet project.           |
| API divergence vs Trust‑DNS updates                           | Encapsulate encoder in small module; rest of edge still uses Trust‑DNS for query parsing/dispatch. |
| In‑place buffer overflow                                      | Unit tests + `debug_assert!(len≤buf.len())`; fuzz with afl/quickcheck.                             |

---

## 6 ▪ Deliverables

* [ ] PoC encoder (`stateless_enc.rs`).
* [ ] Criterion benches vs baseline.
* [ ] Toggle in `Cargo.toml` feature `domain-encode`.
* [ ] Doc update in E1 epic.

---

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

