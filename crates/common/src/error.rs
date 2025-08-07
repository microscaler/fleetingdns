use thiserror::Error;

/// Unified error type for the entire FleetingDNS system
#[derive(Error, Debug, Clone)]
pub enum FleetingDnsError {
    // Authentication & Authorization Errors
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Authorization failed: {0}")]
    AuthorizationFailed(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Token expired: {0}")]
    TokenExpired(String),

    #[error("Invalid token: {0}")]
    InvalidToken(String),

    // Resource & Data Errors
    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Resource already exists: {0}")]
    AlreadyExists(String),

    #[error("Resource conflict: {0}")]
    Conflict(String),

    #[error("Invalid request: {0}")]
    BadRequest(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    // Storage & Database Errors
    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Redis error: {0}")]
    RedisError(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Timeout error: {0}")]
    TimeoutError(String),

    // External Service Errors
    #[error("External service error: {0}")]
    ExternalService(String),

    #[error("GitHub API error: {0}")]
    GitHubApiError(String),

    #[error("Certificate authority error: {0}")]
    CertificateError(String),

    #[error("DNS error: {0}")]
    DnsError(String),

    #[error("SSH error: {0}")]
    SshError(String),

    #[error("TLS error: {0}")]
    TlsError(String),

    // Rate Limiting & Quota Errors
    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    #[error("Quota exceeded: {0}")]
    QuotaExceeded(String),

    #[error("Too many requests: {0}")]
    TooManyRequests(String),

    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),

    // Configuration & System Errors
    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Internal server error: {0}")]
    InternalError(String),

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("Circuit breaker open: {0}")]
    CircuitBreakerOpen(String),

    // Network & Communication Errors
    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Connection refused: {0}")]
    ConnectionRefused(String),

    #[error("Request timeout: {0}")]
    RequestTimeout(String),

    #[error("Payload too large: {0}")]
    PayloadTooLarge(String),

    #[error("Unsupported media type: {0}")]
    UnsupportedMediaType(String),

    // Serialization & Data Errors
    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    #[error("Encoding error: {0}")]
    EncodingError(String),

    #[error("Decoding error: {0}")]
    DecodingError(String),

    // Generic wrapper errors
    #[error("IO error: {0}")]
    Io(String),

    #[error("JSON error: {0}")]
    Json(String),

    #[error("Generic error: {0}")]
    Generic(String),
}

/// Error category for client-side handling
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

/// Enhanced error response format with detailed information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ErrorResponse {
    /// Machine-readable error code
    pub error: String,
    /// Human-readable error message
    pub message: String,
    /// HTTP status code
    pub code: u16,
    /// Unique error ID for tracking
    pub error_id: String,
    /// Timestamp when error occurred
    pub timestamp: String,
    /// Request ID for correlation (if available)
    pub request_id: Option<String>,
    /// Additional error details
    pub details: Option<serde_json::Value>,
    /// Retry information
    pub retry_after: Option<u64>,
    /// Error category for client handling
    pub category: ErrorCategory,
}

/// Error context for enhanced error tracking
#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub request_id: Option<String>,
    pub user_id: Option<String>,
    pub endpoint: Option<String>,
    pub method: Option<String>,
    pub client_ip: Option<String>,
    pub user_agent: Option<String>,
    pub service_name: Option<String>,
    pub operation: Option<String>,
}

