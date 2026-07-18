//! Authentication crate for FleetingDNS
//!
//! This crate provides comprehensive authentication functionality including:
//! - GitHub OAuth integration following official GitHub REST API specifications
//! - JWT token generation and validation
//! - User management and service plan resolution
//! - Development mode bypass support
//! - Proper scope management and token validation

use axum::http::HeaderMap;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use tracing::debug;

/// Authentication errors
#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    #[error("External service error: {0}")]
    ExternalService(String),
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    #[error("Token expired")]
    TokenExpired,
    #[error("Invalid token format")]
    InvalidTokenFormat,
    #[error("Invalid token signature")]
    InvalidTokenSignature,
    #[error("Insufficient scopes: required {required}, granted {granted}")]
    InsufficientScopes { required: String, granted: String },
    #[error("Token revoked")]
    TokenRevoked,
}

/// Result type for authentication operations
pub type AuthResult<T> = Result<T, AuthError>;

/// GitHub user information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubUser {
    pub id: String,
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: String,
    pub public_repos: Option<u32>,
    pub followers: Option<u32>,
    pub following: Option<u32>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// Service plan information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServicePlan {
    pub id: String,
    pub name: String,
    pub api_rate_limit: u32,
    pub tunnel_creation_limit: u32,
    pub dns_provisioning_limit: u32,
    pub max_concurrent_tunnels: u32,
    pub features_json: Option<String>,
    pub created_at: chrono::DateTime<Utc>,
}

/// GitHub OAuth authorization request
#[derive(Debug, Deserialize)]
pub struct GitHubOAuthRequest {
    /// OAuth authorization code from GitHub
    pub code: String,
    /// OAuth state parameter for CSRF protection
    pub state: Option<String>,
}

/// GitHub OAuth authorization response
#[derive(Debug, Serialize)]
pub struct GitHubOAuthResponse {
    /// JWT token for API access
    pub token: String,
    /// Token expiration time
    pub expires_at: String,
    /// GitHub user information
    pub user: GitHubUser,
    /// Granted scopes
    pub scopes: Vec<String>,
}

/// Token exchange request
#[derive(Debug, Deserialize)]
pub struct TokenRequest {
    /// GitHub access token
    pub github_token: String,
}

/// Token exchange response
#[derive(Debug, Serialize)]
pub struct TokenResponse {
    /// JWT token for API access
    pub token: String,
    /// Token expiration time
    pub expires_at: String,
}

/// Internal GitHub token response from OAuth exchange
#[derive(Debug, Deserialize)]
struct GitHubTokenResponse {
    access_token: String,
    #[allow(dead_code)] // deserialized for API fidelity; not read
    token_type: String,
    #[allow(dead_code)] // deserialized for API fidelity; not read
    scope: String,
}

/// Internal GitHub user response from API
#[derive(Debug, Deserialize)]
struct GitHubUserResponse {
    id: u64,
    login: String,
    name: Option<String>,
    email: Option<String>,
    avatar_url: String,
    public_repos: Option<u32>,
    followers: Option<u32>,
    following: Option<u32>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

/// Authenticated user with resolved service plan
#[derive(Debug, Clone)]
pub struct AuthenticatedUserWithPlan {
    pub user: GitHubUser,
    pub service_plan: ServicePlan,
}

/// Required scopes for FleetingDNS
pub const REQUIRED_SCOPES: &[&str] = &["user:email", "read:user"];

/// Extract Bearer token from Authorization header
pub fn extract_bearer_token(headers: &HeaderMap) -> AuthResult<String> {
    extract_bearer_token_with_dev_bypass(headers, false)
}

/// Extract Bearer token from Authorization header with optional development bypass
pub fn extract_bearer_token_with_dev_bypass(
    headers: &HeaderMap,
    development_mode: bool,
) -> AuthResult<String> {
    // Check for development bypass header when in development mode
    if development_mode
        && let Some(bypass_header) = headers.get("x-development-bypass")
        && let Ok(bypass_value) = bypass_header.to_str()
        && bypass_value == "true"
    {
        debug!("Using development bypass token");
        return Ok("dev-bypass-token".to_string());
    }

    // Fall back to normal authentication
    let auth_header = headers.get("authorization").ok_or_else(|| {
        AuthError::AuthenticationFailed("Missing Authorization header".to_string())
    })?;

    let auth_str = auth_header
        .to_str()
        .map_err(|_| AuthError::AuthenticationFailed("Invalid Authorization header".to_string()))?;

    if !auth_str.starts_with("Bearer ") {
        return Err(AuthError::AuthenticationFailed(
            "Authorization header must start with 'Bearer '".to_string(),
        ));
    }

    // SECURITY: `validate_jwt_token` accepts the literal dev-bypass token,
    // so a client presenting it as a Bearer credential would authenticate
    // as dev-user in production. Only development mode may mint it.
    if !development_mode && auth_str[7..].trim() == "dev-bypass-token" {
        return Err(AuthError::AuthenticationFailed(
            "Development bypass token is not accepted".to_string(),
        ));
    }

    Ok(auth_str[7..].to_string())
}

/// Validate GitHub access token and get user info
pub async fn validate_github_token(
    client: &reqwest::Client,
    token: &str,
) -> AuthResult<GitHubUser> {
    let response = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("token {token}"))
        .header("User-Agent", "FleetingDNS/1.0")
        .send()
        .await
        .map_err(|e| AuthError::ExternalService(e.to_string()))?;

