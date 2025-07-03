# 🌐 Fleeting DNS Forwarder (FDF)

**Instant, Secure, and Temporary DNS Endpoints for Development & Testing**

---

## 🚀 What is Ephemeral DNS Forwarder?

Ephemeral DNS Forwarder is a secure, easy-to-use service that provides temporary, publicly accessible DNS endpoints, instantly forwarding traffic to your local machine or continuous integration (CI) environment. Built specifically for developers and CI pipelines, it solves common problems faced during testing, including external integrations such as webhooks, OAuth callbacks, and multi-tenant application routing.

---

## 🛠️ Why Do Developers Need FDF?

Developers frequently face challenges when testing systems that require external services:

* **OAuth flows**: Need public URLs for callback validations.
* **Webhooks**: Services like Stripe, GitHub, or Slack need publicly accessible endpoints.
* **Integration Tests**: Require realistic public DNS scenarios.
* **Multi-tenant Applications**: Need separate subdomains to replicate production routing and authentication.

Using localhost or editing local DNS entries (`/etc/hosts`) is cumbersome and doesn't replicate real-world scenarios accurately.

FDF resolves these pain points by creating secure, ephemeral, and publicly resolvable DNS endpoints that seamlessly route traffic back to your local development or CI environment.

---

## ✅ Key Features

* **Instant DNS Creation**: Generate temporary endpoints instantly from the command line.
* **Automatic Cleanup**: Endpoints self-destruct after their set TTL (time-to-live).
* **TLS Security**: Traffic is encrypted and secured using ephemeral SSL certificates.
* **Auth Support**: Supports Basic Authentication, HMAC verification, and OIDC token validation.
* **Easy Integration**: SDKs available for Python, JavaScript, and Go.
* **CI/CD Ready**: GitHub Actions integration for seamless automated testing.
* **Customizable Plans**: Stripe-powered payment tiers, including free, supporter, team, and organizational options.

---

## 🔍 When Should You Use FDF?

* Testing **OAuth and OpenID Connect flows** locally or in CI.
* Validating external **webhook integrations** (Stripe, GitHub, Twilio).
* Running **integration tests** that require public DNS entries.
* Simulating **multi-tenant routing** (host-header-based routing).
* Debugging scenarios that require realistic public network environments.

---

## 🧑‍💻 Example Scenario: Stripe Webhook Integration

Traditional testing requires exposing a local port publicly or deploying test instances, both tedious and insecure.

With FDF:

```bash
fleetingdns forward --port 3000 --ttl 1800
```

This command instantly creates a public DNS endpoint (e.g., `https://abc123.fleetingdns.run`) that Stripe can use to send webhook events directly to your locally running test server. FDF handles the routing securely, and after 30 minutes, the endpoint and tunnel close automatically.

---



### ✅ Use Case: Developer Using FDF to Test a Webhook

```mermaid
sequenceDiagram
  autonumber
  participant Dev as Developer (CLI)
  participant API as FDF API
  participant CA as FDF CA & Auth
  participant Hub as Tunnel Gateway
  participant Edge as HTTPS Endpoint
  participant Webhook as Webhook Provider (e.g., Stripe)
  participant App as Local Dev Server

  Dev->>Dev: Start dev server (localhost:3000)
  Dev->>Dev: Run `edf forward --port 3000`
  Dev->>API: Authenticate via OAuth or API token
  API->>CA: Request ephemeral TLS cert (30m expiry)
  CA-->>API: Signed TLS certificate (PEM)
  API-->>Dev: Endpoint metadata + signed cert (in-memory only)

  Dev->>Hub: TLS+SSH tunnel initiated using client cert
  Hub-->>Dev: Reverse port bound (e.g. slot 60001)
  API->>Edge: Create DNS A record (e.g. abc123.edf.run → Edge)

  Note over Dev,Edge: ✅ Developer sees live public HTTPS URL

  Dev->>Webhook: Register webhook with public URL (e.g. https://abc123.edf.run/webhook)

  Webhook->>Edge: Send event to public DNS endpoint
  Edge->>Hub: Resolve slot for abc123.edf.run
  Hub->>Dev: Forward POST /webhook through tunnel
  Dev->>App: Inject POST /webhook → localhost:3000
  App-->>Dev: 200 OK
  Dev-->>Hub: Response back through tunnel
  Hub-->>Edge: Return 200 OK to webhook provider
  Edge-->>Webhook: Acknowledge webhook

  Note over Dev: 🔍 CLI shows: “Webhook received – 200 OK”

  alt TTL expires or Dev presses Ctrl+C
    Dev->>API: Delete endpoint
    API->>Hub: Close tunnel
    API->>Edge: Remove DNS record
  end
```

---

### ✅ Use Case: Developer Testing OAuth Login Flow Locally

