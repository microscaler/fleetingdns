//! Certificate Manager for EdgeHub
//!
//! Issues short-lived server certificates for edge subdomains, enabling TLS
//! termination with dynamically generated, CA-signed certificates. Generation
//! is delegated to the FleetingDNS certificate authority ([`edf_ca`]): every
//! certificate carries `serverAuth` EKU and a DNS SAN for its subdomain, is
//! signed by the shared CA, and is returned with the matching private key.

use anyhow::{Result, anyhow, bail};
use chrono::{Duration, Utc};
use edf_ca::{CaConfig, CertificateAuthority};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info};

/// Certificate metadata
#[derive(Debug, Clone)]
pub struct CertificateInfo {
    pub subdomain: String,
    pub certificate: Vec<u8>,
    pub private_key: Vec<u8>,
    pub expires_at: chrono::DateTime<Utc>,
    pub serial_number: String,
}

/// Configuration for certificate generation
#[derive(Debug, Clone)]
pub struct CertificateConfig {
    /// Root domain for certificates
    pub root_domain: String,
    /// Certificate validity duration
    pub validity_duration: Duration,
    /// Maximum number of certificates to cache
    pub max_cache_size: usize,
    /// Whether to use wildcard certificates
    pub use_wildcards: bool,
}

impl Default for CertificateConfig {
    fn default() -> Self {
        Self {
            root_domain: "fleetingdns.run".to_string(),
            validity_duration: Duration::hours(1), // Short-lived for security
            max_cache_size: 1000,
            use_wildcards: false, // Individual certificates for better security
        }
    }
}

/// Certificate manager for generating ephemeral certificates
pub struct CertificateManager {
    config: CertificateConfig,
    ca: Arc<CertificateAuthority>,
    cache: Arc<Mutex<HashMap<String, CertificateInfo>>>,
}

impl CertificateManager {
    /// Create a new certificate manager backed by a freshly initialised CA.
    ///
    /// The CA's issuance TTL cap is aligned with the configured validity
    /// duration so short-lived edge certificates are never rejected.
    pub async fn new(config: CertificateConfig) -> Result<Self> {
        let ca_config = CaConfig {
            default_ttl: config.validity_duration,
            max_ttl: config.validity_duration.max(CaConfig::default().max_ttl),
            ..CaConfig::default()
        };
        let ca = CertificateAuthority::new(ca_config)
            .await
            .map_err(|e| anyhow!("failed to initialise certificate authority: {e}"))?;
        Ok(Self::with_ca(config, Arc::new(ca)))
    }

