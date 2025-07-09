### dnsd-bin Crate Tasks - Hello DNS Prototype

| # | Title | Path / crate | Detailed description & acceptance criteria |
|---|-------|--------------|--------------------------------------------|
| - [x] **T-06** | **dnsd-bin** wrapper crate | `crates/bin/dnsd` | *Desc* Binary depends on `dnsd`, reads `--addr` CLI (default 0.0.0.0:5353), calls `common::init_tracing()` then `dnsd::serve(...)`.<br><br>*AC* Running `cargo run -p dnsd-bin` starts listener, logs “dnsd listening”. |
