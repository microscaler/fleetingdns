# 📗 **E7 – Security & Hardening**  
*Epic → User-story breakdown (v0.1)*

E7 aggregates all advanced security measures: PKI infra, cert lifecycles, penetration‑test findings, central audit logging, zero‑trust ingress, SLSA attestation, and hardened container images.

---

## Epic Goal
> “Achieve a defense‑in‑depth posture that withstands cloud compromise attempts, satisfies SOC2 TypeII controls, and enables continuous pen‑test remediation without service downtime.”

---

## 🗂️ Story List
| ID | Story | Outcome |
|----|-------|---------|
| **E7-S1** | As a *PKI engineer*, run an in-house **Rust CA service** signing 30‑min client certs with root in CloudKMS HSM. |
| **E7-S2** | As *Pen‑Test team*, exploit path scanning; alerts fire in <60s via GuardDuty‑equivalent (Cloud IDS). |
| **E7-S3** | As *SecOps*, enable **binary signing & image SBOM** prior to registry push. |
| **E7-S4** | As *Audit*, have **centralised Loki logs** with encrypted at-rest and immutable 30‑day retention. |
| **E7-S5** | As *Dev*, build containers from **distroless** base + non‑root user; image scan passes Trivy. |
| **E7-S6** | As *SRE*, enforce **NetworkPolicy** zero‑trust (EdgeHub pods may reach Redis only). |
| **E7-S7** | As *Compliance*, run **quarterly chaos/pen test** and track Jira tickets automatically. |

---

## E7-S1 — Rust CA Service & HSM
**Tasks**
1. Write `ca_service` crate calling Cloud KMS Sign API.  
2. Service runs Deployment with K8sHPA.  
3. Rotate intermediate every 7days.

**Functional Reqs**
* Sign CSR ≤ 50ms.  
* Root key never leaves HSM.

**Non‑Functional**
* TPS at least 2000 CSR/s.  
* Availability ≥99.95%.  

---

## E7-S2 — IDS & Alerting
**Tasks**
1. Enable CloudIDS on workload VPC subnets.  
2. Route findings to Security Command Center → PagerDuty.  
3. Simulate NMap scan in staging.

**Functional**
* Alert in Slack/PagerDuty within 60s of scan.  
* Event stored in BigQuery security dataset.

**Non‑Functional**
* False‑positive rate <0.5%.  
* IDS cost kept <€30/mo per region.

---

## E7-S3 — Signature + SBOM
**Tasks**
1. `make_release.sh` step: `syft sbom`, `cosign sign --key=kms://…`.  
2. Enforce `cosign verify` admission controller.  
3. Publish SBOM to Artifact Registry.

**Functional**
* Deploy only if image signature valid & SBOM attached.  
* SLSA provenance attestation attached.

**Non‑Functional**
* Release pipeline overhead <45s.  
* SBOM size <200KB compressed.

---

## E7-S4 — Central Loki Logs
**Tasks**
1. HelmRelease Loki+ Promtail sidecars.  
2. Encrypt bucket with CMEK.  
3. Immutable retention30days with auto‑archive to GCS.

**Functional**
* `kubectl logs` still shows std‑out.  
* Loki query returns EdgeHub request in <2s.

**Non‑Functional**
* Storage growth ≤1GB/day at 10k tunnels.  
* Query concurrency ≥5.

---

## E7-S5 — Distroless & Trivy
**Tasks**
1. Switch Dockerfile base to `gcr.io/distroless/cc`.  
2. Add Trivy scan in CI runner; fail severity ≥HIGH.

**Functional**
* No root user in `/etc/passwd`.  
* 0 HIGH/Critical vulns.

**Non‑Functional**
* Image size <25MB.  
* Scan time ≤20s.

---

## E7-S6 — Zero‑Trust NetworkPolicies
**Tasks**
1. Calico NetworkPolicy default deny.  
2. Allow EdgeHub → Redis, EdgeHub → API only.  
3. Periodic audit with `kubectl‑netpol`.  

**Functional**
* Block pod →cloud‑metadata attempts.  
* All egress logged.

**Non‑Functional**
* Netpol rule count <50.  
* Policy update latency <10s.

---

## E7-S7 — Quarterly Pen‑Test Automation
**Tasks**
1. GitHub Action scheduled workflow: spin staging env, run OWASPZAP + Nuclei.  
2. Parse report; open Jira tickets per CVE.  
3. Auto‑close ticket when commit hash contains `FixCVE‑…`.

**Functional**
* Workflow passes or fails build gate.  
* Jira ticket fields autopopulated.

**Non‑Functional**
* Pen‑test window <40min.  
* False‑positive ticket <5%.

---

©2025 FleetingDNS — Security & Hardening stories

