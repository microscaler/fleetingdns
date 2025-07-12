### CI Tasks - Hello DNS Prototype

| # | Title | Path / crate | Detailed description & acceptance criteria |
|---|-------|--------------|--------------------------------------------|
| - [x] **T-01** | CI job: build & smoke-test | `.github/workflows/rust_ci.yml` | *Desc* Matrix: stable + nightly.<br>Steps:<br>1. `cargo fmt -- --check`<br>2. `cargo clippy --all -- -D warnings`<br>3. `cargo test --workspace`<br>4. Spin up dnsd in background, run `dig` smoke test.<br><br>*AC* PR passes first run. |
| - [ ] **T-31** | CI: DoT + Redis smoke test | `.github/workflows/compose-ci.yml` | *Desc* Extend compose job: after stack starts, run `slot-setter` then `kdig +tls` query expecting the set IP.<br><br>*AC* Workflow passes with DoT and Redis enabled. |
