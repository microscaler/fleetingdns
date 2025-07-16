# Database Entity Model: Service Plan Management

## 1. Current Database Model (as implemented)

```mermaid
erDiagram
    TUNNEL {
        string id PK
        string github_user_id
        string github_username
        string subdomain
        string fqdn
        int local_port
        int slot
        string certificate_serial
        datetime created_at
        datetime expires_at
        string status
        int bytes_transferred
        int request_count
    }
    GITHUB_USER {
        string id PK
        string login
        string name
        string email
        string avatar_url
    }
    AUTH_TOKEN {
        string token PK
        string token_type
        datetime expires_at
        string user_id FK
    }
    CERTIFICATE_INFO {
        string serial PK
        string certificate
        string private_key
        string fingerprint
        datetime issued_at
        datetime expires_at
        string subject
    }
    SSH_KEY_PAIR {
        string private_key
        string public_key
        string fingerprint
    }
    API_STATS {
        int active_tunnels
        int tunnels_created_today
        int bytes_transferred_today
        int uptime_seconds
        string ca_stats_id FK
    }
    CA_STATS {
        int certificates_issued
        int active_certificates
        int expired_certificates
        float issuance_rate
    }
    USER {
        int id PK
        string login
        string name
        string email
        string avatar_url
    }
    USER_SUBSCRIPTION {
        int user_id FK
        string tier
        datetime created_at
        datetime expires_at
        bool active
        string payment_info_id FK
    }
    PAYMENT_INFO {
        string stripe_customer_id
        string stripe_subscription_id
        datetime last_payment_date
        datetime next_payment_date
    }
    USER_USAGE {
        int user_id FK
        datetime period_start
        int api_calls_count
        int tunnels_created_count
        int dns_operations_count
        int active_tunnels_count
    }
    TUNNEL o|--|| GITHUB_USER : owner
    AUTH_TOKEN }|--|| GITHUB_USER : user
    USER_SUBSCRIPTION }|--|| USER : user
    USER_SUBSCRIPTION }|--|| PAYMENT_INFO : payment
    API_STATS }|--|| CA_STATS : ca_stats
    USER_USAGE }|--|| USER : user
```

---

## 2. Proposed Database Model (Service Plan Management)

```mermaid
erDiagram
    USER {
        string id PK
        string github_id
        string username
        string email
        string avatar_url
        datetime created_at
    }
    
    SERVICE_PLAN {
        string id PK
        string name
        int api_rate_limit
        int tunnel_creation_limit
        int dns_provisioning_limit
        int max_concurrent_tunnels
        string features_json
        datetime created_at
    }
    
    USER_SERVICE_PLAN {
        string id PK
        string user_id FK
        string service_plan_id FK
        datetime start_date
        datetime end_date
        bool is_active
    }

    USER ||--o{ USER_SERVICE_PLAN : has
    SERVICE_PLAN ||--o{ USER_SERVICE_PLAN : assigned_to
```

---

## 3. Future-State Database Model (Planned)

```mermaid
erDiagram
    USER {
        string id PK
        string github_id
        string username
        string email
        string avatar_url
        datetime created_at
    }
    SERVICE_PLAN {
        string id PK
        string name
        int api_rate_limit
        int tunnel_creation_limit
        int dns_provisioning_limit
        int max_concurrent_tunnels
        string features_json
        datetime created_at
    }
    USER_SERVICE_PLAN {
        string id PK
        string user_id FK
        string service_plan_id FK
        datetime start_date
        datetime end_date
        bool is_active
    }
    TUNNEL {
        string id PK
        string user_id FK
        string subdomain
        string fqdn
        int local_port
        int slot
        string certificate_serial FK
        datetime created_at
        datetime expires_at
        string status
        int bytes_transferred
        int request_count
    }
    AUTH_TOKEN {
        string token PK
        string token_type
        datetime expires_at
        string user_id FK
    }
    CERTIFICATE_INFO {
        string serial PK
        string certificate
        string private_key
        string fingerprint
        datetime issued_at
        datetime expires_at
        string subject
    }
    SSH_KEY_PAIR {
        string private_key
        string public_key
        string fingerprint
    }
    API_STATS {
        int active_tunnels
        int tunnels_created_today
        int bytes_transferred_today
        int uptime_seconds
        string ca_stats_id FK
    }
    CA_STATS {
        int certificates_issued
        int active_certificates
        int expired_certificates
        float issuance_rate
    }
    PAYMENT_INFO {
        string id PK
        string user_id FK
        string stripe_customer_id
        string stripe_subscription_id
        datetime last_payment_date
        datetime next_payment_date
    }
    USER_USAGE {
        string id PK
        string user_id FK
        datetime period_start
        int api_calls_count
        int tunnels_created_count
        int dns_operations_count
        int active_tunnels_count
    }
    AUDIT_LOG {
        string id PK
        string user_id FK
        string action
        string resource
        datetime timestamp
        string details_json
    }
    BILLING_EVENT {
        string id PK
        string user_id FK
        string service_plan_id FK
        string event_type
        float amount
        datetime event_time
        string details_json
    }
    USER ||--o{ USER_SERVICE_PLAN : has
    SERVICE_PLAN ||--o{ USER_SERVICE_PLAN : assigned_to
    USER ||--o{ TUNNEL : owns
    USER ||--o{ AUTH_TOKEN : has
    USER ||--o{ PAYMENT_INFO : payment
    USER ||--o{ USER_USAGE : usage
    USER ||--o{ AUDIT_LOG : audit
    USER ||--o{ BILLING_EVENT : billing
    TUNNEL ||--|| CERTIFICATE_INFO : cert
    API_STATS }|--|| CA_STATS : ca_stats
```

