//! Certificate Authority implementation for FleetingDNS

use chrono::{DateTime, Duration, Utc};
use rcgen::{Certificate, CertificateParams, DnType, KeyPair};
use std::path::Path;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::Mutex;
use tracing::{debug, info};

use crate::certificate::{
    CertificateBuilder, CertificateMetadata, CertificateRequest, EphemeralCertificate,
    calculate_fingerprint, generate_serial_number,
};
use crate::errors::{CaError, CaResult};
use crate::{ActiveCertificate, CaConfig, CertificateRegistry, IssuanceRequest, IssuanceResponse};

/// Certificate Authority implementation
pub struct CertificateAuthority {
    /// CA configuration
    config: CaConfig,
    /// CA certificate
    ca_certificate: Certificate,
    /// Certificate registry for tracking active certificates
    registry: CertificateRegistry,
    /// Rate limiting state
    rate_limiter: Arc<Mutex<RateLimiter>>,
}

impl CertificateAuthority {
    /// Create a new Certificate Authority
    pub async fn new(config: CaConfig) -> CaResult<Self> {
        let ca_certificate = Self::load_or_generate_ca_certificate(&config).await?;
        let registry = CertificateRegistry::new();
        let rate_limiter = Arc::new(Mutex::new(RateLimiter::new()));

        info!(
            ca_name = %config.ca_name,
            organization = %config.organization,
            "Certificate Authority initialized"
        );

        Ok(Self {
            config,
            ca_certificate,
            registry,
            rate_limiter,
        })
    }

    /// Issue an ephemeral certificate
    pub async fn issue_certificate(&self, request: IssuanceRequest) -> CaResult<IssuanceResponse> {
        info!(
            request_id = %request.request_id,
            client_id = %request.client_id,
            common_name = %request.common_name,
            "Processing certificate issuance request"
        );

        // Validate request
        self.validate_request(&request).await?;

        // Check rate limits
        self.check_rate_limits(&request.client_id).await?;

        // Determine certificate TTL
        let ttl = request.ttl.unwrap_or(self.config.default_ttl);
        if ttl > self.config.max_ttl {
            return Err(CaError::TtlValidation {
                requested_minutes: ttl.num_minutes(),
                max_minutes: self.config.max_ttl.num_minutes(),
            });
        }

        // Generate certificate
        let cert_request = CertificateRequest::for_tunnel_client(&request.client_id, ttl)
            .with_san(request.common_name.clone());

        let ephemeral_cert = self.generate_certificate(cert_request).await?;

        // Register certificate
        let active_cert = ActiveCertificate {
            metadata: ephemeral_cert.metadata.clone(),
            certificate_pem: ephemeral_cert.certificate_pem.clone(),
            client_id: request.client_id.clone(),
            issued_at: Utc::now(),
        };

        self.registry.register_certificate(active_cert).await;

        // Update rate limiter
        self.rate_limiter.lock().await.record_issuance(&request.client_id);

        info!(
            request_id = %request.request_id,
            serial_number = %ephemeral_cert.metadata.serial_number,
            expires_at = %ephemeral_cert.metadata.expires_at,
            "Certificate issued successfully"
        );

        Ok(IssuanceResponse {
            request_id: request.request_id,
            certificate_pem: ephemeral_cert.certificate_pem,
            metadata: ephemeral_cert.metadata,
            ca_chain_pem: ephemeral_cert.ca_chain_pem,
        })
    }

    /// Validate a certificate by serial number
    pub async fn validate_certificate(&self, serial_number: &str) -> CaResult<bool> {
        let is_valid = self.registry.is_certificate_valid(serial_number).await;
        
        debug!(
            serial_number = %serial_number,
            is_valid = %is_valid,
            "Certificate validation check"
        );

        Ok(is_valid)
    }

    /// Get certificate information
    pub async fn get_certificate_info(&self, serial_number: &str) -> CaResult<ActiveCertificate> {
        self.registry
            .get_certificate(serial_number)
            .await
            .ok_or_else(|| CaError::CertificateNotFound {
                serial: serial_number.to_string(),
            })
    }

    /// Revoke a certificate (mark as invalid)
    pub async fn revoke_certificate(&self, serial_number: &str) -> CaResult<()> {
        // For ephemeral certificates, we just remove from registry
        // In production, this might involve CRL or OCSP
        let mut certs = self.registry.active_certs.lock().await;
        if certs.remove(serial_number).is_some() {
            info!(serial_number = %serial_number, "Certificate revoked");
            Ok(())
        } else {
            Err(CaError::CertificateNotFound {
                serial: serial_number.to_string(),
            })
        }
    }

    /// Clean up expired certificates
    pub async fn cleanup_expired_certificates(&self) -> usize {
        self.registry.cleanup_expired().await
    }

    /// Get CA certificate in PEM format
    pub fn get_ca_certificate_pem(&self) -> CaResult<String> {
        self.ca_certificate
            .serialize_pem()
            .map_err(|e| CaError::CertificateGeneration(e.to_string()))
    }

