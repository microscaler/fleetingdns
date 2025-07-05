# 📘 E7 – Security & Hardening (Design v0.1)

## 🧭 Overview

This document describes the **Security and Hardening Plan** for FleetingDNS (FDF), covering all attack surfaces across API, DNS, tunnel transport, certificate handling, endpoint lifecycle, and user auth. It ensures that tunnels and ephemeral domains cannot be abused for persistent access, resource exhaustion, or attack redirection.

FDF adheres to zero-trust, ephemeral-first, identity-bound session design. All long-lived secrets are user-scoped, and all exposed resources (tunnels, DNS entries, certs) have enforced TTLs and cryptographic validation.

---

## 🎯 Objectives

* Prevent unauthorized use of tunnels or domain records
* Secure all network transport layers with encryption & identity validation
* Ensure that expired/abandoned sessions are destroyed
* Defend against brute-force, enumeration, abuse, and privilege escalation

---

## 🔐 Security Model Summary

| Layer        | Security Mechanism                                          |
| ------------ | ----------------------------------------------------------- |
| CLI → API    | HTTPS with JWT access tokens (GitHub/OIDC bound)            |
| CLI → Hub    | TLS-wrapped SSH with ephemeral client cert signed by FDF CA |
| Edge → Hub   | Authenticated HTTP2 stream per slot ID, no shared sockets   |
| DNS records  | Short TTL, etcd key expiry, no wildcard routing fallback    |
| Tunnel user  | Non-login system user, isolated reverse-only port           |
| Audit & Logs | All control-plane operations logged with actor/session      |

---

## 📦 TLS and Certificate Handling

### Ephemeral Client Cert Flow

* Each `edf forward` session receives a signed ephemeral TLS certificate with a 30–60 min expiry
* Signed by FDF-owned internal CA (X.509 root pinned in hub/edge components)
* Private key generated in CLI, stored in memory only
* Certificate + public key stored in etcd and cached in hub for validation

### Certificate Hardening

* TLS 1.3 only (rustls)
* No weak cipher fallback
* Certs scoped to one domain (SAN = `abc123.edf.run`)
* Expiry hard-enforced by edf-hub listener (no session renewal allowed)

---

## 🔐 Endpoint Identity & Auth

### GitHub / OIDC Binding

* All CLI requests use access tokens validated server-side
* CLI tokens stored securely (e.g., local encrypted keyring or memory-only session)
* Optional secondary factor (TOTP/OIDC assertion) for paid org users

### Tunnel Isolation

* Each tunnel maps to a dedicated ephemeral port/slot on the hub
* Each tunnel user is a reverse-only, no-shell Unix account (nologin, scp disabled)
* No long-lived socket multiplexing; one stream = one endpoint

---

## 🧱 DNS Integrity & Abuse Prevention

* CoreDNS only serves explicitly created records (no wildcard handler fallback)
* Records expire at TTL or are forcibly GC’d every 30s
* DNSSEC optional for custom domain subzones (future)
* Rate-limit DNS create/delete to mitigate botnet abuse

---

## 🛡️ Edge Protection

### Input Validation

* All headers and paths normalized and size-checked
* UTF-8 and path traversal sanitization

### Request Rate Limiting

* Per-IP and per-subdomain token buckets at edge proxy
* Block threshold: 100 requests/min IP default, burst tokens per plan

### Signature / Auth Enforcement

* Basic auth must match Argon2id-hashed secret
* HMAC signature (e.g. Stripe-style `X-Signature`) constant-time verified
* OIDC Bearer tokens validated via JWKS with caching and TTL

---

## 🔒 Abuse Scenarios & Mitigations

| Threat                   | Defense                                             |
| ------------------------ | --------------------------------------------------- |
| Brute force on endpoints | Unpredictable subdomains, rate limits               |
| Tunnel reuse             | One-time ephemeral certs and slot reservations      |
| DNS abuse (phishing)     | TTL + auto GC + blacklisting domains with patterns  |
| CI loops / abuse         | Per-IP + per-token rate limits, webhook alerting    |
| Compromised CLI          | Scoped token, session-bound keys, short-lived certs |

---

## 🪪 Audit Logging

* All endpoint lifecycle events logged with:

    * actor (user ID or token ID)
    * IP address and User-Agent
    * timestamps for create/delete/expire
* Sensitive values (e.g. password, JWT) are redacted from logs

---

## 🔁 Session GC & Recovery

* API maintains expiry watchdog (cron/loop)
* If a tunnel is idle or expired, forcibly closes it at the hub
* Any DNS record with no valid tunnel or expired TTL is invalidated
* Cert revocation handled by TTL expiration (no CRL/OCSP necessary for short-lived certs)

---

## ✅ Deliverables for E7 Completion

* [ ] TLS cert chain validation on hub + edge
* [ ] CLI ephemeral cert never touches disk
* [ ] Expired tunnels hard closed by GC
* [ ] Brute-force prevention: login, DNS, API, Edge
* [ ] Auth enforcement (basic, JWT, HMAC, OIDC) in `edf-edge`
* [ ] Security integration tests for misbehavior, fuzz inputs

---

## 🔮 Future Enhancements

* Federated identity (SSO for orgs)
* eBPF firewall hooks for socket-level rate enforcement
* Certificate Transparency log for endpoint visibility
* CSP headers + CORS control for tunneled web apps

---
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

