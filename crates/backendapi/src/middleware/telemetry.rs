use axum::{
    extract::Request,
    http::Method,
    middleware::Next,
    response::Response,
};
use std::time::Instant;
use tracing::info;

use crate::ApiState;

/// Middleware for capturing API telemetry metrics
pub async fn telemetry_middleware(
    _state: axum::extract::State<ApiState>,
    method: Method,
    uri: axum::http::Uri,
    request: Request,
    next: Next,
) -> Response {
    let start_time = Instant::now();
    
    // Create API span for tracing
    let span = common::telemetry::api_span(method.as_str(), uri.path());
    let _enter = span.enter();
    
    // Process the request
    let response = next.run(request).await;
    
    // Calculate response time
    let response_time = start_time.elapsed();
    let response_time_ms = response_time.as_millis() as u64;
    let status_code = response.status().as_u16();
    
    // Record API metrics
    common::telemetry::record_api_metrics(
        method.as_str(),
        uri.path(),
        status_code,
        response_time_ms,
    );
    
    // Log request details
    info!(
        method = %method,
        path = %uri.path(),
        status_code = %status_code,
        response_time_ms = %response_time_ms,
        "API request processed"
    );
    
    response
} 