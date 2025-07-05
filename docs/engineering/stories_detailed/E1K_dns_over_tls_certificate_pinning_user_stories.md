# 📗 **E1k – DNS‑over‑TLS Certificate Pinning**  
*Sub‑Epic → User‑story breakdown (v0.1)*

Adds a DNS‑over‑TLS (DoT, RFC7858) endpoint with public‑key pinning to prevent on‑path TLS interception and guarantee query integrity for CI runners and security‑sensitive customers.

---

## Epic Goal
> “Expose `tls.fleetingdns.run:853` as a first‑class DoT service secured by a pinned SPKI hash, integrate pin‑set distribution into the CLI/SDKs, and rotate certificates without breaking clients.”

---

## 🗂️ Story List
| ID | Story | Outcome |
|----|-------|---------|
| **E1k‑S1** | As a *DevOps*, resolve tunnel labels via **DoT endpoint** that supports TLS1.3 and ALPN `dot`. |
| **E1k‑S2** | As a *CLI user*, have **SPKI pin** embedded so MITM attempts fail hard. |
| **E1k‑S3** | As *Security*, implement **pin‑set rotation** (old+new overlap) every 90days. |
| **E1k‑S4** | As *SDK maintainer*, fetch **pin‑set JSON** from control API to stay up‑to‑date. |
| **E1k‑S5** | As *SRE*, monitor handshake failures with metric `dot_tls_pin_mismatch_total` and alert. |
| **E1k‑S6** | As *Client*, gracefully **fallback** to DoH → UDP if DoT unavailable. |

---

## E1k‑S1— DoT Server Deployment
**Tasks**
1. Add `tcp://0.0.0.0:853` listener in `dnsd` (rustls).  
2. Negotiate ALPN `dot`; enforce TLS1.2+ (pref1.3).  
3. Expose anycast IP via same L4 LB (TCP853).  
4. Cert from ACME wildcard `*.fleetingdns.run`.

**Functional**
* `kdig @tls.fleetingdns.run +tls-ca +tls-host=tls.fleetingdns.run <label>.fleetingdns.run` works.  
* p95 handshake≤50ms.

**Non‑Functional**
* CPU overhead <10% at 10k QPS.  
* HSTS‑style preload header documented.

---

## E1k‑S2— SPKI Pin Embedded in Clients
**Tasks**
1. Calculate base64SHA‑256 of leaf public key (`pin‑v1`).  
2. Publish in `/v1/dot/pins` endpoint.  
3. Embed in `edf‑cli`, SDKs constant list `[pin‑v1]`.

**Functional**
* CLI fails with explicit error if server cert hash ≠ pin set.  
* `--insecure‑dot` bypass flag for dev.

**Non‑Functional**
* Pin list update via `edf update` command (<200ms).  
* No silent fallback without pin.

---

## E1k‑S3— Pin‑Set Rotation Policy
**Tasks**
1. Generate new keypair `pin‑v2` every 90days.  
2. Serve **both** public keys in pin‑set JSON for 30‑day overlap.  
3. Roll server cert to new key after overlap passes.

**Functional**
* Clients with old binaries (pin‑v1) continue to connect until overlap ends.  
* Email reminder to upgrade SDK if pin >180days.

**Non‑Functional**
* Overlap window configurable (env).  
* Rotation cron job logs success.

---

## E1k‑S4— Pin‑Set Fetch in SDKs
**Tasks**
1. SDKs on init GET `/v1/dot/pins` (cached 24h).  
2. Merge with embedded list.  
3. Verify SPKI hash against union.

**Functional**
* Even if client embed outdated, live fetch provides new pins.  
* Offline mode still works with embedded pin.

**Non‑Functional**
* Fetch timeout 500ms; cached response in memory.  
* Pin JSON size ≤1KiB.

---

## E1k‑S5— Metrics & Alerts
**Tasks**
1. Expose `dot_tls_pin_mismatch_total` & `dot_handshake_fail_total`.  
2. Alert if mismatch >10/min.  
3. Grafana panel handshake duration histogram.

**Functional**
* Alert routed to #security within 5min.  
* Panel loads <3s.

**Non‑Functional**
* Metrics cardinality small.  
* Alert false positive <1/mo.

---

## E1k‑S6— Client Fallback Chain
**Tasks**
1. CLI/SDK resolver order: DoT → DoH (`https://dns.fleetingdns.run/dns‑query`) → UDP53.  
2. Record diagnostic reason.  
3. Surface warning if DoT fails but others succeed.

**Functional**
* Test: block 853 → DoH still resolves.  
* Exit code non‑zero if only UDP path worked and `--require‑dot` set.

**Non‑Functional**
* Fallback latency adds <50ms.  
* Stats on fallback reasons collected.

---

©2025FleetingDNS— DNS‑over‑TLS Certificate Pinning stories

