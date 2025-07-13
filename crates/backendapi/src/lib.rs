use axum::{
    Router,
    routing::{delete, get, post},
};
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;

mod auth;
mod config;
mod error;
mod handlers;
mod models;
mod storage;

pub use config::ApiConfig;
pub use error::{ApiError, ApiResult};
pub use models::*;

/// Main API server state
#[derive(Clone)]
pub struct ApiState {
    pub config: Arc<ApiConfig>,
    pub ca: Arc<edf_ca::CertificateAuthority>,
    pub storage: Arc<storage::TunnelStorage>,
    pub github_client: reqwest::Client,
}

/// Run the API server
pub async fn run() -> ApiResult<()> {
    let config = ApiConfig::from_env().map_err(|e| ApiError::ConfigurationError(e.to_string()))?;
    run_with_config(config).await
}

/// Run the API server with custom configuration
pub async fn run_with_config(config: ApiConfig) -> ApiResult<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("Starting FleetingDNS API server on {}", config.bind_address);

    // Initialize certificate authority
    let ca_config = edf_ca::CaConfig::default();
    let ca = Arc::new(edf_ca::CertificateAuthority::new(ca_config).await?);

    // Initialize storage
    let storage = Arc::new(storage::TunnelStorage::new(&config.redis_url).await?);

    // Initialize HTTP client for GitHub OAuth
    let github_client = reqwest::Client::new();

    // Create application state
    let state = ApiState {
        config: Arc::new(config.clone()),
        ca,
        storage,
        github_client,
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
        .route("/v1/tunnels/:id", get(handlers::tunnels::get_tunnel))
        .route("/v1/tunnels/:id", delete(handlers::tunnels::delete_tunnel))
        .route("/v1/tunnels", get(handlers::tunnels::list_tunnels))
        // Certificate management
        .route(
            "/v1/certificates",
            post(handlers::certificates::issue_certificate),
        )
        .route(
            "/v1/certificates/:serial",
            get(handlers::certificates::get_certificate),
        )
        // Statistics and monitoring
        .route("/v1/stats", get(handlers::stats::get_stats))
        // Add middleware
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

    #[tokio::test]
    async fn test_api_config() {
        let config = ApiConfig::default();
        assert!(!config.bind_address.is_empty());
        assert!(!config.github_client_id.is_empty());
    }
}
