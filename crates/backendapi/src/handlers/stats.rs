use crate::{
    ApiResult, ApiState,
    models::{ApiStats, CaStats},
};
use auth::{extract_bearer_token_with_dev_bypass, validate_jwt_token};
use axum::{Json, extract::State, http::HeaderMap};
use serde::Serialize;
use tracing::info;

/// Statistics response
#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub api_stats: ApiStats,
    pub system_info: SystemInfo,
}

/// System information
#[derive(Debug, Serialize)]
pub struct SystemInfo {
    pub service_name: String,
    pub version: String,
    pub rust_version: String,
    pub build_timestamp: String,
}

/// Get system and tunnel statistics
pub async fn get_stats(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> ApiResult<Json<StatsResponse>> {
    // Authenticate user
    let token = extract_bearer_token_with_dev_bypass(&headers, state.config.development_mode)?;
    let _user = validate_jwt_token(&token, &state.config.jwt_secret)?;

    info!("Fetching system statistics");

    // Get tunnel statistics from storage
    let active_tunnels = state.storage.get_active_tunnel_count().await?;

    // Get CA statistics using correct API
    let ca_statistics = state.ca.get_statistics().await;

    let ca_stats = CaStats {
        certificates_issued: ca_statistics.total_issued,
        active_certificates: ca_statistics.active_certificates as u64,
        expired_certificates: 0, // TODO: Track expired certificates
        issuance_rate: 0.0,      // TODO: Calculate issuance rate
    };

    let api_stats = ApiStats {
        active_tunnels,
        tunnels_created_today: 0,   // TODO: Implement daily counters
        bytes_transferred_today: 0, // TODO: Implement daily counters
        ca_stats,
        uptime_seconds: 0, // TODO: Track service uptime
    };

    let system_info = SystemInfo {
        service_name: "fleetingdns-api".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        rust_version: "1.75.0".to_string(), // TODO: Get actual Rust version
        build_timestamp: "2025-01-15T12:00:00Z".to_string(), // TODO: Get actual build timestamp
    };

    Ok(Json(StatsResponse {
        api_stats,
        system_info,
    }))
}
