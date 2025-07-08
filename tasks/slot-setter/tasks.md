### slot-setter Crate Tasks - Hello DNS Prototype

| # | Title | Path / crate | Detailed description & acceptance criteria |
|---|-------|--------------|--------------------------------------------|
| **T-11** | Minimal `slot-setter` CLI | `crates/bin/slot-setter` | *Desc* Command line tool to manually insert `{slot → ip}` mappings into Redis.<br>Args: `slot`, `ip`, optional `--ttl` default 1800 seconds.<br>Uses `dnsd::redis_cache` for storage.<br><br>*AC* Running `cargo run -p slot-setter demo 1.2.3.4 --ttl 600` stores the value in Redis. |
