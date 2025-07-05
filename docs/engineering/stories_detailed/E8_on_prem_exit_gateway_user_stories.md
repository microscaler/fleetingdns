# 📗 **E8 – On‑Prem Exit Gateway (Raw TCP Bundle)**  
*Epic → User-story breakdown (v0.1)*

This epic packages a self‑hosted Rust binary (`exit-gw`) + Helm chart that customers can deploy on‑prem to terminate raw TCP tunnels while still using the FleetingDNS control‑plane.

---

## Epic Goal
> “Ship a turnkey exit‑gateway that enterprises can run on their own VM or cluster, handle raw TCP traffic, register to FleetingDNS API for slot metadata, and feed back usage data for billing — all with the same security guarantees as SaaS edge.”

---

## 🗂️ Story List
| ID | Story | Outcome |
|----|-------|---------|
| **E8-S1** | As an *Enterprise admin*, download `exit-gw` binary, run it on‑prem, and serve tunnels locally. |
| **E8-S2** | As a *DevOps*, install **Helm chart** in k8s; configure TLS cert and Redis endpoint via values file. |
| **E8-S3** | As *Security team*, bind gateway to **static IP allow‑list** and disable interactive SSH. |
| **E8-S4** | As *Billing backend*, receive gateway‑reported **bytes / duration** so SaaS usage is still invoiced. |
| **E8-S5** | As *Customer SRE*, upgrade exit‑gw with **zero downtime** using Helm rolling update. |
| **E8-S6** | As *Sales*, issue **license key** that unlocks gateway binary for one site / DNS wildcard. |

---

## E8-S1 — Binary Runtime
**Tasks**
1. Build `cmd/exit-gw` main (reuse tunnel_core).  
2. CLI flags: `--listen 0.0.0.0:443`, `--api=https://api.fd.run`, `--token=XXXX`.  
3. Connect WebSocket to API `/v1/gateway/register` for slot updates.

**Functional Reqs**
* Accept incoming TLS‑SSH from tunnel client; forward to reverse socket.  
* Pull slot details every 15 s heartbeat.

**Non-Functional**
* Handles 5 k concurrent sockets on 2 vCPU.  
* Memory ≤ 300 MiB.

---

## E8-S2 — Helm Chart
**Tasks**
1. Create `charts/exit-gw/Chart.yaml`, Deployment, Service, ConfigMap.  
2. Values: `apiUrl`, `licenseKey`, `image.tag`, `resources`.

**Functional**
* `helm install fdns-exit .` exposes `LoadBalancer` on 443.  
* Pod healthy per `/healthz` probe.

**Non-Functional**
* Upgrade strategy `RollingUpdate maxSurge=1`.  
* Chart install time < 60 s.

---

## E8-S3 — IP Allow‑list & Hardened SSH
**Tasks**
1. CLI option `--allow 203.0.113.0/24,198.51.100.4`.  
2. Disable shell on reverse tunnels (`no-pty,no-shell` in SSH subsystem).  
3. CIS benchmark script.

**Functional**
* Drop connection if remote addr not in allow list.  
* Only `direct-tcpip` channel accepted.

**Non-Functional**
* Blocked attempt logged with severity WARN.  
* Pen test verifies no interactive shell.

---

## E8-S4 — Usage Export
**Tasks**
1. Meter bytes and duration per slot.  
2. HTTPS POST `/v1/usage` every 5 min with HMAC‑signed payload.  
3. Retry on 5xx backoff 1→32 s.

**Functional**
* Backend reconciles usage with 99.9% accuracy.  
* Duplicate usage record deduped by UUID.

**Non-Functional**
* Payload < 2 KB per 1 k slots.  
* Lost data tolerated ≤ 0.1 % per month.

---

## E8-S5 — Zero‑Downtime Upgrade
**Tasks**
1. Readiness probe drains only when 0 active sessions.  
2. HAProxy ingress example for blue/green.  
3. Helm `preUpgrade` hook to create new Deployment, then delete old.

**Functional**
* During upgrade, new connections routed to new pod; existing continue until done.  
* `helm rollback` possible.

**Non-Functional**
* Connection loss <0.01 %.  
* Upgrade window < 2 min for 1 pod.

---

## E8-S6 — License Key Verification
**Tasks**
1. Gateway reads `--license` JWT signed by SaaS key.  
2. Contains `sub: company`, `aud: exit-gateway`, `exp`.  
3. Refuse start if invalid.

**Functional**
* License limited to `fqdnSuffix` and `sites <= 1`.  
* Renew via `edf-cli license renew`.

**Non-Functional**
* Startup latency for JWT verify < 50 ms.  
* Key rotation every 6 months.

---

© 2025 FleetingDNS — On‑Prem Exit Gateway stories

