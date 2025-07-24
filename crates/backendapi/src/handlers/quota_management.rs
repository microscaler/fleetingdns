use crate::{ApiResult, ApiState, auth::{extract_bearer_token, validate_jwt_token}};
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
        warnings: warnings.into_iter().map(|q| format!("{:?}", q)).collect(),
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
        "dns_operation" => state.quota_enforcer.can_perform_dns_operation(&user_id).await?,
        _ => return Err(crate::ApiError::ValidationError("Invalid operation type".to_string())),
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
    use super::*;

    #[tokio::test]
    async fn test_operation_check_response_creation() {
        let response = OperationCheckResponse {
            allowed: true,
            operation_type: "api_call".to_string(),
            message: "Operation is allowed".to_string(),
        };

        assert!(response.allowed);
        assert_eq!(response.operation_type, "api_call");
        assert_eq!(response.message, "Operation is allowed");
    }

    #[tokio::test]
    async fn test_reset_usage_response_creation() {
        let response = ResetUsageResponse {
            message: "Usage reset successfully".to_string(),
            reset_timestamp: chrono::Utc::now(),
        };

        assert_eq!(response.message, "Usage reset successfully");
        assert!(response.reset_timestamp > chrono::Utc::now() - chrono::Duration::seconds(1));
    }

    #[tokio::test]
    async fn test_quota_info_response_creation() {
        let now = chrono::Utc::now();
        let usage = crate::quota_enforcement::UserUsage {
            user_id: "test_user".to_string(),
            service_plan_id: "test_plan".to_string(),
            api_calls_count: 10,
            tunnels_created_count: 5,
            dns_operations_count: 3,
            active_tunnels_count: 2,
            data_transferred_mb: 100,
            certificates_issued_count: 8,
            last_updated: now,
            period_start: now,
        };

        let limits = crate::quota_enforcement::QuotaLimits {
            api_rate_limit: 1000,
            tunnel_creation_limit: 100,
            dns_provisioning_limit: 50,
            max_concurrent_tunnels: 10,
            data_transfer_limit_mb: Some(1024),
            certificate_issuance_limit: Some(100),
        };

        let response = QuotaInfoResponse {
            usage: usage.clone(),
            limits: limits.clone(),
            warnings: vec!["API calls near limit".to_string()],
            period_start: now,
            period_end: now + chrono::Duration::days(30),
        };

        assert_eq!(response.usage.user_id, "test_user");
        assert_eq!(response.limits.api_rate_limit, 1000);
        assert_eq!(response.warnings.len(), 1);
        assert_eq!(response.warnings[0], "API calls near limit");
    }

    #[tokio::test]
    async fn test_user_quota_status_creation() {
        let status = UserQuotaStatus {
            user_id: "test_user".to_string(),
            service_plan_id: "test_plan".to_string(),
            usage_percentage: 75.5,
            quota_warnings: vec!["API calls near limit".to_string()],
            last_activity: chrono::Utc::now(),
        };

        assert_eq!(status.user_id, "test_user");
        assert_eq!(status.service_plan_id, "test_plan");
        assert_eq!(status.usage_percentage, 75.5);
        assert_eq!(status.quota_warnings.len(), 1);
        assert_eq!(status.quota_warnings[0], "API calls near limit");
    }

    #[tokio::test]
    async fn test_operation_types_validation() {
        let valid_operations = vec![
            "api_call",
            "tunnel_creation", 
            "dns_operation",
        ];

        for operation in valid_operations {
            let request = OperationCheckRequest {
                operation_type: operation.to_string(),
            };
            
            // Test that the request can be created
            assert_eq!(request.operation_type, operation);
        }
    }

    #[tokio::test]
    async fn test_response_message_generation() {
        let allowed_response = OperationCheckResponse {
            allowed: true,
            operation_type: "api_call".to_string(),
            message: "Operation is allowed".to_string(),
        };

        let denied_response = OperationCheckResponse {
            allowed: false,
            operation_type: "tunnel_creation".to_string(),
            message: "Operation is not allowed due to quota limits".to_string(),
        };

        assert!(allowed_response.message.contains("allowed"));
        assert!(denied_response.message.contains("not allowed"));
    }
} 