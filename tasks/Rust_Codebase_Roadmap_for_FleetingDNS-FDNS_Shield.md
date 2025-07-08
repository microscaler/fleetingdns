### Rust Codebase Roadmap for **FleetingDNS / FDNS Shield**

| Layer                      | Crate / Binary         | Purpose                                                                                         | Key crates & tech                                                       |
| -------------------------- | ---------------------- | ----------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| **Edge dataplane**         | `cmd/dnsd`             | Authoritative DNS-over-UDP/TCP/DoT server, stateless label decode, Redis lookup, DNSSEC signing | `trust-dns-server`, `rustls`, `ring`, `hickory-proto`, `tokio`, `redis` |
|                            | `cmd/edgehub`          | Reverse-tunnel edge gateway: TLS-wrapped SSH, rate-limits, eBPF hooks, token bucket             | `tokio`, `openssh`, `rust-iptables`/`aya-bpf`                           |
| **Intelligence pipeline**  | `cmd/intake-collector` | gRPC ingest from honeypots → Pub/Sub                                                            | `tonic`, `prost`, `google-cloud-pubsub`                                 |
|                            | `crates/feature_pipe`  | Reusable lib for feature extraction                                                             | `serde`, `ndarray`, `hashbrown`                                         |
|                            | `cmd/ml-scorer`        | Online LightGBM inference service (JA3, ASN, etc.)                                              | `lightgbm-rs`, `tonic`, `rayon`                                         |
| **Threat-feed**            | `cmd/feed-grpc`        | gRPC bidirectional stream (`ThreatFeed`)                                                        | `tonic`, `jsonwebtoken`, `rustls`                                       |
|                            | `cmd/feed-webhook`     | HMAC-signed webhook pusher                                                                      | `reqwest`, `hmac`, `jsonwebtoken`                                       |
| **Control plane / portal** | `cmd/api`              | REST+GraphQL API, token issuance, billing hooks                                                 | `axum`, `tower`, `sqlx` (Postgres), `stripe-rust`                       |
|                            | `crates/auth`          | JWT + OIDC + mTLS helpers                                                                       | `jsonwebtoken`, `rustls-pemfile`                                        |
| **SDKs / CLI**             | `cli/edf`              | Developer CLI: tunnel setup, DoT client, metrics fetch                                          | `clap`, `tokio`, `rustls`, `openssh`                                    |
| **Internal tooling**       | `crates/common`        | shared error types, tracing, metrics, feature flags                                             | `thiserror`, `tracing`, `metrics`                                       |

---

## Implementation Phases (≈ 12 weeks total)

1. **Skeleton & Common Crate (Week 1)**

    * Workspaces, CI (GitHub Actions), `cargo deny`, `fmt`, `clippy`.
    * `crates/common` with error, tracing, metrics macros.

2. **Authoritative DNS Server (Weeks 2-3)**

    * `dnsd` UDP/TCP handler, stateless parser, Redis cache.
    * DoT handshake via `rustls`, basic RRSIG from HMAC key.
    * Bench: ≥25 k QPS e2-micro.

3. **EdgeHub Tunnel Gateway (Weeks 4-5)**

    * TLS-wrapped SSH reverse-tunnel acceptor.
    * Token bucket rate-limit, per-tenant counters.
    * Prom metrics & healthz.

4. **Telemetry → Pub/Sub Collector (Week 6)**

    * `intake-collector` gRPC receive path from honeypots.
    * Proto `{ ip, ja3, ts, zone, meta }`, push to Pub/Sub.

5. **ML Scoring Micro-service (Weeks 7-8)**

    * LightGBM model loader + REST/gRPC predict.
    * Vertex AI retrain script (Python) – triggered weekly.
    * Unit tests for feature vector correctness.

6. **Threat-Feed Services (Weeks 8-9)**

    * `feed-grpc` bidirectional stream with back-pressure ACK.
    * `feed-webhook` dispatch queue, retry & HMAC sign.
    * JWT/mTLS gate; Redis entitlements cache.

7. **Portal API & Billing (Weeks 10-11)**

    * `api` Axum REST: domains, webhooks, usage meter.
    * Stripe webhook → Postgres usage ledger.
    * Token issuance (JWT/OIDC), pin-set JSON endpoint.

8. **CLI & SDKs (Week 12)**

    * `edf` CLI: `edf tunnel up`, `edf dns query --dot`.
    * Go/Python minimal SDK wrappers auto-generated (grpc-web).
    * Docs & example integrations (Cloudflare Worker script).

---

## Key Rust Libraries

* **DNS:** `trust-dns-server`, `hickory-proto`
* **TLS/HTTP:** `rustls`, `hyper`, `axum`
* **gRPC/Protobuf:** `tonic`, `prost`
* **Async runtime:** `tokio` (with `mio`-backed UDP)
* **Redis client:** `redis::aio`
* **Metrics/Tracing:** `metrics`, `opentelemetry`, `tracing`
* **Auth:** `jsonwebtoken`, `ring`, `argon2`
* **eBPF (optional XDP):** `aya-bpf`, `aya` userspace

---

## Immediate Next Steps

1. **Repo bootstrap**

   ```bash
   cargo new --workspace fleetingdns
   cd fleetingdns
   mkdir -p crates cmd cli
   ```
2. Add `common` crate: errors, tracing setup, metrics macros.
3. Prototype UDP handler in `dnsd` resolving static label → Return `127.0.0.1` until Redis wired.
4. Set up GitHub Actions: build matrix + `cargo fmt -- --check`.
5. Plan model feature schema (JA3, ASN, frequency) & draft protobuf for telemetry.

Once phase 1 scaffolding merges, we can parallelize DNS server and EdgeHub work streams.
