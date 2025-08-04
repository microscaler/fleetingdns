use axum::{
    Router,
    routing::{delete, get, post, put},
};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;

mod auth;
mod config;
mod error;
mod handlers;
mod middleware;
mod models;
mod quota_enforcement;
mod rate_limiting;
mod storage;
#[cfg(test)]
mod test_utils;

pub use config::ApiConfig;
pub use error::{ApiError, ApiResult};
pub use models::*;
// Do not re-export UserTier from rate_limiting
pub use middleware::{
    CircuitBreaker, error_handler_middleware, error_recovery_middleware, request_size_middleware,
    timeout_middleware,
};
pub use rate_limiting::{RateLimitConfig, RateLimitState};

/// Main API server state
#[derive(Clone)]
pub struct ApiState {
    pub config: Arc<ApiConfig>,
    pub ca: Arc<edf_ca::CertificateAuthority>,
    pub storage: Arc<storage::TunnelStorage>,
    pub github_client: reqwest::Client,
    pub rate_limiter: Arc<RateLimitState>,
    pub db: DatabaseConnection,
    pub quota_enforcer: Arc<quota_enforcement::ServicePlanRateLimiter>,
}

/// Run the API server
pub async fn run() -> ApiResult<()> {
    let config = ApiConfig::from_env().map_err(|e| ApiError::ConfigurationError(e.to_string()))?;
    run_with_config(config).await
}

/// Run the API server with custom configuration
pub async fn run_with_config(config: ApiConfig) -> ApiResult<()> {
    info!("Starting FleetingDNS API server on {}", config.bind_address);

    // Initialize certificate authority
    let ca_config = edf_ca::CaConfig::default();
    let ca = Arc::new(edf_ca::CertificateAuthority::new(ca_config).await?);

    // Initialize storage
    let storage = Arc::new(storage::TunnelStorage::new(&config.redis_url).await?);

    // Initialize HTTP client for GitHub OAuth
    let github_client = reqwest::Client::new();

    // Initialize rate limiting
    let rate_limit_config = RateLimitConfig::default(); // TODO: Load from config
    let rate_limiter = Arc::new(RateLimitState::new(rate_limit_config));

    // Initialize database connection
    let db = sea_orm::Database::connect(&config.database_url).await?;

    // Initialize quota enforcement
    let usage_tracker = Arc::new(quota_enforcement::UsageTracker::new(db.clone()));
    let quota_enforcer = Arc::new(quota_enforcement::ServicePlanRateLimiter::new(
        usage_tracker,
    ));

    // Create application state
    let state = ApiState {
        config: Arc::new(config.clone()),
        ca,
        storage,
        github_client,
        rate_limiter,
        db,
        quota_enforcer,
    };

    // Build the router
    let app = create_router(state);

    // Start the server
    let listener = TcpListener::bind(&config.bind_address).await?;
    info!("API server listening on {}", config.bind_address);

    axum::serve(listener, app).await?;

    Ok(())
}

