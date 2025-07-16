# FleetingDNS Migration Crate

This is the canonical migration crate for FleetingDNS. All database migration logic, tests, and binaries must reside here.

- Do **not** use a root-level `migration/` crate.
- All workspace, CI, and Docker references must use `crates/migration/`.
- This crate supports both embedded and CLI/Docker migration workflows.
- See the ServicePlan PRD for policy details and rationale. 