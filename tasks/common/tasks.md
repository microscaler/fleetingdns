### Common Crate Tasks - Hello DNS Prototype

| # | Title | Path / crate | Detailed description & acceptance criteria |
|---|-------|--------------|--------------------------------------------|
| - [x] **T-02** | Scaffold **common** crate | `crates/common` | *Description* Create `lib.rs` with:<br>• `init_tracing()` – sets up `tracing_subscriber` (env filter, pretty).<br>• `AppResult<T>` + `AppError` using `thiserror`.<br>• `metrics` macro re-export (`metrics::{counter,gauge,histogram}`).<br><br>*AC* Calling `common::init_tracing()` from any bin prints “app start” with level-based color. |
| - [ ] **T-29** | OTLP metrics helper | `crates/common/src/metrics.rs` | *Desc* Create `init_metrics()` that configures `metrics-exporter-otel` when `OTEL_EXPORTER_OTLP_ENDPOINT` is set.<br>Export counters and gauges via the `metrics` crate.<br><br>*AC* Unit test records a counter and the exporter reports it to a local collector. |
