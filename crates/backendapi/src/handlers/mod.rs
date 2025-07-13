pub mod auth;
pub mod certificates;
pub mod stats;
pub mod tunnels;

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
