//! Authentication middleware for FleetingDNS API
//!
//! This module provides Tower middleware for JWT token validation and user extraction.
//! It integrates with the existing GitHub OAuth system and provides development bypass support.

use crate::{ApiError, ApiState};
use auth::{extract_bearer_token_with_dev_bypass, is_public_endpoint, validate_jwt_token};
use axum::{
    extract::{Request, State},
    http::HeaderMap,
    middleware::Next,
    response::{IntoResponse, Response},
};
use tracing::{info, warn};

/// Authenticated user context
#[derive(Debug, Clone)]
#[allow(dead_code)] // TODO(TDP-13/auth): populate from validated JWT in middleware
pub struct AuthenticatedUser {
    pub user_id: String,
    pub username: String,
    pub email: Option<String>,
    pub avatar_url: String,
}

/// Authentication middleware that validates JWT tokens and extracts user information
pub async fn auth_middleware(
    State(state): State<ApiState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, Response> {
    let path = request.uri().path();

    // Skip authentication for public endpoints
    if is_public_endpoint(path) {
        return Ok(next.run(request).await);
    }

    // Extract and validate JWT token
    let token = match extract_bearer_token_with_dev_bypass(&headers, state.config.development_mode)
    {
        Ok(token) => token,
        Err(e) => {
            warn!(path = %path, error = %e, "Authentication failed");
            let error_response = ApiError::AuthenticationFailed(e.to_string()).into_response();
            return Err(error_response);
        }
    };

    // Validate JWT token and get user information
    let user = match validate_jwt_token(&token, &state.config.jwt_secret) {
        Ok(user) => user,
        Err(e) => {
            warn!(path = %path, error = %e, "JWT validation failed");
            let error_response = ApiError::AuthenticationFailed(e.to_string()).into_response();
            return Err(error_response);
        }
    };

    info!(user_id = %user.id, username = %user.login, path = %path, "User authenticated");

    // Create authenticated user context
    let authenticated_user = AuthenticatedUser {
        user_id: user.id,
        username: user.login,
        email: user.email,
        avatar_url: user.avatar_url,
    };

    // Add user context to request extensions
    let mut request = request;
    request.extensions_mut().insert(authenticated_user);

    // Process the request
    let response = next.run(request).await;

    // Record authentication metrics
    common::telemetry::record_auth_metrics("middleware_auth", true);

    Ok(response)
}

/// Extract authenticated user from request extensions
#[allow(dead_code)] // TODO(TDP-13/auth): call from protected handlers
pub fn get_authenticated_user(request: &Request) -> Option<AuthenticatedUser> {
    request.extensions().get::<AuthenticatedUser>().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

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

    #[test]
    fn test_extract_bearer_token_missing_header() {
        let headers = HeaderMap::new();
        let result = extract_bearer_token_with_dev_bypass(&headers, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_bearer_token_invalid_format() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", HeaderValue::from_static("Invalid token"));

        let result = extract_bearer_token_with_dev_bypass(&headers, false);
        assert!(result.is_err());
    }
}
