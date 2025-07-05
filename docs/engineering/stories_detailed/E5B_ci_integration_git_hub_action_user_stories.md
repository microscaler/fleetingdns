# 📗 **E5B – CI Integration (GitHub Action)**  
*Sub-Epic → User-story breakdown (v0.1)*

Delivers an official GitHub Action that spins up a FleetingDNS tunnel at job start and tears it down automatically, exposing the public URL as an output for test pipelines.

---

## Epic Goal
> “Enable any GitHub Actions workflow to add `uses: fleetingdns/action@v1` and instantly receive a temporary tunnel for webhooks or end-to-end tests, with zero manual token handling and auto-cleanup on job finish.”

---

## 🗂️ Story List
| ID | Story | Outcome |
|----|-------|---------|
| **E5B-S1** | As a *CI user*, reference the Action and get `$TUNNEL_URL` output env var. |
| **E5B-S2** | As *Security*, authenticate Action via **GitHub OIDC workload identity** — no static token secrets. |
| **E5B-S3** | As *SRE*, ensure tunnel is **deleted on success, failure, or cancelled job** to avoid leaks. |
| **E5B-S4** | As *DX*, Action supports **matrix jobs**, opening independent tunnels per shard. |
| **E5B-S5** | As *Marketplace maintainer*, publish Action with **README + badge** and semantic version tags. |

---

## E5B-S1 — Action Core Logic
**Tasks**
1. Create `action.yml` with `uses: docker://ghcr.io/fdns/gha-runner:<tag>`.  
2. Entry script `start_tunnel.sh` calls `edf-cli tunnel --json` and sets `TUNNEL_URL` via `set-output`.  
3. `post` step stored to run on cleanup.

**Functional Reqs**
* Output variable `url` accessible in workflow.  
* Supports inputs `ttl`, `mode`, `redirect-url`.

**Non-Functional**
* Start time ≤10 s.  
* Image size < 80 MB.

---

## E5B-S2 — OIDC Auth
**Tasks**
1. Configure GCP Workload Identity Pool provider for `repo:*`, audience `fdns-gha`.  
2. Action obtains OIDC JWT via `${{ steps.auth.outputs.id_token }}`.  
3. Exchange JWT for short-lived API token via `/v1/auth/gha-token`.

**Functional**
* No PAT or secret needed in repo.  
* Token TTL ≤ 60 min.

**Non-Functional**
* Token exchange latency < 200 ms.  
* Audit log entry in GCP.

---

## E5B-S3 — Auto Clean-up
**Tasks**
1. Use composite action `post:` script `cleanup_tunnel.sh`.  
2. Trap EXIT, INT signals.  
3. Retry DELETE if 5xx.

**Functional**
* Tunnel deleted within 5 s regardless of job outcome.  
* Logs show `Deleted tunnel id ...`.

**Non-Functional**
* Orphan rate < 0.1 %.  
* Cleanup network errors logged.

---

## E5B-S4 — Matrix Support
**Tasks**
1. Action accepts `matrix-index` input to create unique tunnel per job.  
2. Output var names include index suffix.  
3. Docs example with `strategy.matrix.browser`.

**Functional**
* Each matrix shard gets distinct URL.  
* No collisions.

**Non-Functional**
* Additional runtime per shard < 1 s.  
* Max 10 parallel tunnels (Team quota enforced).

---

## E5B-S5 — Marketplace Publishing
**Tasks**
1. Add README with usage, inputs, outputs, example workflow.  
2. Tag v1, v1.0.0 on GitHub.  
3. Submit listing; add Shields.io badge.

**Functional**
* Action visible in Marketplace search within 24 h.  
* README renders badge and example.

**Non-Functional**
* Release workflow auto-publishes on tag push.  
* Semantic-release changelog generated.

---

© 2025 FleetingDNS — GitHub Action CI Integration stories

