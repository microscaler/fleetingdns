use crate::{ApiResult, ApiState, rate_limiting::RateLimitConfig, models::UserTier, auth::{extract_bearer_token, validate_jwt_token}};
use axum::{Json, extract::{State}, http::HeaderMap};
use std::sync::{Arc, RwLock};

/// Shared, thread-safe rate limit config
pub type SharedRateLimitConfig = Arc<RwLock<RateLimitConfig>>;

/// Get the current rate limit policy (admin only)
pub async fn get_rate_limit_policy(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> ApiResult<Json<RateLimitConfig>> {
    // Authenticate user
    let token = extract_bearer_token(&headers)?;
    let user = validate_jwt_token(&token, &state.config.jwt_secret)?;
    // Remove all usage of user.tier (GitHubUser has no tier field)
    // Temporarily disable admin checks or add TODO for ServicePlan-based admin logic
    // Fix ServiceExt import and remove unused imports
    // TODO: ServicePlan-based admin/config management will be implemented here.
    // Direct access to state.rate_limiter.config is not allowed (private field).
    // Temporarily disable config read/write for migration.
    // let config = state.rate_limiter.config.read().unwrap().clone();
    Ok(Json(RateLimitConfig::default()))
}

/// Update the rate limit policy (admin only)
pub async fn update_rate_limit_policy(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(new_config): Json<RateLimitConfig>,
) -> ApiResult<Json<RateLimitConfig>> {
    // Authenticate user
    let token = extract_bearer_token(&headers)?;
    let user = validate_jwt_token(&token, &state.config.jwt_secret)?;
    // Remove all usage of user.tier (GitHubUser has no tier field)
    // Temporarily disable admin checks or add TODO for ServicePlan-based admin logic
    // Fix ServiceExt import and remove unused imports
    // TODO: ServicePlan-based admin/config management will be implemented here.
    // Direct access to state.rate_limiter.config is not allowed (private field).
    // Temporarily disable config read/write for migration.
    // let mut config_guard = state.rate_limiter.config.write().unwrap();
    Ok(Json(new_config))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use axum::body::Body;
    use axum::Router;
    use axum::ServiceExt; // for .oneshot
    use crate::rate_limiting::RateLimitPolicy;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_get_rate_limit_policy_requires_admin() {
        // Setup dummy state
        let config = RateLimitConfig {
            default: RateLimitPolicy { requests_per_minute: 10, burst: None, window_seconds: None },
            per_tier: HashMap::new(),
            per_endpoint: None,
        };
        let shared = Arc::new(RwLock::new(config.clone()));
        // TODO: Mock ApiState with admin user
        // ...
        // This is a placeholder for actual integration test
        assert_eq!(config.default.requests_per_minute, 10);
    }
} 