impl FleetingDnsError {
    /// Get the error category for this error
    pub fn category(&self) -> ErrorCategory {
        match self {
            // Authentication & Authorization
            FleetingDnsError::AuthenticationFailed(_) |
            FleetingDnsError::TokenExpired(_) |
            FleetingDnsError::InvalidToken(_) => ErrorCategory::Authentication,

            FleetingDnsError::AuthorizationFailed(_) |
            FleetingDnsError::Unauthorized(_) |
            FleetingDnsError::Forbidden(_) => ErrorCategory::Authorization,

            // Rate Limiting
            FleetingDnsError::RateLimitExceeded(_) |
            FleetingDnsError::QuotaExceeded(_) |
            FleetingDnsError::TooManyRequests(_) => ErrorCategory::RateLimit,

            // Resource Issues
            FleetingDnsError::ResourceExhausted(_) |
            FleetingDnsError::PayloadTooLarge(_) => ErrorCategory::Resource,

            // Client Errors
            FleetingDnsError::BadRequest(_) |
            FleetingDnsError::ValidationError(_) |
            FleetingDnsError::UnsupportedMediaType(_) => ErrorCategory::Client,

            // Network Issues
            FleetingDnsError::NetworkError(_) |
            FleetingDnsError::ConnectionRefused(_) |
            FleetingDnsError::RequestTimeout(_) |
            FleetingDnsError::ConnectionError(_) => ErrorCategory::Network,

            // Service Issues
            FleetingDnsError::InternalError(_) |
            FleetingDnsError::ServiceUnavailable(_) |
            FleetingDnsError::CircuitBreakerOpen(_) |
            FleetingDnsError::ExternalService(_) |
            FleetingDnsError::GitHubApiError(_) |
            FleetingDnsError::CertificateError(_) |
            FleetingDnsError::DnsError(_) |
            FleetingDnsError::SshError(_) |
            FleetingDnsError::TlsError(_) |
            FleetingDnsError::StorageError(_) |
            FleetingDnsError::DatabaseError(_) |
            FleetingDnsError::RedisError(_) |
            FleetingDnsError::TimeoutError(_) |
            FleetingDnsError::ConfigurationError(_) |
            FleetingDnsError::SerializationError(_) |
            FleetingDnsError::DeserializationError(_) |
            FleetingDnsError::EncodingError(_) |
            FleetingDnsError::DecodingError(_) |
            FleetingDnsError::Io(_) |
            FleetingDnsError::Json(_) |
            FleetingDnsError::Generic(_) => ErrorCategory::Service,

            // Default to Client for unknown errors
            _ => ErrorCategory::Client,
        }
    }

    /// Get the HTTP status code for this error
    pub fn status_code(&self) -> u16 {
        match self {
            // 4xx Client Errors
            FleetingDnsError::BadRequest(_) => 400,
            FleetingDnsError::Unauthorized(_) => 401,
            FleetingDnsError::AuthenticationFailed(_) => 401,
            FleetingDnsError::TokenExpired(_) => 401,
            FleetingDnsError::InvalidToken(_) => 401,
            FleetingDnsError::Forbidden(_) => 403,
            FleetingDnsError::AuthorizationFailed(_) => 403,
            FleetingDnsError::NotFound(_) => 404,
            FleetingDnsError::Conflict(_) => 409,
            FleetingDnsError::ValidationError(_) => 422,
            FleetingDnsError::PayloadTooLarge(_) => 413,
            FleetingDnsError::UnsupportedMediaType(_) => 415,
            FleetingDnsError::TooManyRequests(_) => 429,
            FleetingDnsError::RateLimitExceeded(_) => 429,
            FleetingDnsError::QuotaExceeded(_) => 429,

            // 5xx Server Errors
            FleetingDnsError::InternalError(_) => 500,
            FleetingDnsError::ServiceUnavailable(_) => 503,
            FleetingDnsError::CircuitBreakerOpen(_) => 503,
            FleetingDnsError::ExternalService(_) => 502,
            FleetingDnsError::GitHubApiError(_) => 502,
            FleetingDnsError::CertificateError(_) => 500,
            FleetingDnsError::DnsError(_) => 500,
            FleetingDnsError::SshError(_) => 500,
            FleetingDnsError::TlsError(_) => 500,
            FleetingDnsError::StorageError(_) => 500,
            FleetingDnsError::DatabaseError(_) => 500,
            FleetingDnsError::RedisError(_) => 500,
            FleetingDnsError::ConfigurationError(_) => 500,
            FleetingDnsError::SerializationError(_) => 500,
            FleetingDnsError::DeserializationError(_) => 500,
            FleetingDnsError::EncodingError(_) => 500,
            FleetingDnsError::DecodingError(_) => 500,
            FleetingDnsError::Io(_) => 500,
            FleetingDnsError::Json(_) => 500,
            FleetingDnsError::Generic(_) => 500,

            // Network/Connection Errors
            FleetingDnsError::NetworkError(_) => 502,
            FleetingDnsError::ConnectionRefused(_) => 502,
            FleetingDnsError::RequestTimeout(_) => 504,
            FleetingDnsError::ConnectionError(_) => 502,
            FleetingDnsError::TimeoutError(_) => 504,

            // Resource Errors
            FleetingDnsError::ResourceExhausted(_) => 503,
            FleetingDnsError::AlreadyExists(_) => 409,
        }
    }

