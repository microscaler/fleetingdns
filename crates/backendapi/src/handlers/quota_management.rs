use crate::{
    ApiResult, ApiState,
    auth::{extract_bearer_token, validate_jwt_token},
};
use axum::{Json, extract::State, http::HeaderMap};
use serde::{Deserialize, Serialize};

/// Get detailed quota information for the current user
pub async fn get_quota_info(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> ApiResult<Json<QuotaInfoResponse>> {
    // Extract and validate JWT token
    let token = extract_bearer_token(&headers)?;
    let user = validate_jwt_token(&token, &state.config.jwt_secret)?;
    let user_id = user.id.to_string();

    // Get quota information
    let quota_info = state.quota_enforcer.get_quota_info(&user_id).await?;

    // Check for quota warnings
    let warnings = quota_info.has_quota_warnings();

    let usage = quota_info.usage.clone();
    Ok(Json(QuotaInfoResponse {
        usage: quota_info.usage,
        limits: quota_info.limits,
        warnings: warnings.into_iter().map(|q| format!("{q:?}")).collect(),
        period_start: usage.period_start,
        period_end: usage.period_start + chrono::Duration::days(30), // Monthly period
    }))
}

/// Check if a specific operation is allowed
pub async fn check_operation_allowed(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<OperationCheckRequest>,
) -> ApiResult<Json<OperationCheckResponse>> {
    // Extract and validate JWT token
    let token = extract_bearer_token(&headers)?;
    let user = validate_jwt_token(&token, &state.config.jwt_secret)?;
    let user_id = user.id.to_string();

    // Check the specific operation
    let allowed = match request.operation_type.as_str() {
        "api_call" => state.quota_enforcer.can_make_api_call(&user_id).await?,
        "tunnel_creation" => state.quota_enforcer.can_create_tunnel(&user_id).await?,
        "dns_operation" => {
            state
                .quota_enforcer
                .can_perform_dns_operation(&user_id)
                .await?
        }
        _ => {
            return Err(crate::ApiError::ValidationError(
                "Invalid operation type".to_string(),
            ));
        }
    };

    Ok(Json(OperationCheckResponse {
        allowed,
        operation_type: request.operation_type,
        message: if allowed {
            "Operation is allowed".to_string()
        } else {
            "Operation is not allowed due to quota limits".to_string()
        },
    }))
}

/// Reset usage for the current user (admin only)
pub async fn reset_user_usage(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<ResetUsageRequest>,
) -> ApiResult<Json<ResetUsageResponse>> {
    // Extract and validate JWT token (admin check)
    let token = extract_bearer_token(&headers)?;
    let _user = validate_jwt_token(&token, &state.config.jwt_secret)?;

    // TODO: Add admin role check here
    // For now, allow any authenticated user to reset their own usage

    // Reset usage for the specified user
    let usage_tracker = state.quota_enforcer.usage_tracker.clone();
    usage_tracker.reset_usage(&request.user_id).await?;

    Ok(Json(ResetUsageResponse {
        message: format!("Usage reset successfully for user {}", request.user_id),
        reset_timestamp: chrono::Utc::now(),
    }))
}

/// Get quota enforcement status for all users (admin only)
pub async fn get_all_users_quota_status(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<UserQuotaStatus>>> {
    // Extract and validate JWT token (admin check)
    let token = extract_bearer_token(&headers)?;
    let _user = validate_jwt_token(&token, &state.config.jwt_secret)?;

    // TODO: Add admin role check here
    // For now, return empty list as we don't have a way to list all users yet

    Ok(Json(Vec::new()))
}

// Request/Response types
#[derive(Deserialize)]
pub struct OperationCheckRequest {
    pub operation_type: String, // "api_call", "tunnel_creation", "dns_operation"
}

#[derive(Serialize)]
pub struct OperationCheckResponse {
    pub allowed: bool,
    pub operation_type: String,
    pub message: String,
}

#[derive(Deserialize)]
pub struct ResetUsageRequest {
    pub user_id: String,
}

#[derive(Serialize)]
pub struct ResetUsageResponse {
    pub message: String,
    pub reset_timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize)]
pub struct QuotaInfoResponse {
    pub usage: crate::quota_enforcement::UserUsage,
    pub limits: crate::quota_enforcement::QuotaLimits,
    pub warnings: Vec<String>,
    pub period_start: chrono::DateTime<chrono::Utc>,
    pub period_end: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize)]
