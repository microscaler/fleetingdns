pub mod auth;
pub mod certificates;
pub mod stats;
pub mod tunnels;
pub mod admin;

use crate::ApiResult;
use axum::Json;
use serde_json::{Value, json};

/// Health check endpoint
pub async fn health_check() -> ApiResult<Json<Value>> {
    Ok(Json(json!({
        "status": "healthy",
        "service": "fleetingdns-api",
        "version": "0.1.0",
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check_response() {
        let response = health_check().await.unwrap();
        let json_value = response.0;

        assert_eq!(json_value["status"], "healthy");
        assert_eq!(json_value["service"], "fleetingdns-api");
        assert_eq!(json_value["version"], "0.1.0");
        assert!(json_value["timestamp"].is_string());
    }

    #[tokio::test]
    async fn test_health_check_timestamp_format() {
        let response = health_check().await.unwrap();
        let json_value = response.0;

        let timestamp = json_value["timestamp"].as_str().unwrap();
        // Verify it's a valid RFC3339 timestamp
        assert!(chrono::DateTime::parse_from_rfc3339(timestamp).is_ok());
    }

    #[tokio::test]
    async fn test_health_check_multiple_calls() {
        let response1 = health_check().await.unwrap();
        let response2 = health_check().await.unwrap();

        // Status should be consistent
        assert_eq!(response1.0["status"], response2.0["status"]);
        assert_eq!(response1.0["service"], response2.0["service"]);
        assert_eq!(response1.0["version"], response2.0["version"]);

        // Timestamps should be different (or at least not fail)
        let timestamp1 = response1.0["timestamp"].as_str().unwrap();
        let timestamp2 = response2.0["timestamp"].as_str().unwrap();

        // Both should be valid timestamps
        assert!(chrono::DateTime::parse_from_rfc3339(timestamp1).is_ok());
        assert!(chrono::DateTime::parse_from_rfc3339(timestamp2).is_ok());
    }

    #[test]
    fn test_health_check_is_async() {
        // Test that the function is async by checking its type
        use std::future::Future;

        fn assert_async<F: Future>(_: F) {}
        assert_async(health_check());
    }
}
