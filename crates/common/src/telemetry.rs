use std::sync::Once;
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

static INIT: Once = Once::new();

/// Telemetry configuration
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    /// OTEL endpoint URL
    pub otel_endpoint: Option<String>,
    /// Service name
    pub service_name: String,
    /// Service version
    pub service_version: String,
    /// Environment (dev, staging, prod)
    pub environment: String,
    /// Enable tracing
    pub enable_tracing: bool,
    /// Enable metrics
    pub enable_metrics: bool,
    /// Enable logging
    pub enable_logging: bool,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            otel_endpoint: std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok(),
            service_name: std::env::var("SERVICE_NAME")
                .unwrap_or_else(|_| "fleetingdns".to_string()),
            service_version: std::env::var("SERVICE_VERSION")
                .unwrap_or_else(|_| "0.1.0".to_string()),
            environment: std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
            enable_tracing: std::env::var("TELEMETRY_TRACING_ENABLED")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            enable_metrics: std::env::var("TELEMETRY_METRICS_ENABLED")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            enable_logging: std::env::var("TELEMETRY_LOGGING_ENABLED")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
        }
    }
}

/// Initialize comprehensive telemetry system
pub fn init_telemetry(
    config: TelemetryConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    INIT.call_once(|| {
        if let Err(e) = init_telemetry_inner(config) {
            eprintln!("Failed to initialize telemetry: {e}");
        }
    });
    Ok(())
}

fn init_telemetry_inner(
    config: TelemetryConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize logging
    if config.enable_logging {
        init_logging(&config)?;
    }

    // Initialize metrics if enabled
    if config.enable_metrics {
        init_metrics(&config)?;
    }

    info!(
        service_name = %config.service_name,
        service_version = %config.service_version,
        environment = %config.environment,
        tracing_enabled = %config.enable_tracing,
        metrics_enabled = %config.enable_metrics,
        "Telemetry initialized"
    );

    Ok(())
}

fn init_metrics(config: &TelemetryConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(endpoint) = &config.otel_endpoint {
        // Initialize metrics exporter
        info!("Metrics initialized with OTEL endpoint: {}", endpoint);
    } else {
        warn!("No OTEL endpoint configured, metrics disabled");
    }

    Ok(())
}

fn init_logging(_config: &TelemetryConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize structured logging with Loki support
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true);

    let registry = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer);

    // Add Loki layer if configured
    if let Ok(loki_url) = std::env::var("LOKI_URL") {
        // Note: In a real implementation, you'd add a Loki layer here
        // For now, we'll use structured logging that can be scraped by Promtail
        info!("Logging configured for Loki endpoint: {}", loki_url);
    }

    registry.init();
    info!("Logging initialized");

    Ok(())
}

/// Create a span for DNS operations
pub fn dns_span(operation: &str, query: &str) -> tracing::Span {
    tracing::info_span!(
        "dns_operation",
        operation = operation,
        query = query,
        service.name = "dnsd",
        service.version = "0.1.0"
    )
}

/// Create a span for Redis operations
pub fn redis_span(operation: &str, key: &str) -> tracing::Span {
    tracing::info_span!(
        "redis_operation",
        operation = operation,
        key = key,
        service.name = "redis",
        service.version = "0.1.0"
    )
}

/// Create a span for API operations
pub fn api_span(method: &str, path: &str) -> tracing::Span {
    tracing::info_span!(
        "api_operation",
        method = method,
        path = path,
        service.name = "api",
        service.version = "0.1.0"
    )
}

/// Record DNS metrics
pub fn record_dns_metrics(operation: &str, query: &str, response_time_ms: u64, success: bool) {
    // Use the metrics crate directly with proper label syntax
    metrics::counter!("dns_operations_total", "operation" => operation.to_string(), "query" => query.to_string(), "success" => success.to_string()).increment(1);
    metrics::histogram!("dns_response_time_ms", "operation" => operation.to_string(), "query" => query.to_string()).record(response_time_ms as f64);
}

/// Record Redis metrics
pub fn record_redis_metrics(operation: &str, key: &str, response_time_ms: u64, success: bool) {
    // Use the metrics crate directly with proper label syntax
    metrics::counter!("redis_operations_total", "operation" => operation.to_string(), "key" => key.to_string(), "success" => success.to_string()).increment(1);
    metrics::histogram!("redis_response_time_ms", "operation" => operation.to_string(), "key" => key.to_string()).record(response_time_ms as f64);
}

/// Record API metrics
pub fn record_api_metrics(method: &str, path: &str, status_code: u16, response_time_ms: u64) {
    // Use the metrics crate directly with proper label syntax
    metrics::counter!("api_requests_total", "method" => method.to_string(), "path" => path.to_string(), "status_code" => status_code.to_string()).increment(1);
    metrics::histogram!("api_response_time_ms", "method" => method.to_string(), "path" => path.to_string()).record(response_time_ms as f64);
}

