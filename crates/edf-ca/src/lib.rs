//! FleetingDNS Certificate Authority (edf-ca)
//!
//! This crate implements a certificate authority for issuing ephemeral certificates
//! used in FleetingDNS SSH tunnels. It provides short-lived (30 minute) certificates
//! for client authentication in the SSH-over-TLS tunnel system.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;
use uuid::Uuid;

pub mod batch_operations;
pub mod ca;
pub mod certificate;
pub mod errors;

pub use ca::CertificateAuthority;
pub use certificate::{CertificateMetadata, CertificateRequest, EphemeralCertificate};
pub use errors::CaError;

/// Default certificate validity duration (30 minutes)
pub const DEFAULT_CERT_TTL: Duration = Duration::minutes(30);

/// Maximum certificate validity duration (2 hours)
pub const MAX_CERT_TTL: Duration = Duration::hours(2);

/// Certificate authority configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaConfig {
    /// CA certificate common name
    pub ca_name: String,
    /// CA certificate organization
    pub organization: String,
    /// CA certificate organizational unit
    pub organizational_unit: String,
    /// Default certificate TTL
    pub default_ttl: Duration,
    /// Maximum certificate TTL
    pub max_ttl: Duration,
    /// CA private key file path (optional, will generate if not provided)
    pub ca_key_path: Option<String>,
    /// CA certificate file path (optional, will generate if not provided)
    pub ca_cert_path: Option<String>,
    /// Maximum certificates issued per hour per client (abuse guard).
    pub certs_per_hour_per_client: u32,
}

impl Default for CaConfig {
    fn default() -> Self {
        Self {
            ca_name: "FleetingDNS-CA".to_string(),
            organization: "FleetingDNS".to_string(),
            organizational_unit: "Tunnel Services".to_string(),
            default_ttl: DEFAULT_CERT_TTL,
            max_ttl: MAX_CERT_TTL,
            ca_key_path: None,
            ca_cert_path: None,
            certs_per_hour_per_client: 10,
        }
    }
}