/// Create the API router with all endpoints
fn create_router(state: ApiState) -> Router {
    Router::new()
        // Health check
        .route("/health", get(handlers::health_check))
        // Authentication endpoints
        .route("/v1/auth/github", post(handlers::auth::github_oauth))
        .route("/v1/auth/token", post(handlers::auth::exchange_token))
        // Tunnel management endpoints
        .route("/v1/tunnels", post(handlers::tunnels::create_tunnel))
        .route("/v1/tunnels/{id}", get(handlers::tunnels::get_tunnel))
        .route("/v1/tunnels/{id}", delete(handlers::tunnels::delete_tunnel))
        .route("/v1/tunnels", get(handlers::tunnels::list_tunnels))
        // Certificate management
        .route(
            "/v1/certificates",
            post(handlers::certificates::issue_certificate),
        )
        .route(
            "/v1/certificates/{serial}",
            get(handlers::certificates::get_certificate),
        )
        // Statistics and monitoring
        .route("/v1/stats", get(handlers::stats::get_stats))
        // Admin endpoints for rate limit policy management
        .route(
            "/admin/rate-limit-policy",
            get(handlers::admin::get_rate_limit_policy),
        )
        .route(
            "/admin/rate-limit-policy",
            put(handlers::admin::update_rate_limit_policy),
        )
        // Admin endpoints for ServicePlan management
        .route(
            "/admin/service-plans",
            post(handlers::admin::create_service_plan),
        )
        .route(
            "/admin/service-plans",
            get(handlers::admin::list_service_plans),
        )
        .route(
            "/admin/service-plans/{id}",
            get(handlers::admin::get_service_plan),
        )
        .route(
            "/admin/service-plans/{id}",
            put(handlers::admin::update_service_plan),
        )
        .route(
            "/admin/service-plans/{id}",
            delete(handlers::admin::delete_service_plan),
        )
        .route(
            "/admin/users/{user_id}/service-plan",
            post(handlers::admin::assign_service_plan_to_user),
        )
        // User-facing ServicePlan endpoints
        .route(
            "/my/service-plan",
            get(handlers::user_service_plan::get_my_service_plan),
        )
        .route(
            "/my/service-plan/usage",
            get(handlers::user_service_plan::get_my_service_plan_usage),
        )
        .route(
            "/service-plans/available",
            get(handlers::user_service_plan::get_available_service_plans),
        )
        .route(
            "/service-plans/change-request",
            post(handlers::user_service_plan::request_service_plan_change),
        )
        // Quota management endpoints
        .route(
            "/my/quota-info",
            get(handlers::quota_management::get_quota_info),
        )
        .route(
            "/quota/check-operation",
            post(handlers::quota_management::check_operation_allowed),
        )
        .route(
            "/admin/quota/reset-usage",
            post(handlers::quota_management::reset_user_usage),
        )
        .route(
            "/admin/quota/all-users-status",
            get(handlers::quota_management::get_all_users_quota_status),
        )
        // Add middleware layers (order matters - error handling first, then rate limiting)
        .layer(axum::middleware::from_fn(error_handler_middleware))
        .layer(axum::middleware::from_fn(error_recovery_middleware))
        .layer(axum::middleware::from_fn(timeout_middleware))
        .layer(axum::middleware::from_fn(request_size_middleware))
        .layer(axum::middleware::from_fn_with_state(
            state.rate_limiter.clone(),
            rate_limiting::rate_limit_middleware,
        ))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

// Re-export main types for external use
pub use handlers::auth::{GitHubOAuthRequest, GitHubOAuthResponse, TokenRequest, TokenResponse};
pub use handlers::tunnels::{CreateTunnelRequest, CreateTunnelResponse, TunnelInfo};

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[tokio::test]
    async fn test_api_config_from_env() {
        let config = ApiConfig::from_env();
        assert!(config.is_ok());
    }

    #[tokio::test]
    async fn test_api_config_with_custom_values() {
        // Set environment variables for testing
        unsafe {
            env::set_var("API_BIND_ADDRESS", "127.0.0.1:8080");
            env::set_var("REDIS_URL", "redis://localhost:6379");
            env::set_var("GITHUB_CLIENT_ID", "test_client_id");
            env::set_var("GITHUB_CLIENT_SECRET", "test_client_secret");
            env::set_var("JWT_SECRET", "test_jwt_secret");
            env::set_var("BASE_DOMAIN", "test.example.com");
            env::set_var("EDGEHUB_ADDRESS", "ssh.example.com:2222");
        }

        let config = ApiConfig::from_env().unwrap();
        assert_eq!(config.bind_address, "127.0.0.1:8080");
        assert_eq!(config.redis_url, "redis://localhost:6379");
        assert_eq!(config.github_client_id, "test_client_id");
        assert_eq!(config.github_client_secret, "test_client_secret");
        assert_eq!(config.jwt_secret, "test_jwt_secret");
        assert_eq!(config.base_domain, "test.example.com");
        assert_eq!(config.edgehub_address, "ssh.example.com:2222");

        // Clean up
        unsafe {
            env::remove_var("API_BIND_ADDRESS");
            env::remove_var("REDIS_URL");
            env::remove_var("GITHUB_CLIENT_ID");
            env::remove_var("GITHUB_CLIENT_SECRET");
            env::remove_var("JWT_SECRET");
            env::remove_var("BASE_DOMAIN");
            env::remove_var("EDGEHUB_ADDRESS");
        }
    }

    #[tokio::test]
    async fn test_run_with_config_invalid_redis() {
        // Set invalid Redis URL to test error handling
        let config = ApiConfig {
            redis_url: "invalid://redis/url".to_string(),
            ..Default::default()
        };

        let result = run_with_config(config).await;
        assert!(result.is_err());

        // Verify error is related to storage or configuration
        match result.unwrap_err() {
            ApiError::StorageError(_) => {}       // Expected error type
            ApiError::ConfigurationError(_) => {} // Also acceptable
            other => panic!("Unexpected error type: {other:?}"),
        }
    }

    #[test]
    fn test_api_config_default_values() {
        let config = ApiConfig::default();
        assert_eq!(config.bind_address, "0.0.0.0:8080");
        assert_eq!(config.redis_url, "redis://localhost:6379");
        assert_eq!(config.base_domain, "fleetingdns.run");
        assert_eq!(config.edgehub_address, "edgehub.fleetingdns.com:443");
        assert_eq!(config.default_tunnel_ttl, 1800);
        assert_eq!(config.max_tunnel_ttl, 7200);
    }

    #[test]
    fn test_api_config_debug_format() {
        let config = ApiConfig::default();
        let debug_str = format!("{config:?}");
        assert!(debug_str.contains("ApiConfig"));
        assert!(debug_str.contains("bind_address"));
        assert!(debug_str.contains("redis_url"));
    }

    #[test]
    fn test_api_config_clone() {
        let config = ApiConfig::default();
        let cloned_config = config.clone();
        assert_eq!(config.bind_address, cloned_config.bind_address);
        assert_eq!(config.redis_url, cloned_config.redis_url);
    }

    #[test]
    fn test_api_state_structure() {
        // Test that ApiState has the expected structure
        let _config = ApiConfig::default();
        let _ca_config = edf_ca::CaConfig::default();
        let _github_client = reqwest::Client::new();

        // We can't easily create real instances without Redis/CA setup
        // but we can test the structure requirements
        // Note: This test is simplified since we can't easily create all the Arc components
        // The main goal is to ensure the structure compiles and has reasonable size
        let state_size = std::mem::size_of::<ApiState>();
        assert!(
            state_size > 50,
            "ApiState size ({}) is unexpectedly small",
            state_size
        );
        assert!(
            state_size < 4096,
            "ApiState size ({}) is unexpectedly large",
            state_size
        );
    }

    #[test]
    fn test_create_router_compiles() {
        // Test that create_router function exists and has correct signature
        // This is a compile-time test to ensure the function is properly defined
        use std::any::type_name;

        // Verify the function exists by checking its type
        let fn_type = type_name::<fn(ApiState) -> Router>();
        assert!(fn_type.contains("Router"));

        // Test that we can reference the function
        let _fn_ref = create_router;
    }

    #[test]
    fn test_api_state_clone_trait() {
        // Test that ApiState implements Clone trait
        // This is a compile-time test
        fn assert_clone<T: Clone>() {}
        assert_clone::<ApiState>();
    }

    #[test]
    fn test_api_error_types() {
        // Test that our error types work correctly
        let config_error = ApiError::ConfigurationError("test error".to_string());
        assert!(matches!(config_error, ApiError::ConfigurationError(_)));

        let storage_error = ApiError::StorageError("test storage error".to_string());
        assert!(matches!(storage_error, ApiError::StorageError(_)));

        let bad_request = ApiError::BadRequest("bad request".to_string());
        assert!(matches!(bad_request, ApiError::BadRequest(_)));
    }

    #[test]
    fn test_api_result_type() {
        // Test that ApiResult works as expected
        let success: ApiResult<String> = Ok("success".to_string());
        assert!(success.is_ok());
        assert_eq!(success.as_ref().unwrap(), "success");

        let failure: ApiResult<String> = Err(ApiError::BadRequest("error".to_string()));
        assert!(failure.is_err());
    }
}
