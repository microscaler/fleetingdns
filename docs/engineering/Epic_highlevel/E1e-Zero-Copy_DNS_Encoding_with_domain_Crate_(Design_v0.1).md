# 📘 E1e – Zero‑Copy DNS Encoding with `domain` Crate (Design v0.1)

## 1 ▪ WHY

Our current Trust‑DNS implementation for stateless responses executes:

* `rr::Record::to_bytes()` → allocates a new `Vec<u8>`
* Serialises labels & RDATA per request (even though label bytes are already present in the query)

At 200 k QPS the allocations become non‑trivial (\~70 MB/s) and pressure the allocator.  The [`domain`](https://crates.io/crates/domain) crate offers **zero‑copy parsing & composable builders** built on `bytes::BytesMut`, enabling us to:

1. Re‑use the caller’s `Bytes` for owner name
2. Reserve exactly RDATA length (no re‑alloc)
3. Encode directly into the UDP buffer passed by Tokio, skipping the `Vec→Cursor→Bytes` hop that Trust‑DNS does.

Goal: **shave \~15 µs off encode path** at p99 and drop heap alloc count by 80 % for the stateless hot loop.

---

## 2 ▪ WHAT (evaluation scope)

* Prototype replacement **`StatelessEncoder`** using `domain::base::octets` builder.
* Bench vs current Trust‑DNS pipeline under:

    * A‑record (28 B) response
    * A+RRSIG (≈ 340 B) response
    * TXT (≤ 255 B) response
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

Target 1 M iterations w/ 512‑byte buffer.

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

© 2025 Ephemeral DNS Forwarder – Zero‑Copy Encode Investigation
