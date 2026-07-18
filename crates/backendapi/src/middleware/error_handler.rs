//! Enhanced error handling middleware for FleetingDNS API
//!
//! This module provides comprehensive error handling with context extraction,
//! structured error responses, and enhanced error tracking for production monitoring.

use crate::{ApiError, error::ErrorContext};
use axum::{
    extract::Request,
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};
use std::time::Instant;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Enhanced error handling middleware
pub async fn error_handler_middleware(request: Request, next: Next) -> Result<Response, Response> {
    let start_time = Instant::now();
    let request_id = Uuid::new_v4().to_string();

    // Extract request context
    let context = extract_request_context(&request, &request_id);

    // Add request ID to headers for correlation
    let mut request = request;
    request.headers_mut().insert(
        "x-request-id",
        HeaderValue::from_str(&request_id).unwrap_or_else(|_| HeaderValue::from_static("unknown")),
    );

    // Process the request
    let response = next.run(request).await;

    // Log request completion
    let duration = start_time.elapsed();
    log_request_completion(&context, response.status(), duration);

    Ok(response)
}

/// Extract comprehensive request context for error tracking
fn extract_request_context(request: &Request, request_id: &str) -> ErrorContext {
    let headers = request.headers();
    let uri = request.uri();

    // Extract user ID from JWT token if present
    let user_id = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .and_then(|token| {
            // In a real implementation, you would decode the JWT here
            // For now, we'll just extract a user identifier from the token
            if token.len() > 10 {
                Some(token[..10].to_string())
            } else {
                None
            }
        });

    // Extract client IP
    let client_ip = headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|h| h.to_str().ok())
        .and_then(|ip_str| ip_str.split(',').next())
        .map(|ip| ip.trim().to_string());

    // Extract user agent
    let user_agent = headers
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .map(std::string::ToString::to_string);

    ErrorContext {
        request_id: Some(request_id.to_string()),
        user_id,
        endpoint: Some(uri.path().to_string()),
        method: Some(request.method().as_str().to_string()),
        client_ip,
        user_agent,
    }
}

/// Log request completion with performance metrics
fn log_request_completion(
    context: &ErrorContext,
    status: StatusCode,
    duration: std::time::Duration,
) {
    let log_message = format!(
        "Request completed | ID: {} | User: {:?} | Endpoint: {:?} | Method: {:?} | Status: {} | Duration: {:?}",
        context.request_id.as_deref().unwrap_or("unknown"),
        context.user_id,
        context.endpoint,
        context.method,
        status.as_u16(),
        duration
    );

    if status.is_success() {
        info!("{}", log_message);
    } else if status.is_client_error() {
        warn!("{}", log_message);
    } else {
        error!("{}", log_message);
    }
}

/// Enhanced error recovery middleware
pub async fn error_recovery_middleware(request: Request, next: Next) -> Result<Response, Response> {
    // Attempt to process the request
    match next.run(request).await {
        response if response.status().is_success() => Ok(response),
        response => {
            // Apply error recovery strategies based on status code
            let recovered_response = apply_error_recovery(response).await;
            Ok(recovered_response)
        }
    }
}

/// Apply error recovery strategies
async fn apply_error_recovery(response: Response) -> Response {
    let status = response.status();

    match status {
        StatusCode::SERVICE_UNAVAILABLE => {
            // For service unavailable, add retry headers
            let mut response = response;
            response
                .headers_mut()
                .insert("retry-after", HeaderValue::from_static("30"));
            response
        }
        StatusCode::TOO_MANY_REQUESTS => {
            // For rate limiting, add rate limit headers
            let mut response = response;
            response
                .headers_mut()
                .insert("retry-after", HeaderValue::from_static("60"));
            response
        }
        StatusCode::INTERNAL_SERVER_ERROR => {
            // For internal errors, add correlation headers
            let mut response = response;
            response.headers_mut().insert(
                "x-error-correlation",
                HeaderValue::from_static("internal-error"),
            );
            response
        }
        _ => response,
    }
}

