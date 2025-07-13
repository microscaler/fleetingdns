use crate::{models::*, ApiError, ApiResult};
use axum::http::HeaderMap;
use chrono::Utc;
use reqwest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// GitHub OAuth authorization request
#[derive(Debug, Deserialize)]
pub struct GitHubOAuthRequest {
    /// OAuth authorization code from GitHub
    pub code: String,
    
    /// State parameter for CSRF protection
    pub state: Option<String>,
}

/// GitHub OAuth response
#[derive(Debug, Serialize)]
pub struct GitHubOAuthResponse {
    /// Generated access token
    pub access_token: String,
    
    /// Token type (Bearer)
    pub token_type: String,
    
    /// Token expiration time
    pub expires_at: String,
    
    /// GitHub user information
    pub user: GitHubUser,
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
    /// JWT access token for API
    pub access_token: String,
    
    /// Token type (Bearer)
    pub token_type: String,
    
    /// Token expiration time
    pub expires_at: String,
}

/// GitHub access token response
#[derive(Debug, Deserialize)]
struct GitHubTokenResponse {
    access_token: String,
    token_type: String,
    scope: String,
}

/// GitHub user API response
#[derive(Debug, Deserialize)]
struct GitHubUserResponse {
    id: u64,
    login: String,
    name: Option<String>,
    email: Option<String>,
    avatar_url: String,
}

/// Extract Bearer token from Authorization header
pub fn extract_bearer_token(headers: &HeaderMap) -> ApiResult<String> {
    let auth_header = headers
        .get("authorization")
        .ok_or_else(|| ApiError::AuthenticationFailed("Missing Authorization header".to_string()))?;
    
    let auth_str = auth_header
        .to_str()
        .map_err(|_| ApiError::AuthenticationFailed("Invalid Authorization header".to_string()))?;
    
    if !auth_str.starts_with("Bearer ") {
        return Err(ApiError::AuthenticationFailed(
            "Authorization header must start with 'Bearer '".to_string(),
        ));
    }
    
    Ok(auth_str[7..].to_string())
}

/// Validate GitHub access token and get user info
pub async fn validate_github_token(
    client: &reqwest::Client,
    token: &str,
) -> ApiResult<GitHubUser> {
    let response = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("token {}", token))
        .header("User-Agent", "FleetingDNS/1.0")
        .send()
        .await?;
    
    if !response.status().is_success() {
        return Err(ApiError::AuthenticationFailed(
            "Invalid GitHub token".to_string(),
        ));
    }
    
    let github_user: GitHubUserResponse = response.json().await?;
    
    Ok(GitHubUser {
        id: github_user.id.to_string(),
        login: github_user.login,
        name: github_user.name,
        email: github_user.email,
        avatar_url: github_user.avatar_url,
    })
}

/// Exchange GitHub OAuth code for access token
pub async fn exchange_github_code(
    client: &reqwest::Client,
    client_id: &str,
    client_secret: &str,
    code: &str,
) -> ApiResult<String> {
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
        .await?;
    
    if !response.status().is_success() {
        return Err(ApiError::AuthenticationFailed(
            "Failed to exchange GitHub code".to_string(),
        ));
    }
    
    let token_response: GitHubTokenResponse = response.json().await?;
    Ok(token_response.access_token)
}

/// Generate JWT token for authenticated user
pub fn generate_jwt_token(user: &GitHubUser, secret: &str) -> ApiResult<String> {
    // For now, return a simple signed token
    // In production, use a proper JWT library like jsonwebtoken
    let payload = format!("{}:{}:{}", user.id, user.login, Utc::now().timestamp());
    let signature = format!("{:x}", md5::compute(format!("{}{}", payload, secret)));
    Ok(format!("{}.{}", payload, signature))
}

/// Validate JWT token
pub fn validate_jwt_token(token: &str, secret: &str) -> ApiResult<GitHubUser> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 2 {
        return Err(ApiError::AuthenticationFailed("Invalid token format".to_string()));
    }
    
    let payload = parts[0];
    let signature = parts[1];
    
    // Verify signature
    let expected_signature = format!("{:x}", md5::compute(format!("{}{}", payload, secret)));
    if signature != expected_signature {
        return Err(ApiError::AuthenticationFailed("Invalid token signature".to_string()));
    }
    
    // Parse payload
    let payload_parts: Vec<&str> = payload.split(':').collect();
    if payload_parts.len() != 3 {
        return Err(ApiError::AuthenticationFailed("Invalid token payload".to_string()));
    }
    
    let user_id = payload_parts[0].to_string();
    let username = payload_parts[1].to_string();
    let timestamp: i64 = payload_parts[2].parse()
        .map_err(|_| ApiError::AuthenticationFailed("Invalid timestamp".to_string()))?;
    
    // Check if token is expired (24 hours)
    let token_time = Utc::now().timestamp() - timestamp;
    if token_time > 86400 {
        return Err(ApiError::AuthenticationFailed("Token expired".to_string()));
    }
    
    Ok(GitHubUser {
        id: user_id,
        login: username,
        name: None,
        email: None,
        avatar_url: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_jwt_token_generation_and_validation() {
        let user = GitHubUser {
            id: "12345".to_string(),
            login: "testuser".to_string(),
            name: Some("Test User".to_string()),
            email: Some("test@example.com".to_string()),
            avatar_url: "https://example.com/avatar.png".to_string(),
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
} 