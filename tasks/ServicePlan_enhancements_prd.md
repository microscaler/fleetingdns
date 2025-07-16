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

---

## Functional Requirements

### 1. ServicePlan CRUD & Admin Management
- Admin endpoints to create, update, delete, and list ServicePlans.
- Admin endpoints to assign/unassign ServicePlans to users (UserServicePlan management).
- Validation: prevent deletion of plans in use, enforce unique names.

### 2. Feature Flags & Plan Capabilities
- Add a `features` field (JSON or bitflags) to ServicePlan for dynamic feature matrix.
- Middleware/utilities to check if a user’s plan enables a feature.
- Use feature flags to gate API endpoints and UI features.

### 3. Plan Limits & Quotas
- Store per-plan rate limits and endpoint-specific overrides in ServicePlan.
- Add resource quota fields (max tunnels, max domains, max data transfer, etc.).
- Enforce quotas in business logic and return clear errors when exceeded.

### 4. Plan Assignment & Lifecycle
- Enhance UserServicePlan with `start_date`, `end_date`, `is_active`, `cancellation_reason`.
- Support scheduled plan changes and plan history tracking.

### 5. Billing & Pricing Integration
- Store price, currency, and billing interval in ServicePlan or related table.
- Support region-specific and promotional pricing.
- Emit billing events for plan changes, renewals, and overages.
- Integrate with Stripe or other payment providers.

### 6. Self-Service User Portal
- User endpoints to view current plan, usage, and available upgrades.
- Support self-service upgrades/downgrades (with payment integration).
- Usage dashboard showing current usage vs. plan limits.

### 7. Extensibility & Future-Proofing
- Allow creation of custom plans for enterprise customers.
- Support plan versioning and grandfathered users.
- Enable feature flagging for experimental features by plan.

---

## Non-Functional Requirements
- Secure admin endpoints (admin-only access).
- All changes must be covered by tests (unit, integration, and API tests).
- Backward compatibility for existing users during migration.
- Documentation for all new endpoints and features.

---

## Detailed Task Breakdown

### 1. ServicePlan CRUD & Admin Management
- [ ] Design ServicePlan and UserServicePlan DB schema (add fields for features, quotas, pricing, etc.)
- [ ] Implement admin API endpoints for ServicePlan CRUD
- [ ] Implement admin API endpoints for UserServicePlan assignment
- [ ] Add validation logic (unique names, prevent deletion in use)
- [ ] Write tests for all admin endpoints

### 2. Feature Flags & Plan Capabilities
- [ ] Add `features` field to ServicePlan (JSON/bitflags/table)
- [ ] Implement feature check utility/middleware
- [ ] Update business logic to use feature checks for gated features
- [ ] Write tests for feature gating

### 3. Plan Limits & Quotas
- [ ] Add rate limit and quota fields to ServicePlan
- [ ] Update rate limiting middleware to use plan-based limits
- [ ] Enforce quotas in tunnel/domain/data logic
- [ ] Write tests for quota enforcement

### 4. Plan Assignment & Lifecycle
- [ ] Add lifecycle fields to UserServicePlan (start_date, end_date, etc.)
- [ ] Implement scheduled plan change logic
- [ ] Track plan history for users
- [ ] Write tests for plan assignment and history

### 5. Billing & Pricing Integration
- [ ] Add pricing fields to ServicePlan or related table
- [ ] Implement region-specific and promotional pricing logic
- [ ] Emit billing events for plan changes/renewals/overages
- [ ] Integrate with Stripe/payment provider (API, webhook handling)
- [ ] Write tests for billing logic

### 6. Self-Service User Portal
- [ ] Implement user API endpoints for plan/usage viewing
- [ ] Implement endpoints for self-service upgrades/downgrades
- [ ] Build usage dashboard (API, UI if applicable)
- [ ] Write tests for user endpoints

### 7. Extensibility & Future-Proofing
- [ ] Support custom plan creation (admin API)
- [ ] Implement plan versioning logic
- [ ] Add feature flagging for experimental features
- [ ] Write tests for extensibility features

---

## Milestones & Phases
1. **Phase 1:** Admin CRUD, DB schema, and basic plan assignment
2. **Phase 2:** Feature matrix, quotas, and middleware integration
3. **Phase 3:** Billing/pricing, user portal, and extensibility

---

## Acceptance Criteria
- All admin and user endpoints are implemented and tested
- Feature gating and quotas are enforced per plan
- Billing and pricing logic is integrated and tested
- Documentation is updated for all new features
- Migration is backward compatible and does not disrupt existing users

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