```mermaid
sequenceDiagram
  autonumber
  participant Dev as Developer (CLI)
  participant API as FDF API
  participant CA as FDF CA
  participant Hub as Tunnel Gateway
  participant Edge as HTTPS Endpoint
  participant Browser as Browser (User/Dev)
  participant IdP as OAuth Provider (Google, GitHub)
  participant App as Local Dev App

  Dev->>Dev: Start local app on http://localhost:3000
  Dev->>Dev: Run `edf forward --port 3000`
  Dev->>API: Authenticate + request cert
  API->>CA: Sign ephemeral cert (valid 30m)
  CA-->>API: Signed TLS cert
  API-->>Dev: Return fqdn + cert
  Dev->>Hub: Establish TLS-wrapped SSH tunnel
  API->>Edge: DNS → abc123.edf.run → Edge

  Dev->>IdP: Register OAuth app with redirect URI: https://abc123.edf.run/oauth/callback

  Browser->>App: Click “Login with GitHub”
  App->>Browser: Redirect → https://github.com/login/oauth/authorize?...&redirect_uri=https://abc123.edf.run/oauth/callback
  Browser->>IdP: Login and consent
  IdP->>Browser: Redirect to https://abc123.edf.run/oauth/callback?code=XYZ

  Browser->>Edge: GET /oauth/callback?code=XYZ
  Edge->>Hub: Route to slot abc123
  Hub->>Dev: Forward request via tunnel
  Dev->>App: GET /oauth/callback?code=XYZ → localhost:3000
  App->>IdP: Exchange code for token
  IdP-->>App: Access token response
  App->>Browser: Respond with 200 OK / redirect to dashboard

  Note over Dev: 🔒 OAuth callback tested securely & end-to-end
```

---

### ✅ Use Case: Multi-Tenant App Routing with Subdomain Matching

```mermaid
sequenceDiagram
  autonumber
  participant Dev as Developer (CLI)
  participant API as FDF API
  participant CA as FDF CA
  participant Hub as Tunnel Gateway
  participant Edge as HTTPS Endpoint
  participant Browser as QA/User
  participant App as Local Dev App

  Dev->>Dev: Start multi-tenant app on localhost:3000
  Dev->>Dev: Run `edf forward --port 3000 --subdomain myapp`
  Dev->>API: Authenticate + request cert for *.myapp.edf.run
  API->>CA: Sign wildcard cert
  CA-->>API: Cert for *.myapp.edf.run
  API-->>Dev: fqdn: tenantA.myapp.edf.run, cert, TTL
  Dev->>Hub: Connect reverse tunnel using wildcard cert
  API->>Edge: Create wildcard DNS entries

  Note over Dev,Edge: 🌐 Now routing *.myapp.edf.run → tunnel

  Browser->>Edge: GET https://tenantA.myapp.edf.run
  Edge->>Hub: Lookup tunnel for *.myapp.edf.run
  Hub->>Dev: Forward to local tunnel
  Dev->>App: Host header = tenantA.myapp.edf.run
  App-->>Dev: Render tenantA dashboard
  Dev-->>Browser: HTML response via tunnel

  Note over App: 🏷️ App routes by Host header to tenant-specific logic
```

---

### 🧭 What It Shows

This diagram represents the normal “happy path” flow for a developer:

* Starts a local app
* Opens a secure, time-limited public endpoint
* Tests an integration (e.g. a webhook or OAuth redirect)
* Tunnels are secure and isolated per session
* DNS and tunnel clean up automatically when done

---

## 📚 Documentation and SDKs

SDKs available for easy integration:

* [Python SDK](https://pypi.org/project/fleetingdns-client)
* [JavaScript SDK](https://npmjs.com/package/@fleetingdns/client)
* [Go SDK](https://github.com/fleetingdns/sdk-go)
* Java
* Kotlin
* C\#
* TypeScript
* Ruby
* Swift
* Rust

Detailed documentation and API references available [here](https://docs.fleetingdns.run).

---

## 🔒 Security First

* Ephemeral TLS certificates for each session.
* Secure tunnels with TLS-wrapped SSH.
* Rate limiting, brute-force prevention, and comprehensive audit logs.
* Authentication support (Basic, HMAC, OAuth/OIDC).
* Zero-trust, ephemeral infrastructure ensuring no persistent exposure.

---

## 🚧 GitHub Actions Integration

Easily integrate with your automated workflows:

```yaml
- uses: fleetingdns/fleetingdns-action@v1
  with:
    port: 3000
    ttl: 1200
    auth: true
```

---

## 🌟 Get Started Today

Visit our [GitHub repository](https://github.com/fleetingdns) or install directly:

```bash
curl -sSfL https://fleetingdns.sh/install | sh
```

Transform your development and testing workflow today!

---

© 2025 Ephemeral DNS Forwarder
