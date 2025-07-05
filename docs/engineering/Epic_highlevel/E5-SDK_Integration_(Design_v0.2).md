# 📘 E5 – SDK Integration (Design v0.2)

## 🧭 Overview

This document outlines the design of **language-specific SDKs** to make FleetingDNS (FDF) trivially accessible from developer tools, test harnesses, and other programmatic contexts. These SDKs wrap the CLI binary (`edf`) or expose native bindings where appropriate, enabling endpoint creation and teardown via Python, JavaScript/TypeScript, and Go.

---

## 🎯 Objectives

* Offer developer-friendly SDKs for common languages
* Support fully automated ephemeral endpoint lifecycles
* Abstract away CLI complexity while maintaining compatibility
* Enable structured feedback (e.g., FQDN, basic-auth, status)

---

## 📦 Supported Languages (MVP)

| Language   | Delivery Method             | Distribution Target  |
| ---------- | --------------------------- | -------------------- |
| Python     | PyO3 bindings               | PyPI                 |
| JavaScript | WASM or node child\_process | npm                  |
| Go         | cgo or CLI wrapper          | go get / precompiled |

---

## 📁 SDK Interface Design

### Python (via PyO3)

```python
from epdns import Endpoint

with Endpoint(port=3000, ttl=1800, auth=True) as ep:
    print(ep.fqdn)
    print(ep.auth)
    requests.post(ep.fqdn, headers={...})
```

### JavaScript (via WASM CLI shim or node wrapper)

```ts
import { Epdns } from '@epdns/client';
const endpoint = await Epdns.create({ port: 3000, ttl: 1800 });
console.log(endpoint.fqdn);
await endpoint.destroy();
```

### Go

```go
import "github.com/epdns/sdk-go"

client := epdns.New()
endpoint, err := client.Create(3000)
fmt.Println(endpoint.FQDN)
defer client.Delete(endpoint.ID)
```

---

## 🛡️ Security Notes

* CLI credentials and certs handled via secure subprocess env
* JSON output scrubbed for sensitive material if `--no-auth` flag used
* No write to disk unless explicitly specified by SDK (e.g., `to_file()`)

---

## 🧩 CLI JSON Output Schema

```json
{
  "fqdn": "abc123.edf.run",
  "id": "ep-uuid",
  "port": 3000,
  "auth": {
    "username": "uX12q",
    "password": "r8j!D"
  },
  "expires_at": "2025-07-02T14:55:00Z"
}
```

---

## 📊 Metrics for SDK Use

* `edf-sdk.lang.init_total` (labels: lang, version)
* `edf-sdk.endpoint.created_total`
* `edf-sdk.endpoint.destroyed_total`

---

## ✅ Deliverables for E5 Completion

* [ ] Python SDK published to PyPI
* [ ] Node/NPM package with CLI bridge
* [ ] Go SDK with subprocess wrapper or direct HTTP binding
* [ ] End-to-end example repo: Python + Node

---

## 🔮 Future Extensions

* IDE plugin to preview endpoint info during test runs
* WASI support for browser-based SDK tests
* Visual Studio Code test helper integration

---

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

