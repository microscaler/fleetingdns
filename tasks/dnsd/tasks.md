### dnsd Crate Tasks - Hello DNS Prototype

| # | Title | Path / crate | Detailed description & acceptance criteria |
|---|-------|--------------|--------------------------------------------|
| - [x] **T-04** | New **dnsd** library crate skeleton | `crates/dnsd` | *Desc* `lib.rs` exposes:<br>`pub fn serve(cfg: Config) -> AppResult<()>` (async).<br>`Config { addr: SocketAddr }`.<br>No real protocol yet—just binds UDP socket & logs packet count.<br><br>*AC* Unit test starts server on `127.0.0.1:0`, sends one byte, receives none but server logs “received X bytes”. |
| - [x] **T-05** | Minimal DNS packet echo parser | `crates/dnsd/src/udp.rs` | *Desc* Parse first 12-byte DNS header (ID, flags). Ignore queries but craft response with:<br>• same ID<br>• QR=1, RCODE=0<br>• QDCOUNT=ANCOUNT=1<br>• Answer record: A 127.0.0.1 (hard-code).<br>Use `hickory-proto` for encode.<br><br>*AC* Integration test: `dig @127.0.0.1 test.fdns.run +short` outputs 127.0.0.1. |

| - [x] **T-24** | Wire Redis lookup into dnsd answer path | `crates/dnsd` | *Desc* Replace hard-coded 127.0.0.1 with Redis result; if none, NXDOMAIN.
Existing tests updated. |
| - [x] **T-32** | Document `dot::serve` public API | crates/dnsd/src/lib.rs | Add rustdoc describing parameters, behavior and graceful shutdown. |
| - [ ] **T-29** | `dns_queries_total` metric | `crates/dnsd/src/lib.rs` | *Desc* Increment `dns_queries_total{protocol="udp"}` and `{protocol="dot"}` for every request using `metrics::counter!`.<br><br>*AC* Integration test asserts counter increases after a query. |
