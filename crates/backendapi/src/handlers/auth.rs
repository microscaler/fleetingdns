use crate::{
    ApiResult, ApiState,
    auth::{exchange_github_code, generate_jwt_token, validate_github_token},
};
use axum::{Json, extract::State};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// OAuth callback response
#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct OAuthCallbackResponse {
    /// JWT token for API access
    pub token: String,
    /// Token expiration time
    pub expires_at: String,
    /// GitHub user information
    pub user: crate::models::GitHubUser,
}

/// GitHub OAuth request
#[derive(Debug, Deserialize)]
pub struct GitHubOAuthRequest {
    pub code: String,
    pub state: Option<String>,
}

/// GitHub OAuth response
#[derive(Debug, Serialize)]
pub struct GitHubOAuthResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_at: String,
    pub user: crate::models::GitHubUser,
}

/// Token exchange request
#[derive(Debug, Deserialize)]
pub struct TokenRequest {
    pub github_token: String,
}

/// Token exchange response
#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_at: String,
}

/// Handle GitHub OAuth code exchange
pub async fn github_oauth(
    State(state): State<ApiState>,
    Json(request): Json<GitHubOAuthRequest>,
) -> ApiResult<Json<GitHubOAuthResponse>> {
    debug!("Processing GitHub OAuth request");

    // Exchange code for GitHub access token
    let github_token = exchange_github_code(
        &state.github_client,
        &state.config.github_client_id,
        &state.config.github_client_secret,
        &request.code,
    )
    .await?;

    // Get user information from GitHub
    let user = validate_github_token(&state.github_client, &github_token).await?;

    info!("User {} authenticated via GitHub", user.login);

    // Generate JWT token
    let jwt_token = generate_jwt_token(&user, &state.config.jwt_secret)?;

    let expires_at = Utc::now() + chrono::Duration::hours(24);

    Ok(Json(GitHubOAuthResponse {
        access_token: jwt_token,
        token_type: "Bearer".to_string(),
        expires_at: expires_at.to_rfc3339(),
        user,
    }))
}

/// Exchange GitHub token for API token
pub async fn exchange_token(
    State(state): State<ApiState>,
    Json(request): Json<TokenRequest>,
) -> ApiResult<Json<TokenResponse>> {
    debug!("Processing token exchange request");

    // Validate GitHub token and get user info
    let user = validate_github_token(&state.github_client, &request.github_token).await?;

    info!("Token exchange for user {}", user.login);

    // Generate JWT token
    let jwt_token = generate_jwt_token(&user, &state.config.jwt_secret)?;

    let expires_at = Utc::now() + chrono::Duration::hours(24);

    Ok(Json(TokenResponse {
        access_token: jwt_token,
        token_type: "Bearer".to_string(),
        expires_at: expires_at.to_rfc3339(),
    }))
}
