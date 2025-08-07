use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use thiserror::Error;

/// Enhanced API error types with detailed error information
#[derive(Error, Debug, Clone)]
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

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    // MEDIUM-1 ENHANCEMENT: New error types for enhanced error handling
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Precondition failed: {0}")]
    PreconditionFailed(String),

    #[error("Request timeout: {0}")]
    RequestTimeout(String),

    #[error("Payload too large: {0}")]
    PayloadTooLarge(String),

    #[error("Unsupported media type: {0}")]
    UnsupportedMediaType(String),

    #[error("Too many requests: {0}")]
    TooManyRequests(String),

    #[error("Quota exceeded: {0}")]
    QuotaExceeded(String),

    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),

    #[error("Circuit breaker open: {0}")]
    CircuitBreakerOpen(String),
}

/// API result type
pub type ApiResult<T> = Result<T, ApiError>;

/// Enhanced error response format with detailed information
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ErrorResponse {
    /// Error type/code
    pub error: String,

    /// Human-readable error message
    pub message: String,

    /// HTTP status code
    pub code: u16,

    /// Unique error ID for tracking
    pub error_id: String,

    /// Timestamp when error occurred
    pub timestamp: DateTime<Utc>,

    /// Request ID for correlation (if available)
    pub request_id: Option<String>,

    /// Additional error details
    pub details: Option<HashMap<String, serde_json::Value>>,

    /// Retry information
    pub retry_after: Option<u64>,

    /// Error category for client handling
    pub category: ErrorCategory,
}

/// Error categories for client-side handling
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ErrorCategory {
    /// Client error - user should fix request
    Client,

    /// Authentication error - user should re-authenticate
    Authentication,

    /// Authorization error - user lacks permissions
    Authorization,

    /// Rate limiting error - user should wait
    RateLimit,

    /// Resource error - system resource constraints
    Resource,

    /// Service error - internal system issue
    Service,

    /// Network error - connectivity issues
    Network,

    /// Validation error - data validation failed
    Validation,
}

/// Error context for enhanced error tracking
#[derive(Debug, Clone, Default)]
pub struct ErrorContext {
    pub request_id: Option<String>,
    pub user_id: Option<String>,
    pub endpoint: Option<String>,
    pub method: Option<String>,
    pub client_ip: Option<String>,
    pub user_agent: Option<String>,
}

impl ApiError {
    /// Create enhanced error response with context
    pub fn into_response_with_context(self, context: ErrorContext) -> Response {
        let (status_code, error_type, message, category, retry_after) = self.get_error_info();

        let error_response = ErrorResponse {
            error: error_type.to_string(),
            message,
            code: status_code.as_u16(),
            error_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            request_id: context.request_id.clone(),
            details: self.get_error_details(&context),
            retry_after,
            category,
        };

        // Log error with context for monitoring
        self.log_error(&context);

        (status_code, Json(error_response)).into_response()
    }

