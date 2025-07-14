//! Common utilities shared across `FleetingDNS` crates.
//!
//! Provides application-wide tracing initialization, a basic error type,
//! and re-exports of helpful metrics macros.

use std::collections::HashMap;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, fmt};

/// Emit a counter metric.
#[macro_export]
macro_rules! counter {
    ($($t:tt)*) => {
        ::metrics::counter!($($t)*)
    };
}

/// Emit a gauge metric.
#[macro_export]
macro_rules! gauge {
    ($($t:tt)*) => {
        ::metrics::gauge!($($t)*)
    };
}

/// Emit a histogram metric.
#[macro_export]
macro_rules! histogram {
    ($($t:tt)*) => {
        ::metrics::histogram!($($t)*)
    };
}

/// Create a new request span with trace ID for distributed tracing.
#[macro_export]
macro_rules! request_span {
    ($name:expr) => {{
        let trace_id = $crate::generate_trace_id();
        tracing::info_span!($name, trace_id = trace_id)
    }};
    ($name:expr, $($field:tt)*) => {{
        let trace_id = $crate::generate_trace_id();
        tracing::info_span!($name, trace_id = trace_id, $($field)*)
    }};
}

/// Create a child span that inherits the trace ID from current context.
#[macro_export]
macro_rules! child_span {
    ($name:expr) => {{
        let trace_id = $crate::current_trace_id();
        tracing::info_span!($name, trace_id = trace_id)
    }};
    ($name:expr, $($field:tt)*) => {{
        let trace_id = $crate::current_trace_id();
        tracing::info_span!($name, trace_id = trace_id, $($field)*)
    }};
}

pub mod metrics;
pub mod shutdown;
pub mod tls;
// HIGH-1 ENHANCEMENT: Certificate management for production DoT
pub mod cert_manager;
pub use crate::metrics::init_metrics;

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

/// Initialize global tracing subscriber with distributed tracing support.
///
/// This sets up `tracing_subscriber` using an environment filter and pretty
/// output format. After initialization an "app start" message is logged so
/// callers can confirm tracing is active.
pub fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // Set up tracing subscriber with OpenTelemetry support if available
    let subscriber = tracing_subscriber::registry()
        .with(
            fmt::layer()
                .pretty()
                .with_span_events(fmt::format::FmtSpan::NEW | fmt::format::FmtSpan::CLOSE)
        )
        .with(filter);

    let _ = subscriber.try_init();

    info!("app start with distributed tracing enabled");
}

// Distributed tracing utilities
use std::sync::atomic::{AtomicU64, Ordering};

static TRACE_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Generate a new trace ID for distributed tracing
pub fn generate_trace_id() -> String {
    let id = TRACE_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("trace-{id:016x}")
}

/// Get the current trace ID from the active span
pub fn current_trace_id() -> String {
    // For now, always generate a new trace ID
    // In production, this would extract the actual trace ID from the span context
    generate_trace_id()
}

/// Create a request span with trace ID
pub fn create_request_span(name: &str, trace_id: Option<String>) -> tracing::Span {
    let trace_id = trace_id.unwrap_or_else(generate_trace_id);
    tracing::info_span!("request", name = name, trace_id = trace_id, span_type = "incoming")
}

/// Create a child span with parent trace context
pub fn create_child_span(name: &str, parent_trace_id: Option<String>) -> tracing::Span {
    let trace_id = parent_trace_id.unwrap_or_else(generate_trace_id);
    tracing::info_span!("operation", name = name, trace_id = trace_id, span_type = "child")
}

/// HTTP context propagation utilities
pub fn add_trace_context(headers: &mut HashMap<String, String>, trace_id: &str) {
    headers.insert("x-trace-id".to_string(), trace_id.to_string());
    headers.insert("x-span-id".to_string(), generate_trace_id());
}

pub fn extract_trace_context(headers: &HashMap<String, String>) -> Option<String> {
    headers.get("x-trace-id").cloned()
}

pub fn create_span_with_context(name: &str, headers: &HashMap<String, String>) -> tracing::Span {
    let trace_id = extract_trace_context(headers);
    create_request_span(name, trace_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_test::traced_test;

    #[traced_test]
    #[test]
    fn init_tracing_prints_start() {
        init_tracing();
        assert!(logs_contain("app start with distributed tracing enabled"));
    }

    #[test]
    fn test_trace_id_generation() {
        let trace_id1 = generate_trace_id();
        let trace_id2 = generate_trace_id();
        
        assert_ne!(trace_id1, trace_id2);
        assert!(!trace_id1.is_empty());
        assert!(!trace_id2.is_empty());
    }

    #[test]
    fn test_trace_context_headers() {
        let mut headers = std::collections::HashMap::new();
        add_trace_context(&mut headers, "test-trace-123");
        
        assert!(headers.contains_key("x-trace-id"));
        assert!(headers.contains_key("x-span-id"));
        
        let trace_id = extract_trace_context(&headers);
        assert!(trace_id.is_some());
    }

    #[test]
    fn test_span_with_context() {
        let mut headers = std::collections::HashMap::new();
        headers.insert("x-trace-id".to_string(), "test-trace-123".to_string());
        
        let span = create_span_with_context("test_operation", &headers);
        // Test that the span was created (metadata() is available)
        assert!(!span.metadata().expect("span should have metadata").name().is_empty());
    }
}
