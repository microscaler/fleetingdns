//! Error types for the FleetingDNS Certificate Authority

use thiserror::Error;

/// Certificate Authority errors
#[derive(Error, Debug)]
pub enum CaError {
    /// Certificate generation failed
    #[error("Certificate generation failed: {0}")]
    CertificateGeneration(String),

    /// Invalid certificate request
    #[error("Invalid certificate request: {0}")]
    InvalidRequest(String),

    /// Certificate validation failed
    #[error("Certificate validation failed: {0}")]
    ValidationFailed(String),

    /// Certificate has expired
    #[error("Certificate has expired: serial {serial}, expired at {expired_at}")]
    CertificateExpired {
        serial: String,
        expired_at: chrono::DateTime<chrono::Utc>,
    },

    /// Certificate not found
    #[error("Certificate not found: serial {serial}")]
    CertificateNotFound { serial: String },

    /// CA initialization failed
    #[error("CA initialization failed: {0}")]
    InitializationFailed(String),

    /// Key management error
    #[error("Key management error: {0}")]
    KeyManagement(String),

    /// File I/O error
    #[error("File I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// PEM parsing error
    #[error("PEM parsing error: {0}")]
    PemParsing(String),

    /// Certificate parsing error
    #[error("Certificate parsing error: {0}")]
    CertificateParsing(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// TTL validation error
    #[error("TTL validation error: requested {requested_minutes} minutes, max allowed {max_minutes} minutes")]
    TtlValidation {
        requested_minutes: i64,
        max_minutes: i64,
    },

    /// Client authorization error
    #[error("Client authorization error: client {client_id} not authorized for operation")]
    ClientUnauthorized { client_id: String },

    /// Rate limiting error
    #[error("Rate limit exceeded for client {client_id}: {limit} certificates per {window}")]
    RateLimitExceeded {
        client_id: String,
        limit: u32,
        window: String,
    },

    /// Generic internal error
    #[error("Internal CA error: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for CaError {
    fn from(err: anyhow::Error) -> Self {
        CaError::Internal(err.to_string())
    }
}

impl From<rcgen::Error> for CaError {
    fn from(err: rcgen::Error) -> Self {
        CaError::CertificateGeneration(err.to_string())
    }
}

/// Result type for CA operations
pub type CaResult<T> = Result<T, CaError>; 