use crate::{ApiError, ApiResult, ApiState, models::*};
use auth::{extract_bearer_token_with_dev_bypass, validate_jwt_token};
use axum::{Json, extract::{Path, State}, http::HeaderMap};
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;
use chrono::Utc;

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
    let user_id = user.id.to_string();
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

    let cert_request =
        edf_ca::IssuanceRequest::new(format!("tunnel-client-{}", user.id), user.id.to_string());

    // Issue certificate first
    let cert_response = state
        .ca
        .issue_certificate(cert_request)
        .await
        .map_err(|e| ApiError::CertificateError(e.to_string()))?;

    // Calculate certificate TTL in seconds
    let certificate_ttl = (cert_response.metadata.expires_at - chrono::Utc::now()).num_seconds() as u64;
    
    // Allocate a port for the tunnel using certificate TTL
    let allocated_port = state.storage.allocate_port(&user.id, certificate_ttl).await?;

    // Create tunnel with certificate using the allocated port
    let tunnel = Tunnel::new(
        user.id.to_string(),
        user.login.clone(),
        subdomain.clone(),
        "fleetingdns.run",
        request.port,
        allocated_port, // Use the allocated port instead of random slot
        cert_response.metadata.serial_number.clone(),
        request.ttl.unwrap_or(3600),
    );

    // Store tunnel metadata
    state.storage.store_tunnel(&tunnel).await?;

    info!("Created tunnel {} -> {}", tunnel.fqdn, tunnel.local_port);

    // Generate SSH key pair
    let ssh_key = generate_ssh_key_pair()?;

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
    common::telemetry::record_tunnel_metrics("create", &tunnel.id.to_string(), response_time_ms, true);
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
    if tunnel.github_user_id != user.id.to_string() {
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
    if tunnel.github_user_id != user.id.to_string() {
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
    let tunnels = state
        .storage
        .list_user_tunnels(&user.id.to_string())
        .await?;

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

/// Generate a random subdomain
async fn generate_random_subdomain() -> String {
    use rand::Rng;

    // Generate a random 8-character alphanumeric string
    let chars: String = (0..8)
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

/// Allocate an SSH server slot
/// Allocate a random SSH slot for tunnel - REMOVED OLD SYSTEM
// async fn allocate_ssh_slot(_state: &ApiState) -> ApiResult<u16> {
//     // For now, return a random port in the ephemeral range
//     // In production, this should coordinate with EdgeHub to allocate actual slots
//     use rand::Rng;
//     let mut rng = rand::thread_rng();
//     Ok(rng.gen_range(10000..20000))
// }

/// Generate SSH key pair
fn generate_ssh_key_pair() -> ApiResult<SshKeyPair> {
    // For now, return placeholder keys
    // In production, generate actual Ed25519 key pairs
    Ok(SshKeyPair {
        private_key: "-----BEGIN OPENSSH PRIVATE KEY-----\n...\n-----END OPENSSH PRIVATE KEY-----"
            .to_string(),
        public_key: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5... tunnel-key".to_string(),
        fingerprint: "SHA256:abcd1234...".to_string(),
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
