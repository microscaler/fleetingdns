//! Certificate Manager for EdgeHub
//!
//! This module handles the generation of ephemeral certificates for subdomains,
//! enabling TLS termination with dynamic certificate generation.

use anyhow::{Context, Result};
use chrono::{Duration, Utc};
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
    cache: Arc<Mutex<HashMap<String, CertificateInfo>>>,
}

impl CertificateManager {
    /// Create a new certificate manager
    pub fn new(config: CertificateConfig) -> Result<Self> {
        Ok(Self {
            config,
            cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Generate an ephemeral certificate for a subdomain
    pub async fn generate_certificate(&self, subdomain: &str) -> Result<CertificateInfo> {
        // Check cache first
        if let Some(cached) = self.get_cached_certificate(subdomain).await? {
            if cached.expires_at > Utc::now() {
                debug!(subdomain = %subdomain, "Using cached certificate");
                return Ok(cached);
            }
        }

        info!(subdomain = %subdomain, "Generating new ephemeral certificate");

        // TODO: Implement actual certificate generation with rcgen
        // For now, create a placeholder certificate
        let certificate_info = CertificateInfo {
            subdomain: subdomain.to_string(),
            certificate: self.generate_placeholder_certificate(subdomain),
            private_key: self.generate_placeholder_private_key(),
            expires_at: Utc::now() + self.config.validity_duration,
            serial_number: self.generate_serial_number(),
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

        let certificate_info = CertificateInfo {
            subdomain: pattern.to_string(),
            certificate: self.generate_placeholder_certificate(&wildcard_domain),
            private_key: self.generate_placeholder_private_key(),
            expires_at: Utc::now() + self.config.validity_duration,
            serial_number: self.generate_serial_number(),
        };

        info!(
            pattern = %pattern,
            expires_at = %certificate_info.expires_at,
            "Generated wildcard certificate"
        );

        Ok(certificate_info)
    }

    /// Convert certificate info to Rustls format
    pub fn to_rustls_certificate(&self, cert_info: &CertificateInfo) -> Result<(Vec<u8>, Vec<u8>)> {
        Ok((cert_info.certificate.clone(), cert_info.private_key.clone()))
    }

    /// Get cached certificate if available and not expired
    async fn get_cached_certificate(&self, subdomain: &str) -> Result<Option<CertificateInfo>> {
        let cache = self.cache.lock().await;
        if let Some(cert_info) = cache.get(subdomain) {
            if cert_info.expires_at > Utc::now() {
                return Ok(Some(cert_info.clone()));
            }
        }
        Ok(None)
    }

    /// Cache a certificate
    async fn cache_certificate(&self, subdomain: &str, cert_info: &CertificateInfo) -> Result<()> {
        let mut cache = self.cache.lock().await;
        
        // Remove expired certificates
        cache.retain(|_, cert| cert.expires_at > Utc::now());
        
        // Add new certificate
        cache.insert(subdomain.to_string(), cert_info.clone());
        
        // Enforce cache size limit
        if cache.len() > self.config.max_cache_size {
            // Remove oldest certificates
            let entries: Vec<_> = cache.iter().collect();
            let mut sorted_entries: Vec<_> = entries.into_iter().collect();
            sorted_entries.sort_by_key(|(_, cert)| cert.expires_at);
            
            let to_remove = sorted_entries.len() - self.config.max_cache_size;
            for (key, _) in sorted_entries.iter().take(to_remove) {
                cache.remove(*key);
            }
        }
        
        Ok(())
    }

    /// Generate a unique serial number
    fn generate_serial_number(&self) -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let serial: u64 = rng.gen();
        format!("{:016x}", serial)
    }

    /// Generate placeholder certificate (for testing)
    fn generate_placeholder_certificate(&self, domain: &str) -> Vec<u8> {
        format!("-----BEGIN CERTIFICATE-----\nPLACEHOLDER CERT FOR {}\n-----END CERTIFICATE-----", domain)
            .into_bytes()
    }

    /// Generate placeholder private key (for testing)
    fn generate_placeholder_private_key(&self) -> Vec<u8> {
        b"-----BEGIN PRIVATE KEY-----\nPLACEHOLDER PRIVATE KEY\n-----END PRIVATE KEY-----".to_vec()
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
        let expired_certificates = cache.values()
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
        let manager = CertificateManager::new(config).unwrap();
        
        let cert_info = manager.generate_certificate("test").await.unwrap();
        
        assert_eq!(cert_info.subdomain, "test");
        assert!(cert_info.expires_at > Utc::now());
        assert!(!cert_info.certificate.is_empty());
        assert!(!cert_info.private_key.is_empty());
        assert!(!cert_info.serial_number.is_empty());
    }

    #[tokio::test]
    async fn test_wildcard_certificate_generation() {
        let config = CertificateConfig {
            use_wildcards: true,
            ..Default::default()
        };
        let manager = CertificateManager::new(config).unwrap();
        
        let cert_info = manager.generate_wildcard_certificate("test").await.unwrap();
        
        assert_eq!(cert_info.subdomain, "test");
        assert!(cert_info.expires_at > Utc::now());
        assert!(!cert_info.certificate.is_empty());
        assert!(!cert_info.private_key.is_empty());
    }

    #[tokio::test]
    async fn test_certificate_caching() {
        let config = CertificateConfig::default();
        let manager = CertificateManager::new(config).unwrap();
        
        // Generate certificate
        let cert1 = manager.generate_certificate("test").await.unwrap();
        
        // Generate again - should use cache
        let cert2 = manager.generate_certificate("test").await.unwrap();
        
        assert_eq!(cert1.serial_number, cert2.serial_number);
    }

    #[tokio::test]
    async fn test_certificate_stats() {
        let config = CertificateConfig::default();
        let manager = CertificateManager::new(config).unwrap();
        
        // Generate a certificate
        manager.generate_certificate("test").await.unwrap();
        
        let stats = manager.get_stats().await;
        assert_eq!(stats.total_certificates, 1);
        assert_eq!(stats.expired_certificates, 0);
    }
} 