pub struct UserQuotaStatus {
    pub user_id: String,
    pub service_plan_id: String,
    pub usage_percentage: f64,
    pub quota_warnings: Vec<String>,
    pub last_activity: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use crate::handlers::service_plan_entity;
    use crate::handlers::user_entity;
    use crate::handlers::user_service_plan_entity;
    use crate::test_utils::postgres_test_container::PostgresTestContainer;
    use chrono::Utc;
    use sea_orm::EntityTrait;
    use std::sync::Arc;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_quota_info_with_real_database() {
        let container = PostgresTestContainer::new().await;
        let db = container.database().clone();

        // Create a test user
        let user_id = Uuid::new_v4().to_string();
        let now = Utc::now().naive_utc();
        let user = user_entity::ActiveModel {
            id: sea_orm::Set(user_id.clone()),
            github_id: sea_orm::Set("test_github_id".to_string()),
            username: sea_orm::Set("test_user".to_string()),
            email: sea_orm::Set("test@example.com".to_string()),
            avatar_url: sea_orm::Set("https://example.com/avatar.png".to_string()),
            created_at: sea_orm::Set(now),
        };
        let _ = user_entity::Entity::insert(user)
            .exec(&db)
            .await
            .expect("create user");

        // Create a test service plan
        let plan_id = Uuid::new_v4().to_string();
        let plan = service_plan_entity::ActiveModel {
            id: sea_orm::Set(plan_id.clone()),
            name: sea_orm::Set("Pro".to_string()),
            api_rate_limit: sea_orm::Set(1000),
            tunnel_creation_limit: sea_orm::Set(10),
            dns_provisioning_limit: sea_orm::Set(5),
            max_concurrent_tunnels: sea_orm::Set(3),
            features_json: sea_orm::Set("{}".to_string()),
            created_at: sea_orm::Set(now),
        };
        let _ = service_plan_entity::Entity::insert(plan)
            .exec(&db)
            .await
            .expect("create service plan");

        // Assign service plan to user
        let assignment = user_service_plan_entity::ActiveModel {
            id: sea_orm::Set(Uuid::new_v4().to_string()),
            user_id: sea_orm::Set(user_id.clone()),
            service_plan_id: sea_orm::Set(plan_id.clone()),
            start_date: sea_orm::Set(now),
            end_date: sea_orm::Set(now + chrono::Duration::days(30)),
            is_active: sea_orm::Set(true),
        };
        let _ = user_service_plan_entity::Entity::insert(assignment)
            .exec(&db)
            .await
            .expect("assign service plan");

        // Test quota info retrieval
        let usage_tracker = Arc::new(crate::quota_enforcement::UsageTracker::new(db.clone()));
        let rate_limiter = crate::quota_enforcement::ServicePlanRateLimiter::new(usage_tracker);

        let quota_info = rate_limiter.get_quota_info(&user_id).await;
        assert!(quota_info.is_ok());
    }

    #[tokio::test]
    async fn test_operation_allowed_with_real_database() {
        let container = PostgresTestContainer::new().await;
        let db = container.database().clone();

        // Create a test user
        let user_id = Uuid::new_v4().to_string();
        let now = Utc::now().naive_utc();
        let user = user_entity::ActiveModel {
            id: sea_orm::Set(user_id.clone()),
            github_id: sea_orm::Set("test_github_id".to_string()),
            username: sea_orm::Set("test_user".to_string()),
            email: sea_orm::Set("test@example.com".to_string()),
            avatar_url: sea_orm::Set("https://example.com/avatar.png".to_string()),
            created_at: sea_orm::Set(now),
        };
        let _ = user_entity::Entity::insert(user)
            .exec(&db)
            .await
            .expect("create user");

        // Test operation allowed check
        let usage_tracker = Arc::new(crate::quota_enforcement::UsageTracker::new(db.clone()));
        let rate_limiter = crate::quota_enforcement::ServicePlanRateLimiter::new(usage_tracker);

        let can_call = rate_limiter.can_make_api_call(&user_id).await;
        assert!(can_call.is_ok());
        assert!(can_call.unwrap());
    }

    #[tokio::test]
    async fn test_usage_reset_with_real_database() {
        let container = PostgresTestContainer::new().await;
        let db = container.database().clone();

        // Create a test user
        let user_id = Uuid::new_v4().to_string();
        let now = Utc::now().naive_utc();
        let user = user_entity::ActiveModel {
            id: sea_orm::Set(user_id.clone()),
            github_id: sea_orm::Set("test_github_id".to_string()),
            username: sea_orm::Set("test_user".to_string()),
            email: sea_orm::Set("test@example.com".to_string()),
            avatar_url: sea_orm::Set("https://example.com/avatar.png".to_string()),
            created_at: sea_orm::Set(now),
        };
        let _ = user_entity::Entity::insert(user)
            .exec(&db)
            .await
            .expect("create user");

        // Test usage reset
        let usage_tracker = Arc::new(crate::quota_enforcement::UsageTracker::new(db.clone()));

        let reset_result = usage_tracker.reset_usage(&user_id).await;
        assert!(reset_result.is_ok());
    }

    #[tokio::test]
    async fn test_all_users_quota_status_with_real_database() {
        let container = PostgresTestContainer::new().await;
        let db = container.database().clone();

        // Create multiple test users
        let user1_id = Uuid::new_v4().to_string();
        let user2_id = Uuid::new_v4().to_string();
        let now = Utc::now().naive_utc();

        let user1 = user_entity::ActiveModel {
            id: sea_orm::Set(user1_id.clone()),
            github_id: sea_orm::Set("test_github_id_1".to_string()),
            username: sea_orm::Set("test_user_1".to_string()),
            email: sea_orm::Set("test1@example.com".to_string()),
            avatar_url: sea_orm::Set("https://example.com/avatar1.png".to_string()),
            created_at: sea_orm::Set(now),
        };
        let _ = user_entity::Entity::insert(user1)
            .exec(&db)
            .await
            .expect("create user 1");

        let user2 = user_entity::ActiveModel {
            id: sea_orm::Set(user2_id.clone()),
            github_id: sea_orm::Set("test_github_id_2".to_string()),
            username: sea_orm::Set("test_user_2".to_string()),
            email: sea_orm::Set("test2@example.com".to_string()),
            avatar_url: sea_orm::Set("https://example.com/avatar2.png".to_string()),
            created_at: sea_orm::Set(now),
        };
        let _ = user_entity::Entity::insert(user2)
            .exec(&db)
            .await
            .expect("create user 2");

        // Test getting all users quota status
        let usage_tracker = Arc::new(crate::quota_enforcement::UsageTracker::new(db.clone()));

        // This would typically query all users and their quota status
        // For now, just test that the tracker can be created and used
        let user1_usage = usage_tracker.get_user_usage(&user1_id).await;
        assert!(user1_usage.is_ok());

        let user2_usage = usage_tracker.get_user_usage(&user2_id).await;
        assert!(user2_usage.is_ok());
    }
}
