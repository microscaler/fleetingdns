use crate::{ApiResult, ApiState};
use auth::{
    exchange_github_code, generate_jwt_token, validate_github_token,
    GitHubOAuthRequest, GitHubOAuthResponse, TokenRequest, TokenResponse, GitHubUser
};
use axum::{Json, extract::State};
use chrono::Utc;
use tracing::{debug, info};

/// Handle GitHub OAuth code exchange
pub async fn github_oauth(
    State(state): State<ApiState>,
    Json(request): Json<GitHubOAuthRequest>,
) -> ApiResult<Json<GitHubOAuthResponse>> {
    let start_time = std::time::Instant::now();
    
    // Create auth span for tracing
    let span = common::telemetry::auth_span("github_oauth");
    let _enter = span.enter();
    
    debug!("Processing GitHub OAuth request");

    // Exchange code for GitHub access token
    let github_token = exchange_github_code(
        &state.github_client,
        &state.config.github_client_id,
        &state.config.github_client_secret,
        &request.code,
    )
    .await
    .map_err(|e| crate::ApiError::AuthenticationFailed(e.to_string()))?;

    // Get user information from GitHub
    let user = validate_github_token(&state.github_client, &github_token)
        .await
        .map_err(|e| crate::ApiError::AuthenticationFailed(e.to_string()))?;

    info!("User {} authenticated via GitHub", user.login);

    // Generate JWT token
    let jwt_token = generate_jwt_token(&user, &state.config.jwt_secret)
        .map_err(|e| crate::ApiError::AuthenticationFailed(e.to_string()))?;

    let expires_at = Utc::now() + chrono::Duration::hours(24);

    // Record authentication metrics
    let response_time = start_time.elapsed();
    let response_time_ms = response_time.as_millis() as u64;
    common::telemetry::record_auth_metrics("github_oauth", true);
    common::telemetry::record_api_metrics("POST", "/v1/auth/github", 200, response_time_ms);

    Ok(Json(GitHubOAuthResponse {
        token: jwt_token,
        expires_at: expires_at.to_rfc3339(),
        user,
    }))
}

/// Exchange GitHub token for API token
pub async fn exchange_token(
    State(state): State<ApiState>,
    Json(request): Json<TokenRequest>,
) -> ApiResult<Json<TokenResponse>> {
    let start_time = std::time::Instant::now();
    
    // Create auth span for tracing
    let span = common::telemetry::auth_span("token_exchange");
    let _enter = span.enter();
    
    debug!("Processing token exchange request");

    // Validate GitHub token and get user info
    let user = validate_github_token(&state.github_client, &request.github_token)
        .await
        .map_err(|e| crate::ApiError::AuthenticationFailed(e.to_string()))?;

    info!("Token exchange for user {}", user.login);

    // Generate JWT token
    let jwt_token = generate_jwt_token(&user, &state.config.jwt_secret)
        .map_err(|e| crate::ApiError::AuthenticationFailed(e.to_string()))?;

    let expires_at = Utc::now() + chrono::Duration::hours(24);

    // Record authentication metrics
    let response_time = start_time.elapsed();
    let response_time_ms = response_time.as_millis() as u64;
    common::telemetry::record_auth_metrics("token_exchange", true);
    common::telemetry::record_api_metrics("POST", "/v1/auth/token", 200, response_time_ms);

    Ok(Json(TokenResponse {
        token: jwt_token,
        expires_at: expires_at.to_rfc3339(),
    }))
}
