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

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Database error: {0}")]
    DatabaseError(String),
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
        let (status_code, error_type, message) = match self {
            ApiError::AuthenticationFailed(_) => (
                StatusCode::UNAUTHORIZED,
                "authentication_failed",
                self.to_string(),
            ),
            ApiError::AuthorizationFailed(_) => (
                StatusCode::FORBIDDEN,
                "authorization_failed",
                self.to_string(),
            ),
            ApiError::Unauthorized(_) => {
                (StatusCode::UNAUTHORIZED, "unauthorized", self.to_string())
            }
            ApiError::Forbidden(_) => (StatusCode::FORBIDDEN, "forbidden", self.to_string()),
            ApiError::TunnelNotFound(_) => {
                (StatusCode::NOT_FOUND, "tunnel_not_found", self.to_string())
            }
            ApiError::CertificateError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "certificate_error",
                self.to_string(),
            ),
            ApiError::StorageError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "storage_error",
                self.to_string(),
            ),
            ApiError::ConfigurationError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "configuration_error",
                self.to_string(),
            ),
            ApiError::GitHubApiError(_) => (
                StatusCode::BAD_GATEWAY,
                "github_api_error",
                self.to_string(),
            ),
            ApiError::ExternalService(_) => (
                StatusCode::BAD_GATEWAY,
                "external_service_error",
                self.to_string(),
            ),
            ApiError::RateLimitExceeded => (
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limit_exceeded",
                self.to_string(),
            ),
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request", self.to_string()),
            ApiError::InternalError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                self.to_string(),
            ),
            ApiError::ValidationError(_) => (
                StatusCode::BAD_REQUEST,
                "validation_error",
                self.to_string(),
            ),
            ApiError::NotFound(_) => (
                StatusCode::NOT_FOUND,
                "not_found",
                self.to_string(),
            ),
            ApiError::DatabaseError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
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
        let ca_error = edf_ca::CaError::InvalidRequest("Invalid cert request".to_string());
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
