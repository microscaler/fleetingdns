# ServicePlan Enhancements – Product Requirements Document (PRD)

## Overview
This PRD defines the requirements and implementation plan for enhancing the ServicePlan and UserServicePlan system in FleetingDNS. The goal is to provide robust, flexible, and production-grade subscription, feature, and quota management, supporting both admin and user workflows, billing integration, and future extensibility.

---

## Objectives
- Replace legacy tier logic with a fully DB-driven, extensible ServicePlan system.
- Enable dynamic feature gating, quotas, and rate limits per plan.
- Provide admin and user APIs for plan management.
- Integrate with billing and pricing systems.
- Support future extensibility (custom plans, versioning, A/B testing).
- **Ensure all new logic is covered by comprehensive unit tests and integration tests using a PostgreSQL database.**
- **Require perfect clippy linting (zero warnings/errors) at each step.**
- **Require 'just nt' (test suite) to run without errors at the end of each major task section.**

---

## Functional Requirements

### 1. ServicePlan CRUD & Admin Management
- Admin endpoints to create, update, delete, and list ServicePlans.
- Admin endpoints to assign/unassign ServicePlans to users (UserServicePlan management).
- Validation: prevent deletion of plans in use, enforce unique names.
- **Unit tests:** Model validation, endpoint logic, error handling.
- **Integration tests:** Full CRUD flow using Postgres DB, including migration checks and data validation.
- **Clippy:** All code must pass clippy with zero warnings/errors after this section.
- **Test Suite:** Run 'just nt' and ensure all tests pass after this section.

### 2. Feature Flags & Plan Capabilities
- Add a `features` field (JSON or bitflags) to ServicePlan for dynamic feature matrix.
- Middleware/utilities to check if a user’s plan enables a feature.
- Use feature flags to gate API endpoints and UI features.
- **Unit tests:** Feature parsing, flag logic, middleware checks.
- **Integration tests:** End-to-end feature gating with DB-backed plans.
- **Clippy:** All code must pass clippy with zero warnings/errors after this section.
- **Test Suite:** Run 'just nt' and ensure all tests pass after this section.

### 3. Plan Limits & Quotas
- Store per-plan rate limits and endpoint-specific overrides in ServicePlan.
- Add resource quota fields (max tunnels, max domains, max data transfer, etc.).
- Enforce quotas in business logic and return clear errors when exceeded.
- **Unit tests:** Quota enforcement logic, error cases.
- **Integration tests:** Simulate usage and verify quota enforcement with Postgres DB.
- **Clippy:** All code must pass clippy with zero warnings/errors after this section.
- **Test Suite:** Run 'just nt' and ensure all tests pass after this section.

### 4. Plan Assignment & Lifecycle
- Enhance UserServicePlan with `start_date`, `end_date`, `is_active`, `cancellation_reason`.
- Support scheduled plan changes and plan history tracking.
- **Unit tests:** Assignment logic, lifecycle transitions.
- **Integration tests:** Assignments, scheduled changes, and history tracking in DB.
- **Clippy:** All code must pass clippy with zero warnings/errors after this section.
- **Test Suite:** Run 'just nt' and ensure all tests pass after this section.

### 5. Billing & Pricing Integration
- Store price, currency, and billing interval in ServicePlan or related table.
- Support region-specific and promotional pricing.
- Emit billing events for plan changes, renewals, and overages.
- Integrate with Stripe or other payment providers.
- **Unit tests:** Pricing logic, event emission, Stripe integration stubs/mocks.
- **Integration tests:** Pricing changes, event persistence, and webhook handling with Postgres DB.
- **Clippy:** All code must pass clippy with zero warnings/errors after this section.
- **Test Suite:** Run 'just nt' and ensure all tests pass after this section.