---

### Entity Summary Table

| Entity           | Main Fields / Role                                                                 |
|------------------|-----------------------------------------------------------------------------------|
| USER             | id, github_id, username, email, avatar_url, created_at                             |
| SERVICE_PLAN     | id, name, api_rate_limit, tunnel_creation_limit, dns_provisioning_limit, features  |
| USER_SERVICE_PLAN| id, user_id, service_plan_id, start_date, end_date, is_active                      |
| TUNNEL           | id, user_id, subdomain, fqdn, local_port, slot, certificate_serial, status         |
| AUTH_TOKEN       | token, token_type, expires_at, user_id                                             |
| CERTIFICATE_INFO | serial, certificate, private_key, fingerprint, issued_at, expires_at, subject      |
| SSH_KEY_PAIR     | private_key, public_key, fingerprint                                               |
| API_STATS        | active_tunnels, tunnels_created_today, bytes_transferred_today, uptime_seconds     |
| CA_STATS         | certificates_issued, active_certificates, expired_certificates, issuance_rate      |
| PAYMENT_INFO     | id, user_id, stripe_customer_id, subscription_id, last/next payment                |
| USER_USAGE       | id, user_id, period_start, api_calls_count, tunnels_created_count, dns_ops_count   |
| AUDIT_LOG        | id, user_id, action, resource, timestamp, details_json                             |
| BILLING_EVENT    | id, user_id, service_plan_id, event_type, amount, event_time, details_json         |

---

### Migration and Extensibility Notes

- **Migration:**
  - Migrate from enum-based `UserTier`/`UserSubscription` to DB-driven `ServicePlan`/`UserServicePlan`.
  - Move all tier/plan logic to the DB, removing hardcoded limits from code.
  - Update API logic to resolve user plan and limits via join tables.
  - No data migration needed if starting from scratch; otherwise, migrate user subscriptions and usage to new tables.

- **Extensibility:**
  - The future model supports:
    - Plan upgrades/downgrades and history (via `UserServicePlan`)
    - Audit logging for compliance and security (`AuditLog`)
    - Billing events for analytics and invoicing (`BillingEvent`)
    - Advanced usage tracking (`UserUsage`)
    - Feature flags and custom plan features (`features_json` on `ServicePlan`)
    - Easy integration with payment providers (via `PaymentInfo`)
    - Analytics and monitoring (via `ApiStats`, `CaStats`)
  - New entities can be added as needed (e.g., support tickets, notifications, etc.)

- **Actionable Next Steps:**
  - Refactor codebase to use the new model for all user/plan logic.
  - Implement API endpoints for plan management, audit, and billing as needed.
  - Document all new relationships and update onboarding guides for developers.

---

## Summary: Differences and Migration Path

- **Current Model:**
  - User tier/subscription is managed via `UserSubscription` and `UserTier` enums, with payment info and usage tracked in separate tables.
  - Tunnel, certificate, and stats entities are directly linked to users and their GitHub identity.
  - No explicit service plan abstraction; tier is an enum, not a DB entity.

- **Proposed Model:**
  - Introduces `ServicePlan` as a first-class entity, allowing flexible plan definitions and features.
  - `UserServicePlan` join table enables users to have plan history, upgrades, and custom plans.
  - Decouples user identity from plan/tier logic, supporting future extensibility (e.g., trials, enterprise features).

- **Migration Path:**
  - Migrate `UserTier`/`UserSubscription` to `ServicePlan`/`UserServicePlan`.
  - Move tier logic from enums to DB-driven configuration.
  - Update API logic to resolve user plan via join, not enum.
  - Retain usage and payment tracking, but link to new plan model. 