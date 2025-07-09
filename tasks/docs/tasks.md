### Documentation Tasks - Hello DNS Prototype

| # | Title | Path / crate | Detailed description & acceptance criteria |
|---|-------|--------------|--------------------------------------------|
| - [x] **T-08** | README quick-start for spike | `README.md` | *Desc* Add “Prototype 0.1” section with commands:<br>• `./scripts/bootstrap_crates.sh`<br>• `cargo run -p dnsd-bin`<br>• `dig @127.0.0.1 -p5353 test.fdns.run +short` → 127.0.0.1.<br><br>*AC* New developer can reproduce in <5 min. |
