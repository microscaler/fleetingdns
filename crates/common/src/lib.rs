//! Common utilities shared across `FleetingDNS` crates.
//!
//! Provides application-wide tracing initialization, a basic error type,
//! and re-exports of helpful metrics macros.

use tracing_subscriber::{EnvFilter, fmt};

pub use metrics::{counter, gauge, histogram};

pub mod shutdown;
pub mod tls;

use thiserror::Error;

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

/// Initialize global tracing subscriber.
///
/// This sets up `tracing_subscriber` using an environment filter and pretty
/// output format. After initialization an "app start" message is logged so
/// callers can confirm tracing is active.
pub fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let _ = fmt().with_env_filter(filter).pretty().try_init();

    tracing::info!("app start");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_test::traced_test;

    #[traced_test]
    #[test]
    fn init_tracing_prints_start() {
        init_tracing();
        assert!(logs_contain("app start"));
    }
}
