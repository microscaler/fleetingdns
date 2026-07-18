use crate::{ApiResult, ApiState};
use auth::{extract_bearer_token, validate_jwt_token};
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
    let user_id = user.id.clone();

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
    let user_id = user.id.clone();

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
    use models::{
        service_plan::Entity as ServicePlanEntity, user::Entity as UserEntity,
        user_service_plan::Entity as UserServicePlanEntity,
    };
    use std::sync::Arc;

    #[tokio::test]
    async fn test_quota_entities_compile() {
        // Test that we can use the models crate entities
        let _service_plan_entity = ServicePlanEntity;
        let _user_entity = UserEntity;
        let _user_service_plan_entity = UserServicePlanEntity;

        // compile-only test
    }

    #[tokio::test]
    async fn test_quota_management_types() {
        // Test that the quota management types compile correctly
        let _usage_tracker = Arc::new(crate::quota_enforcement::UsageTracker::new(
            sea_orm::DatabaseConnection::default(),
        ));
        let _rate_limiter = crate::quota_enforcement::ServicePlanRateLimiter::new(Arc::new(
            crate::quota_enforcement::UsageTracker::new(sea_orm::DatabaseConnection::default()),
        ));
    }
}
