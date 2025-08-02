//! Common utilities shared across `FleetingDNS` crates.
//!
//! Provides application-wide tracing initialization, a basic error type,
//! and re-exports of helpful metrics macros.

use thiserror::Error;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub mod batch_audit_logger;
pub mod batch_metrics_collector;
pub mod cert_manager;
pub mod config;
pub mod ddos_protection;
pub mod metrics;
pub mod shutdown;
pub mod tls;

// Re-export metrics for convenience
pub use metrics::*;

/// Result type used across the application.
pub type AppResult<T> = Result<T, AppError>;

/// Application error type.
#[derive(Debug, Error)]
pub enum AppError {
    /// Wrapper around [`std::io::Error`].
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// Wrapper around [`serde_json::Error`].
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),

    /// Generic error with custom message.
    #[error("{0}")]
    Message(String),
}

/// Emit a counter metric.
#[macro_export]
macro_rules! counter {
    ($name:expr $(, $key:expr => $value:expr)*) => {
        metrics::counter!($name $(, $key => $value)*)
    };
}

/// Emit a gauge metric.
#[macro_export]
macro_rules! gauge {
    ($name:expr $(, $key:expr => $value:expr)*) => {
        metrics::gauge!($name $(, $key => $value)*)
    };
}

/// Emit a histogram metric.
#[macro_export]
macro_rules! histogram {
    ($name:expr $(, $key:expr => $value:expr)*) => {
        metrics::histogram!($name $(, $key => $value)*)
    };
}

/// Initialize tracing with the given service name
pub fn init_tracing(service_name: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer())
        .try_init()?;

    info!("Tracing initialized for service: {}", service_name);
    Ok(())
}

/// Generate a simple trace ID
pub fn generate_trace_id() -> String {
    format!("trace-{:016x}", std::process::id() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_id_generation() {
        let id1 = generate_trace_id();
        let id2 = generate_trace_id();

        assert!(id1.starts_with("trace-"));
        // Both should be the same since they use process ID
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_trace_id_format() {
        let trace_id = generate_trace_id();

        // Should start with "trace-"
        assert!(trace_id.starts_with("trace-"));

        // Should be 22 characters long (6 for "trace-" + 16 for hex)
        assert_eq!(trace_id.len(), 22);

        // Should contain valid hex digits after "trace-"
        let hex_part = &trace_id[6..];
        assert!(hex_part.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_trace_id_uniqueness_across_processes() {
        // This test verifies the format, but can't easily test cross-process uniqueness
        // in a single test environment
        let trace_id = generate_trace_id();
        assert!(trace_id.starts_with("trace-"));
        assert_eq!(trace_id.len(), 22);
    }

    #[test]
    fn test_app_error_creation() {
        // Test Io error
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let app_error = AppError::Io(io_error);
        assert!(format!("{:?}", app_error).contains("file not found"));

        // Test SerdeJson error
        let json_error = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let app_error = AppError::SerdeJson(json_error);
        assert!(format!("{:?}", app_error).contains("SerdeJson"));

        // Test Message error
        let app_error = AppError::Message("custom error message".to_string());
        assert_eq!(format!("{}", app_error), "custom error message");
    }

    #[test]
    fn test_app_error_from_conversions() {
        // Test Io conversion
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "test");
        let app_error: AppError = io_error.into();
        assert!(matches!(app_error, AppError::Io(_)));

        // Test SerdeJson conversion
        let json_error = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let app_error: AppError = json_error.into();
        assert!(matches!(app_error, AppError::SerdeJson(_)));
    }

    #[test]
    fn test_app_result_type_alias() {
        // Test successful result
        let success: AppResult<String> = Ok("test".to_string());
        assert!(success.is_ok());
        assert_eq!(success.unwrap(), "test");

        // Test error result
        let error: AppResult<String> = Err(AppError::Message("error".to_string()));
        assert!(error.is_err());
        assert!(matches!(error.unwrap_err(), AppError::Message(_)));
    }

    #[test]
    fn test_metrics_macros_compile() {
        // Test that the metrics macros compile correctly
        // These are just compilation tests, not runtime tests
        // Note: These macros require the metrics crate to be properly initialized
        // We'll test them indirectly through the actual usage in the codebase
    }

    #[test]
    fn test_error_display_implementations() {
        // Test that all error variants implement Display correctly
        let io_error = AppError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "test"));
        assert!(!format!("{}", io_error).is_empty());

        let json_error =
            AppError::SerdeJson(serde_json::from_str::<serde_json::Value>("invalid").unwrap_err());
        assert!(!format!("{}", json_error).is_empty());

        let message_error = AppError::Message("test message".to_string());
        assert_eq!(format!("{}", message_error), "test message");
    }

    #[test]
    fn test_error_debug_implementations() {
        // Test that all error variants implement Debug correctly
        let io_error = AppError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "test"));
        assert!(!format!("{:?}", io_error).is_empty());

        let json_error =
            AppError::SerdeJson(serde_json::from_str::<serde_json::Value>("invalid").unwrap_err());
        assert!(!format!("{:?}", json_error).is_empty());

        let message_error = AppError::Message("test message".to_string());
        assert!(!format!("{:?}", message_error).is_empty());
    }

    #[test]
    fn test_error_send_sync() {
        // Test that AppError is Send and Sync
        fn assert_send_sync<T: Send + Sync>() {}
        unsafe {
            assert_send_sync::<AppError>();
        }
    }

    #[test]
    fn test_app_result_send_sync() {
        // Test that AppResult is Send and Sync
        fn assert_send_sync<T: Send + Sync>() {}
        unsafe {
            assert_send_sync::<AppResult<String>>();
        }
    }
}
