# Contributing to FleetingDNS

## Git hooks (fmt + clippy + tests)

This repo ships a versioned pre-commit hook under `.githooks/` that runs the
same quality gate as CI, so failures are caught locally before they reach a PR.

Enable it once per clone:

```bash
just setup-hooks
# equivalent to:
git config core.hooksPath .githooks
```

On every commit that touches Rust sources or the cargo manifests/lockfile, the
hook runs:

1. `cargo fmt --all -- --check` — formatting must match `cargo fmt`.
2. `cargo clippy --workspace --all-targets -- -D warnings` — `clippy::pedantic`
   is configured in `[workspace.lints]`; any warning (including in test code)
   fails the commit.
3. `cargo test --workspace --lib --bins` — fast unit/bin tests.

Commits that touch only docs/config skip the Rust checks. The heavier
Docker/Redis end-to-end tests (`crates/edgehub/tests/e2e_*`, integration
tests) are **not** run by the hook — they run in CI.

To bypass the hook for a work-in-progress commit:

```bash
git commit --no-verify
```

## Before opening a PR

Run the e2e suites at least once locally (they need Docker for the
testcontainers Redis/Postgres):

```bash
cargo test --workspace          # includes e2e + integration tests
```
