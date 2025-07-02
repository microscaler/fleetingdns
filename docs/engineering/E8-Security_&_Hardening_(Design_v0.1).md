# 📘 E7 – Security & Hardening (Design v0.1)

## 🧭 Overview

This document describes the **Security and Hardening Plan** for Ephemeral DNS Forwarder (FDF), covering all attack surfaces across API, DNS, tunnel transport, certificate handling, endpoint lifecycle, and user auth. It ensures that tunnels and ephemeral domains cannot be abused for persistent access, resource exhaustion, or attack redirection.

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

© 2025 Ephemeral DNS Forwarder