    if !response.status().is_success() {
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(AuthError::TokenRevoked);
        }
        return Err(AuthError::AuthenticationFailed(
            "Invalid GitHub token".to_string(),
        ));
    }

    // Check granted scopes from response headers
    if let Some(scopes_header) = response.headers().get("x-oauth-scopes")
        && let Ok(scopes_str) = scopes_header.to_str()
    {
        let granted_scopes: Vec<&str> = scopes_str.split(", ").collect();
        if !has_required_scopes(&granted_scopes) {
            return Err(AuthError::InsufficientScopes {
                required: REQUIRED_SCOPES.join(", "),
                granted: granted_scopes.join(", "),
            });
        }
    }

    let github_user: GitHubUserResponse = response
        .json()
        .await
        .map_err(|e| AuthError::ExternalService(e.to_string()))?;

    Ok(GitHubUser {
        id: github_user.id.to_string(),
        login: github_user.login,
        name: github_user.name,
        email: github_user.email,
        avatar_url: github_user.avatar_url,
        public_repos: github_user.public_repos,
        followers: github_user.followers,
        following: github_user.following,
        created_at: github_user.created_at,
        updated_at: github_user.updated_at,
    })
}

/// Check if granted scopes include required scopes
fn has_required_scopes(granted_scopes: &[&str]) -> bool {
    for required_scope in REQUIRED_SCOPES {
        let has_scope = granted_scopes.iter().any(|scope| {
            // Exact match
            scope == required_scope ||
            // Hierarchical match (e.g., "user" grants "user:email")
            (required_scope.contains(':') && scope == &required_scope.split(':').next().unwrap()) ||
            // Broader scope (e.g., "user" grants "read:user")
            (scope.contains(':') && required_scope.contains(':') &&
             scope.split(':').next() == required_scope.split(':').next())
        });

        if !has_scope {
            return false;
        }
    }
    true
}

/// Get GitHub user information from access token
pub async fn get_github_user(token: &str) -> AuthResult<GitHubUser> {
    let response = reqwest::Client::new()
        .get("https://api.github.com/user")
        .header("User-Agent", "FleetingDNS-API")
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| AuthError::ExternalService(e.to_string()))?;

    if !response.status().is_success() {
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(AuthError::TokenRevoked);
        }
        return Err(AuthError::Unauthorized("Invalid GitHub token".to_string()));
    }

    let github_user: GitHubUserResponse = response
        .json()
        .await
        .map_err(|e| AuthError::ExternalService(e.to_string()))?;

    Ok(GitHubUser {
        id: github_user.id.to_string(),
        login: github_user.login,
        name: github_user.name,
        email: github_user.email,
        avatar_url: github_user.avatar_url,
        public_repos: github_user.public_repos,
        followers: github_user.followers,
        following: github_user.following,
        created_at: github_user.created_at,
        updated_at: github_user.updated_at,
    })
}