    /// Create a certificate manager over an existing certificate authority.
    ///
    /// Useful when several components share one CA (and therefore one trust
    /// root), or for injecting a pre-seeded CA in tests.
    pub fn with_ca(config: CertificateConfig, ca: Arc<CertificateAuthority>) -> Self {
        Self {
            config,
            ca,
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Generate an ephemeral certificate for a subdomain
    pub async fn generate_certificate(&self, subdomain: &str) -> Result<CertificateInfo> {
        // Check cache first
        if let Some(cached) = self.get_cached_certificate(subdomain).await?
            && cached.expires_at > Utc::now()
        {
            debug!(subdomain = %subdomain, "Using cached certificate");
            return Ok(cached);
        }

        info!(subdomain = %subdomain, "Generating new ephemeral certificate");

        // Full hostname the certificate must be valid for.
        let fqdn = format!("{}.{}", subdomain, self.config.root_domain);

        // Delegate to the CA: a real, CA-signed serverAuth certificate whose
        // returned private key matches the certificate's public key.
        let ephemeral = self
            .ca
            .issue_server_certificate(&fqdn, Vec::new(), Some(self.config.validity_duration))
            .await
            .map_err(|e| anyhow!("CA failed to issue certificate for {fqdn}: {e}"))?;

        let certificate_info = CertificateInfo {
            subdomain: subdomain.to_string(),
            certificate: ephemeral.certificate_pem.into_bytes(),
            private_key: ephemeral.private_key_pem.into_bytes(),
            expires_at: ephemeral.metadata.expires_at,
            serial_number: ephemeral.metadata.serial_number,
        };

        // Cache the certificate
        self.cache_certificate(subdomain, &certificate_info).await?;

        info!(
            subdomain = %subdomain,
            expires_at = %certificate_info.expires_at,
            "Generated ephemeral certificate"
        );

        Ok(certificate_info)
    }

    /// Generate a wildcard certificate for a subdomain pattern
    pub async fn generate_wildcard_certificate(&self, pattern: &str) -> Result<CertificateInfo> {
        let wildcard_domain = format!("*.{}.{}", pattern, self.config.root_domain);

        info!(pattern = %pattern, wildcard = %wildcard_domain, "Generating wildcard certificate");

        let ephemeral = self
            .ca
            .issue_server_certificate(
                &wildcard_domain,
                Vec::new(),
                Some(self.config.validity_duration),
            )
            .await
            .map_err(|e| {
                anyhow!("CA failed to issue wildcard certificate for {wildcard_domain}: {e}")
            })?;

        let certificate_info = CertificateInfo {
            subdomain: pattern.to_string(),
            certificate: ephemeral.certificate_pem.into_bytes(),
            private_key: ephemeral.private_key_pem.into_bytes(),
            expires_at: ephemeral.metadata.expires_at,
            serial_number: ephemeral.metadata.serial_number,
        };

        // Cache the certificate
        self.cache_certificate(pattern, &certificate_info).await?;

        Ok(certificate_info)
    }

    /// Parse the stored PEM certificate and key into rustls DER types, ready to
    /// build a [`rustls::ServerConfig`]. Returns the certificate chain and the
    /// matching private key.
    pub fn to_rustls_certificate(
        &self,
        cert_info: &CertificateInfo,
    ) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)> {
        let mut cert_reader = std::io::BufReader::new(cert_info.certificate.as_slice());
        let certs = rustls_pemfile::certs(&mut cert_reader)
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| anyhow!("certificate PEM parse error: {e}"))?;
        if certs.is_empty() {
            bail!("no certificate found in PEM for {}", cert_info.subdomain);
        }

        let mut key_reader = std::io::BufReader::new(cert_info.private_key.as_slice());
        let key = rustls_pemfile::private_key(&mut key_reader)
            .map_err(|e| anyhow!("private key PEM parse error: {e}"))?
            .ok_or_else(|| anyhow!("no private key found in PEM for {}", cert_info.subdomain))?;

        Ok((certs, key))
    }

    /// Get cached certificate
    async fn get_cached_certificate(&self, subdomain: &str) -> Result<Option<CertificateInfo>> {
        let cache = self.cache.lock().await;
        Ok(cache.get(subdomain).cloned())
    }

    /// Cache a certificate
    async fn cache_certificate(&self, subdomain: &str, cert_info: &CertificateInfo) -> Result<()> {
        let mut cache = self.cache.lock().await;
        cache.insert(subdomain.to_string(), cert_info.clone());
        Ok(())
    }

    /// Clean up expired certificates
    pub async fn cleanup_expired_certificates(&self) -> Result<usize> {
        let mut cache = self.cache.lock().await;
        let initial_size = cache.len();

        cache.retain(|_, cert| cert.expires_at > Utc::now());

        let removed = initial_size - cache.len();
        if removed > 0 {
            info!(removed = %removed, "Cleaned up expired certificates");
        }

        Ok(removed)
    }

    /// Get certificate statistics
    pub async fn get_stats(&self) -> CertificateStats {
        let cache = self.cache.lock().await;
        let total_certificates = cache.len();
        let expired_certificates = cache
            .values()
            .filter(|cert| cert.expires_at <= Utc::now())
            .count();

        CertificateStats {
            total_certificates,
            expired_certificates,
            cache_size: self.config.max_cache_size,
        }
    }
}