    /// Get statistics about the CA
    pub async fn get_statistics(&self) -> CaStatistics {
        CaStatistics {
            active_certificates: self.registry.active_count().await,
            total_issued: self.rate_limiter.lock().await.total_issued(),
            ca_name: self.config.ca_name.clone(),
            uptime: Utc::now(), // Would be actual uptime in production
        }
    }

    /// Generate an ephemeral certificate
    async fn generate_certificate(&self, request: CertificateRequest) -> CaResult<EphemeralCertificate> {
        // Generate key pair for the certificate
        let key_pair = KeyPair::generate(&rcgen::PKCS_ED25519)
            .map_err(|e| CaError::CertificateGeneration(e.to_string()))?;

        // Build certificate
        let cert = CertificateBuilder::new()
            .common_name(request.common_name.clone())
            .subject_alt_names(request.subject_alt_names)
            .validity_duration(request.validity_duration)
            .key_usage(request.key_usage)
            .extended_key_usage(request.extended_key_usage)
            .generate()?;

        // Sign with CA
        let cert_pem = cert
            .serialize_pem_with_signer(&self.ca_certificate)
            .map_err(|e| CaError::CertificateGeneration(e.to_string()))?;

        let private_key_pem = key_pair.serialize_pem();

        // Calculate fingerprint
        let fingerprint = calculate_fingerprint(&cert_pem)?;

        // Create metadata
        let serial_number = generate_serial_number();
        let now = Utc::now();
        let expires_at = now + request.validity_duration;

        let metadata = CertificateMetadata {
            serial_number,
            subject: format!("CN={}", request.common_name),
            issuer: format!("CN={}", self.config.ca_name),
            issued_at: now,
            expires_at,
            fingerprint,
        };

        // Get CA chain
        let ca_chain_pem = self.get_ca_certificate_pem()?;

        Ok(EphemeralCertificate::new(
            cert_pem,
            private_key_pem,
            metadata,
            ca_chain_pem,
        ))
    }

    /// Load existing CA certificate or generate a new one
    async fn load_or_generate_ca_certificate(config: &CaConfig) -> CaResult<Certificate> {
        if let (Some(cert_path), Some(key_path)) = (&config.ca_cert_path, &config.ca_key_path) {
            if Path::new(cert_path).exists() && Path::new(key_path).exists() {
                info!(cert_path = %cert_path, "Loading existing CA certificate");
                return Self::load_ca_certificate(cert_path, key_path).await;
            }
        }

        info!("Generating new CA certificate");
        let ca_cert = Self::generate_ca_certificate(config).await?;

        // Save if paths are provided
        if let (Some(cert_path), Some(key_path)) = (&config.ca_cert_path, &config.ca_key_path) {
            Self::save_ca_certificate(&ca_cert, cert_path, key_path).await?;
        }

        Ok(ca_cert)
    }

    /// Generate a new CA certificate
    async fn generate_ca_certificate(config: &CaConfig) -> CaResult<Certificate> {
        let mut params = CertificateParams::new(Vec::new());
        params.distinguished_name.push(DnType::CommonName, config.ca_name.clone());
        params.distinguished_name.push(DnType::OrganizationName, config.organization.clone());
        params.distinguished_name.push(DnType::OrganizationalUnitName, config.organizational_unit.clone());

        params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        params.key_usages = vec![
            rcgen::KeyUsagePurpose::KeyCertSign,
            rcgen::KeyUsagePurpose::CrlSign,
        ];

        // Set validity for 10 years
        let now = time::OffsetDateTime::now_utc();
        params.not_before = now;
        params.not_after = now + time::Duration::days(3650);

        Certificate::from_params(params)
            .map_err(|e| CaError::CertificateGeneration(e.to_string()))
    }

    /// Load CA certificate from files
    async fn load_ca_certificate(_cert_path: &str, _key_path: &str) -> CaResult<Certificate> {
        // For now, just generate a new certificate
        // In production, this would load from files
        let config = CaConfig::default();
        Self::generate_ca_certificate(&config).await
    }

    /// Save CA certificate to files
    async fn save_ca_certificate(cert: &Certificate, cert_path: &str, key_path: &str) -> CaResult<()> {
        let cert_pem = cert
            .serialize_pem()
            .map_err(|e| CaError::CertificateGeneration(e.to_string()))?;

        let key_pem = cert.get_key_pair().serialize_pem();

        fs::write(cert_path, cert_pem)
            .await
            .map_err(CaError::Io)?;

        fs::write(key_path, key_pem)
            .await
            .map_err(CaError::Io)?;

        info!(cert_path = %cert_path, key_path = %key_path, "CA certificate saved");
        Ok(())
    }

    /// Validate certificate issuance request
    async fn validate_request(&self, request: &IssuanceRequest) -> CaResult<()> {
        if request.common_name.is_empty() {
            return Err(CaError::InvalidRequest("Common name cannot be empty".to_string()));
        }

        if request.client_id.is_empty() {
            return Err(CaError::InvalidRequest("Client ID cannot be empty".to_string()));
        }

        // Additional validation can be added here
        Ok(())
    }

