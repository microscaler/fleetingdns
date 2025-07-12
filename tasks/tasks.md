### 🚧 Next Task Batch — “Prototype 0.4” (DoT + Redis + EdgeHub)

| ID       | Title                                        | Path / crate(s)                   | Detailed description & acceptance criteria |
| -------- | -------------------------------------------- | --------------------------------- | -------------------------------------------------------------------------------- |
| - [x] **T-21** | **DNS-over-TLS support**                     | `crates/dnsd`                     | *Desc* Add TLS listener on :853 using **rustls**. Provide a self-signed cert in `dev/certs/`. ALPN `dot`, reuse UDP decode.<br>*AC* `kdig @127.0.0.1 -p853 +tls-ca +tls-host=localhost test.fdns.run +short` prints `127.0.0.1`; CI job asserts handshake success. |
| - [ ] **T-22** | Redis async cache module                     | `crates/dnsd/src/redis_cache.rs`  | *Desc* Implement `async fn get_a(slot: &str) -> Option<Ipv4Addr>` with **bb8** + `redis::aio` pool (env `REDIS_URL`). TTL respect (EXPIRE).<br>*AC* Unit test sets key, dnsd returns mapped IP; miss → NXDOMAIN. |
| - [ ] **T-23** | **slot-setter** CLI                          | `crates/bin/slot_setter`          | *Desc* Binary `slot-setter <slot> <ip> --ttl 1800` writes Redis `SETEX`. Helpful for manual tests.<br>*AC* Integration test sets slot, dig returns mapped IP. |
| - [x] **T-24** | Wire Redis lookup into dnsd answer path      | `crates/dnsd`                     | *Desc* Replace hard-coded 127.0.0.1 with Redis result; if none, NXDOMAIN.<br>*AC* Existing UDP/DoT tests updated and passing. |
| - [ ] **T-25** | Minimal DNSSEC HMAC RRSIG                    | `crates/dnsd/src/sign.rs`         | *Desc* Sign answer with pre-shared HMAC key (env `FDNS_HMAC_KEY`). Use `ring::hmac`. Add placeholder RRSIG record.<br>*AC* `delv` validates signature with same key. |
| - [ ] **T-26a** | Embed Rust SSH server in EdgeHub             | `crates/edgehub/src/ssh.rs`       | *Desc* Integrate **`russh`**; accept mTLS cert, implement `server::Server`; on `tcpip-forward` allocate a local port and acknowledge.<br>*AC* `ssh -p2222 -R 8080:localhost:80 user@edgehub` prints “Allocated port …” and EdgeHub logs “tunnel open`. |
| - [ ] **T-26b** | TCP proxy to developer channel               | `edgehub::proxy`                  | *Desc* For each allocated port, open a channel to the client and pipe bytes via `tokio::io::copy_bidirectional`.<br>*AC* `nc` echo → curl via EdgeHub returns payload. |
| - [ ] **T-26c** | Redis lifecycle hooks                        | `edgehub::redis`                  | *Desc* `SETEX slot VALUE ttl` on open, `DEL` on close.<br>*AC* Integration test mocks Redis and asserts key exists while tunnel active. |
| - [ ] **T-26d** | Docker, Compose & Tilt updates               | `docker/docker-compose.yml`, `tilt/k8s/app/edgehub.yaml` | *Desc* Expose port 2222 TCP, mount certs, set `EDGE_TLS_CERT` and `EDGE_TLS_KEY`. |
| - [ ] **T-26e** | Grafana dashboard: `edge_tunnels_open` gauge | `docker/grafana/dashboards/fdns-unified.json` | *Desc* Increment metric on `tcpip-forward` open/close. |
| - [ ] **T-28**  | **e2e_tunnel.rs** full flow                  | `crates/edgehub/tests/e2e_tunnel.rs` | *Desc* Start dnsd and edgehub, spawn `python -m http.server 8000`; run `ssh -i dev_key -R 8080:localhost:8000 certuser@127.0.0.1 -p2222`. Curl `http://demo.<slot>.fdns.run:8080` returns HTML index.<br>*AC* Test passes locally and in CI. |
| - [ ] **T-29** | OTLP metrics instrumentation                 | `crates/common`, `dnsd`, `edgehub` | *Desc* Expose counters: `dns_queries_total`, `edge_tunnels_open`. Send OTLP to collector on `OTEL_EXPORTER_OTLP_ENDPOINT`.<br>*AC* Prometheus scrape shows metrics, Grafana dashboard updated. |
| - [ ] **T-30** | Compose update: add edgehub + Redis env vars | `docker/docker-compose.yml`       | *Desc* Include `edgehub` container on port 2222, link to Redis; mount dev certs; open 853 for dnsd. |
| - [ ] **T-31** | CI: DoT + Redis smoke test                   | `.github/workflows/compose-ci.yml` | *Desc* Extend existing job: after stack up, run slot-setter, `kdig +tls` query must return set IP.<br>*AC* Workflow green. |

> **Suggested order:** T-21 → 22 → 23 → 24 (DNS path) ⇒ parallel start T-26a–T-26e (EdgeHub) ⇒ T-25 then T-28 (full flow) ⇒ T-29–31 (observability & CI).

Complete these to reach Prototype 0.4: encrypted DNS, Redis-backed stateless mapping, first reverse tunnel through EdgeHub, with OTEL metrics—and all green in GitHub Actions.

---

For more details take a look at ./tasks/Rust_Codebase_Roadmap_for_FleetingDNS-FDNS_Shield.md

As well as the detailed epics in the ./docs/engineering/Epic_highlevel/E1*-*.md
---
