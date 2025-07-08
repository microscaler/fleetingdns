### Prototype 0.1 — “Hello DNS” end-to-end spike

*(goal: `dig @127.0.0.1 test.fdns.run` returns **127.0.0.1** via UDP; no Redis, no DoT yet)*

Below are **8 Codex tasks** (issue-style). Drop them into your tracker and assign; completing all gives a runnable binary and CI green.

| #        | Title                               | Path / crate                    | Detailed description & acceptance criteria                                                                                                                                                                                                                                                                                                       |
|----------| ----------------------------------- |---------------------------------| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **T-01** | CI job: build & smoke-test          | `.github/workflows/rust_ci.yml` | *Desc*  Matrix: stable + nightly.<br>Steps:<br>1. `cargo fmt -- --check`<br>2. `cargo clippy --all -- -D warnings`<br>3. `cargo test --workspace`<br>4. Spin up dnsd in background, run `dig` smoke test.<br><br>*AC*  PR passes first run.                                                                                                      |
| **T-02** | Scaffold **common** crate           | `crates/common`                 | *Description*  Create `lib.rs` with:<br>• `init_tracing()` – sets up `tracing_subscriber` (env filter, pretty).<br>• `AppResult<T>` + `AppError` using `thiserror`.<br>• `metrics` macro re-export (`metrics::{counter,gauge,histogram}`).<br><br>*AC*  Calling `common::init_tracing()` from any bin prints “app start” with level-based color. |
| **T-03** | Add workspace dev-dependencies      | root `Cargo.toml`               | *Desc*  Top-level `[workspace.dependencies]` for:<br>`tracing`, `tracing-subscriber`, `thiserror`, `metrics`, `metrics-exporter-prometheus`, `tokio` (`full`).<br><br>*AC*  `cargo check --workspace` succeeds (no warnings).                                                                                                                    |
| **T-04** | New **dnsd** library crate skeleton | `crates/dnsd`                   | *Desc*  `lib.rs` exposes:<br>`pub fn serve(cfg: Config) -> AppResult<()>` (async).<br>`Config { addr: SocketAddr }`.<br>No real protocol yet—just binds UDP socket & logs packet count.<br><br>*AC*  Unit test starts server on `127.0.0.1:0`, sends one byte, receives none but server logs “received X bytes”.                                 |
| **T-05** | Minimal DNS packet echo parser      | `crates/dnsd/src/udp.rs`        | *Desc*  Parse first 12-byte DNS header (ID, flags). Ignore queries but craft response with:<br>• same ID<br>• QR=1, RCODE=0<br>• QDCOUNT=ANCOUNT=1<br>• Answer record: A 127.0.0.1 (hard-code).<br>Use `hickory-proto` for encode.<br><br>*AC*  Integration test: `dig @127.0.0.1 test.fdns.run +short` outputs 127.0.0.1.                       |
| **T-06** | **dnsd-bin** wrapper crate          | `crates/bin/dnsd`               | *Desc*  Binary depends on `dnsd`, reads `--addr` CLI (default 0.0.0.0:5353), calls `common::init_tracing()` then `dnsd::serve(...)`.<br><br>*AC*  Running `cargo run -p dnsd-bin` starts listener, logs “dnsd listening”.                                                                                                                        |
| **T-07** | Dockerfile for dnsd                 | `deploy/docker/Dockerfile.dnsd` | *Desc*  Scratch-based alpine, build with `cargo install --path crates/bin/dnsd`, entrypoint `dnsd --addr 0.0.0.0:53`. Expose UDP/TCP 53.<br><br>*AC*  `docker build -f ...` succeeds, container responds to `dig`.                                                                                                                               |
| **T-08** | README quick-start for spike        | `README.md`                     | *Desc*  Add “Prototype 0.1” section with commands:<br>• `./scripts/bootstrap_crates.sh`<br>• `cargo run -p dnsd-bin`<br>• `dig @127.0.0.1 -p5353 test.fdns.run +short` → 127.0.0.1.<br><br>*AC*  New developer can reproduce in <5 min.                                                                                                          |

---

### Suggested order

5. T-01 (CI) to lock safety net
1. T-02 → T-02 (foundation)
2. T-03 (library skeleton)
3. T-04 (basic DNS encode/decode)
4. T-05 (binary launcher)
6. T-06 (Docker)
7. T-08 (docs)

Once this spike runs, extend `dnsd` to DoT + Redis while another pair starts **edgehub** under the same crate pattern.

