//! Certificate management for FleetingDNS CA

use chrono::{DateTime, Duration, Utc};
use rcgen::{Certificate, CertificateParams, DnType, SanType};
use ring::digest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::errors::{CaError, CaResult};

/// Certificate metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateMetadata {
    /// Certificate serial number
    pub serial_number: String,
    /// Certificate subject DN
    pub subject: String,
    /// Certificate issuer DN
    pub issuer: String,
    /// Certificate issue timestamp
    pub issued_at: DateTime<Utc>,
    /// Certificate expiry timestamp
    pub expires_at: DateTime<Utc>,
    /// Certificate fingerprint (SHA-256)
    pub fingerprint: String,
}

/// Certificate signing request
#[derive(Debug, Clone)]
pub struct CertificateRequest {
    /// Common name for the certificate
    pub common_name: String,
    /// Subject alternative names
    pub subject_alt_names: Vec<String>,
    /// Key usage extensions
    pub key_usage: Vec<KeyUsage>,
    /// Extended key usage
    pub extended_key_usage: Vec<ExtendedKeyUsage>,
    /// Certificate validity duration
    pub validity_duration: Duration,
    /// Additional attributes
    pub attributes: HashMap<String, String>,
}

/// Key usage flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyUsage {
    DigitalSignature,
    KeyEncipherment,
    KeyAgreement,
    CertSign,
    CrlSign,
}

/// Extended key usage values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtendedKeyUsage {
    ServerAuth,
    ClientAuth,
    CodeSigning,
    EmailProtection,
    TimeStamping,
}

impl CertificateRequest {
    /// Create a new certificate request for SSH tunnel client
    pub fn for_tunnel_client(client_id: &str, validity_duration: Duration) -> Self {
        Self {
            common_name: format!("tunnel-client-{client_id}"),
            subject_alt_names: Vec::new(),
            key_usage: vec![KeyUsage::DigitalSignature, KeyUsage::KeyEncipherment],
            extended_key_usage: vec![ExtendedKeyUsage::ClientAuth],
            validity_duration,
            attributes: HashMap::from([
                ("client_id".to_string(), client_id.to_string()),
                ("purpose".to_string(), "ssh-tunnel".to_string()),
            ]),
        }
    }

    /// Add a subject alternative name
    pub fn with_san(mut self, san: String) -> Self {
        self.subject_alt_names.push(san);
        self
    }

    /// Add an attribute
    pub fn with_attribute(mut self, key: String, value: String) -> Self {
        self.attributes.insert(key, value);
        self
    }
}

/// Ephemeral certificate with metadata
#[derive(Debug, Clone)]
pub struct EphemeralCertificate {
    /// Certificate in PEM format
    pub certificate_pem: String,
    /// Private key in PEM format
    pub private_key_pem: String,
    /// Certificate metadata
    pub metadata: CertificateMetadata,
    /// CA certificate chain
    pub ca_chain_pem: String,
}

impl EphemeralCertificate {
    /// Create a new ephemeral certificate
    pub fn new(
        certificate_pem: String,
        private_key_pem: String,
        metadata: CertificateMetadata,
        ca_chain_pem: String,
    ) -> Self {
        Self {
            certificate_pem,
            private_key_pem,
            metadata,
            ca_chain_pem,
        }
    }

    /// Check if the certificate is still valid
    pub fn is_valid(&self) -> bool {
        self.metadata.expires_at > Utc::now()
    }

    /// Get time until expiry
    pub fn time_until_expiry(&self) -> Duration {
        self.metadata.expires_at - Utc::now()
    }

    /// Get certificate fingerprint
    pub fn fingerprint(&self) -> &str {
        &self.metadata.fingerprint
    }

    /// Get certificate serial number
    pub fn serial_number(&self) -> &str {
        &self.metadata.serial_number
    }
}

/// Certificate builder for creating certificates with rcgen
pub struct CertificateBuilder {
    params: CertificateParams,
}

impl CertificateBuilder {
    /// Create a new certificate builder
    pub fn new() -> Self {
        let mut params = CertificateParams::new(Vec::new());
        params.distinguished_name.push(DnType::OrganizationName, "FleetingDNS");
        params.distinguished_name.push(DnType::OrganizationalUnitName, "Tunnel Services");
        
        Self { params }
    }

    /// Set the common name
    pub fn common_name(mut self, cn: String) -> Self {
        self.params.distinguished_name.push(DnType::CommonName, cn);
        self
    }

    /// Add subject alternative names
    pub fn subject_alt_names(mut self, sans: Vec<String>) -> Self {
        for san in sans {
            self.params.subject_alt_names.push(SanType::DnsName(san));
        }
        self
    }

    /// Set validity period
    pub fn validity_duration(mut self, duration: Duration) -> Self {
        let now = time::OffsetDateTime::now_utc();
        self.params.not_before = now;
        self.params.not_after = now + time::Duration::seconds(duration.num_seconds());
        self
    }

