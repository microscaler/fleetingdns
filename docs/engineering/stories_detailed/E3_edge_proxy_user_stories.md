# 📗 **E3 – Edge Proxy**  
*Epic → User-story breakdown (v0.1)*

Edge Proxy is the **EdgeHub data‑plane module** that terminates client tunnels (TLS‑wrapped SSH or WireGuard), validates certs, enforces auth/redirect policies, compresses traffic, and forwards streams to the developer’s reverse‑tunnel socket.

---

## Epic Goal
> “Accept public traffic on anycast IP, validate short‑lived client credentials, multiplex compressed TCP streams to the correct reverse tunnel, and expose metrics for billing — all within 1 ms added latency.”

---

## 🗂️ Story List
| ID        | Story                                                                                        | Outcome |
|-----------|----------------------------------------------------------------------------------------------|---------|
| **E3-S1** | As a *tunnel client*, initiate **TLS‑wrapped SSH** to edge and get a working reverse tunnel. |
| **E3-S2** | As *EdgeHub*, decode `fqdn` label → Redis slot and **route traffic** to correct Unix socket. |
| **E3-S3** | As a *dev*, enable **HTTP Basic Auth / redirect** so only test harness can hit my tunnel.    |
| **E3-S4** | As a *Finance team*, count **bytes in/out** per tunnel for billing.                          |
| **E3-S5** | As *NetOps*, compress traffic with **zstd** when `accept‑encoding: zstd` to save egress.     |
| **E3-S6** | As *SRE*, have **K8s HPA / Flagger Canary** rules for Edge pod autoscale + health checks.    |

---

## E3-S1 — TLS‑SSH Handshake
**Tasks**
1. Implement listener on port 443 in `edge-hub/src/tls_ssh.rs`.  
2. Use `tokio-rustls` server config with **client‑cert auth required**.  
3. On handshake, extract `tunnel_id` from cert SAN.

**Functional Requirements**
* Accepts TLS 1.3 only; certificate chain must be signed by in‑house CA & notExpired.  
* Upgrades to OpenSSH `direct‑tcpip` reverse tunnel over same connection.

**Non-Functional**
* p95 handshake < 50 ms (including TLS).  
* Reject invalid certs with alert `bad_certificate`.

---

## E3-S2 — Slot Router
**Tasks**
1. Parse incoming `:authority` / SNI host → label.  
2. Validate HMAC with shared key.  
3. Lookup `slot:{id}` in Redis; retrieve Unix socket path.  
4. Proxy bytes bidirectionally (`tokio::io::copy_bidirectional`).

**Functional**
* 404 (HTTP) or `Channel open failure` (SSH) if slot missing/expired.  
* Close connection when Redis key TTL <= 0.

**Non-Functional**
* Added latency per packet ≤ 1 ms.  
* Concurrent sockets per pod ≥ 2 000.

---

## E3-S3 — Basic Auth & Redirect Mode
**Tasks**
1. Add `--auth=basic|none|oidc` flag.  
2. For basic: read `Authorization` header, compare bcrypt hash from Redis slot metadata.  
3. For redirect‑only: respond 302 to `location:` set in metadata.

**Functional**
* CLI receives `basic_user`/`basic_pass` JSON when requesting tunnel.  
* Edge returns `401` if creds missing/wrong.

**Non-Functional**
* Hash comparison constant‑time.  
* Redirect preserves query params.

---

## E3-S4 — Byte Accounting Metrics
**Tasks**
1. Wrap proxy streams in `ByteMeter` struct counting IN and OUT.  
2. Push to Redis `HINCRBY slot:{id} tx_bytes rx_bytes` every 5 s.  
3. On slot expiry, push usage record to Stripe via API.

**Functional**
* Accuracy ±1 % compared to tcpdump sample.  
* Stripe usage record includes plan_id.

**Non-Functional**
* Meter overhead < 5 µs per flush.  
* Redis pipeline batch keeps updates <500 ops/s.

---

## E3-S5 — zstd Compression
**Tasks**
1. Detect `accept-encoding: zstd`.  
2. Wrap stream in `zstd::stream::Encoder` / `Decoder`.  
3. Add `--compression-threshold=16k` flag.

**Functional**
* Throughput gain >30 % at compression ratio >1.3.  
* Falls back gracefully if header absent.

**Non-Functional**
* CPU overhead < 5 % at 100 Mbps.  
* Latency budget increase < 2 ms for compressed path.

---

## E3-S6 — Autoscaling & Health Probes
**Tasks**
1. Add liveness `/healthz` and readiness `/readyz`.  
2. Define Flagger Canary metrics (p95 < 200 ms, 5xx <2 %).  
3. HPA: scale pods 1→8 on `connections_per_pod > 1 000`.

**Functional**
* Pod ready in < 5 s.  
* Canary rollback on failed metrics.

**Non-Functional**
* Scale‑up time < 30 s to add pod.  
* No dropped connections during rollout.

---

© 2025 FleetingDNS — Edge Proxy stories

