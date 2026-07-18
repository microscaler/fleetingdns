# Capability-URL access model

**Decision (2026-07-05, owner):** FleetingDNS tunnels do NOT require a
user-auth gate (no Clerk, no session cookies) by default. Tunnels are
short-lived and their subdomain is unique and unguessable on each
inception — **possession of the link IS the authorization**, for the
finite lifetime of the tunnel. This is the classic capability-URL model
(same trust shape as an unlisted-video link or a presigned S3 URL).

## What this makes load-bearing

The random subdomain is the credential. Requirements that follow:

1. **Entropy** — `generate_random_subdomain`
   (`crates/backendapi/src/handlers/tunnels.rs`) emits
   `tunnel-` + 20 base-36 chars ≈ 103 bits from the thread-local CSPRNG
   (`rand::thread_rng`, ChaCha-based). Guarded by the
   `random_subdomain_is_high_entropy_capability` test. Was 8 chars
   (≈41 bits) before 2026-07-05 — too guessable to be a capability.
2. **TTL** — the tunnel record's Redis TTL bounds the exposure window;
   expiry removes both DNS resolution and edge routing.
3. **No subdomain leakage** — the edge must serve a *wildcard* TLS cert
   (per-subdomain certs would publish every tunnel FQDN to Certificate
   Transparency logs). DNS queries to public resolvers remain a known,
   accepted leak (same as ngrok et al).

## Custom subdomains are a user opt-out

`CreateTunnelRequest.custom_subdomain` lets a user pick a guessable name
— that is their explicit choice to weaken the capability, mirroring
ngrok's named domains.

## Relation to the dormant session-grant gate (FR-EDGE-3)

The opt-in `protected` flag + `POST /v1/tunnels/{id}/session` grant
machinery (commit 5dd8b08) stays in the codebase but **defaults off and
is not the plan of record**. It exists in case per-user binding is
revived (cylon PRD success criterion #5). Client-side certificate
validation is the acknowledged "much later" option; do not build more
auth layers on the edge path without a new decision here.