/// Exchange GitHub OAuth code for access token
pub async fn exchange_github_code(
    client: &reqwest::Client,
    client_id: &str,
    client_secret: &str,
    code: &str,
) -> AuthResult<String> {
    let mut params = HashMap::new();
    params.insert("client_id", client_id);
    params.insert("client_secret", client_secret);
    params.insert("code", code);

    let response = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .header("User-Agent", "FleetingDNS/1.0")
        .form(&params)
        .send()
        .await
        .map_err(|e| AuthError::ExternalService(e.to_string()))?;

    if !response.status().is_success() {
        return Err(AuthError::AuthenticationFailed(
            "Failed to exchange GitHub code".to_string(),
        ));
    }

    let token_response: GitHubTokenResponse = response
        .json()
        .await
        .map_err(|e| AuthError::ExternalService(e.to_string()))?;
    Ok(token_response.access_token)
}

/// Generate JWT token for authenticated user
pub fn generate_jwt_token(user: &GitHubUser, secret: &str) -> AuthResult<String> {
    // For now, return a simple signed token
    // In production, use a proper JWT library like jsonwebtoken
    let payload = format!("{}:{}:{}", user.id, user.login, Utc::now().timestamp());
    let signature = format!("{:x}", md5::compute(format!("{payload}{secret}")));
    Ok(format!("{payload}.{signature}"))
}

/// Validate JWT token
pub fn validate_jwt_token(token: &str, secret: &str) -> AuthResult<GitHubUser> {
    // Handle development bypass token
    if token == "dev-bypass-token" {
        return Ok(GitHubUser {
            id: "0".to_string(),
            login: "dev-user".to_string(),
            name: Some("Development User".to_string()),
            email: Some("dev@fleetingdns.run".to_string()),
            avatar_url: "https://github.com/github.png".to_string(),
            public_repos: None,
            followers: None,
            following: None,
            created_at: None,
            updated_at: None,
        });
    }

    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 2 {
        return Err(AuthError::InvalidTokenFormat);
    }

    let payload = parts[0];
    let signature = parts[1];

    // Verify signature
    let expected_signature = format!("{:x}", md5::compute(format!("{payload}{secret}")));
    if signature != expected_signature {
        return Err(AuthError::InvalidTokenSignature);
    }

    // Parse payload
    let payload_parts: Vec<&str> = payload.split(':').collect();
    if payload_parts.len() != 3 {
        return Err(AuthError::InvalidTokenFormat);
    }

    let user_id = payload_parts[0].to_string();
    let username = payload_parts[1].to_string();
    let timestamp: i64 = payload_parts[2]
        .parse()
        .map_err(|_| AuthError::InvalidTokenFormat)?;

    // Check if token is expired (24 hours)
    let token_time = Utc::now().timestamp() - timestamp;
    if token_time > 86400 {
        return Err(AuthError::TokenExpired);
    }

    Ok(GitHubUser {
        id: user_id,
        login: username,
        name: None,
        email: None,
        avatar_url: String::new(),
        public_repos: None,
        followers: None,
        following: None,
        created_at: None,
        updated_at: None,
    })
}

/// Validate JWT token and resolve user's active service plan
///
/// Returns AuthenticatedUserWithPlan (user + plan)
pub fn validate_jwt_token_with_plan(
    token: &str,
    secret: &str,
) -> AuthResult<AuthenticatedUserWithPlan> {
    // Validate JWT as before
    let user = validate_jwt_token(token, secret)?;

    // --- BEGIN MOCK DB LOOKUP ---
    // In production, replace this with a real DB lookup for UserServicePlan and ServicePlan
    // For now, return a hardcoded "Pro" plan for demonstration
    let plan = ServicePlan {
        id: "pro".to_string(),
        name: "Pro".to_string(),
        api_rate_limit: 300,
        tunnel_creation_limit: 50,
        dns_provisioning_limit: 100,
        max_concurrent_tunnels: 20,
        features_json: None,
        created_at: chrono::Utc::now(),
    };
    // --- END MOCK DB LOOKUP ---

    Ok(AuthenticatedUserWithPlan {
        user,
        service_plan: plan,
    })
}