/// Certificate statistics
#[derive(Debug, Clone)]
pub struct CertificateStats {
    pub total_certificates: usize,
    pub expired_certificates: usize,
    pub cache_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_certificate_config_default() {
        let config = CertificateConfig::default();
        assert_eq!(config.root_domain, "fleetingdns.run");
        assert_eq!(config.validity_duration, Duration::hours(1));
        assert_eq!(config.max_cache_size, 1000);
        assert!(!config.use_wildcards);
    }

    #[tokio::test]
    async fn test_certificate_generation() {
        let config = CertificateConfig::default();
        let manager = CertificateManager::new(config).await.unwrap();

        let cert_info = manager.generate_certificate("test").await.unwrap();

        assert_eq!(cert_info.subdomain, "test");
        assert!(cert_info.expires_at > Utc::now());
        assert!(!cert_info.serial_number.is_empty());

        // The certificate must be a real, parseable X.509 certificate (not the
        // former placeholder text).
        let cert_pem = String::from_utf8(cert_info.certificate.clone()).unwrap();
        assert!(cert_pem.contains("BEGIN CERTIFICATE"));
        assert!(!cert_pem.contains("PLACEHOLDER"));
        let key_pem = String::from_utf8(cert_info.private_key.clone()).unwrap();
        assert!(key_pem.contains("PRIVATE KEY"));
        assert!(!key_pem.contains("PLACEHOLDER"));

        // And it must parse into rustls DER types.
        let (certs, _key) = manager.to_rustls_certificate(&cert_info).unwrap();
        assert_eq!(certs.len(), 1);
    }

    /// The strongest guarantee: the issued certificate and its private key
    /// actually correspond, i.e. they form a usable TLS server identity. This
    /// is what the earlier placeholder (and the CA's former key/cert mismatch)
    /// would have failed.
    #[tokio::test]
    async fn test_issued_key_matches_certificate() {
        let config = CertificateConfig::default();
        let manager = CertificateManager::new(config).await.unwrap();

        let cert_info = manager.generate_certificate("match-test").await.unwrap();
        let (certs, key) = manager.to_rustls_certificate(&cert_info).unwrap();

        let provider = rustls::crypto::aws_lc_rs::default_provider();
        let signing_key = provider
            .key_provider
            .load_private_key(key)
            .expect("private key must load");
        let certified = rustls::sign::CertifiedKey::new(certs, signing_key);
        certified
            .keys_match()
            .expect("private key must match the issued certificate");
    }

    #[tokio::test]
    async fn test_wildcard_certificate_generation() {
        let config = CertificateConfig::default();
        let manager = CertificateManager::new(config).await.unwrap();

        let cert_info = manager.generate_wildcard_certificate("test").await.unwrap();

        assert_eq!(cert_info.subdomain, "test");
        assert!(cert_info.expires_at > Utc::now());
        let cert_pem = String::from_utf8(cert_info.certificate.clone()).unwrap();
        assert!(cert_pem.contains("BEGIN CERTIFICATE"));
        assert!(!cert_pem.contains("PLACEHOLDER"));
        // The wildcard cert+key must also form a valid identity.
        let (certs, key) = manager.to_rustls_certificate(&cert_info).unwrap();
        let provider = rustls::crypto::aws_lc_rs::default_provider();
        let signing_key = provider.key_provider.load_private_key(key).unwrap();
        rustls::sign::CertifiedKey::new(certs, signing_key)
            .keys_match()
            .expect("wildcard key must match certificate");
    }

    #[tokio::test]
    async fn test_certificate_caching() {
        let config = CertificateConfig::default();
        let manager = CertificateManager::new(config).await.unwrap();

        // Generate certificate
        let _cert_info = manager.generate_certificate("test-cache").await.unwrap();

        // Check that it's cached
        let cached = manager.get_cached_certificate("test-cache").await.unwrap();
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().subdomain, "test-cache");
    }

    #[tokio::test]
    async fn test_certificate_stats() {
        let config = CertificateConfig::default();
        let manager = CertificateManager::new(config).await.unwrap();

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_certificates, 0);
        assert_eq!(stats.expired_certificates, 0);
        assert_eq!(stats.cache_size, 1000);
    }
}