### 6. Self-Service User Portal
- User endpoints to view current plan, usage, and available upgrades.
- Support self-service upgrades/downgrades (with payment integration).
- Usage dashboard showing current usage vs. plan limits.
- **Unit tests:** Endpoint logic, usage calculations.
- **Integration tests:** User flows, upgrades/downgrades, and dashboard data with Postgres DB.
- **Clippy:** All code must pass clippy with zero warnings/errors after this section.
- **Test Suite:** Run 'just nt' and ensure all tests pass after this section.

### 7. Extensibility & Future-Proofing
- Allow creation of custom plans for enterprise customers.
- Support plan versioning and grandfathered users.
- Enable feature flagging for experimental features by plan.
- **Unit tests:** Custom plan logic, versioning, feature flag checks.
- **Integration tests:** Custom plan creation, version migrations, and experimental feature toggling in DB.
- **Clippy:** All code must pass clippy with zero warnings/errors after this section.
- **Test Suite:** Run 'just nt' and ensure all tests pass after this section.

---

## Non-Functional Requirements
- Secure admin endpoints (admin-only access).
- **All changes must be covered by unit tests and integration tests (using a real or test Postgres DB).**
- **All code must pass clippy linting (zero warnings/errors) at every step.**
- **'just nt' must run without errors at the end of each major task section.**
- Backward compatibility for existing users during migration.
- Documentation for all new endpoints and features.

---

## Detailed Task Breakdown

### 1. ServicePlan CRUD & Admin Management
- [ ] Design ServicePlan and UserServicePlan DB schema (add fields for features, quotas, pricing, etc.)
- [ ] Implement admin API endpoints for ServicePlan CRUD
- [ ] Implement admin API endpoints for UserServicePlan assignment
- [ ] Add validation logic (unique names, prevent deletion in use)
- [ ] Write unit tests for all admin endpoints and model logic
- [ ] Write integration tests for CRUD flows using Postgres DB
- [ ] Run clippy and ensure zero warnings/errors
- [ ] Run 'just nt' and ensure all tests pass

### 2. Feature Flags & Plan Capabilities
- [ ] Add `features` field to ServicePlan (JSON/bitflags/table)
- [ ] Implement feature check utility/middleware
- [ ] Update business logic to use feature checks for gated features
- [ ] Write unit tests for feature parsing and gating logic
- [ ] Write integration tests for feature gating with DB-backed plans
- [ ] Run clippy and ensure zero warnings/errors
- [ ] Run 'just nt' and ensure all tests pass

### 3. Plan Limits & Quotas
- [ ] Add rate limit and quota fields to ServicePlan
- [ ] Update rate limiting middleware to use plan-based limits
- [ ] Enforce quotas in tunnel/domain/data logic
- [ ] Write unit tests for quota enforcement logic
- [ ] Write integration tests for quota enforcement with Postgres DB
- [ ] Run clippy and ensure zero warnings/errors
- [ ] Run 'just nt' and ensure all tests pass

### 4. Plan Assignment & Lifecycle
- [ ] Add lifecycle fields to UserServicePlan (start_date, end_date, etc.)
- [ ] Implement scheduled plan change logic
- [ ] Track plan history for users
- [ ] Write unit tests for assignment and lifecycle logic
- [ ] Write integration tests for plan assignment and history in DB
- [ ] Run clippy and ensure zero warnings/errors
- [ ] Run 'just nt' and ensure all tests pass

### 5. Billing & Pricing Integration
- [ ] Add pricing fields to ServicePlan or related table
- [ ] Implement region-specific and promotional pricing logic
- [ ] Emit billing events for plan changes/renewals/overages
- [ ] Integrate with Stripe/payment provider (API, webhook handling)
- [ ] Write unit tests for pricing and billing logic
- [ ] Write integration tests for billing flows and event persistence with Postgres DB
- [ ] Run clippy and ensure zero warnings/errors
- [ ] Run 'just nt' and ensure all tests pass