/// Certificate issuance request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssuanceRequest {
    /// Unique request ID
    pub request_id: String,
    /// Certificate subject common name
    pub common_name: String,
    /// Subject alternative names
    pub subject_alt_names: Vec<String>,
    /// Certificate validity duration
    pub ttl: Option<Duration>,
    /// Developer/client identifier
    pub client_id: String,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl IssuanceRequest {
    /// Create a new certificate issuance request
    pub fn new(common_name: String, client_id: String) -> Self {
        Self {
            request_id: Uuid::new_v4().to_string(),
            common_name,
            subject_alt_names: Vec::new(),
            ttl: None,
            client_id,
            metadata: HashMap::new(),
        }
    }

    /// Add a subject alternative name
    pub fn with_san(mut self, san: String) -> Self {
        self.subject_alt_names.push(san);
        self
    }

    /// Set the certificate TTL
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Certificate issuance response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssuanceResponse {
    /// Request ID this response corresponds to
    pub request_id: String,
    /// Issued certificate in PEM format
    pub certificate_pem: String,
    /// Certificate metadata
    pub metadata: CertificateMetadata,
    /// CA certificate chain in PEM format
    pub ca_chain_pem: String,
}

/// Active certificate tracking
#[derive(Debug, Clone)]
pub struct ActiveCertificate {
    /// Certificate metadata
    pub metadata: CertificateMetadata,
    /// Certificate PEM data
    pub certificate_pem: String,
    /// Client ID that owns this certificate
    pub client_id: String,
    /// Issue timestamp
    pub issued_at: DateTime<Utc>,
}

/// Certificate registry for tracking active certificates
#[derive(Debug, Default)]
pub struct CertificateRegistry {
    /// Active certificates by serial number
    active_certs: Arc<Mutex<HashMap<String, ActiveCertificate>>>,
}

impl CertificateRegistry {
    /// Create a new certificate registry
    pub fn new() -> Self {
        Self {
            active_certs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register a new certificate
    pub async fn register_certificate(&self, cert: ActiveCertificate) {
        let serial = cert.metadata.serial_number.clone();
        self.active_certs.lock().await.insert(serial, cert);
    }

    /// Check if a certificate is active and valid
    pub async fn is_certificate_valid(&self, serial_number: &str) -> bool {
        let certs = self.active_certs.lock().await;
        if let Some(cert) = certs.get(serial_number) {
            cert.metadata.expires_at > Utc::now()
        } else {
            false
        }
    }

    /// Get certificate by serial number
    pub async fn get_certificate(&self, serial_number: &str) -> Option<ActiveCertificate> {
        self.active_certs.lock().await.get(serial_number).cloned()
    }

    /// Remove expired certificates
    pub async fn cleanup_expired(&self) -> usize {
        let mut certs = self.active_certs.lock().await;
        let now = Utc::now();
        let initial_count = certs.len();

        certs.retain(|_, cert| cert.metadata.expires_at > now);

        let removed_count = initial_count - certs.len();
        if removed_count > 0 {
            info!("Cleaned up {} expired certificates", removed_count);
        }
        removed_count
    }

    /// Get count of active certificates
    pub async fn active_count(&self) -> usize {
        self.active_certs.lock().await.len()
    }

    /// Get certificates for a specific client
    pub async fn get_client_certificates(&self, client_id: &str) -> Vec<ActiveCertificate> {
        self.active_certs
            .lock()
            .await
            .values()
            .filter(|cert| cert.client_id == client_id)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_certificate_registry() {
        let registry = CertificateRegistry::new();

        // Test empty registry
        assert_eq!(registry.active_count().await, 0);
        assert!(!registry.is_certificate_valid("nonexistent").await);

        // Create test certificate
        let metadata = CertificateMetadata {
            serial_number: "test-123".to_string(),
            subject: "CN=test".to_string(),
            issuer: "CN=FleetingDNS-CA".to_string(),
            issued_at: Utc::now(),
            expires_at: Utc::now() + Duration::minutes(30),
            fingerprint: "test-fingerprint".to_string(),
        };

        let cert = ActiveCertificate {
            metadata: metadata.clone(),
            certificate_pem: "test-pem".to_string(),
            client_id: "test-client".to_string(),
            issued_at: Utc::now(),
        };

        // Register certificate
        registry.register_certificate(cert).await;
        assert_eq!(registry.active_count().await, 1);
        assert!(registry.is_certificate_valid("test-123").await);

        // Test client certificate lookup
        let client_certs = registry.get_client_certificates("test-client").await;
        assert_eq!(client_certs.len(), 1);
        assert_eq!(client_certs[0].metadata.serial_number, "test-123");
    }

    #[tokio::test]
    async fn test_issuance_request() {
        let request =
            IssuanceRequest::new("test.example.com".to_string(), "client-123".to_string())
                .with_san("alt.example.com".to_string())
                .with_ttl(Duration::hours(1))
                .with_metadata("project".to_string(), "test-project".to_string());

        assert_eq!(request.common_name, "test.example.com");
        assert_eq!(request.client_id, "client-123");
        assert_eq!(request.subject_alt_names.len(), 1);
        assert_eq!(request.subject_alt_names[0], "alt.example.com");
        assert_eq!(request.ttl, Some(Duration::hours(1)));
        assert_eq!(
            request.metadata.get("project"),
            Some(&"test-project".to_string())
        );
    }

    #[tokio::test]
    async fn test_ca_config_default() {
        let config = CaConfig::default();
        assert_eq!(config.ca_name, "FleetingDNS-CA");
        assert_eq!(config.organization, "FleetingDNS");
        assert_eq!(config.default_ttl, DEFAULT_CERT_TTL);
        assert_eq!(config.max_ttl, MAX_CERT_TTL);
    }
}