/// Circuit breaker for external service calls
pub struct CircuitBreaker {
    failure_threshold: u32,
    timeout_duration: std::time::Duration,
    failure_count: std::sync::atomic::AtomicU32,
    last_failure_time: std::sync::Mutex<Option<Instant>>,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, timeout_duration: std::time::Duration) -> Self {
        Self {
            failure_threshold,
            timeout_duration,
            failure_count: std::sync::atomic::AtomicU32::new(0),
            last_failure_time: std::sync::Mutex::new(None),
        }
    }

    pub fn is_open(&self) -> bool {
        let failure_count = self
            .failure_count
            .load(std::sync::atomic::Ordering::Relaxed);
        if failure_count >= self.failure_threshold
            && let Ok(last_failure) = self.last_failure_time.lock()
            && let Some(last_failure_time) = *last_failure
        {
            return last_failure_time.elapsed() < self.timeout_duration;
        }
        false
    }

    pub fn record_success(&self) {
        self.failure_count
            .store(0, std::sync::atomic::Ordering::Relaxed);
        if let Ok(mut last_failure) = self.last_failure_time.lock() {
            *last_failure = None;
        }
    }

    pub fn record_failure(&self) {
        self.failure_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if let Ok(mut last_failure) = self.last_failure_time.lock() {
            *last_failure = Some(Instant::now());
        }
    }
}

/// Timeout middleware for request handling
pub async fn timeout_middleware(request: Request, next: Next) -> Result<Response, Response> {
    let timeout_duration = std::time::Duration::from_secs(30);

    if let Ok(response) = tokio::time::timeout(timeout_duration, next.run(request)).await {
        Ok(response)
    } else {
        let error = ApiError::RequestTimeout("Request timed out after 30 seconds".to_string());
        let context = ErrorContext::default();
        Ok(error.into_response_with_context(context))
    }
}

/// Request size validation middleware
pub async fn request_size_middleware(request: Request, next: Next) -> Result<Response, Response> {
    const MAX_REQUEST_SIZE: usize = 1024 * 1024; // 1MB

    if let Some(content_length) = request.headers().get("content-length")
        && let Ok(size_str) = content_length.to_str()
        && let Ok(size) = size_str.parse::<usize>()
        && size > MAX_REQUEST_SIZE
    {
        let error = ApiError::PayloadTooLarge(format!(
            "Request size {size} bytes exceeds maximum allowed size of {MAX_REQUEST_SIZE} bytes"
        ));
        let context = ErrorContext::default();
        return Ok(error.into_response_with_context(context));
    }

    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;

    #[test]
    fn test_circuit_breaker_creation() {
        let cb = CircuitBreaker::new(3, std::time::Duration::from_secs(60));
        assert!(!cb.is_open());
    }

    #[test]
    fn test_circuit_breaker_failure_recording() {
        let cb = CircuitBreaker::new(2, std::time::Duration::from_secs(1));

        // Record failures
        cb.record_failure();
        cb.record_failure();

        // Should be open after threshold
        assert!(cb.is_open());

        // Wait for timeout
        std::thread::sleep(std::time::Duration::from_millis(1100));
        assert!(!cb.is_open());
    }

    #[test]
    fn test_circuit_breaker_success_reset() {
        let cb = CircuitBreaker::new(2, std::time::Duration::from_secs(60));

        // Record failures
        cb.record_failure();
        cb.record_failure();
        assert!(cb.is_open());

        // Record success should reset
        cb.record_success();
        assert!(!cb.is_open());
    }

    #[test]
    fn test_extract_request_context() {
        let mut request = Request::new(axum::body::Body::empty());
        request.headers_mut().insert(
            "authorization",
            HeaderValue::from_static("Bearer test-token-12345"),
        );
        request
            .headers_mut()
            .insert("x-forwarded-for", HeaderValue::from_static("192.168.1.1"));
        request
            .headers_mut()
            .insert("user-agent", HeaderValue::from_static("test-agent"));

        let context = extract_request_context(&request, "test-request-id");

        assert_eq!(context.request_id, Some("test-request-id".to_string()));
        assert_eq!(context.user_id, Some("test-token".to_string()));
        assert_eq!(context.client_ip, Some("192.168.1.1".to_string()));
        assert_eq!(context.user_agent, Some("test-agent".to_string()));
    }
}