### 6. Self-Service User Portal
- [ ] Implement user API endpoints for plan/usage viewing
- [ ] Implement endpoints for self-service upgrades/downgrades
- [ ] Build usage dashboard (API, UI if applicable)
- [ ] Write unit tests for user endpoint logic
- [ ] Write integration tests for user flows and dashboard data with Postgres DB
- [ ] Run clippy and ensure zero warnings/errors
- [ ] Run 'just nt' and ensure all tests pass

### 7. Extensibility & Future-Proofing
- [ ] Support custom plan creation (admin API)
- [ ] Implement plan versioning logic
- [ ] Add feature flagging for experimental features
- [ ] Write unit tests for extensibility features
- [ ] Write integration tests for custom plans and versioning in DB
- [ ] Run clippy and ensure zero warnings/errors
- [ ] Run 'just nt' and ensure all tests pass

---

## Milestones & Phases
1. **Phase 1:** Admin CRUD, DB schema, and basic plan assignment (with unit and integration tests, clippy clean, and all tests passing)
2. **Phase 2:** Feature matrix, quotas, and middleware integration (with unit and integration tests, clippy clean, and all tests passing)
3. **Phase 3:** Billing/pricing, user portal, and extensibility (with unit and integration tests, clippy clean, and all tests passing)

---

## Acceptance Criteria
- All admin and user endpoints are implemented and tested
- **All new logic is covered by unit tests (≥80% coverage) and integration tests using a Postgres DB**
- **All code passes clippy linting (zero warnings/errors) at every step**
- **'just nt' runs without errors at the end of each major task section**
- Feature gating and quotas are enforced per plan
- Billing and pricing logic is integrated and tested
- Documentation is updated for all new features
- Migration is backward compatible and does not disrupt existing users

---

## Testing Strategy

### Unit Tests
- Cover all model logic, validation, and business rules.
- Test all API endpoint handlers in isolation (mocking DB as needed).
- Test feature flag parsing, quota enforcement, and plan assignment logic.
- Use mocks/stubs for external dependencies (e.g., Stripe API).
- **Goal:** ≥80% line and branch coverage for all new code.
- **Clippy:** All unit test code must pass clippy with zero warnings/errors.

#### Example Unit Test Cases
- Creating, updating, and deleting ServicePlans (validation, error cases)
- Assigning and unassigning plans to users
- Parsing and checking feature flags
- Enforcing quotas and rate limits
- Handling plan lifecycle transitions
- Emitting billing events

### Integration Tests (with Postgres DB)
- Use a real or test Postgres DB (e.g., testcontainers, docker-compose, or CI-provided DB).
- Run DB migrations before tests; clean up after.
- Test full API flows: CRUD, plan assignment, feature gating, quota enforcement, billing events, and user portal flows.
- Validate data persistence, migration correctness, and cross-entity relationships.
- **Goal:** All critical user/admin flows are covered end-to-end.
- **Clippy:** All integration test code must pass clippy with zero warnings/errors.
- **Test Suite:** Run 'just nt' and ensure all integration tests pass.

#### Example Integration Test Cases
- Creating a ServicePlan via API and verifying DB state
- Assigning a plan to a user and checking access/feature gating
- Simulating usage to hit quotas and verifying enforcement
- Upgrading/downgrading plans and checking billing events
- End-to-end user portal flows (viewing plan, usage, upgrades)

---

## Appendix: Example ServicePlan Table
| Field             | Type        | Description                        |
|-------------------|------------|------------------------------------|
| id                | UUID        | Primary key                        |
| name              | String      | Plan name (unique)                 |
| description       | String      | Human-readable description         |
| price_cents       | Integer     | Price in cents                     |
| currency          | String      | Currency code (e.g., USD, EUR)     |
| billing_interval  | String      | "monthly", "yearly", etc.          |
| rate_limit        | Integer     | Requests per minute                |
| max_tunnels       | Integer     | Max concurrent tunnels             |
| features          | JSON/Flags  | Feature matrix                     |
| is_active         | Boolean     | Plan availability                  |
| created_at        | Timestamp   | Creation time                      |
| updated_at        | Timestamp   | Last update                        | 