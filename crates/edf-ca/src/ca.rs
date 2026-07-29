//! Certificate Authority implementation for FleetingDNS

use chrono::{DateTime, Duration, Utc};
use common::counter;
use rcgen::{Certificate, CertificateParams, DnType, KeyPair};
use std::path::Path;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::Mutex;
use tracing::{debug, info};

use crate::certificate::{
    calculate_fingerprint, generate_serial_number, CertificateBuilder, CertificateMetadata,
    CertificateRequest, EphemeralCertificate,
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
    /// Create a new Certificate Authority with a memory-only registry.
    ///
    /// Issued certificates are forgotten when the process exits, so
    /// [`Self::validate_certificate`] reports previously issued serials as
    /// invalid after a restart. Use [`Self::with_store`] where validation must
    /// outlive the process.
    pub async fn new(config: CaConfig) -> CaResult<Self> {
        Self::build(config, CertificateRegistry::new()).await
    }

    /// Create a Certificate Authority whose registry is persisted to Redis, so
    /// certificates issued before a restart still validate afterwards.
    pub async fn with_store(config: CaConfig, pool: common::redis::RedisPool) -> CaResult<Self> {
        Self::build(config, CertificateRegistry::with_store(pool)).await
    }

    async fn build(config: CaConfig, registry: CertificateRegistry) -> CaResult<Self> {
        let ca_certificate = Self::load_or_generate_ca_certificate(&config).await?;
        let rate_limiter = Arc::new(Mutex::new(RateLimiter::new(
            config.certs_per_hour_per_client,
        )));

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

    /// Issue an ephemeral certificate.
    ///
    /// The returned [`IssuanceResponse`] deliberately omits the private key.
    /// Callers that must hand the key to the certificate's owner (for example
    /// the API's create-tunnel path, where the client cannot use a certificate
    /// it has no key for) should call [`Self::issue_client_certificate`]
    /// instead, which returns the key alongside the certificate.
    pub async fn issue_certificate(&self, request: IssuanceRequest) -> CaResult<IssuanceResponse> {
        let request_id = request.request_id.clone();
        let ephemeral_cert = self.issue_client_certificate(request).await?;

        Ok(IssuanceResponse {
            request_id,
            certificate_pem: ephemeral_cert.certificate_pem,
            metadata: ephemeral_cert.metadata,
            ca_chain_pem: ephemeral_cert.ca_chain_pem,
        })
    }

    /// Issue an ephemeral tunnel-client certificate together with its matching
    /// private key.
    ///
    /// Identical to [`Self::issue_certificate`] except that the private key is
    /// returned rather than discarded — [`Self::issue_certificate`] is a thin
    /// wrapper over this method, so validation, rate limiting, registration and
    /// metrics cannot drift between the two.
    pub async fn issue_client_certificate(
        &self,
        request: IssuanceRequest,
    ) -> CaResult<EphemeralCertificate> {
        counter!("certificate_operations_total", "operation" => "issue", "status" => "requested")
            .increment(1);

        info!(
            request_id = %request.request_id,
            client_id = %request.client_id,
            common_name = %request.common_name,
            "Processing certificate issuance request"
        );

        // Validate request
        if let Err(e) = self.validate_request(&request).await {
            counter!("certificate_operations_total", "operation" => "issue", "status" => "validation_failed").increment(1);
            return Err(e);
        }

        // Check rate limits
        if let Err(e) = self.check_rate_limits(&request.client_id).await {
            counter!("certificate_operations_total", "operation" => "issue", "status" => "rate_limited").increment(1);
            return Err(e);
        }

        // Determine certificate TTL
        let ttl = request.ttl.unwrap_or(self.config.default_ttl);
        if ttl > self.config.max_ttl {
            counter!("certificate_operations_total", "operation" => "issue", "status" => "ttl_validation_failed").increment(1);
            return Err(CaError::ValidationError(format!(
                "TTL validation error: requested {} minutes, max allowed {} minutes",
                ttl.num_minutes(),
                self.config.max_ttl.num_minutes()
            )));
        }

        // Generate certificate
        let cert_request = CertificateRequest::for_tunnel_client(&request.client_id, ttl)
            .with_san(request.common_name.clone());

        let ephemeral_cert = match self.generate_certificate(cert_request).await {
            Ok(cert) => cert,
            Err(e) => {
                counter!("certificate_operations_total", "operation" => "issue", "status" => "generation_failed").increment(1);
                return Err(e);
            }
        };

        // Register certificate
        let active_cert = ActiveCertificate {
            metadata: ephemeral_cert.metadata.clone(),
            certificate_pem: ephemeral_cert.certificate_pem.clone(),
            client_id: request.client_id.clone(),
            issued_at: Utc::now(),
        };

        self.registry.register_certificate(active_cert).await;

        // Update rate limiter
        self.rate_limiter
            .lock()
            .await
            .record_issuance(&request.client_id);

        counter!("certificate_operations_total", "operation" => "issue", "status" => "success")
            .increment(1);

        info!(
            request_id = %request.request_id,
            serial_number = %ephemeral_cert.metadata.serial_number,
            expires_at = %ephemeral_cert.metadata.expires_at,
            "Certificate issued successfully"
        );

        Ok(ephemeral_cert)
    }

    /// Issue a server (TLS termination) certificate for an edge subdomain.
    ///
    /// The certificate carries `serverAuth` extended key usage and a DNS SAN
    /// for `fqdn` (plus any `extra_sans`), is signed by the CA, and is returned
    /// as an [`EphemeralCertificate`] so the caller receives the matching
    /// private key (unlike [`Self::issue_certificate`], whose response omits
    /// the private key). Rate limiting is keyed on `fqdn`.
    pub async fn issue_server_certificate(
        &self,
        fqdn: &str,
        extra_sans: Vec<String>,
        ttl: Option<Duration>,
    ) -> CaResult<EphemeralCertificate> {
        counter!("certificate_operations_total", "operation" => "issue_server", "status" => "requested").increment(1);

        if fqdn.is_empty() {
            counter!("certificate_operations_total", "operation" => "issue_server", "status" => "validation_failed").increment(1);
            return Err(CaError::BadRequest("FQDN cannot be empty".to_string()));
        }

        // Rate limit keyed on the FQDN.
        if let Err(e) = self.check_rate_limits(fqdn).await {
            counter!("certificate_operations_total", "operation" => "issue_server", "status" => "rate_limited").increment(1);
            return Err(e);
        }

        let ttl = ttl.unwrap_or(self.config.default_ttl);
        if ttl > self.config.max_ttl {
            counter!("certificate_operations_total", "operation" => "issue_server", "status" => "ttl_validation_failed").increment(1);
            return Err(CaError::ValidationError(format!(
                "TTL validation error: requested {} minutes, max allowed {} minutes",
                ttl.num_minutes(),
                self.config.max_ttl.num_minutes()
            )));
        }

        let mut cert_request = CertificateRequest::for_server(fqdn, ttl);
        for san in extra_sans {
            cert_request = cert_request.with_san(san);
        }

        let ephemeral_cert = match self.generate_certificate(cert_request).await {
            Ok(cert) => cert,
            Err(e) => {
                counter!("certificate_operations_total", "operation" => "issue_server", "status" => "generation_failed").increment(1);
                return Err(e);
            }
        };

        // Register and account for the issuance.
        let active_cert = ActiveCertificate {
            metadata: ephemeral_cert.metadata.clone(),
            certificate_pem: ephemeral_cert.certificate_pem.clone(),
            client_id: fqdn.to_string(),
            issued_at: Utc::now(),
        };
        self.registry.register_certificate(active_cert).await;
        self.rate_limiter.lock().await.record_issuance(fqdn);

        counter!("certificate_operations_total", "operation" => "issue_server", "status" => "success").increment(1);

        info!(
            fqdn = %fqdn,
            serial_number = %ephemeral_cert.metadata.serial_number,
            expires_at = %ephemeral_cert.metadata.expires_at,
            "Server certificate issued successfully"
        );

        Ok(ephemeral_cert)
    }

    /// Validate a certificate by serial number
    pub async fn validate_certificate(&self, serial_number: &str) -> CaResult<bool> {
        counter!("certificate_operations_total", "operation" => "validate", "status" => "requested").increment(1);

        let is_valid = self.registry.is_certificate_valid(serial_number).await;

        counter!("certificate_operations_total", "operation" => "validate", "status" => if is_valid { "success" } else { "invalid" }).increment(1);

        debug!(
            serial_number = %serial_number,
            is_valid = %is_valid,
            "Certificate validation check"
        );

        Ok(is_valid)
    }

    /// Get certificate information
    pub async fn get_certificate_info(&self, serial_number: &str) -> CaResult<ActiveCertificate> {
        counter!("certificate_operations_total", "operation" => "get_info", "status" => "requested").increment(1);

        if let Some(cert) = self.registry.get_certificate(serial_number).await {
            counter!("certificate_operations_total", "operation" => "get_info", "status" => "success").increment(1);
            Ok(cert)
        } else {
            counter!("certificate_operations_total", "operation" => "get_info", "status" => "not_found").increment(1);
            Err(CaError::NotFound(format!(
                "Certificate not found: serial {}",
                serial_number
            )))
        }
    }

    /// Revoke a certificate (mark as invalid)
    pub async fn revoke_certificate(&self, serial_number: &str) -> CaResult<()> {
        counter!("certificate_operations_total", "operation" => "revoke", "status" => "requested")
            .increment(1);

        // For ephemeral certificates, we just remove from the registry.
        // In production, this might involve CRL or OCSP.
        //
        // Go through the registry rather than its internal map: revocation must
        // also delete the persisted record, or the certificate would reappear
        // on the next lookup and validate again after a restart.
        //
        // `remove_certificate` reports only whether the entry was cached
        // locally, so consult the registry first — a certificate issued before
        // a restart lives in the store but not yet in memory.
        let known = self.registry.get_certificate(serial_number).await.is_some();
        let removed = self.registry.remove_certificate(serial_number).await;
        if known || removed {
            counter!("certificate_operations_total", "operation" => "revoke", "status" => "success").increment(1);
            info!(serial_number = %serial_number, "Certificate revoked");
            Ok(())
        } else {
            counter!("certificate_operations_total", "operation" => "revoke", "status" => "not_found").increment(1);
            Err(CaError::NotFound(format!(
                "Certificate not found: serial {}",
                serial_number
            )))
        }
    }

    /// Clean up expired certificates
    pub async fn cleanup_expired_certificates(&self) -> usize {
        counter!("certificate_operations_total", "operation" => "cleanup", "status" => "requested")
            .increment(1);

        let cleaned_count = self.registry.cleanup_expired().await;

        counter!("certificate_operations_total", "operation" => "cleanup", "status" => "success")
            .increment(1);

        cleaned_count
    }

    /// Get CA certificate in PEM format
    pub fn get_ca_certificate_pem(&self) -> CaResult<String> {
        self.ca_certificate
            .serialize_pem()
            .map_err(|e| CaError::CertificateError(format!("Certificate generation failed: {}", e)))
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
    async fn generate_certificate(
        &self,
        request: CertificateRequest,
    ) -> CaResult<EphemeralCertificate> {
        // Generate the certificate key pair (Ed25519) and bind it to the
        // certificate so the returned private key matches the certificate's
        // public key. Previously a separate key pair was generated and
        // serialized while `Certificate::from_params` created its own internal
        // key — the returned key did not correspond to the issued certificate,
        // so any TLS handshake using the pair would have failed.
        let key_pair = KeyPair::generate(&rcgen::PKCS_ED25519).map_err(|e| {
            CaError::CertificateError(format!("Certificate generation failed: {}", e))
        })?;

        // Build certificate with the explicit key pair bound.
        let cert = CertificateBuilder::new()
            .common_name(request.common_name.clone())
            .subject_alt_names(request.subject_alt_names)
            .validity_duration(request.validity_duration)
            .key_usage(request.key_usage)
            .extended_key_usage(request.extended_key_usage)
            .key_pair(key_pair)
            .generate()?;

        // Sign with CA
        let cert_pem = cert
            .serialize_pem_with_signer(&self.ca_certificate)
            .map_err(|e| {
                CaError::CertificateError(format!("Certificate generation failed: {}", e))
            })?;

        // Serialize the certificate's own key pair — guaranteed to match the
        // issued certificate.
        let private_key_pem = cert.get_key_pair().serialize_pem();

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
        params
            .distinguished_name
            .push(DnType::CommonName, config.ca_name.clone());
        params
            .distinguished_name
            .push(DnType::OrganizationName, config.organization.clone());
        params.distinguished_name.push(
            DnType::OrganizationalUnitName,
            config.organizational_unit.clone(),
        );

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
            .map_err(|e| CaError::CertificateError(format!("Certificate generation failed: {}", e)))
    }

    /// Load CA certificate from files
    async fn load_ca_certificate(_cert_path: &str, _key_path: &str) -> CaResult<Certificate> {
        // For now, just generate a new certificate
        // In production, this would load from files
        let config = CaConfig::default();
        Self::generate_ca_certificate(&config).await
    }

    /// Save CA certificate to files
    async fn save_ca_certificate(
        cert: &Certificate,
        cert_path: &str,
        key_path: &str,
    ) -> CaResult<()> {
        let cert_pem = cert.serialize_pem().map_err(|e| {
            CaError::CertificateError(format!("Certificate generation failed: {}", e))
        })?;

        let key_pem = cert.get_key_pair().serialize_pem();

        fs::write(cert_path, cert_pem).await?;

        fs::write(key_path, key_pem).await?;

        info!(cert_path = %cert_path, key_path = %key_path, "CA certificate saved");
        Ok(())
    }

    /// Validate certificate issuance request
    async fn validate_request(&self, request: &IssuanceRequest) -> CaResult<()> {
        if request.common_name.is_empty() {
            return Err(CaError::BadRequest(
                "Common name cannot be empty".to_string(),
            ));
        }

        if request.client_id.is_empty() {
            return Err(CaError::BadRequest("Client ID cannot be empty".to_string()));
        }

        // Additional validation can be added here
        Ok(())
    }

    /// Check rate limits for client
    async fn check_rate_limits(&self, client_id: &str) -> CaResult<()> {
        let rate_limiter = self.rate_limiter.lock().await;
        if rate_limiter.is_rate_limited(client_id) {
            return Err(CaError::RateLimitExceeded(format!(
                "Rate limit exceeded for client {}: {} certificates per hour",
                client_id, rate_limiter.limit
            )));
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
    fn new(limit: u32) -> Self {
        Self {
            limit,
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
        let entry = self
            .client_counts
            .entry(client_id.to_string())
            .or_insert((0, now));

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
        let is_valid = ca
            .validate_certificate(&response.metadata.serial_number)
            .await
            .unwrap();
        assert!(is_valid);
    }

    /// The private key handed back with a client certificate must be
    /// cryptographically bound to that certificate: compare the SubjectPublicKeyInfo
    /// embedded in the certificate with the public key derived from the returned
    /// private key. This rejects both historical faults — the CA returning a key
    /// generated independently of the certificate, and the API substituting a
    /// literal placeholder string for the key.
    #[tokio::test]
    async fn test_client_certificate_key_matches_certificate() {
        let ca = CertificateAuthority::new(CaConfig::default())
            .await
            .unwrap();

        let cert = ca
            .issue_client_certificate(IssuanceRequest::new(
                "tunnel.fleetingdns.run".to_string(),
                "client-123".to_string(),
            ))
            .await
            .unwrap();

        assert!(cert.private_key_pem.contains("PRIVATE KEY"));
        assert!(
            !cert.private_key_pem.contains("..."),
            "private key must not be a placeholder"
        );

        // Public key derived from the returned private key.
        let key_spki = rcgen::KeyPair::from_pem(&cert.private_key_pem)
            .expect("returned private key must parse")
            .public_key_der();

        // Public key embedded in the issued certificate.
        let (_, pem) = x509_parser::pem::parse_x509_pem(cert.certificate_pem.as_bytes())
            .expect("issued certificate must parse as PEM");
        let x509 = pem
            .parse_x509()
            .expect("issued certificate must parse as X.509");
        let cert_spki = x509.public_key().raw.to_vec();

        assert_eq!(
            cert_spki, key_spki,
            "returned private key does not correspond to the issued certificate"
        );
    }

    #[tokio::test]
    async fn test_server_certificate_issuance() {
        let config = CaConfig::default();
        let ca = CertificateAuthority::new(config).await.unwrap();

        let cert = ca
            .issue_server_certificate("app.fleetingdns.run", Vec::new(), None)
            .await
            .unwrap();

        // Real, CA-signed certificate with a matching private key returned.
        assert!(cert.certificate_pem.contains("BEGIN CERTIFICATE"));
        assert!(cert.private_key_pem.contains("PRIVATE KEY"));
        assert!(!cert.ca_chain_pem.is_empty());
        assert!(cert.metadata.expires_at > Utc::now());

        // Registered and validatable by serial.
        assert!(ca
            .validate_certificate(&cert.metadata.serial_number)
            .await
            .unwrap());
    }

    /// A memory-only CA cannot validate a certificate it issued before the
    /// process restarted. This documents the limitation that motivates
    /// [`CertificateAuthority::with_store`]: the API uses the persistent
    /// variant so tunnel health does not mark every live certificate invalid
    /// on each deploy.
    #[tokio::test]
    async fn test_memory_only_registry_forgets_across_restart() {
        let issued = {
            let ca = CertificateAuthority::new(CaConfig::default())
                .await
                .unwrap();
            let response = ca
                .issue_certificate(IssuanceRequest::new(
                    "restart.fleetingdns.run".to_string(),
                    "client-restart".to_string(),
                ))
                .await
                .unwrap();
            assert!(
                ca.validate_certificate(&response.metadata.serial_number)
                    .await
                    .unwrap(),
                "certificate must validate while the issuing CA is alive"
            );
            response.metadata.serial_number
        };

        // A fresh CA stands in for the restarted process.
        let restarted = CertificateAuthority::new(CaConfig::default())
            .await
            .unwrap();
        assert!(
            !restarted.validate_certificate(&issued).await.unwrap(),
            "a memory-only registry cannot know a certificate issued before restart"
        );
    }

    #[tokio::test]
    async fn test_server_certificate_rejects_empty_fqdn() {
        let config = CaConfig::default();
        let ca = CertificateAuthority::new(config).await.unwrap();

        let result = ca.issue_server_certificate("", Vec::new(), None).await;
        assert!(matches!(result, Err(CaError::BadRequest(_))));
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
        assert!(matches!(result, Err(CaError::ValidationError(_))));
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
        assert!(matches!(result, Err(CaError::RateLimitExceeded(_))));
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
