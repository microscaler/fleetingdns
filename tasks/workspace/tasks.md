### Workspace Tasks - Hello DNS Prototype

| # | Title | Path / crate | Detailed description & acceptance criteria |
|---|-------|--------------|--------------------------------------------|
| **T-03** | Add workspace dev-dependencies | root `Cargo.toml` | *Desc* Top-level `[workspace.dependencies]` for:<br>`tracing`, `tracing-subscriber`, `thiserror`, `metrics`, `metrics-exporter-prometheus`, `tokio` (`full`).<br><br>*AC* `cargo check --workspace` succeeds (no warnings). |
