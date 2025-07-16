//! Common utilities shared across `FleetingDNS` crates.
//!
//! Provides application-wide tracing initialization, a basic error type,
//! and re-exports of helpful metrics macros.


use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, fmt};
use thiserror::Error;

pub mod metrics;
pub mod shutdown;
pub mod cert_manager;
pub mod ddos_protection;
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
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

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
}
