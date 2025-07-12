### Deploy Tasks - Hello DNS Prototype

| # | Title | Path / crate | Detailed description & acceptance criteria |
|---|-------|--------------|--------------------------------------------|
| - [x] **T-07** | Dockerfile for dnsd | `deploy/docker/Dockerfile.dnsd` | *Desc* Scratch-based alpine, build with `cargo install --path crates/bin/dnsd`, entrypoint `dnsd --addr 0.0.0.0:53`. Expose UDP/TCP 53.<br><br>*AC* `docker build -f ...` succeeds, container responds to `dig`. |
| - [ ] **T-26d** | EdgeHub container & Tilt config | `docker/docker-compose.yml`, `tilt/k8s/app/edgehub.yaml` | Add port 2222 exposure, mount certs, configure `EDGE_TLS_CERT` and `EDGE_TLS_KEY` for the EdgeHub service. |
| - [ ] **T-26e** | Grafana dashboard metric | `docker/grafana/dashboards/fdns-unified.json` | Display `edge_tunnels_open` gauge increasing on tunnel open and decreasing on close. |
