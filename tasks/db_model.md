# Database Entity Model: Service Plan Management

This model supports associating users with service plans (tiers) independently of the GitHubUser struct.

---

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

## Notes
- **GitHubUser** does not have a `tier` field. User tier/plan is determined by joining `User` to `ServicePlan` via `UserServicePlan`.
- The API should look up the user's active service plan to determine rate limits and features.
- This model supports plan upgrades, history, and future extensibility (e.g., trial periods, custom plans). 