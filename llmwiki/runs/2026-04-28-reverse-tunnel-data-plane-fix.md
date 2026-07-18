# 2026-04-28: Reverse-tunnel data-plane fixes (S1 + S2)

## S1: edgehub ssh_server.rs — Session.clone() -> Handle pattern

**Problem:** `russh::server::Session` doesn't implement `Clone`. The `tcpip_forward` listener handler used `session_clone.clone()` to spawn tasks, which won't compile.

**Fix:** Use `Session::handle()` to get a `Handle` (which IS cloneable), then call `Handle::channel_open_forwarded_tcpip()` instead of `Session::channel_open_forwarded_tcpip()`. The spawned task then uses `Handle::channel_open_forwarded_tcpip()` with the cloned handle.

Key change in `crates/edgehub/src/ssh_server.rs`:
```rust
// Before: session_clone.clone() - Session is not Clone
// After: session.handle() - Handle is Clone
let handle = session.handle();
tokio::spawn(async move {
    let mut channel = match handle.clone().channel_open_forwarded_tcpip(...).await {
        // ...
    };
    // ...copy_bidirectional
});
```

**Verification:** `cargo check -p edgehub` — 0 errors. `cargo test --workspace --lib` — 335 tests pass.

## S2: edf-cli ssh_client.rs — Compilation errors

Multiple issues fixed in `cmd/edf-cli/src/ssh_client.rs`:

1. **`ChannelStream::split()` doesn't exist** — `russh`'s `ChannelStream<S>` implements both `AsyncRead` and `AsyncWrite` directly. Removed `.split()` call, use `channel.into_stream()` directly.

2. **`session.shutdown()` doesn't exist** — Removed the `session.shutdown()` select arm. `copy_bidirectional` completes when either side closes.

3. **`handle_forwarded_tcpip_channel` nested in wrong impl block** — Was inside `impl Handler for SshClientHandler` as a trait method. Moved to `impl SshClientHandler` as a regular helper method.

4. **`handle.tcpip_forward()` borrow issue** — `Handle` doesn't implement `Clone` (it's a single-use struct). Changed `if let Some(handle) = &self.handle` to `if let Some(handle) = self.handle.as_mut()` to get `&mut Handle`.

5. **`Self::Error` ambiguity** — Since `handle_forwarded_tcpip_channel` is in a regular impl (not a trait impl), `Self::Error` is ambiguous. Changed to `anyhow::Error`.

**Verification:** `cargo check -p edf-cli` — 0 errors, 9 warnings (unused code, expected).

## Pre-existing test failures (not fixed)

`cargo test --workspace` (with tests/) fails on pre-existing issues:
- `fleetingdns_integration_tests` unresolved import
- redis `MultiplexedConnection` doesn't implement `ConnectionLike`
- Old API usage in test files (russh 0.40 API changes)

These are not related to the reverse-tunnel data-plane work.