    /// Get retry information for this error
    pub fn retry_after(&self) -> Option<u64> {
        match self {
            FleetingDnsError::RateLimitExceeded(_) => Some(60), // 1 minute
            FleetingDnsError::TooManyRequests(_) => Some(60), // 1 minute
            FleetingDnsError::QuotaExceeded(_) => Some(3600), // 1 hour
            FleetingDnsError::ServiceUnavailable(_) => Some(30), // 30 seconds
            FleetingDnsError::CircuitBreakerOpen(_) => Some(300), // 5 minutes
            FleetingDnsError::RequestTimeout(_) => Some(5), // 5 seconds
            FleetingDnsError::TimeoutError(_) => Some(5), // 5 seconds
            _ => None,
        }
    }

    /// Convert to ErrorResponse with context
    pub fn into_error_response(self, context: &ErrorContext) -> ErrorResponse {
        let error_id = uuid::Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().to_rfc3339();

        ErrorResponse {
            error: self.error_code(),
            message: self.to_string(),
            code: self.status_code(),
            error_id,
            timestamp,
            request_id: context.request_id.clone(),
            details: self.error_details(context),
            retry_after: self.retry_after(),
            category: self.category(),
        }
    }

    /// Get machine-readable error code
    fn error_code(&self) -> String {
        match self {
            FleetingDnsError::AuthenticationFailed(_) => "AUTH_FAILED".to_string(),
            FleetingDnsError::AuthorizationFailed(_) => "AUTHZ_FAILED".to_string(),
            FleetingDnsError::Unauthorized(_) => "UNAUTHORIZED".to_string(),
            FleetingDnsError::Forbidden(_) => "FORBIDDEN".to_string(),
            FleetingDnsError::TokenExpired(_) => "TOKEN_EXPIRED".to_string(),
            FleetingDnsError::InvalidToken(_) => "INVALID_TOKEN".to_string(),
            FleetingDnsError::NotFound(_) => "NOT_FOUND".to_string(),
            FleetingDnsError::AlreadyExists(_) => "ALREADY_EXISTS".to_string(),
            FleetingDnsError::Conflict(_) => "CONFLICT".to_string(),
            FleetingDnsError::BadRequest(_) => "BAD_REQUEST".to_string(),
            FleetingDnsError::ValidationError(_) => "VALIDATION_ERROR".to_string(),
            FleetingDnsError::StorageError(_) => "STORAGE_ERROR".to_string(),
            FleetingDnsError::DatabaseError(_) => "DATABASE_ERROR".to_string(),
            FleetingDnsError::RedisError(_) => "REDIS_ERROR".to_string(),
            FleetingDnsError::ConnectionError(_) => "CONNECTION_ERROR".to_string(),
            FleetingDnsError::TimeoutError(_) => "TIMEOUT_ERROR".to_string(),
            FleetingDnsError::ExternalService(_) => "EXTERNAL_SERVICE_ERROR".to_string(),
            FleetingDnsError::GitHubApiError(_) => "GITHUB_API_ERROR".to_string(),
            FleetingDnsError::CertificateError(_) => "CERTIFICATE_ERROR".to_string(),
            FleetingDnsError::DnsError(_) => "DNS_ERROR".to_string(),
            FleetingDnsError::SshError(_) => "SSH_ERROR".to_string(),
            FleetingDnsError::TlsError(_) => "TLS_ERROR".to_string(),
            FleetingDnsError::RateLimitExceeded(_) => "RATE_LIMIT_EXCEEDED".to_string(),
            FleetingDnsError::QuotaExceeded(_) => "QUOTA_EXCEEDED".to_string(),
            FleetingDnsError::TooManyRequests(_) => "TOO_MANY_REQUESTS".to_string(),
            FleetingDnsError::ResourceExhausted(_) => "RESOURCE_EXHAUSTED".to_string(),
            FleetingDnsError::ConfigurationError(_) => "CONFIGURATION_ERROR".to_string(),
            FleetingDnsError::InternalError(_) => "INTERNAL_ERROR".to_string(),
            FleetingDnsError::ServiceUnavailable(_) => "SERVICE_UNAVAILABLE".to_string(),
            FleetingDnsError::CircuitBreakerOpen(_) => "CIRCUIT_BREAKER_OPEN".to_string(),
            FleetingDnsError::NetworkError(_) => "NETWORK_ERROR".to_string(),
            FleetingDnsError::ConnectionRefused(_) => "CONNECTION_REFUSED".to_string(),
            FleetingDnsError::RequestTimeout(_) => "REQUEST_TIMEOUT".to_string(),
            FleetingDnsError::PayloadTooLarge(_) => "PAYLOAD_TOO_LARGE".to_string(),
            FleetingDnsError::UnsupportedMediaType(_) => "UNSUPPORTED_MEDIA_TYPE".to_string(),
            FleetingDnsError::SerializationError(_) => "SERIALIZATION_ERROR".to_string(),
            FleetingDnsError::DeserializationError(_) => "DESERIALIZATION_ERROR".to_string(),
            FleetingDnsError::EncodingError(_) => "ENCODING_ERROR".to_string(),
            FleetingDnsError::DecodingError(_) => "DECODING_ERROR".to_string(),
            FleetingDnsError::Io(_) => "IO_ERROR".to_string(),
            FleetingDnsError::Json(_) => "JSON_ERROR".to_string(),
            FleetingDnsError::Generic(_) => "GENERIC_ERROR".to_string(),
        }
    }

