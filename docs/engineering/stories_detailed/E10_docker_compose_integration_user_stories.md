# 📗 **E10 – Docker‑Compose Integration**  
*Epic → User‑story breakdown (v0.1)*

Makes multi‑service Compose stacks “just work” via a side‑car container that auto‑proxies multiple services under one FleetingDNS tunnel.

---

## Epic Goal
> “Let developers add a single `edf‑proxy` container to any docker‑compose.yml and instantly expose all services through a single public URL, with zero port clashes, live logs, and automatic tunnel teardown when the stack stops.”

---

## 🗂️ Story List
| ID | Story | Outcome |
|----|-------|---------|
| **E10‑S1** | As a *Node dev*, drop `edf.compose.yml` override and get **one URL** for both `web` & `api` services. |
| **E10‑S2** | As *QA*, run `docker compose up` on macOS **without binding host ports**. |
| **E10‑S3** | As *CLI user*, `edf tunnel ls` shows compose‑sidecar tunnel; stopping compose removes it. |
| **E10‑S4** | As *Security*, secrets (certs/keys) stay **inside container tmpfs** and wiped on stop. |
| **E10‑S5** | As *SRE*, side‑car emits **Prometheus metrics** (`proxy_http_latency_ms`) for each service. |
| **E10‑S6** | As *DX*, `edf-compose plugin` scaffolds override file automatically. |

---

## E10‑S1 — Side‑car Proxy Container
**Tasks**
1. Build `client-images/compose-sidecar/Dockerfile` (scratch + edf-cli + socat).  
2. Entrypoint parses `$EDF_PORT_MAP` env or Docker API.  
3. Calls `edf-cli tunnel --map-file routes.json` and spawns socat proxies.

**Functional Reqs**
* Supports mapping `service:containerPort -> /servicePath` or SNI `service.fqdn`.  
* Outputs tunnel FQDN to stdout.

**Non‑Functional**
* Start‑up time < 3 s.  
* Image size < 25MB.

---

## E10‑S2 — Host‑less Networking (Desktop)
**Tasks**
1. Validate path routing on Docker Desktop (VM NAT).  
2. Document `--publish` fallback for Windows.  
3. Provide troubleshooting `edf diag compose`.

**Functional**
* No host ports exposed (`ports:` optional).  
* Webhook hits succeed to container service.

**Non‑Functional**
* Added latency ≤1ms vs host networking.  
* Works with Mutagen/Colima.

---

## E10‑S3 — Lifecycle Sync
**Tasks**
1. Side‑car catches SIGTERM on `docker compose down`.  
2. Calls `DELETE /v1/tunnels/{id}` then exits.  
3. `edf tunnel ls` reflects removal.

**Functional**
* No orphan tunnels 5s after compose stop.  
* Re‑`up` creates new tunnel.

**Non‑Functional**
* Cleanup race‑condition < 1%.  
* Redis slots freed promptly.

---

## E10‑S4 — Secret Hygiene
**Tasks**
1. Mount tmpfs `/run/edf`.  
2. `shred` keys on exit handler.  
3. Validate with `docker diff` (no key file).

**Functional**
* Private key never on persistent volume.  
* Certs wiped after tunnel close.

**Non‑Functional**
* Memory overhead ≤2MiB.  
* Pass Docker Bench security checks.

---

## E10‑S5 — Metrics Export
**Tasks**
1. Expose `/metrics` in side‑car, labels `service` and `code`.  
2. Promtail scrape config example.  
3. Grafana dashboard panel.

**Functional**
* Histogram `proxy_http_latency_ms_bucket`.  
* Query lat p95 < 5ms.

**Non‑Functional**
* Metrics export <0.5% CPU.  
* Dashboard auto‑discovers new services.

---

## E10‑S6 — Scaffold Plugin
**Tasks**
1. Extend `edf-cli compose init` to write `edf.compose.yml` with port map guesses.  
2. Detect conflicts and suggest env edits.  
3. Unit‑test against sample stacks.

**Functional**
* Running plugin then `docker compose up -d` yields working tunnel.  
* Idempotent: re‑run overwrites file safely.

**Non‑Functional**
* Generation time < 500ms.  
* Coverage: detects ≥90% common Compose patterns.

---

© 2025 FleetingDNS — Docker‑Compose Integration stories

