use crate::{ApiError, ApiResult, ApiState};
use auth::{extract_bearer_token_with_dev_bypass, validate_jwt_token};
use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// Certificate issuance request
#[derive(Debug, Deserialize)]
pub struct IssueCertificateRequest {
    /// Certificate subject/common name
    pub common_name: String,

    /// Certificate TTL in seconds
    pub ttl: Option<u64>,
}

/// Certificate issuance response
#[derive(Debug, Serialize)]
pub struct IssueCertificateResponse {
    /// Certificate serial number
    pub serial: String,

    /// PEM-encoded certificate
    pub certificate: String,

    /// Certificate fingerprint
    pub fingerprint: String,

    /// Certificate expiration time
    pub expires_at: String,
}

/// Certificate information response
#[derive(Debug, Serialize)]
pub struct CertificateInfoResponse {
    pub serial: String,
    pub subject: String,
    pub fingerprint: String,
    pub issued_at: String,
    pub expires_at: String,
    pub status: String,
}

/// Issue a new ephemeral certificate
pub async fn issue_certificate(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<IssueCertificateRequest>,
) -> ApiResult<Json<IssueCertificateResponse>> {
    // Authenticate user
    let token = extract_bearer_token_with_dev_bypass(&headers, state.config.development_mode)?;
    let user = validate_jwt_token(&token, &state.config.jwt_secret)?;

    debug!(
        "Issuing certificate for user {} with common name {}",
        user.login, request.common_name
    );

    // Validate TTL
    let ttl_seconds = request.ttl.unwrap_or(state.config.default_tunnel_ttl);
    if ttl_seconds > state.config.max_tunnel_ttl {
        return Err(ApiError::BadRequest(format!(
            "Certificate TTL {} exceeds maximum allowed TTL of {} seconds",
            ttl_seconds, state.config.max_tunnel_ttl
        )));
    }

    // Create certificate issuance request using correct edf-ca API
    let cert_request = edf_ca::IssuanceRequest::new(request.common_name, user.id.clone())
        .with_ttl(chrono::Duration::seconds(ttl_seconds as i64));

    // Issue certificate via CA
    let cert_response = state.ca.issue_certificate(cert_request).await?;

    info!(
        "Issued certificate {} for user {}",
        cert_response.metadata.serial_number, user.login
    );

    Ok(Json(IssueCertificateResponse {
        serial: cert_response.metadata.serial_number,
        certificate: cert_response.certificate_pem,
        fingerprint: cert_response.metadata.fingerprint,
        expires_at: cert_response.metadata.expires_at.to_rfc3339(),
    }))
}

/// Get certificate information
pub async fn get_certificate(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(serial): Path<String>,
) -> ApiResult<Json<CertificateInfoResponse>> {
    // Authenticate user
    let token = extract_bearer_token_with_dev_bypass(&headers, state.config.development_mode)?;
    let _user = validate_jwt_token(&token, &state.config.jwt_secret)?;

    // Get certificate information from CA
    let cert_info = state.ca.get_certificate_info(&serial).await?;

    let status = if cert_info.metadata.expires_at < chrono::Utc::now() {
        "expired"
    } else {
        "active"
    };

    Ok(Json(CertificateInfoResponse {
        serial: cert_info.metadata.serial_number,
        subject: cert_info.metadata.subject,
        fingerprint: cert_info.metadata.fingerprint,
        issued_at: cert_info.metadata.issued_at.to_rfc3339(),
        expires_at: cert_info.metadata.expires_at.to_rfc3339(),
        status: status.to_string(),
    }))
}
