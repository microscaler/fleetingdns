# 📗 **E4 – Authentication & Redirect Modes**  
*Epic → User-story breakdown (v0.1)*

E4 layers access‑control on top of the Edge Proxy: Basic Auth, OIDC provider chaining, HMAC-signed webhook validation and per‑tunnel redirect policies.

---

## Epic Goal
> “Provide lightweight yet flexible authentication options (Basic, OIDC callback, HMAC header) and automatic redirect modes, so developers can secure tunnels during tests without external proxies.”

---

## 🗂️ Story List
| ID        | Story                                                                                                                 | Outcome |
|-----------|-----------------------------------------------------------------------------------------------------------------------|---------|
| **E4-S1** | As a *developer*, receive **Basic‑Auth creds** with tunnel response and protect my webhook endpoint.                  |
| **E4-S2** | As *QA*, configure **static 302 redirect** so hitting my tunnel bounces to a staging site.                            |
| **E4-S3** | As a *SaaS team*, request **HMAC-shared secret** so webhooks from FleetingDNS carry `X-EDF-Sig` header.               |
| **E4-S4** | As an *enterprise customer*, register **custom OIDC endpoint** (Auth0, Okta) so EdgeHub verifies JWT before proxying. |
| **E4-S5** | As *DevOps*, rotate Basic Auth + HMAC secrets automatically at tunnel TTL expiry.                                     |

---

## E4-S1 — Basic Auth Flow
**Tasks**
1. Extend Tunnel API to generate `basic_user`, `basic_pass` using `rand` crate.  
2. Store bcrypt hash in Redis slot meta.  
3. Edge Proxy middleware validates `Authorization` header.

**Functional**
* Client receives creds JSON `{auth: {user, pass}}`.  
* Requests without / wrong creds get `401` with `WWW-Authenticate: Basic`.

**Non-Functional**
* Hash calc time constant ±5 %.  
* Credential length 16 chars alphanum.

---

## E4-S2 — Static Redirect Mode
**Tasks**
1. Add `redirect_url` field in `/tunnels` request body.  
2. Edge returns 302 for any HTTP request, Location=`redirect_url`.

**Functional**
* Redirect preserves original path + query.  
* Works only for `mode=http` tunnels.

**Non-Functional**
* Redirect latency < 1 ms.  
* Disallowed for raw TCP mode.

---

## E4-S3 — HMAC Header Validation
**Tasks**
1. Tunnel API issues `shared_hmac_key` (base64).  
2. Edge attaches `X-EDF-Sig: hmac_sha256(body)` header.  
3. CLI/SDK helper `verify_sig()`.

**Functional**
* Signature verified equals server‑side calc for body bytes.  
* Key rotation when tunnel expires.

**Non-Functional**
* Added CPU < 3 µs per request @ 64 KB body.  
* Header size 44 B.

---

## E4-S4 — Custom OIDC Provider
**Tasks**
1. Extend API `/users/{id}/oidc` to store provider metadata, JWKS URL.  
2. Edge Proxy JWKS fetch & cache 5 min.  
3. Validate `Authorization: Bearer` JWT on inbound request.

**Functional**
* 200 OK if token valid `aud=tunnel_id` & `exp>now`.  
* 401/403 otherwise.

**Non-Functional**
* JWKS cache miss p95 ≤ 300 ms.  
* Max token size 2 KB.

---

## E4-S5 — Secret Rotation & Cleanup
**Tasks**
1. Redis key TTL governs Basic/HMAC secret lifetime.  
2. Edge Hub clears in‑memory cache on `keyspace:expired` event.  
3. Stripe usage record includes bytes until expiry.

**Functional**
* Secrets invalid immediately after expiry (≤2 s).  
* New tunnel get fresh secrets, not reused.

**Non-Functional**
* Memory leak < 1 KB per expired slot.  
* Event loss tolerance: Edge polls Redis fallback every 30 s.

---

© 2025 FleetingDNS — Auth & Redirect stories

