# 📗 **E9 – Point‑to‑Point WireGuard Tunnels**  
*Epic → User-story breakdown (v0.1)*

Adds UDP‑based WireGuard P2P transport alongside HTTP/TLS tunnels for ultra‑low latency and UDP protocol support.

---

## Epic Goal
> “Offer developers a one‑command WireGuard tunnel that hides NAT, provides /32 IP per session, and maintains FleetingDNS security model—with <10ms latency budget and automatic expiry.”

---

## 🗂️ Story List
| ID | Story | Outcome |
|----|-------|---------|
| **E9-S1** | As a *CLI user*, run `edf tunnel --mode wg` and receive `wg.conf` snippet + FQDN. |
| **E9-S2** | As *EdgeHub*, accept WG handshake on UDP/51820 and route packets to reverse interface. |
| **E9-S3** | As *API*, allocate unique **/32 IP** from `10.10.0.0/16` pool and include in response. |
| **E9-S4** | As *SRE*, expose **Prom metrics** (`wg_handshakes_total`, `wg_bytes`) for capacity planning. |
| **E9-S5** | As *Security*, rotate WG keys every session; optional PSK layer. |
| **E9-S6** | As *Network admin*, configure **LB forwarding rule** UDP 51820 and health checks. |

---

## E9-S1 — CLI WireGuard Provision
**Tasks**
1. Extend `edf-cli` arg parser (`--mode wg`).  
2. On 201 response, write `~/edf/wg.conf`.  
3. Call `wg-quick up` if binary found else print instructions.

**Functional**
* File contains `[Interface]` with private key and `[Peer]` pointing to `wg.edf.run:51820`.  
* CLI prints assigned IP and expiry.

**Non-Functional**
* Conf file chmod 600.  
* Setup function completes < 1s.

---

## E9-S2 — EdgeHub WG Listener
**Tasks**
1. Integrate `boringtun` userspace WG.  
2. Map `peer_public_key → unix socket` via slot metadata.  
3. Handle `wgctrl` dump for metrics.

**Functional**
* Handshake accepted only if slot exists & not expired.  
* Packets forwarded bi‑directionally.

**Non-Functional**
* Sustains ≥8000 peers per pod.  
* Added latency < 1ms.

---

## E9-S3 — IPAM Allocation
**Tasks**
1. `wg_ipam` crate: bitmap allocator in Redis `wg:ip_pool`.  
2. API assigns `/32` client IP & `/32` server IP.  
3. Release on tunnel expiry.

**Functional**
* Collision probability <1e‑9.  
* Supports 65k addresses.

**Non-Functional**
* Allocation/dealloc O(1).  
* Redis storage <10KB.

---

## E9-S4 — Metrics
**Tasks**
1. Expose `/metrics` counters from `boringtun` stats.  
2. Dashboard panel latency, bytes, peers.  
3. Alert: peers >7500 triggers HPA scale.

**Functional**
* `wg_handshakes_total` increments every handshake.  
* HPA rule uses `peers_per_pod`.

**Non-Functional**
* Metrics scrape ≤0.5% CPU.  
* Alert false positive <1/mo.

---

## E9-S5 — Key Rotation & PSK
**Tasks**
1. CLI generates Curve25519 pair; never stored on disk (mem only).  
2. API returns server pubkey & optional PSK.  
3. PSK enabled via `--psk` flag.

**Functional**
* Keys expire with tunnel; new tunnel new key.  
* Server rejects reuse.

**Non-Functional**
* Keygen ≤1ms.  
* PSK overhead negligible.

---

## E9-S6 — Load Balancer Rule
**Tasks**
1. Add `forwarding-rule-wg.yaml` UDP/51820 anycast.  
2. Health check: UDP echo.  
3. Terraform / Cloud console disabled via IAM policy.

**Functional**
* WG traffic hits nearest POP to edge pod.  
* Health check determines pod readiness.

**Non-Functional**
* Cold‑start <90s rule propagation.  
* LB charge ~€4.70/mo.

---

©2025 FleetingDNS — WireGuard Tunnels stories

