# 📗 **E5 – SDK Libraries & CLI Enhancements**  
*Epic → User-story breakdown (v0.1)*

This epic delivers client libraries for major languages plus extra CLI ergonomics so developers never hand‑roll HTTP calls.

---

## Epic Goal
> “Offer idiomatic SDKs (Rust, Go, Node, Python, Java) generated from the OpenAPI spec, each with test harnesses, Pact contracts, and clear docs; extend CLI with quality-of-life commands for tunnelling, logs, and troubleshooting.”

---

## 🗂️ Story List
| ID | Story | Outcome |
|----|-------|---------|
| **E5-S1** | As a *Rust dev*, import `fleetingdns` crate and call `Tunnel::create()` with one line. |
| **E5-S2** | As a *Go developer*, use `go get github.com/fdns/sdk-go` and get typed client, Retry + context. |
| **E5-S3** | As a *JS/TS developer*, install `npm i @fleetingdns/sdk` — fully typed via OpenAPI. |
| **E5-S4** | As a *Python engineer*, `pip install fleetingdns` and get asyncio client. |
| **E5-S5** | As a *Java/Kotlin* team, consume Maven package `io.fdns:fleetingdns-sdk`. |
| **E5-S6** | As *QA*, have **Pact contracts** auto‑verified in CI for all SDKs. |
| **E5-S7** | As a *CLI user*, run `edf tunnel ls`, `edf tunnel logs`, and `edf diag` for connectivity. |

---

## E5-S1 — Rust SDK Crate
**Tasks**
1. Run `openapi-generator` rust‑reqwest template to scaffold.  
2. Replace default with `rustls` TLS + serde.  
3. Publish `0.1.0` to crates.io with docs.rs.

**Functional**
* `Tunnel::create(ttl=1800)` returns struct with `fqdn`.  
* `Tunnel::delete(id)` returns Result<()>.

**Non‑Functional**
* Zero unsafe code.  
* Build size < 1 MB.

---

## E5-S2 — Go SDK
**Tasks**
1. Generate with `oapi-codegen`.  
2. Wrap in `fdns.Client` adding retry (backoff) & context.  
3. CI linter `golangci-lint` passes.

**Functional**
* Supports `WithAPIKey(token)` auth helper.  
* Context cancel propagates to HTTP client.

**Non‑Functional**
* go‑vet zero issues.  
* Module SEMVER tags.

---

## E5-S3 — Node SDK
**Tasks**
1. OpenAPI → `typescript-fetch` template.  
2. Publish `@fleetingdns/sdk` on npm.  
3. Bundle types with `tsc --declaration`.

**Functional**
* `import { createTunnel } from '@fleetingdns/sdk';`  
* Promise resolves with typed `TunnelResponse`.

**Non‑Functional**
* Tree‑shakable build < 20 KB min‑gzip.  
* Node ≥18 + browser support.

---

## E5-S4 — Python SDK
**Tasks**
1. `openapi-python-client` generate asyncio code.  
2. Publish to PyPI (`twine upload`).  
3. Add `pydantic` models.

**Functional**
* Async context manager for tunnel: `async with client.create_tunnel(): …`.

**Non‑Functional**
* Wheels manylinux, mac, win.  
* mypy passes – 0 type errors.

---

## E5-S5 — Java/Kotlin SDK
**Tasks**
1. openapi-generator `java-kotlin` template.  
2. Publish to Maven Central via Sonatype.  
3. Provide Spring‑Boot autoconfig.

**Functional**
* Fluent API `new TunnelRequest().ttl(1800).execute()`.

**Non‑Functional**
* Bytecode target 1.8.  
* Javadoc generated.

---

## E5-S6 — Pact Contracts
**Tasks**
1. Define consumer pact in each SDK test.  
2. Publish to Pactflow broker.  
3. Provider (Tunnel API) CI verifies against all.

**Functional**
* CI fails if breaking API change.  
* At least 90% interaction coverage.

**Non‑Functional**
* Pact test suite time < 2 min per SDK.  
* Broker retention 90 days.

---

## E5-S7 — CLI Enhancements
**Tasks**
1. Add `edf tunnel ls` (calls `/tunnels?self=true`).  
2. Add `edf tunnel logs --follow`.  
3. Add `edf diag` ping, DNS lookup, tls‑handshake check.

**Functional**
* `ls` prints table ID, FQDN, Expires.  
* `diag` exits 0 on success; non‑zero otherwise.

**Non‑Functional**
* CLI sub‑command latency < 300 ms.  
* Log follow buffers 100 lines.

---

© 2025 FleetingDNS — SDK & CLI stories

