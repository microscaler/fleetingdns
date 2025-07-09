### Deploy Tasks - Hello DNS Prototype

| # | Title | Path / crate | Detailed description & acceptance criteria |
|---|-------|--------------|--------------------------------------------|
| - [x] **T-07** | Dockerfile for dnsd | `deploy/docker/Dockerfile.dnsd` | *Desc* Scratch-based alpine, build with `cargo install --path crates/bin/dnsd`, entrypoint `dnsd --addr 0.0.0.0:53`. Expose UDP/TCP 53.<br><br>*AC* `docker build -f ...` succeeds, container responds to `dig`. |