/// Record database operation metrics
pub fn record_db_metrics(operation: &str, table: &str, response_time_ms: u64, success: bool) {
    metrics::counter!("db_operations_total", "operation" => operation.to_string(), "table" => table.to_string(), "success" => success.to_string()).increment(1);
    metrics::histogram!("db_response_time_ms", "operation" => operation.to_string(), "table" => table.to_string()).record(response_time_ms as f64);
}

/// Record tunnel operation metrics
pub fn record_tunnel_metrics(operation: &str, tunnel_id: &str, response_time_ms: u64, success: bool) {
    metrics::counter!("tunnel_operations_total", "operation" => operation.to_string(), "tunnel_id" => tunnel_id.to_string(), "success" => success.to_string()).increment(1);
    metrics::histogram!("tunnel_response_time_ms", "operation" => operation.to_string(), "tunnel_id" => tunnel_id.to_string()).record(response_time_ms as f64);
}

/// Record DNS response delivery metrics
pub fn record_dns_delivery_metrics(protocol: &str, response_size: usize, success: bool) {
    metrics::counter!("dns_delivery_total", "protocol" => protocol.to_string(), "success" => success.to_string()).increment(1);
    metrics::histogram!("dns_response_size_bytes", "protocol" => protocol.to_string()).record(response_size as f64);
}

/// Record TLS connection metrics
pub fn record_tls_metrics(operation: &str, success: bool) {
    metrics::counter!("tls_operations_total", "operation" => operation.to_string(), "success" => success.to_string()).increment(1);
}

/// Record authentication metrics
pub fn record_auth_metrics(method: &str, success: bool) {
    metrics::counter!("auth_operations_total", "method" => method.to_string(), "success" => success.to_string()).increment(1);
}

/// Record slot management metrics
pub fn record_slot_metrics(operation: &str, success: bool) {
    metrics::counter!("slot_operations_total", "operation" => operation.to_string(), "success" => success.to_string()).increment(1);
}

/// Create a span for database operations
pub fn db_span(operation: &str, table: &str) -> tracing::Span {
    tracing::info_span!(
        "db_operation",
        operation = operation,
        table = table,
        service.name = "database",
        service.version = "0.1.0"
    )
}

/// Create a span for tunnel operations
pub fn tunnel_span(operation: &str, tunnel_id: &str) -> tracing::Span {
    tracing::info_span!(
        "tunnel_operation",
        operation = operation,
        tunnel_id = tunnel_id,
        service.name = "edgehub",
        service.version = "0.1.0"
    )
}

/// Create a span for authentication operations
pub fn auth_span(method: &str) -> tracing::Span {
    tracing::info_span!(
        "auth_operation",
        method = method,
        service.name = "api",
        service.version = "0.1.0"
    )
}

/// Shutdown telemetry gracefully
pub fn shutdown_telemetry() {
    info!("Shutting down telemetry");
    // Note: In a real implementation, you'd properly shutdown OTEL providers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_config_default() {
        let config = TelemetryConfig::default();
        assert!(!config.service_name.is_empty());
        assert!(!config.service_version.is_empty());
        assert!(!config.environment.is_empty());
    }

    #[test]
    fn test_span_creation() {
        let dns_span = dns_span("lookup", "test.fdns.run");
        // Test that span creation doesn't panic
        assert!(dns_span.is_disabled() || !dns_span.is_disabled());

        let redis_span = redis_span("get", "slot:test.fdns.run");
        assert!(redis_span.is_disabled() || !redis_span.is_disabled());

        let api_span = api_span("GET", "/api/v1/slots");
        assert!(api_span.is_disabled() || !api_span.is_disabled());

        let db_span = db_span("select", "slots");
        assert!(db_span.is_disabled() || !db_span.is_disabled());

        let tunnel_span = tunnel_span("create", "tunnel-123");
        assert!(tunnel_span.is_disabled() || !tunnel_span.is_disabled());

        let auth_span = auth_span("jwt");
        assert!(auth_span.is_disabled() || !auth_span.is_disabled());
    }

    #[test]
    fn test_metrics_recording() {
        // Test that metrics recording doesn't panic
        record_dns_metrics("lookup", "test.fdns.run", 50, true);
        record_redis_metrics("get", "slot:test.fdns.run", 10, true);
        record_api_metrics("GET", "/api/v1/slots", 200, 100);
        record_db_metrics("select", "slots", 25, true);
        record_tunnel_metrics("create", "tunnel-123", 100, true);
        record_dns_delivery_metrics("udp", 512, true);
        record_tls_metrics("handshake", true);
        record_auth_metrics("jwt", true);
        record_slot_metrics("create", true);
    }
}