    /// Get additional error details for debugging
    fn error_details(&self, context: &ErrorContext) -> Option<serde_json::Value> {
        let mut details = serde_json::Map::new();

        // Add context information
        if let Some(user_id) = &context.user_id {
            details.insert("user_id".to_string(), serde_json::Value::String(user_id.clone()));
        }
        if let Some(endpoint) = &context.endpoint {
            details.insert("endpoint".to_string(), serde_json::Value::String(endpoint.clone()));
        }
        if let Some(method) = &context.method {
            details.insert("method".to_string(), serde_json::Value::String(method.clone()));
        }
        if let Some(service_name) = &context.service_name {
            details.insert("service".to_string(), serde_json::Value::String(service_name.clone()));
        }
        if let Some(operation) = &context.operation {
            details.insert("operation".to_string(), serde_json::Value::String(operation.clone()));
        }

        // Add error-specific details
        match self {
            FleetingDnsError::ValidationError(msg) => {
                details.insert("validation_message".to_string(), serde_json::Value::String(msg.clone()));
            }
            FleetingDnsError::RateLimitExceeded(msg) => {
                details.insert("rate_limit_message".to_string(), serde_json::Value::String(msg.clone()));
            }
            FleetingDnsError::QuotaExceeded(msg) => {
                details.insert("quota_message".to_string(), serde_json::Value::String(msg.clone()));
            }
            _ => {}
        }

        if details.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(details))
        }
    }

    /// Log the error with context
    pub fn log_error(&self, context: &ErrorContext) {
        let level = match self.category() {
            ErrorCategory::Client | ErrorCategory::Validation => tracing::Level::WARN,
            ErrorCategory::Authentication | ErrorCategory::Authorization => tracing::Level::INFO,
            ErrorCategory::RateLimit => tracing::Level::INFO,
            ErrorCategory::Resource | ErrorCategory::Service | ErrorCategory::Network => tracing::Level::ERROR,
        };

        let error_type = self.error_code();
        let category = self.category();
        let status_code = self.status_code();

        let span = tracing::span!(tracing::Level::ERROR, "error", 
            error_type = %error_type,
            category = ?category,
            status_code = status_code,
        );

        if let Some(request_id) = &context.request_id {
            span.record("request_id", &request_id);
        }
        if let Some(user_id) = &context.user_id {
            span.record("user_id", &user_id);
        }
        if let Some(endpoint) = &context.endpoint {
            span.record("endpoint", &endpoint);
        }

        let _enter = span.enter();
        match level {
            tracing::Level::ERROR => tracing::error!("Error occurred: {}", self),
            tracing::Level::WARN => tracing::warn!("Error occurred: {}", self),
            tracing::Level::INFO => tracing::info!("Error occurred: {}", self),
            _ => tracing::debug!("Error occurred: {}", self),
        }
    }
}

