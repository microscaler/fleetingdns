# 📗 **E1j – WireGuard & eBPF Orchestration**  
*Sub-Epic → User-story breakdown (v0.1)*

Brings kernel‑mode performance and policy enforcement to FleetingDNS WireGuard transport by orchestrating peer lifecycle, key rotation, NAT‑less routing, and tenant fire‑walls through eBPF (Cilium) programs.

---

## Epic Goal
> “Evolve the userspace `boringtun` prototype into a fully‑orchestrated WireGuard dataplane using eBPF (either Cilium or standalone XDP), delivering sub‑millisecond latency, automatic peer provisioning, per‑tenant policy, and seamless rotation—without giving up stateless tunnel semantics.”

---

## 🗂️ Story List
| ID | Story | Outcome |
|----|-------|---------|
| **E1j-S1** | As a *NetOps*, spin up an **Edge node** that loads Cilium with WireGuard encryption enabled and advertises the anycast IP. |
| **E1j-S2** | As *API*, generate **peer config CRDs** (`FdnsPeer`) that Cilium agent consumes to create wg endpoints. |
| **E1j-S3** | As *Security*, rotate WireGuard public keys every 30min automatically with zero‑packet loss. |
| **E1j-S4** | As *SRE*, attach **eBPF L4 firewall** that enforces per‑tenant byte quota & blocks port scans. |
| **E1j-S5** | As *Perf engineer*, offload NAT‑less routing via **eBPF XDP** achieving ≥3Gbit/s on e2‑standard‑4 node. |
| **E1j-S6** | As *Observability*, export **eBPF perf events** to Otel → Mimir (`wg_packets_dropped_total`). |

---

## E1j-S1 — Cilium WireGuard Edge Node
**Tasks**
1. HelmRelease `cilium` with `encryption.mode=wireguard` and `encryption.interface=wg0`.  
2. Advertise anycast IP via kube‑router `bgp` or keep local route table.  
3. Health probe: `cilium status --wait`.

**Functional Reqs**
* Packets from client decrypt → pod in ≤1ms.  
* Anycast IP reachable from internet.

**Non-Functional**
* Node CPU overhead <5%.  
* Control‑plane latency unaffected.

---

## E1j-S2 — FdnsPeer CRD & Controller
**Tasks**
1. Define CRD `FdnsPeer` (spec: pubKey, allowedIps, expiresAt, tenantId).  
2. Controller (Rust operator) watches Redis keyspace events and creates peer CR objs.  
3. Cilium agent reconciles to `ciliumwireguardendpoint`.

**Functional**
* Peer appears within 5s of tunnel creation.  
* Deletes when TTL expired / tunnel closed.

**Non-Functional**
* Controller memory <128MiB.  
* CRD prop latency p95 ≤3s.

---

## E1j-S3 — Zero‑Downtime Key Rotation
**Tasks**
1. Gateway issues new pubKey (`pubKey2`) 5min before expire.  
2. Controller adds second peer entry; client allowedIps unchanged.  
3. After ACK, remove old key.

**Functional**
* No packet loss measured in iperf.  
* Rotation event logged.

**Non-Functional**
* Over‑the‑air rotation ≤30s.  
* No duplicate peers leak.

---

## E1j-S4 — eBPF Tenant Firewall / Quota
**Tasks**
1. Insert Cilium policy: `TenantID` label → allow port 80/443, deny others.  
2. eBPF map counts bytes per `tenant_id`.  
3. If > quota, drop & send trace event.

**Functional**
* Quota enforcement within ±1%.  
* 403 (RST) returned once quota reached.

**Non-Functional**
* Map memory ≤32B * tenants.  
* Policy compile <200ms.

---

## E1j-S5 — XDP Fast‑Path Routing
**Tasks**
1. Write XDP program in C → attach to `eth0`.  
2. Skip stack; use BPF map `tunnel_id -> pod veth`.  
3. Fallback to kernel if map miss.

**Functional**
* iperf shows ≥3Gbit/s throughput.  
* CPU util drop ≥30% vs userspace path.

**Non-Functional**
* Program size <8KB.  
* Verified with `bpftool prog load` CI.

---

## E1j-S6 — eBPF Metrics Export
**Tasks**
1. Use `libbpf-rs` perf events → user‑space scraper.  
2. Otel metric `wg_packets_dropped_total{reason, tenant}`.  
3. 1s scraping interval.

**Functional**
* Metric visible in Grafana within 10s.  
* Alert packets_dropped >100 / min.

**Non-Functional**
* Scraper CPU <2%.  
* Perf event rate limited (ring buffer 64KB).

---

© 2025 FleetingDNS — WireGuard & eBPF Orchestration stories

