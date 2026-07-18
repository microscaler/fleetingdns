use crate::{
    ApiError, ApiResult, ApiState,
    models::{SshKeyPair, Tunnel, TunnelStatus},
};
use auth::{extract_bearer_token_with_dev_bypass, validate_jwt_token};
use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;

/// Request to create a new tunnel
#[derive(Debug, Deserialize)]
pub struct CreateTunnelRequest {
    /// Local port to forward
    pub port: u16,

    /// Tunnel TTL in seconds (optional, uses default if not specified)
    pub ttl: Option<u64>,

    /// Custom subdomain (optional, generates random if not specified)
    pub custom_subdomain: Option<String>,

    /// Enable basic authentication (optional, default false)
    pub auth: Option<bool>,

    /// Require a session grant cookie at the edge before any bytes are
    /// forwarded (optional, default false). Grants are minted via
    /// `POST /v1/tunnels/{id}/session` by the tunnel owner (FR-EDGE-3).
    pub protected: Option<bool>,

    /// Teardown lifecycle policy (optional, default `ttl_only`).
    /// `ttl_only`: dies on TTL/DELETE/SSH-disconnect only — pick this for
    /// automation (agents driving Playwright close browsers between runs).
    /// `viewer_idle`: additionally reaped after ~60 s with no viewer
    /// connection (human portal tabs).
    pub teardown_policy: Option<common::redis::TeardownPolicy>,
}

/// Response for tunnel creation
#[derive(Debug, Serialize)]
pub struct CreateTunnelResponse {
    /// Tunnel ID
    pub id: String,

    /// Full FQDN for the tunnel
    pub fqdn: String,

    /// SSH server slot assigned
    pub slot: u16,

    /// TLS certificate for client authentication
    pub tls_cert: String,

    /// Private key for TLS certificate - NOTE: This should be in the response from edf-ca
    pub private_key: String,

    /// SSH key pair for tunnel authentication
    pub ssh_key: SshKeyPair,

    /// Tunnel expiration time
    pub expires_at: String,

    /// Basic auth credentials (if enabled)
    pub auth: Option<BasicAuthCredentials>,
}

/// Basic authentication credentials
#[derive(Debug, Serialize)]
pub struct BasicAuthCredentials {
    pub username: String,
    pub password: String,
}

/// Tunnel information response
#[derive(Debug, Serialize)]
pub struct TunnelInfo {
    pub id: String,
    pub fqdn: String,
    pub local_port: u16,
    pub slot: u16,
    pub status: TunnelStatus,
    pub created_at: String,
    pub expires_at: String,
    pub bytes_transferred: u64,
    pub request_count: u64,
    pub remaining_ttl: i64,
}

/// Complete tunnel health information
#[derive(Debug, Serialize, Clone)]
pub struct TunnelHealthInfo {
    /// Overall health status
    pub status: TunnelHealthStatus,
    /// Connection status
    pub connection_status: ConnectionStatus,
    /// Detailed health information
    pub details: HealthDetails,
    /// Last health check timestamp
    pub last_check: chrono::DateTime<Utc>,
    /// Response time in milliseconds
    pub response_time_ms: u64,
}

/// Tunnel health status enumeration
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TunnelHealthStatus {
    /// Tunnel is healthy and responding
    Healthy,

    /// Tunnel is degraded but functional
    Degraded,

    /// Tunnel is unhealthy and not responding
    Unhealthy,

    /// Tunnel health is unknown
    Unknown,
}

/// Connection status enumeration
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionStatus {
    /// SSH connection is active
    Connected,

    /// SSH connection is inactive
    Disconnected,

    /// SSH connection is being established
    Connecting,

    /// SSH connection failed
    Failed,
}

/// Detailed health information for a tunnel
#[derive(Debug, Serialize, Clone)]
pub struct HealthDetails {
    /// SSH connection status
    pub ssh_connected: bool,
    /// Certificate validity status
    pub certificate_valid: bool,
    /// DNS resolution status
    pub dns_resolvable: bool,
    /// Port forwarding status
    pub port_forwarding_active: bool,
    /// Successful requests in the last hour
    pub successful_requests_last_hour: u64,
}

