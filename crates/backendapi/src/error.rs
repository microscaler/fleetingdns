use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// API error types
#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Authorization failed: {0}")]
    AuthorizationFailed(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Tunnel not found: {0}")]
    TunnelNotFound(String),

    #[error("Certificate error: {0}")]
    CertificateError(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("GitHub API error: {0}")]
    GitHubApiError(String),

    #[error("External service error: {0}")]
    ExternalService(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Invalid request: {0}")]
    BadRequest(String),

    #[error("Internal server error: {0}")]
    InternalError(String),
}

/// API result type
pub type ApiResult<T> = Result<T, ApiError>;

/// Error response format
#[derive(Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    pub code: u16,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status_code, error_type, message) = match &self {
            &ApiError::AuthenticationFailed(_) => (
                StatusCode::UNAUTHORIZED,
                "authentication_failed",
                self.to_string(),
            ),
            &ApiError::AuthorizationFailed(_) => (
                StatusCode::FORBIDDEN,
                "authorization_failed",
                self.to_string(),
            ),
            &ApiError::Unauthorized(_) => {
                (StatusCode::UNAUTHORIZED, "unauthorized", self.to_string())
            }
            &ApiError::Forbidden(_) => (StatusCode::FORBIDDEN, "forbidden", self.to_string()),
            &ApiError::TunnelNotFound(_) => {
                (StatusCode::NOT_FOUND, "tunnel_not_found", self.to_string())
            }
            &ApiError::CertificateError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "certificate_error",
                self.to_string(),
            ),
            &ApiError::StorageError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "storage_error",
                self.to_string(),
            ),
            &ApiError::ConfigurationError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "configuration_error",
                self.to_string(),
            ),
            &ApiError::GitHubApiError(_) => (
                StatusCode::BAD_GATEWAY,
                "github_api_error",
                self.to_string(),
            ),
            &ApiError::ExternalService(_) => (
                StatusCode::BAD_GATEWAY,
                "external_service_error",
                self.to_string(),
            ),
            &ApiError::RateLimitExceeded => (
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limit_exceeded",
                self.to_string(),
            ),
            &ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request", self.to_string()),
            &ApiError::InternalError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                self.to_string(),
            ),
        };

        let error_response = ErrorResponse {
            error: error_type.to_string(),
            message,
            code: status_code.as_u16(),
        };

        (status_code, Json(error_response)).into_response()
    }
}

// Conversions from other error types
impl From<edf_ca::CaError> for ApiError {
    fn from(err: edf_ca::CaError) -> Self {
        ApiError::CertificateError(err.to_string())
    }
}

impl From<redis::RedisError> for ApiError {
    fn from(err: redis::RedisError) -> Self {
        ApiError::StorageError(err.to_string())
    }
}

impl From<reqwest::Error> for ApiError {
    fn from(err: reqwest::Error) -> Self {
        ApiError::GitHubApiError(err.to_string())
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(err: serde_json::Error) -> Self {
        ApiError::BadRequest(format!("JSON parsing error: {}", err))
    }
}

impl From<std::io::Error> for ApiError {
    fn from(err: std::io::Error) -> Self {
        ApiError::InternalError(err.to_string())
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        ApiError::InternalError(err.to_string())
    }
}