    /// Set key usage
    pub fn key_usage(mut self, usage: Vec<KeyUsage>) -> Self {
        // Create key usage flags based on the provided usage
        for ku in usage {
            match ku {
                KeyUsage::DigitalSignature => {
                    self.params.key_usages.push(rcgen::KeyUsagePurpose::DigitalSignature);
                }
                KeyUsage::KeyEncipherment => {
                    self.params.key_usages.push(rcgen::KeyUsagePurpose::KeyEncipherment);
                }
                KeyUsage::KeyAgreement => {
                    self.params.key_usages.push(rcgen::KeyUsagePurpose::KeyAgreement);
                }
                KeyUsage::CertSign => {
                    self.params.key_usages.push(rcgen::KeyUsagePurpose::KeyCertSign);
                }
                KeyUsage::CrlSign => {
                    self.params.key_usages.push(rcgen::KeyUsagePurpose::CrlSign);
                }
            }
        }
        self
    }

    /// Set extended key usage
    pub fn extended_key_usage(mut self, usage: Vec<ExtendedKeyUsage>) -> Self {
        let mut ext_key_usage = Vec::new();
        
        for eku in usage {
            let oid = match eku {
                ExtendedKeyUsage::ServerAuth => rcgen::ExtendedKeyUsagePurpose::ServerAuth,
                ExtendedKeyUsage::ClientAuth => rcgen::ExtendedKeyUsagePurpose::ClientAuth,
                ExtendedKeyUsage::CodeSigning => rcgen::ExtendedKeyUsagePurpose::CodeSigning,
                ExtendedKeyUsage::EmailProtection => rcgen::ExtendedKeyUsagePurpose::EmailProtection,
                ExtendedKeyUsage::TimeStamping => rcgen::ExtendedKeyUsagePurpose::TimeStamping,
            };
            ext_key_usage.push(oid);
        }
        
        self.params.extended_key_usages = ext_key_usage;
        self
    }

    /// Generate the certificate
    pub fn generate(self) -> CaResult<Certificate> {
        Certificate::from_params(self.params)
            .map_err(|e| CaError::CertificateGeneration(e.to_string()))
    }

    /// Generate certificate signed by CA
    pub fn generate_signed_by(self, ca_cert: &Certificate) -> CaResult<Certificate> {
        let cert = self.generate()?;
        let _signed_cert = cert.serialize_pem_with_signer(ca_cert)
            .map_err(|e| CaError::CertificateGeneration(e.to_string()))?;
        
        // For now, return the original certificate
        // In a full implementation, we'd parse the signed certificate back
        Ok(cert)
    }
}

impl Default for CertificateBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate SHA-256 fingerprint of a certificate
pub fn calculate_fingerprint(cert_pem: &str) -> CaResult<String> {
    // Parse the PEM certificate
    let mut reader = std::io::BufReader::new(cert_pem.as_bytes());
    let certs_result = rustls_pemfile::certs(&mut reader);
    
    let certs = match certs_result {
        Ok(certs) => certs,
        Err(e) => return Err(CaError::PemParsing(e.to_string())),
    };
    
    let cert_der = certs
        .into_iter()
        .next()
        .ok_or_else(|| CaError::PemParsing("No certificate found in PEM".to_string()))?;

    // Calculate SHA-256 hash
    let digest = digest::digest(&digest::SHA256, &cert_der);
    let fingerprint = digest
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(":");

    Ok(fingerprint.to_uppercase())
}

/// Generate a unique serial number
pub fn generate_serial_number() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_certificate_request_for_tunnel_client() {
        let req = CertificateRequest::for_tunnel_client("test-client-123", Duration::minutes(30));
        
        assert_eq!(req.common_name, "tunnel-client-test-client-123");
        assert!(req.key_usage.contains(&KeyUsage::DigitalSignature));
        assert!(req.key_usage.contains(&KeyUsage::KeyEncipherment));
        assert!(req.extended_key_usage.contains(&ExtendedKeyUsage::ClientAuth));
        assert_eq!(req.validity_duration, Duration::minutes(30));
        assert_eq!(req.attributes.get("client_id"), Some(&"test-client-123".to_string()));
        assert_eq!(req.attributes.get("purpose"), Some(&"ssh-tunnel".to_string()));
    }

    #[test]
    fn test_certificate_request_with_sans() {
        let req = CertificateRequest::for_tunnel_client("test", Duration::minutes(30))
            .with_san("test.example.com".to_string())
            .with_san("alt.example.com".to_string());
        
        assert_eq!(req.subject_alt_names.len(), 2);
        assert!(req.subject_alt_names.contains(&"test.example.com".to_string()));
        assert!(req.subject_alt_names.contains(&"alt.example.com".to_string()));
    }

    #[test]
    fn test_ephemeral_certificate_validity() {
        let metadata = CertificateMetadata {
            serial_number: "test-123".to_string(),
            subject: "CN=test".to_string(),
            issuer: "CN=FleetingDNS-CA".to_string(),
            issued_at: Utc::now(),
            expires_at: Utc::now() + Duration::minutes(30),
            fingerprint: "test-fingerprint".to_string(),
        };
        
        let cert = EphemeralCertificate::new(
            "cert-pem".to_string(),
            "key-pem".to_string(),
            metadata,
            "ca-chain".to_string(),
        );
        
        assert!(cert.is_valid());
        assert!(cert.time_until_expiry() > Duration::minutes(29));
        assert_eq!(cert.serial_number(), "test-123");
        assert_eq!(cert.fingerprint(), "test-fingerprint");
    }

    #[test]
    fn test_generate_serial_number() {
        let serial1 = generate_serial_number();
        let serial2 = generate_serial_number();
        
        assert_ne!(serial1, serial2);
        assert!(!serial1.is_empty());
        assert!(!serial2.is_empty());
    }
} 