/// Create a new tunnel
pub async fn create_tunnel(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<CreateTunnelRequest>,
) -> ApiResult<Json<CreateTunnelResponse>> {
    let start_time = std::time::Instant::now();

    // Create tunnel span for tracing
    let span = common::telemetry::tunnel_span("create", "new");
    let _enter = span.enter();

    // Authenticate user with development mode bypass support
    let token = extract_bearer_token_with_dev_bypass(&headers, state.config.development_mode)?;
    let user = validate_jwt_token(&token, &state.config.jwt_secret)?;

    info!(
        "Creating tunnel for user {} on port {}",
        user.login, request.port
    );

    // Validate TTL
    let ttl_seconds = request.ttl.unwrap_or(state.config.default_tunnel_ttl);
    if ttl_seconds > state.config.max_tunnel_ttl {
        return Err(ApiError::BadRequest(format!(
            "TTL {} exceeds maximum allowed TTL of {} seconds",
            ttl_seconds, state.config.max_tunnel_ttl
        )));
    }

    // Check quota for tunnel creation
    let user_id = user.id.clone();
    let can_create = state.quota_enforcer.can_create_tunnel(&user_id).await?;
    if !can_create {
        return Err(ApiError::BadRequest(
            "Tunnel creation quota exceeded. Please upgrade your ServicePlan or wait until next billing period.".to_string()
        ));
    }

    // Generate subdomain
    let subdomain = if let Some(custom) = &request.custom_subdomain {
        validate_subdomain(custom)?;
        custom.clone()
    } else {
        generate_random_subdomain().await
    };

    // Subdomain is the routing key (SNI → subdomain → slot): a duplicate
    // would make `get_tunnel_by_subdomain` nondeterministic and could route
    // one tunnel's viewers into another tunnel — cross-tunnel isolation
    // depends on this uniqueness check.
    if !state.storage.is_subdomain_available(&subdomain).await? {
        return Err(ApiError::BadRequest(format!(
            "Subdomain '{subdomain}' is already in use by an active tunnel"
        )));
    }

    let cert_request =
        edf_ca::IssuanceRequest::new(format!("tunnel-client-{}", user.id), user.id.clone());

    // Issue certificate first
    let cert_response = state
        .ca
        .issue_certificate(cert_request)
        .await
        .map_err(|e| ApiError::CertificateError(e.to_string()))?;

    // Calculate certificate TTL in seconds
    let certificate_ttl =
        (cert_response.metadata.expires_at - chrono::Utc::now()).num_seconds() as u64;

    // Allocate a port for the tunnel using certificate TTL
    let allocated_port = state
        .storage
        .allocate_port(&user.id, certificate_ttl)
        .await?;

    // Create tunnel with certificate using the allocated port
    let mut tunnel = Tunnel::new(
        user.id.clone(),
        user.login.clone(),
        subdomain.clone(),
        "fleetingdns.run",
        request.port,
        allocated_port, // Use the allocated port instead of random slot
        cert_response.metadata.serial_number.clone(),
        request.ttl.unwrap_or(3600),
    );
    tunnel.protected = request.protected.unwrap_or(false);
    tunnel.teardown_policy = request.teardown_policy.unwrap_or_default();

    // Store tunnel metadata
    state.storage.store_tunnel(&tunnel).await?;

    info!("Created tunnel {} -> {}", tunnel.fqdn, tunnel.local_port);

    // Generate SSH key pair and store the session so the hub can authenticate
    // the SSH connection against this issued key (TDP-13). The session id is
    // the tunnel id; the hub reads `session:{id}` and compares fingerprints.
    let ssh_key = generate_ssh_key_pair()?;
    state
        .storage
        .store_ssh_session(
            &tunnel.id.to_string(),
            &user.id,
            &ssh_key.public_key,
            &ssh_key.fingerprint,
            tunnel.expires_at,
        )
        .await?;

    // Generate basic auth credentials if requested
    let auth_credentials = if request.auth.unwrap_or(false) {
        Some(BasicAuthCredentials {
            username: generate_random_string(8),
            password: generate_random_string(12),
        })
    } else {
        None
    };

    // Record tunnel creation metrics
    let response_time = start_time.elapsed();
    let response_time_ms = response_time.as_millis() as u64;
    common::telemetry::record_tunnel_metrics(
        "create",
        &tunnel.id.to_string(),
        response_time_ms,
        true,
    );
    common::telemetry::record_slot_metrics("create", true);

    Ok(Json(CreateTunnelResponse {
        id: tunnel.id.to_string(),
        fqdn: tunnel.fqdn,
        slot: allocated_port, // Return the allocated port for the client
        tls_cert: cert_response.certificate_pem,
        private_key: "-----BEGIN PRIVATE KEY-----\n...\n-----END PRIVATE KEY-----".to_string(), // TODO: Get from edf-ca
        ssh_key,
        expires_at: tunnel.expires_at.to_rfc3339(),
        auth: auth_credentials,
    }))
}

