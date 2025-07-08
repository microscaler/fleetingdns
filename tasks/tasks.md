🎉 **Fantastic news!** A working UDP path means the plumbing—workspace, tracing, CI, and packet encode/decode—is solid. Let’s capitalise on that momentum.

---

## 🚀 Next-step milestone (“Prototype 0.2” – secure + stateless)

| Phase                               | Goal                                       | Crate(s) touched                      | Key tasks                                                                                    |
| ----------------------------------- |--------------------------------------------| ------------------------------------- | -------------------------------------------------------------------------------------------- |
| **A. DoT handshake**                | Add DNS-over-TLS on port 853               | `dnsd`, `common`                      | \* rustls server config<br>\* ALPN `dot`<br>\* unit test: `kdig +tls-ca +tls-host=tls.local` |
| **B. Stateless label → Redis slot** | Resolve `<slot>.fleetingdns.run` via Redis | `dnsd::redis_cache`                   | \* async Redis pool (`bb8 + redis`)<br>\* fallback stub if key missing (NXDOMAIN)            |
| **C. Redis write path**             | “register tunnel” helper for later         | new tiny bin `crates/bin/slot-setter` | \* CLI: `slot-setter <slot> <ip> --ttl 1800`                                                 |
| **D. EdgeHub skeleton**             | Accept reverse-tunnel (no eBPF yet)        | `edgehub` (lib+bin)                   | \* accept TLS-wrapped SSH on 2222<br>\* map `<slot>` to `127.0.0.1:<rand>` for now           |
| **E. E2E smoke via `edgehub`**      | curl through tunnel → local mock web       | `intake_collector` test harness       | \* spin dnsd + edgehub + register slot + `curl https://demo.<slot>.fleetingdns.run` expects 200     |

> *Tip*: merge A → B quickly so other devs can use Redis look-ups while EdgeHub is under construction.

---

## Suggested task tickets

1. **T-09** – Add rustls DoT listener (update config, new integration test).
2. **T-10** – Implement `redis_cache.rs` (get/set, TTL respect).
3. **T-11** – Minimal `slot-setter` CLI for manual Redis inserts.
4. **T-12** – EdgeHub listener scaffolding (accept connection, print debug).
5. **T-13** – Wire dnsd ↔ EdgeHub end-to-end demo (docs + CI job).

---

## Development tips

* **Feature flags** – gate DoT with `--features dot` initially to keep CI fast.
* **Replayable tests** – store a PCAP of a good DoT handshake; integration test can assert bytes (good safety net).
* **Metrics early** – export `dns_queries_total{protocol="udp|dot"}` to Prom so we watch latency regressions when switching to TLS.

---

