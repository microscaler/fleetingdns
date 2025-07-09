### Common Crate Tasks - Hello DNS Prototype

| # | Title | Path / crate | Detailed description & acceptance criteria |
|---|-------|--------------|--------------------------------------------|
| - [x] **T-02** | Scaffold **common** crate | `crates/common` | *Description* Create `lib.rs` with:<br>• `init_tracing()` – sets up `tracing_subscriber` (env filter, pretty).<br>• `AppResult<T>` + `AppError` using `thiserror`.<br>• `metrics` macro re-export (`metrics::{counter,gauge,histogram}`).<br><br>*AC* Calling `common::init_tracing()` from any bin prints “app start” with level-based color. |