/// Get tunnel information
pub async fn get_tunnel(
    State(state): State<ApiState>,
    Path(tunnel_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Json<TunnelInfo>> {
    let start_time = std::time::Instant::now();

    // Create tunnel span for tracing
    let span = common::telemetry::tunnel_span("get", &tunnel_id);
    let _enter = span.enter();

    // Authenticate user
    let token = extract_bearer_token_with_dev_bypass(&headers, state.config.development_mode)?;
    let user = validate_jwt_token(&token, &state.config.jwt_secret)?;

    // Parse tunnel ID
    let tunnel_uuid = Uuid::parse_str(&tunnel_id)
        .map_err(|_| ApiError::BadRequest("Invalid tunnel ID format".to_string()))?;

    // Get tunnel from storage
    let tunnel = state
        .storage
        .get_tunnel(&tunnel_uuid)
        .await?
        .ok_or_else(|| ApiError::NotFound("Tunnel not found".to_string()))?;

    // Check if user owns this tunnel
    if tunnel.github_user_id != user.id.clone() {
        return Err(ApiError::Forbidden("Access denied".to_string()));
    }

    // Calculate remaining TTL
    let now = Utc::now();
    let remaining_ttl = (tunnel.expires_at - now).num_seconds();

    let tunnel_info = TunnelInfo {
        id: tunnel.id.to_string(),
        fqdn: tunnel.fqdn,
        local_port: tunnel.local_port,
        slot: tunnel.slot,
        status: tunnel.status,
        created_at: tunnel.created_at.to_rfc3339(),
        expires_at: tunnel.expires_at.to_rfc3339(),
        bytes_transferred: tunnel.bytes_transferred,
        request_count: tunnel.request_count,
        remaining_ttl,
    };

    // Record tunnel retrieval metrics
    let response_time = start_time.elapsed();
    let response_time_ms = response_time.as_millis() as u64;
    common::telemetry::record_tunnel_metrics("get", &tunnel_id, response_time_ms, true);
    common::telemetry::record_slot_metrics("retrieve", true);

    Ok(Json(tunnel_info))
}

/// Name of the cookie the edge router checks on protected tunnels
/// (shared contract, defined in `common::redis`).
pub use common::redis::SESSION_COOKIE_NAME;

/// TTL for edge session grants, in seconds (15 minutes, per FR-API-2).
pub const SESSION_GRANT_TTL_SECONDS: u64 = 900;

/// Response for a session grant on a protected tunnel (FR-EDGE-3).
#[derive(Debug, Serialize)]
pub struct SessionGrantResponse {
    /// Opaque grant token; present it as the `fdns_session` cookie.
    pub token: String,
    /// Ready-to-set cookie string for the tunnel's FQDN.
    pub cookie: String,
    /// Tunnel FQDN the grant is scoped to.
    pub fqdn: String,
    /// When the grant expires (RFC 3339).
    pub expires_at: String,
}

/// Mint a short-lived session grant for a protected tunnel.
///
/// Only the tunnel owner may mint grants. The edge router rejects
/// connections to protected tunnels with 403 unless the request carries
/// a cookie naming a live grant.
pub async fn create_tunnel_session(
    State(state): State<ApiState>,
    Path(tunnel_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Json<SessionGrantResponse>> {
    let span = common::telemetry::tunnel_span("session", &tunnel_id);
    let _enter = span.enter();

    // Authenticate user
    let token = extract_bearer_token_with_dev_bypass(&headers, state.config.development_mode)?;
    let user = validate_jwt_token(&token, &state.config.jwt_secret)?;

    // Parse tunnel ID
    let tunnel_uuid = Uuid::parse_str(&tunnel_id)
        .map_err(|_| ApiError::BadRequest("Invalid tunnel ID format".to_string()))?;

    // Get tunnel from storage
    let tunnel = state
        .storage
        .get_tunnel(&tunnel_uuid)
        .await?
        .ok_or_else(|| ApiError::NotFound("Tunnel not found".to_string()))?;

    // Only the owner may mint grants (FR-EDGE-3: owner of the agent).
    if tunnel.github_user_id != user.id.clone() {
        return Err(ApiError::Forbidden("Access denied".to_string()));
    }

    let grant_token = generate_random_string(32);
    state
        .storage
        .store_session_grant(
            &tunnel.subdomain,
            &grant_token,
            &user.id,
            SESSION_GRANT_TTL_SECONDS,
        )
        .await?;

    let expires_at = Utc::now() + chrono::Duration::seconds(SESSION_GRANT_TTL_SECONDS as i64);
    info!(
        "Minted session grant for tunnel {} (subdomain {}) for user {}",
        tunnel_id, tunnel.subdomain, user.login
    );

    Ok(Json(SessionGrantResponse {
        cookie: format!(
            "{SESSION_COOKIE_NAME}={grant_token}; Domain={}; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age={SESSION_GRANT_TTL_SECONDS}",
            tunnel.fqdn
        ),
        token: grant_token,
        fqdn: tunnel.fqdn,
        expires_at: expires_at.to_rfc3339(),
    }))
}

/// Delete a tunnel
pub async fn delete_tunnel(
    State(state): State<ApiState>,
    Path(tunnel_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Json<serde_json::Value>> {
    let start_time = std::time::Instant::now();

    // Create tunnel span for tracing
    let span = common::telemetry::tunnel_span("delete", &tunnel_id);
    let _enter = span.enter();

    // Authenticate user
    let token = extract_bearer_token_with_dev_bypass(&headers, state.config.development_mode)?;
    let user = validate_jwt_token(&token, &state.config.jwt_secret)?;

    // Parse tunnel ID
    let tunnel_uuid = Uuid::parse_str(&tunnel_id)
        .map_err(|_| ApiError::BadRequest("Invalid tunnel ID format".to_string()))?;

    // Get tunnel from storage
    let tunnel = state
        .storage
        .get_tunnel(&tunnel_uuid)
        .await?
        .ok_or_else(|| ApiError::NotFound("Tunnel not found".to_string()))?;

    // Check if user owns this tunnel
    if tunnel.github_user_id != user.id.clone() {
        return Err(ApiError::Forbidden("Access denied".to_string()));
    }

    // Delete tunnel from storage
    let deleted = state.storage.delete_tunnel(&tunnel_uuid).await?;

    if deleted {
        info!("Deleted tunnel {} for user {}", tunnel_id, user.login);
    }

    // Record tunnel deletion metrics
    let response_time = start_time.elapsed();
    let response_time_ms = response_time.as_millis() as u64;
    common::telemetry::record_tunnel_metrics("delete", &tunnel_id, response_time_ms, deleted);
    common::telemetry::record_slot_metrics("delete", deleted);

    Ok(Json(serde_json::json!({
        "deleted": deleted,
        "message": if deleted { "Tunnel deleted successfully" } else { "Tunnel not found" }
    })))
}

/// Get tunnel health information
pub async fn get_tunnel_health(
    State(state): State<ApiState>,
    Path(tunnel_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Json<TunnelHealthInfo>> {
    let start_time = std::time::Instant::now();

    // Create tunnel span for tracing
    let span = common::telemetry::tunnel_span("health", &tunnel_id);
    let _enter = span.enter();

    // Authenticate user
    let token = extract_bearer_token_with_dev_bypass(&headers, state.config.development_mode)?;
    let user = validate_jwt_token(&token, &state.config.jwt_secret)?;

    // Parse tunnel ID
    let tunnel_uuid = Uuid::parse_str(&tunnel_id)
        .map_err(|_| ApiError::BadRequest("Invalid tunnel ID format".to_string()))?;

    // Get tunnel from storage
    let tunnel = state
        .storage
        .get_tunnel(&tunnel_uuid)
        .await?
        .ok_or_else(|| ApiError::NotFound("Tunnel not found".to_string()))?;

    // Verify tunnel ownership
    if tunnel.github_user_id != user.id.clone() {
        return Err(ApiError::Forbidden("Access denied to tunnel".to_string()));
    }

    // Perform health checks
    let health_info = perform_tunnel_health_checks(&tunnel, &state).await?;

    // Log health check completion
    info!(
        "Tunnel health check completed for tunnel {} in {:?}",
        tunnel_id,
        start_time.elapsed()
    );

    Ok(Json(health_info))
}

/// Get bulk health status for multiple tunnels
pub async fn get_bulk_tunnel_health(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<BulkHealthRequest>,
) -> ApiResult<Json<BulkHealthResponse>> {
    let start_time = std::time::Instant::now();

    // Create bulk health span for tracing
    let span = common::telemetry::tunnel_span("bulk_health", "multiple");
    let _enter = span.enter();

    // Authenticate user
    let token = extract_bearer_token_with_dev_bypass(&headers, state.config.development_mode)?;
    let user = validate_jwt_token(&token, &state.config.jwt_secret)?;

    // Validate request
    if request.tunnel_ids.is_empty() {
        return Err(ApiError::BadRequest("No tunnel IDs provided".to_string()));
    }

    if request.tunnel_ids.len() > 100 {
        return Err(ApiError::BadRequest(
            "Maximum 100 tunnels per request".to_string(),
        ));
    }

    // Get tunnels from storage
    let mut health_results = Vec::new();
    let mut errors = Vec::new();

    for tunnel_id in &request.tunnel_ids {
        let tunnel_uuid = if let Ok(uuid) = Uuid::parse_str(tunnel_id) {
            uuid
        } else {
            errors.push(HealthError {
                tunnel_id: tunnel_id.clone(),
                error: format!("Invalid tunnel ID format: {}", tunnel_id),
            });
            continue;
        };

        match state.storage.get_tunnel(&tunnel_uuid).await {
            Ok(Some(tunnel)) => {
                // Verify tunnel ownership
                if tunnel.github_user_id != user.id.clone() {
                    errors.push(HealthError {
                        tunnel_id: tunnel_id.clone(),
                        error: "Access denied to tunnel".to_string(),
                    });
                    continue;
                }

                // Perform health checks
                match perform_tunnel_health_checks(&tunnel, &state).await {
                    Ok(health_info) => {
                        health_results.push(TunnelHealthResult {
                            tunnel_id: tunnel_id.clone(),
                            health_info,
                        });
                    }
                    Err(e) => {
                        errors.push(HealthError {
                            tunnel_id: tunnel_id.clone(),
                            error: e.to_string(),
                        });
                    }
                }
            }
            Ok(None) => {
                errors.push(HealthError {
                    tunnel_id: tunnel_id.clone(),
                    error: "Tunnel not found".to_string(),
                });
            }
            Err(_) => {
                errors.push(HealthError {
                    tunnel_id: tunnel_id.clone(),
                    error: "Failed to retrieve tunnel".to_string(),
                });
            }
        }
    }

    // Log bulk health check completion
    info!(
        "Bulk tunnel health check completed for {} tunnels in {:?}",
        request.tunnel_ids.len(),
        start_time.elapsed()
    );

    let successful_checks = health_results.len();
    let failed_checks = errors.len();

    Ok(Json(BulkHealthResponse {
        health_results,
        errors,
        total_tunnels: request.tunnel_ids.len(),
        successful_checks,
        failed_checks,
    }))
}

/// Request for bulk health status
#[derive(Debug, Deserialize)]
pub struct BulkHealthRequest {
    /// List of tunnel IDs to check
    pub tunnel_ids: Vec<String>,
}

/// Response for bulk health status
#[derive(Debug, Serialize)]
pub struct BulkHealthResponse {
    /// Successful health check results
    pub health_results: Vec<TunnelHealthResult>,
    /// Failed health check errors
    pub errors: Vec<HealthError>,
    /// Total number of tunnels requested
    pub total_tunnels: usize,
    /// Number of successful health checks
    pub successful_checks: usize,
    /// Number of failed health checks
    pub failed_checks: usize,
}

/// Individual tunnel health result
#[derive(Debug, Serialize, Clone)]
pub struct TunnelHealthResult {
    /// Tunnel ID
    pub tunnel_id: String,
    /// Health information
    pub health_info: TunnelHealthInfo,
}

/// Health check error
#[derive(Debug, Serialize, Clone)]
pub struct HealthError {
    /// Tunnel ID that failed
    pub tunnel_id: String,
    /// Error message
    pub error: String,
}

/// Perform comprehensive health checks for a tunnel
async fn perform_tunnel_health_checks(
    tunnel: &Tunnel,
    _state: &ApiState,
) -> ApiResult<TunnelHealthInfo> {
    let now = Utc::now();

    // Check if tunnel is expired
    if tunnel.is_expired() {
        return Ok(TunnelHealthInfo {
            status: TunnelHealthStatus::Unhealthy,
            connection_status: ConnectionStatus::Disconnected,
            details: HealthDetails {
                ssh_connected: false,
                certificate_valid: false,
                dns_resolvable: false,
                port_forwarding_active: false,
                successful_requests_last_hour: 0,
            },
            last_check: now,
            response_time_ms: 0, // Simplified - in production this would be actual response time
        });
    }

    // Check certificate validity
    let certificate_valid = if tunnel.certificate_serial.is_empty() {
        false
    } else {
        // In a real implementation, this would validate the certificate with edf-ca
        // For now, we'll assume it's valid if it exists and tunnel is not expired
        true
    };

    // Check DNS resolution (simplified - in production this would do actual DNS lookup)
    let dns_resolvable = true; // Simplified for now

    // Check SSH connection status (simplified - in production this would check actual SSH connections)
    let ssh_connected = match tunnel.status {
        TunnelStatus::Active => true,
        TunnelStatus::Creating => false,
        TunnelStatus::Destroying => false,
        TunnelStatus::Expired => false,
        TunnelStatus::Error => false,
    };

    // Check port forwarding (simplified - in production this would check actual port forwarding)
    let port_forwarding_active = ssh_connected && certificate_valid;

    // Calculate error rate based on request count and failed requests
    // In a real implementation, this would track actual failed requests
    let total_requests = tunnel.request_count as f64;
    let failed_requests = if tunnel.status == TunnelStatus::Error {
        total_requests
    } else {
        0.0
    };
    let error_rate_percent = if total_requests > 0.0 {
        (failed_requests / total_requests) * 100.0
    } else {
        0.0
    };

    // Calculate bandwidth (simplified - in production this would track actual bandwidth)
    let _bandwidth_bps = if tunnel.bytes_transferred > 0 {
        // Estimate bandwidth based on total bytes transferred and time since creation
        let duration = (now - tunnel.created_at).num_seconds() as f64;
        if duration > 0.0 {
            (tunnel.bytes_transferred as f64 / duration) as u64
        } else {
            0
        }
    } else {
        0
    };

    // Determine health status
    let health_status = if error_rate_percent > 50.0 || !ssh_connected {
        TunnelHealthStatus::Unhealthy
    } else if error_rate_percent > 10.0 || !certificate_valid {
        TunnelHealthStatus::Degraded
    } else if ssh_connected && certificate_valid {
        TunnelHealthStatus::Healthy
    } else {
        TunnelHealthStatus::Unknown
    };

    // Determine connection status
    let connection_status = if !ssh_connected {
        ConnectionStatus::Disconnected
    } else if tunnel.status == TunnelStatus::Creating {
        ConnectionStatus::Connecting
    } else if tunnel.status == TunnelStatus::Error {
        ConnectionStatus::Failed
    } else {
        ConnectionStatus::Connected
    };

    // Get recent request statistics (simplified - in production this would track actual recent requests)
    let successful_requests_last_hour = if health_status == TunnelHealthStatus::Healthy {
        tunnel.request_count
    } else {
        0
    };
    let _failed_requests_last_hour = if health_status == TunnelHealthStatus::Unhealthy {
        tunnel.request_count
    } else {
        0
    };

    Ok(TunnelHealthInfo {
        status: health_status,
        connection_status,
        details: HealthDetails {
            ssh_connected,
            certificate_valid,
            dns_resolvable,
            port_forwarding_active,
            successful_requests_last_hour,
        },
        last_check: now,
        response_time_ms: 50, // Simplified - in production this would be actual response time
    })
}

/// List user's tunnels
pub async fn list_tunnels(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<TunnelInfo>>> {
    let start_time = std::time::Instant::now();

    // Create tunnel span for tracing
    let span = common::telemetry::tunnel_span("list", "user_tunnels");
    let _enter = span.enter();

    // Authenticate user
    let token = extract_bearer_token_with_dev_bypass(&headers, state.config.development_mode)?;
    let user = validate_jwt_token(&token, &state.config.jwt_secret)?;

    // Get user's tunnels from storage
    let tunnels = state.storage.list_user_tunnels(&user.id.clone()).await?;

    // Convert to TunnelInfo
    let now = Utc::now();
    let tunnel_infos: Vec<TunnelInfo> = tunnels
        .into_iter()
        .map(|tunnel| {
            let remaining_ttl = (tunnel.expires_at - now).num_seconds();
            TunnelInfo {
                id: tunnel.id.to_string(),
                fqdn: tunnel.fqdn,
                local_port: tunnel.local_port,
                slot: tunnel.slot,
                status: tunnel.status,
                created_at: tunnel.created_at.to_rfc3339(),
                expires_at: tunnel.expires_at.to_rfc3339(),
                bytes_transferred: tunnel.bytes_transferred,
                request_count: tunnel.request_count,
                remaining_ttl,
            }
        })
        .collect();

    // Record tunnel listing metrics
    let response_time = start_time.elapsed();
    let response_time_ms = response_time.as_millis() as u64;
    common::telemetry::record_tunnel_metrics("list", "user_tunnels", response_time_ms, true);
    common::telemetry::record_slot_metrics("list", true);

    Ok(Json(tunnel_infos))
}

/// Get port allocation statistics
#[allow(dead_code)] // TODO: register route in router (stats endpoint backlog)
pub async fn get_port_stats(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> ApiResult<Json<serde_json::Value>> {
    // Authenticate user with development mode bypass support
    let token = extract_bearer_token_with_dev_bypass(&headers, state.config.development_mode)?;
    let _user = validate_jwt_token(&token, &state.config.jwt_secret)?;

    let (allocated, available, total) = state.storage.get_port_stats().await?;

    Ok(Json(serde_json::json!({
        "allocated_ports": allocated,
        "available_ports": available,
        "total_ports": total,
        "utilization_percentage": (allocated as f64 / total as f64 * 100.0).round() as u8
    })))
}

/// Validate subdomain format
fn validate_subdomain(subdomain: &str) -> ApiResult<()> {
    if subdomain.is_empty() || subdomain.len() > 63 {
        return Err(ApiError::BadRequest(
            "Subdomain must be 1-63 characters".to_string(),
        ));
    }

    if !subdomain
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(ApiError::BadRequest(
            "Subdomain can only contain alphanumeric characters and hyphens".to_string(),
        ));
    }

    if subdomain.starts_with('-') || subdomain.ends_with('-') {
        return Err(ApiError::BadRequest(
            "Subdomain cannot start or end with hyphen".to_string(),
        ));
    }

    Ok(())
}

/// Generate a random subdomain.
///
/// The subdomain is a capability URL: tunnels are short-lived and, by
/// default, anyone holding the link may access the tunnel for its
/// lifetime (no cookie/cert gate). The random part must therefore be
/// computationally unguessable: 20 base-36 chars ≈ 103 bits from the
/// thread-local CSPRNG. Client-side cert validation may be layered on
/// later; entropy here is the load-bearing control today.
async fn generate_random_subdomain() -> String {
    use rand::Rng;

    let chars: String = (0..20)
        .map(|_| {
            let idx = rand::thread_rng().gen_range(0..36);
            if idx < 10 {
                (b'0' + idx) as char
            } else {
                (b'a' + idx - 10) as char
            }
        })
        .collect();

    format!("tunnel-{chars}")
}

// Allocate a random SSH slot for tunnel - REMOVED OLD SYSTEM
// async fn allocate_ssh_slot(_state: &ApiState) -> ApiResult<u16> {
//     // For now, return a random port in the ephemeral range
//     // In production, this should coordinate with EdgeHub to allocate actual slots
//     use rand::Rng;
//     let mut rng = rand::thread_rng();
//     Ok(rng.gen_range(10000..20000))
// }

/// Generate a real ephemeral Ed25519 SSH key pair for the tunnel session
/// (TDP-13). The public key's fingerprint is stored in Redis so the hub can
/// authenticate the SSH connection against this issued key.
fn generate_ssh_key_pair() -> ApiResult<SshKeyPair> {
    let kp = common::ssh_keys::generate_ed25519_keypair()
        .map_err(|e| ApiError::InternalError(format!("SSH key generation failed: {e}")))?;
    Ok(SshKeyPair {
        private_key: kp.private_key_openssh,
        public_key: kp.public_key_openssh,
        fingerprint: kp.fingerprint,
    })
}

/// Generate random string
fn generate_random_string(length: usize) -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

    (0..length)
        .map(|_| {
            let idx = rand::thread_rng().gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The random subdomain IS the access credential (capability-URL model:
    /// short-lived tunnel, unguessable link, no cookie/cert gate by default),
    /// so it must carry enough entropy to be computationally unguessable.
    /// 20 base-36 chars ≈ 103 bits.
    #[tokio::test]
    async fn random_subdomain_is_high_entropy_capability() {
        let sub = generate_random_subdomain().await;

        let rand_part = sub.strip_prefix("tunnel-").expect("keeps tunnel- prefix");
        assert_eq!(rand_part.len(), 20, "20 base-36 chars ≈ 103 bits");
        assert!(
            rand_part
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        );

        // Valid DNS label and passes our own validation
        assert!(sub.len() <= 63);
        validate_subdomain(&sub).expect("generated subdomain must validate");

        // Sanity: no repeats across a small sample
        let mut seen = std::collections::HashSet::new();
        for _ in 0..100 {
            assert!(
                seen.insert(generate_random_subdomain().await),
                "duplicate subdomain generated"
            );
        }
    }

    #[test]
    fn test_tunnel_health_status_variants() {
        // Test that all health status variants can be serialized
        let statuses = vec![
            TunnelHealthStatus::Healthy,
            TunnelHealthStatus::Degraded,
            TunnelHealthStatus::Unhealthy,
            TunnelHealthStatus::Unknown,
        ];

        for status in statuses {
            let serialized = serde_json::to_string(&status).unwrap();
            let deserialized: TunnelHealthStatus = serde_json::from_str(&serialized).unwrap();
            assert_eq!(status, deserialized);
        }
    }

    #[test]
    fn test_connection_status_variants() {
        // Test that all connection status variants can be serialized
        let statuses = vec![
            ConnectionStatus::Connected,
            ConnectionStatus::Disconnected,
            ConnectionStatus::Connecting,
            ConnectionStatus::Failed,
        ];

        for status in statuses {
            let serialized = serde_json::to_string(&status).unwrap();
            let deserialized: ConnectionStatus = serde_json::from_str(&serialized).unwrap();
            assert_eq!(status, deserialized);
        }
    }

    #[test]
    fn test_health_details_structure() {
        // Test that HealthDetails can be created and serialized
        let health_details = HealthDetails {
            ssh_connected: true,
            certificate_valid: true,
            dns_resolvable: true,
            port_forwarding_active: true,
            successful_requests_last_hour: 100,
        };

        let serialized = serde_json::to_string(&health_details).unwrap();
        assert!(serialized.contains("ssh_connected"));
        assert!(serialized.contains("certificate_valid"));
        assert!(serialized.contains("dns_resolvable"));
        assert!(serialized.contains("port_forwarding_active"));
    }

    #[test]
    fn test_tunnel_health_info_structure() {
        // Test that TunnelHealthInfo can be created and serialized
        let health_info = TunnelHealthInfo {
            status: TunnelHealthStatus::Healthy,
            connection_status: ConnectionStatus::Connected,
            details: HealthDetails {
                ssh_connected: true,
                certificate_valid: true,
                dns_resolvable: true,
                port_forwarding_active: true,
                successful_requests_last_hour: 100,
            },
            last_check: Utc::now(),
            response_time_ms: 50,
        };

        let serialized = serde_json::to_string(&health_info).unwrap();
        assert!(serialized.contains("healthy"));
        assert!(serialized.contains("connected"));
        assert!(serialized.contains("50"));
    }
}
