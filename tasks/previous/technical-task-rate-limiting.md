# Technical Tasks: Rate Limiting & DDoS Protection

## Context
This document tracks technical debt, bug fixes, and library compatibility work required to finalize the HIGH-5: Rate Limiting and DDoS Protection implementation. These tasks do not change user-facing features or acceptance criteria, but are required for production readiness.

---

## Task List

### 1. Governor RateLimiter Compatibility
- **Description:** Refactor all usages of `governor::RateLimiter` to use the correct type parameters and state store (e.g., `InMemoryState` or `DashMap`).
- **Steps:**
  - Audit all usages of `RateLimiter` in `crates/backendapi/src/rate_limiting.rs`.
  - Update type signatures to match the expected generics (K, S, C, MW).
  - Use `InMemoryState` or `DashMap` as the state store as required by governor.
  - Update construction and usage patterns accordingly.
  - Add/adjust tests to ensure correct rate limiting behavior.
- **Owner:** Backend engineer familiar with async Rust and governor.

### 2. ApiError Variant Refactor
- **Description:** Refactor the usage of `ApiError::RateLimitExceeded` to match the enum definition and ensure proper error construction.
- **Steps:**
  - Update all usages to use the correct variant or constructor.
  - Add/adjust tests for error handling and response codes.
- **Owner:** Backend engineer.

### 3. UserTier Import/Visibility Cleanup
- **Description:** Refactor imports so `UserTier` is only re-exported from `models`, not from `rate_limiting`.
- **Steps:**
  - Remove `pub use` of `UserTier` from `rate_limiting`.
  - Update all usages to import from `models`.
  - Ensure all modules use the correct import path.
- **Owner:** Backend engineer.

### 4. fleetingdns-ctl Tracing Init
- **Description:** Ensure all binaries call `init_tracing` with the correct argument and error handling.
- **Steps:**
  - Audit all binaries for `init_tracing` usage.
  - Update to pass the service name and handle errors gracefully.
- **Owner:** All maintainers.

---

## Tracking
- [ ] All tasks above are tracked in this file and should be checked off as completed.
- [ ] Optionally, create corresponding GitHub issues for visibility and assignment.

---

**Last updated:** {{DATE}} 