/// Check if an endpoint is public (doesn't require authentication)
pub fn is_public_endpoint(path: &str) -> bool {
    let public_paths = [
        "/health",
        "/v1/auth/github",
        "/v1/auth/token",
        "/metrics",
        "/docs",
        "/openapi.json",
    ];

    public_paths
        .iter()
        .any(|public_path| path.starts_with(public_path))
}

/// Generate GitHub OAuth authorization URL
pub fn generate_github_oauth_url(
    client_id: &str,
    redirect_uri: &str,
    state: Option<&str>,
) -> String {
    let mut url = format!(
        "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&scope={}",
        client_id,
        redirect_uri,
        REQUIRED_SCOPES.join("%20")
    );

    if let Some(state_param) = state {
        use std::fmt::Write as _;
        let _ = write!(url, "&state={state_param}");
    }

    url
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_jwt_token_generation_and_validation() {
        let user = GitHubUser {
            id: "12345".to_string(),
            login: "testuser".to_string(),
            name: Some("Test User".to_string()),
            email: Some("test@example.com".to_string()),
            avatar_url: "https://example.com/avatar.png".to_string(),
            public_repos: None,
            followers: None,
            following: None,
            created_at: None,
            updated_at: None,
        };

        let secret = "test-secret";
        let token = generate_jwt_token(&user, secret).unwrap();
        let validated_user = validate_jwt_token(&token, secret).unwrap();

        assert_eq!(user.id, validated_user.id);
        assert_eq!(user.login, validated_user.login);
    }

    #[test]
    fn test_invalid_token() {
        let result = validate_jwt_token("invalid.token", "secret");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer test-token"),
        );

        let result = extract_bearer_token_with_dev_bypass(&headers, false);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test-token");
    }

    #[test]
    fn test_extract_bearer_token_with_dev_bypass() {
        let mut headers = HeaderMap::new();
        headers.insert("x-development-bypass", HeaderValue::from_static("true"));

        let result = extract_bearer_token_with_dev_bypass(&headers, true);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "dev-bypass-token");
    }

    /// SECURITY: the literal dev-bypass token authenticates as dev-user in
    /// `validate_jwt_token`, so it must never survive extraction when
    /// development mode is off — neither via the bypass header nor when
    /// smuggled in as a Bearer credential.
    #[test]
    fn test_dev_bypass_rejected_in_production_mode() {
        // Bypass header alone is ignored outside development mode.
        let mut headers = HeaderMap::new();
        headers.insert("x-development-bypass", HeaderValue::from_static("true"));
        assert!(extract_bearer_token_with_dev_bypass(&headers, false).is_err());

        // The literal token as a Bearer credential is rejected outright.
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer dev-bypass-token"),
        );
        assert!(extract_bearer_token_with_dev_bypass(&headers, false).is_err());

        // ...but still accepted while in development mode.
        let result = extract_bearer_token_with_dev_bypass(&headers, true);
        assert_eq!(result.unwrap(), "dev-bypass-token");
    }

    #[test]
    fn test_is_public_endpoint() {
        assert!(is_public_endpoint("/health"));
        assert!(is_public_endpoint("/v1/auth/github"));
        assert!(is_public_endpoint("/v1/auth/token"));
        assert!(is_public_endpoint("/metrics"));
        assert!(!is_public_endpoint("/v1/tunnels"));
        assert!(!is_public_endpoint("/v1/certificates"));
    }

    #[test]
    fn test_has_required_scopes() {
        let granted_scopes = vec!["user:email", "read:user"];
        assert!(has_required_scopes(&granted_scopes));

        let granted_scopes = vec!["user:email"];
        assert!(!has_required_scopes(&granted_scopes));

        let granted_scopes = vec!["user", "read:user"];
        assert!(has_required_scopes(&granted_scopes));
    }

    #[test]
    fn test_generate_github_oauth_url() {
        let url =
            generate_github_oauth_url("test_client_id", "http://localhost:8080/callback", None);
        assert!(url.contains("client_id=test_client_id"));
        assert!(url.contains("redirect_uri=http://localhost:8080/callback"));
        assert!(url.contains("scope=user:email%20read:user"));

        let url = generate_github_oauth_url(
            "test_client_id",
            "http://localhost:8080/callback",
            Some("test_state"),
        );
        assert!(url.contains("state=test_state"));
    }
}