    /// Check rate limits for client
    async fn check_rate_limits(&self, client_id: &str) -> CaResult<()> {
        let rate_limiter = self.rate_limiter.lock().await;
        if rate_limiter.is_rate_limited(client_id) {
            return Err(CaError::RateLimitExceeded {
                client_id: client_id.to_string(),
                limit: rate_limiter.limit,
                window: "hour".to_string(),
            });
        }
        Ok(())
    }
}

/// CA statistics
#[derive(Debug, Clone)]
pub struct CaStatistics {
    pub active_certificates: usize,
    pub total_issued: u64,
    pub ca_name: String,
    pub uptime: DateTime<Utc>,
}

/// Simple rate limiter for certificate issuance
#[derive(Debug)]
struct RateLimiter {
    /// Maximum certificates per hour per client
    limit: u32,
    /// Client issuance counts
    client_counts: std::collections::HashMap<String, (u32, DateTime<Utc>)>,
    /// Total certificates issued
    total_issued: u64,
}

impl RateLimiter {
    fn new() -> Self {
        Self {
            limit: 10, // 10 certificates per hour per client
            client_counts: std::collections::HashMap::new(),
            total_issued: 0,
        }
    }

    fn is_rate_limited(&self, client_id: &str) -> bool {
        if let Some((count, last_reset)) = self.client_counts.get(client_id) {
            let now = Utc::now();
            if now - *last_reset < Duration::hours(1) {
                *count >= self.limit
            } else {
                false // Window expired, allow
            }
        } else {
            false // First request from this client
        }
    }

    fn record_issuance(&mut self, client_id: &str) {
        let now = Utc::now();
        let entry = self.client_counts.entry(client_id.to_string()).or_insert((0, now));
        
        // Reset if window expired
        if now - entry.1 >= Duration::hours(1) {
            entry.0 = 0;
            entry.1 = now;
        }
        
        entry.0 += 1;
        self.total_issued += 1;
    }

    fn total_issued(&self) -> u64 {
        self.total_issued
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ca_creation() {
        let config = CaConfig::default();
        let ca = CertificateAuthority::new(config).await.unwrap();
        
        let ca_pem = ca.get_ca_certificate_pem().unwrap();
        assert!(!ca_pem.is_empty());
        assert!(ca_pem.contains("BEGIN CERTIFICATE"));
    }

    #[tokio::test]
    async fn test_certificate_issuance() {
        let config = CaConfig::default();
        let ca = CertificateAuthority::new(config).await.unwrap();
        
        let request = IssuanceRequest::new(
            "test-client.fleetingdns.run".to_string(),
            "test-client-123".to_string(),
        );
        
        let response = ca.issue_certificate(request).await.unwrap();
        
        assert!(!response.certificate_pem.is_empty());
        assert!(!response.ca_chain_pem.is_empty());
        assert!(response.metadata.expires_at > Utc::now());
        
        // Verify certificate is registered
        let is_valid = ca.validate_certificate(&response.metadata.serial_number).await.unwrap();
        assert!(is_valid);
    }

    #[tokio::test]
    async fn test_certificate_expiry_validation() {
        let config = CaConfig::default();
        let ca = CertificateAuthority::new(config).await.unwrap();
        
        // Request certificate with TTL exceeding maximum
        let mut request = IssuanceRequest::new(
            "test-client.fleetingdns.run".to_string(),
            "test-client-123".to_string(),
        );
        request.ttl = Some(Duration::hours(5)); // Exceeds 2 hour maximum
        
        let result = ca.issue_certificate(request).await;
        assert!(matches!(result, Err(CaError::TtlValidation { .. })));
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let config = CaConfig::default();
        let ca = CertificateAuthority::new(config).await.unwrap();
        
        // Issue certificates up to the limit
        for i in 0..10 {
            let request = IssuanceRequest::new(
                format!("test-{i}.fleetingdns.run"),
                "test-client-123".to_string(),
            );
            
            let result = ca.issue_certificate(request).await;
            assert!(result.is_ok());
        }
        
        // Next request should be rate limited
        let request = IssuanceRequest::new(
            "test-11.fleetingdns.run".to_string(),
            "test-client-123".to_string(),
        );
        
        let result = ca.issue_certificate(request).await;
        assert!(matches!(result, Err(CaError::RateLimitExceeded { .. })));
    }

    #[tokio::test]
    async fn test_certificate_revocation() {
        let config = CaConfig::default();
        let ca = CertificateAuthority::new(config).await.unwrap();
        
        let request = IssuanceRequest::new(
            "test-client.fleetingdns.run".to_string(),
            "test-client-123".to_string(),
        );
        
        let response = ca.issue_certificate(request).await.unwrap();
        let serial = &response.metadata.serial_number;
        
        // Verify certificate is valid
        assert!(ca.validate_certificate(serial).await.unwrap());
        
        // Revoke certificate
        ca.revoke_certificate(serial).await.unwrap();
        
        // Verify certificate is no longer valid
        assert!(!ca.validate_certificate(serial).await.unwrap());
    }
} 