# 📘 E5 – SDK Integration (Design v0.2)

## 🧭 Overview

This document outlines the design of **language-specific SDKs** to make Ephemeral DNS Forwarder (EDF) trivially accessible from developer tools, test harnesses, and other programmatic contexts. These SDKs wrap the CLI binary (`edf`) or expose native bindings where appropriate, enabling endpoint creation and teardown via Python, JavaScript/TypeScript, and Go.

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

© 2025 Ephemeral DNS Forwarder
