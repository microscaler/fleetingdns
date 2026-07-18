# FleetingDNS — Next Steps

## Current Status (2026-04-28)

### Completed R-items (postmortem remediation)

| ID | Action | Status | Date |
|---|---|---|---|
| R1 | Unblock build (`generic-array 0.14.7` vs nightly-2025-06-28) | DONE | 2026-04-27 |
| R2 | Client `tcpip_forward` + `server_channel_open_forwarded_tcpip` handler | DONE | 2026-04-28 |
| R3 | Server `Handler::tcpip_forward` with real TcpListener + `forwarded-tcpip` channels + `copy_bidirectional` | DONE | 2026-04-28 |
| R4 | Delete orphaned `direct-tcpip` path | DONE | 2026-04-28 |

### Remaining R-items

| ID | Action | Priority | Notes |
|---|---|---|---|
|| R5 | CLI uses API-issued slot (not UUID-derived port) | HIGH | **DONE** (2026-04-28) — SshKeyPair.slot extracted from POST /v1/tunnels |
|| R6 | Real edge-router forward in `serve_https_router` | HIGH | **DONE** (2026-04-28) — replaced stub with real bidirectional proxy via `tokio::io::copy_bidirectional`-style split streams to tunnel port |
|| R7 | Fix `--addr` / `--ssh-addr` defaults; proper SNI sniffing | MEDIUM | Both default to `:8443` causing bind collision; `extract_sni_from_tls` hard-coded to return `None` |
| R8 | CLI uses `SshKeyManager`-issued keypair | LOW | CLI generates fresh Ed25519 per run instead of reusing from `SshKeyManager` |
| R9 | E2E reverse-tunnel test proving bytes reach `localhost` | CRITICAL | Would have caught the original bug; needs real SshServer + echo HTTP server + CLI tunnel registration + curl assertion |

### Non-R items

| Priority | Item | Notes |
|---|---|---|
| HIGH | Pre-existing test failures | `fleetingdns_integration_tests` unresolved import, redis `MultiplexedConnection` trait issues, old russh API usage in test files |
| MEDIUM | TLS router module (`tls_router`) disabled | Disabled in tree with comment "Disabled due to compilation issues" |

## Decision Log

- **2026-04-28**: Picked R2→R3→R4 as the immediate next sequence. R2 (client side) was already partially wired up — confirmed `handle.tcpip_forward()` + `server_channel_open_forwarded_tcpip` handler working. R3 (server listener) was already implemented with `Handle` pattern for `Session.clone()` fix. R4 deleted the orphan `channel_open_direct_tcpip` handler.
- **Next decision point**: R5 (CLI API slot integration) vs R9 (E2E test). R9 should probably come first as it validates the entire chain. However, R5 is a prerequisite for a meaningful R9 test.

## Recommended Order

1. **R5** — Fix CLI to use API-issued port (small, targeted)
2. **R9** — E2E reverse-tunnel test (validates R2+R3+R5 together)
3. **R6** — Real edge-router forward (completes the data plane end-to-end)
4. **R7** — Fix bind address defaults + SNI
5. **R8** — SSH key manager integration