    /// Get error information for response
    fn get_error_info(&self) -> (StatusCode, &'static str, String, ErrorCategory, Option<u64>) {
        match self {
            ApiError::AuthenticationFailed(msg) => (
                StatusCode::UNAUTHORIZED,
                "authentication_failed",
                msg.clone(),
                ErrorCategory::Authentication,
                None,
            ),
            ApiError::AuthorizationFailed(msg) => (
                StatusCode::FORBIDDEN,
                "authorization_failed",
                msg.clone(),
                ErrorCategory::Authorization,
                None,
            ),
            ApiError::Unauthorized(msg) => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                msg.clone(),
                ErrorCategory::Authentication,
                None,
            ),
            ApiError::Forbidden(msg) => (
                StatusCode::FORBIDDEN,
                "forbidden",
                msg.clone(),
                ErrorCategory::Authorization,
                None,
            ),
            ApiError::TunnelNotFound(msg) => (
                StatusCode::NOT_FOUND,
                "tunnel_not_found",
                msg.clone(),
                ErrorCategory::Client,
                None,
            ),
            ApiError::CertificateError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "certificate_error",
                msg.clone(),
                ErrorCategory::Service,
                None,
            ),
            ApiError::StorageError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "storage_error",
                msg.clone(),
                ErrorCategory::Service,
                None,
            ),
            ApiError::ConfigurationError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "configuration_error",
                msg.clone(),
                ErrorCategory::Service,
                None,
            ),
            ApiError::GitHubApiError(msg) => (
                StatusCode::BAD_GATEWAY,
                "github_api_error",
                msg.clone(),
                ErrorCategory::Service,
                Some(60), // Retry after 1 minute
            ),
            ApiError::ExternalService(msg) => (
                StatusCode::BAD_GATEWAY,
                "external_service_error",
                msg.clone(),
                ErrorCategory::Service,
                Some(30), // Retry after 30 seconds
            ),
            ApiError::RateLimitExceeded => (
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limit_exceeded",
                "Rate limit exceeded".to_string(),
                ErrorCategory::RateLimit,
                Some(60), // Retry after 1 minute
            ),
            ApiError::BadRequest(msg) => (
                StatusCode::BAD_REQUEST,
                "bad_request",
                msg.clone(),
                ErrorCategory::Client,
                None,
            ),
            ApiError::InternalError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                msg.clone(),
                ErrorCategory::Service,
                None,
            ),
            ApiError::ValidationError(msg) => (
                StatusCode::BAD_REQUEST,
                "validation_error",
                msg.clone(),
                ErrorCategory::Validation,
                None,
            ),
            ApiError::NotFound(msg) => (
                StatusCode::NOT_FOUND,
                "not_found",
                msg.clone(),
                ErrorCategory::Client,
                None,
            ),
            ApiError::DatabaseError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                msg.clone(),
                ErrorCategory::Service,
                Some(5), // Retry after 5 seconds
            ),
            // MEDIUM-1 ENHANCEMENT: New error types
            ApiError::ServiceUnavailable(msg) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "service_unavailable",
                msg.clone(),
                ErrorCategory::Service,
                Some(30), // Retry after 30 seconds
            ),
            ApiError::Conflict(msg) => (
                StatusCode::CONFLICT,
                "conflict",
                msg.clone(),
                ErrorCategory::Client,
                None,
            ),
            ApiError::PreconditionFailed(msg) => (
                StatusCode::PRECONDITION_FAILED,
                "precondition_failed",
                msg.clone(),
                ErrorCategory::Client,
                None,
            ),
            ApiError::RequestTimeout(msg) => (
                StatusCode::REQUEST_TIMEOUT,
                "request_timeout",
                msg.clone(),
                ErrorCategory::Network,
                Some(10), // Retry after 10 seconds
            ),
            ApiError::PayloadTooLarge(msg) => (
                StatusCode::PAYLOAD_TOO_LARGE,
                "payload_too_large",
                msg.clone(),
                ErrorCategory::Client,
                None,
            ),
            ApiError::UnsupportedMediaType(msg) => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "unsupported_media_type",
                msg.clone(),
                ErrorCategory::Client,
                None,
            ),
            ApiError::TooManyRequests(msg) => (
                StatusCode::TOO_MANY_REQUESTS,
                "too_many_requests",
                msg.clone(),
                ErrorCategory::RateLimit,
                Some(60), // Retry after 1 minute
            ),
            ApiError::QuotaExceeded(msg) => (
                StatusCode::FORBIDDEN,
                "quota_exceeded",
                msg.clone(),
                ErrorCategory::Resource,
                None,
            ),
            ApiError::ResourceExhausted(msg) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "resource_exhausted",
                msg.clone(),
                ErrorCategory::Resource,
                Some(300), // Retry after 5 minutes
            ),
            ApiError::CircuitBreakerOpen(msg) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "circuit_breaker_open",
                msg.clone(),
                ErrorCategory::Service,
                Some(60), // Retry after 1 minute
            ),
        }
    }

    /// Get additional error details for enhanced debugging
    fn get_error_details(
        &self,
        context: &ErrorContext,
    ) -> Option<HashMap<String, serde_json::Value>> {
        let mut details = HashMap::new();

        // Add context information
        if let Some(user_id) = &context.user_id {
            details.insert(
                "user_id".to_string(),
                serde_json::Value::String(user_id.clone()),
            );
        }
        if let Some(endpoint) = &context.endpoint {
            details.insert(
                "endpoint".to_string(),
                serde_json::Value::String(endpoint.clone()),
            );
        }
        if let Some(method) = &context.method {
            details.insert(
                "method".to_string(),
                serde_json::Value::String(method.clone()),
            );
        }
        if let Some(client_ip) = &context.client_ip {
            details.insert(
                "client_ip".to_string(),
                serde_json::Value::String(client_ip.clone()),
            );
        }

        // Add error-specific details
        match self {
            ApiError::RateLimitExceeded | ApiError::TooManyRequests(_) => {
                details.insert(
                    "retry_after_seconds".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(60)),
                );
            }
            ApiError::QuotaExceeded(_) => {
                details.insert(
                    "quota_type".to_string(),
                    serde_json::Value::String("user_quota".to_string()),
                );
            }
            ApiError::ValidationError(_) => {
                details.insert(
                    "validation_type".to_string(),
                    serde_json::Value::String("request_validation".to_string()),
                );
            }
            _ => {}
        }

        if details.is_empty() {
            None
        } else {
            Some(details)
        }
    }

    /// Log error with context for monitoring and debugging
    fn log_error(&self, context: &ErrorContext) {
        use tracing::{error, info, warn};

        let log_message = format!(
            "API Error: {} | User: {:?} | Endpoint: {:?} | Method: {:?} | Client IP: {:?}",
            self, context.user_id, context.endpoint, context.method, context.client_ip
        );

        match self {
            ApiError::InternalError(_)
            | ApiError::DatabaseError(_)
            | ApiError::CertificateError(_) => {
                error!("{}", log_message);
            }
            ApiError::RateLimitExceeded | ApiError::TooManyRequests(_) => {
                warn!("{}", log_message);
            }
            _ => {
                info!("{}", log_message);
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        self.into_response_with_context(ErrorContext::default())
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
        ApiError::BadRequest(format!("JSON parsing error: {err}"))
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

impl From<sea_orm::DbErr> for ApiError {
    fn from(err: sea_orm::DbErr) -> Self {
        ApiError::DatabaseError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn test_api_error_display() {
        let error = ApiError::BadRequest("Invalid input".to_string());
        assert_eq!(error.to_string(), "Invalid request: Invalid input");

        let error = ApiError::Unauthorized("Access denied".to_string());
        assert_eq!(error.to_string(), "Unauthorized: Access denied");

        let error = ApiError::TunnelNotFound("tunnel-123".to_string());
        assert_eq!(error.to_string(), "Tunnel not found: tunnel-123");
    }

    #[test]
    fn test_api_error_into_response() {
        let error = ApiError::BadRequest("Invalid input".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let error = ApiError::Unauthorized("Access denied".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let error = ApiError::Forbidden("Forbidden".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let error = ApiError::TunnelNotFound("tunnel-123".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let error = ApiError::StorageError("Database error".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_api_error_variants() {
        let errors = vec![
            ApiError::BadRequest("bad request".to_string()),
            ApiError::Unauthorized("unauthorized".to_string()),
            ApiError::Forbidden("forbidden".to_string()),
            ApiError::TunnelNotFound("tunnel-123".to_string()),
            ApiError::StorageError("storage error".to_string()),
            ApiError::CertificateError("cert error".to_string()),
            ApiError::ConfigurationError("config error".to_string()),
            ApiError::InternalError("internal error".to_string()),
        ];

        // Test that all variants can be created and converted to strings
        for error in errors {
            let error_string = error.to_string();
            assert!(!error_string.is_empty());

            // Test that they can be converted to responses
            let response = error.into_response();
            assert!(response.status().is_client_error() || response.status().is_server_error());
        }
    }

    #[test]
    fn test_api_error_debug() {
        let error = ApiError::BadRequest("Invalid input".to_string());
        let debug_str = format!("{error:?}");
        assert!(debug_str.contains("BadRequest"));
        assert!(debug_str.contains("Invalid input"));
    }

    #[test]
    fn test_api_error_from_edf_ca_error() {
        let ca_error = edf_ca::CaError::BadRequest("Invalid cert request".to_string());
        let api_error: ApiError = ca_error.into();

        match api_error {
            ApiError::CertificateError(msg) => {
                assert!(msg.contains("Invalid cert request"));
            }
            _ => panic!("Expected CertificateError"),
        }
    }

    #[test]
    fn test_api_error_from_io_error() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let api_error: ApiError = io_error.into();

        match api_error {
            ApiError::InternalError(msg) => {
                assert!(msg.contains("File not found"));
            }
            _ => panic!("Expected InternalError"),
        }
    }

    #[test]
    fn test_api_result_type_alias() {
        // Test that ApiResult is properly aliased
        let success: ApiResult<String> = Ok("success".to_string());
        assert!(success.is_ok());
        assert_eq!(success.as_ref().unwrap(), "success");

        let error: ApiResult<String> = Err(ApiError::BadRequest("error".to_string()));
        assert!(error.is_err());
    }

    #[test]
    fn test_api_error_status_codes() {
        let test_cases = vec![
            (
                ApiError::BadRequest("test".to_string()),
                StatusCode::BAD_REQUEST,
            ),
            (
                ApiError::Unauthorized("test".to_string()),
                StatusCode::UNAUTHORIZED,
            ),
            (
                ApiError::Forbidden("test".to_string()),
                StatusCode::FORBIDDEN,
            ),
            (
                ApiError::TunnelNotFound("test".to_string()),
                StatusCode::NOT_FOUND,
            ),
            (
                ApiError::StorageError("test".to_string()),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            (
                ApiError::CertificateError("test".to_string()),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            (
                ApiError::ConfigurationError("test".to_string()),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            (
                ApiError::InternalError("test".to_string()),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        ];

        for (error, expected_status) in test_cases {
            let response = error.into_response();
            assert_eq!(response.status(), expected_status);
        }
    }

    #[tokio::test]
    async fn test_api_error_response_body() {
        let error = ApiError::BadRequest("Invalid input".to_string());
        let response = error.into_response();

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();

        // Should contain error information in JSON format
        assert!(body_str.contains("error"));
        assert!(body_str.contains("Invalid input"));
    }

    #[test]
    fn test_api_error_is_send_sync() {
        // Test that ApiError implements Send and Sync
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ApiError>();
    }
}