// Conversion implementations from other error types
impl From<std::io::Error> for FleetingDnsError {
    fn from(err: std::io::Error) -> Self {
        FleetingDnsError::Io(err.to_string())
    }
}

impl From<serde_json::Error> for FleetingDnsError {
    fn from(err: serde_json::Error) -> Self {
        FleetingDnsError::Json(err.to_string())
    }
}

impl From<redis::RedisError> for FleetingDnsError {
    fn from(err: redis::RedisError) -> Self {
        FleetingDnsError::RedisError(err.to_string())
    }
}

impl From<bb8_redis::redis::RedisError> for FleetingDnsError {
    fn from(err: bb8_redis::redis::RedisError) -> Self {
        FleetingDnsError::RedisError(err.to_string())
    }
}

impl From<reqwest::Error> for FleetingDnsError {
    fn from(err: reqwest::Error) -> Self {
        FleetingDnsError::ExternalService(err.to_string())
    }
}

impl From<anyhow::Error> for FleetingDnsError {
    fn from(err: anyhow::Error) -> Self {
        FleetingDnsError::Generic(err.to_string())
    }
}



// Result type alias for the common crate
pub type CommonResult<T> = Result<T, FleetingDnsError>;

// Error conversion traits for other crates
pub trait IntoFleetingDnsError {
    fn into_fleeting_dns_error(self) -> FleetingDnsError;
}

pub trait FromFleetingDnsError {
    fn from_fleeting_dns_error(err: FleetingDnsError) -> Self;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_categories() {
        assert_eq!(FleetingDnsError::BadRequest("test".to_string()).category(), ErrorCategory::Client);
        assert_eq!(FleetingDnsError::AuthenticationFailed("test".to_string()).category(), ErrorCategory::Authentication);
        assert_eq!(FleetingDnsError::RateLimitExceeded("test".to_string()).category(), ErrorCategory::RateLimit);
        assert_eq!(FleetingDnsError::InternalError("test".to_string()).category(), ErrorCategory::Service);
    }

    #[test]
    fn test_status_codes() {
        assert_eq!(FleetingDnsError::BadRequest("test".to_string()).status_code(), 400);
        assert_eq!(FleetingDnsError::Unauthorized("test".to_string()).status_code(), 401);
        assert_eq!(FleetingDnsError::NotFound("test".to_string()).status_code(), 404);
        assert_eq!(FleetingDnsError::InternalError("test".to_string()).status_code(), 500);
    }

    #[test]
    fn test_retry_after() {
        assert_eq!(FleetingDnsError::RateLimitExceeded("test".to_string()).retry_after(), Some(60));
        assert_eq!(FleetingDnsError::ServiceUnavailable("test".to_string()).retry_after(), Some(30));
        assert_eq!(FleetingDnsError::BadRequest("test".to_string()).retry_after(), None);
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(FleetingDnsError::BadRequest("test".to_string()).error_code(), "BAD_REQUEST");
        assert_eq!(FleetingDnsError::NotFound("test".to_string()).error_code(), "NOT_FOUND");
        assert_eq!(FleetingDnsError::InternalError("test".to_string()).error_code(), "INTERNAL_ERROR");
    }

    #[test]
    fn test_error_response_conversion() {
        let error = FleetingDnsError::BadRequest("Invalid input".to_string());
        let context = ErrorContext {
            request_id: Some("req-123".to_string()),
            user_id: Some("user-456".to_string()),
            endpoint: Some("/api/test".to_string()),
            method: Some("POST".to_string()),
            client_ip: None,
            user_agent: None,
            service_name: None,
            operation: None,
        };

        let response = error.into_error_response(&context);
        
        assert_eq!(response.error, "BAD_REQUEST");
        assert_eq!(response.code, 400);
        assert_eq!(response.category, ErrorCategory::Client);
        assert!(response.error_id.len() > 0);
        assert!(response.timestamp.len() > 0);
    }

    #[test]
    fn test_error_conversions() {
        // Test IO error conversion
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let fleeting_error: FleetingDnsError = io_error.into();
        assert!(matches!(fleeting_error, FleetingDnsError::Io(_)));

        // Test JSON error conversion
        let json_error = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let fleeting_error: FleetingDnsError = json_error.into();
        assert!(matches!(fleeting_error, FleetingDnsError::Json(_)));
    }
} 