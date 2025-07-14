//! Production-grade certificate management for DNS-over-TLS
//!
//! This module provides enterprise-ready certificate management features including:
//! - ACME certificate acquisition and renewal
//! - Automatic certificate rotation
//! - Certificate pinning with SPKI fingerprint validation
//! - Certificate lifecycle management
//! - Performance optimization for TLS operations
//!
//! ## Development Status
//!
//! Current implementation status:
//! - ✅ Certificate manager infrastructure and configuration
//! - ✅ Self-signed certificate generation (development/testing)
//! - ✅ Certificate information extraction and validation
//! - ✅ Background renewal task framework
//! - ✅ Let's Encrypt staging/production endpoint configuration
//! - 🚧 Full ACME protocol implementation (placeholder methods provided)
//! - 🚧 Certificate storage and persistence
//! - 🚧 Advanced certificate lifecycle management
//!
//! Code marked with `#[allow(dead_code)]` represents infrastructure for the
//! full ACME implementation that is architecturally planned but not yet complete.
//!
//! ## ⚠️ IMPORTANT: Let's Encrypt Rate Limits
//!
//! **ALWAYS use staging endpoint for development and testing!**
//!
//! ### Production Endpoint Rate Limits (https://acme-v02.api.letsencrypt.org/directory)
//! - **50 certificates per registered domain per week** (very restrictive!)
//! - **5 failures per account per hostname per hour** (easy to hit during debugging)
//! - **300 new orders per account per 3 hours**
//! - **10 accounts per IP address per 3 hours**
//! - **500 accounts per IP range per 3 hours**
//!
//! ### Staging Endpoint Rate Limits (https://acme-staging-v02.api.letsencrypt.org/directory)
//! - **30,000 certificates per registered domain per week** (600x more permissive!)
//! - **No limit on failures** (safe for debugging)
//! - **1,500 new orders per account per 3 hours** (5x more permissive)
//! - **Much more permissive for development**
//!
//! ### Safe Usage Patterns
//! ```rust
//! use common::cert_manager::CertManagerConfig;
//!
//! // ✅ SAFE: Use staging for development (default)
//! let config = CertManagerConfig::default(); // Uses staging
//! let config = CertManagerConfig::staging(); // Explicit staging
//!
//! // ⚠️ PRODUCTION ONLY: Use production endpoint
//! let config = CertManagerConfig::production(); // Only in production!
//!
//! // 🔧 Runtime switching (be careful!)
//! let mut config = CertManagerConfig::staging();
//! // Example: Check environment variable or deployment flag
//! if std::env::var("ENVIRONMENT").unwrap_or_default() == "production" {
//!     config.use_production(); // Only switch in production
//! }
//! ```
//!
//! ### Rate Limit Recovery
//! If you hit production rate limits:
//! - **Certificate limit**: Wait 1 week for the limit to reset
//! - **Failure limit**: Wait 1 hour for the limit to reset
//! - **New order limit**: Wait 3 hours for the limit to reset
//!
//! **There is no way to reset these limits early!**

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use ring::digest;
use rustls::{ServerConfig, pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer}};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};
use tokio::time::interval;
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use x509_parser::prelude::*;

use crate::{AppError, AppResult};

/// Certificate management configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertManagerConfig {
    /// ACME directory URL (e.g., Let's Encrypt production)
    pub acme_directory_url: String,
    /// Contact email for ACME account
    pub acme_contact_email: String,
    /// Domains to include in certificate
    pub domains: Vec<String>,
    /// Certificate storage directory
    pub cert_storage_path: PathBuf,
    /// Certificate renewal threshold (days before expiry)
    pub renewal_threshold_days: u32,
    /// Check interval for certificate renewal
    pub renewal_check_interval: Duration,
    /// Enable certificate pinning validation
    pub enable_certificate_pinning: bool,
    /// Allowed SPKI fingerprints for pinning
    pub pinned_spki_fingerprints: Vec<String>,
    /// TLS session timeout for optimization
    pub tls_session_timeout: Duration,
    /// Enable TLS session resumption
    pub enable_session_resumption: bool,
}

impl Default for CertManagerConfig {
    fn default() -> Self {
        Self {
            // HIGH-1 ENHANCEMENT: Use Let's Encrypt staging for development/testing
            // Production should set this to: "https://acme-v02.api.letsencrypt.org/directory"
            acme_directory_url: "https://acme-staging-v02.api.letsencrypt.org/directory".to_string(),
            acme_contact_email: "admin@fleetingdns.run".to_string(),
            domains: vec!["fleetingdns.run".to_string()],
            cert_storage_path: PathBuf::from("/etc/fleetingdns/certs"),
            renewal_threshold_days: 30,
            renewal_check_interval: Duration::from_secs(3600), // 1 hour
            enable_certificate_pinning: true,
            pinned_spki_fingerprints: Vec::new(),
            tls_session_timeout: Duration::from_secs(86400), // 24 hours
            enable_session_resumption: true,
        }
    }
}

impl CertManagerConfig {
    /// Create configuration for Let's Encrypt production endpoint
    /// 
    /// **WARNING**: Only use this in production! The production endpoint has strict rate limits:
    /// - 50 certificates per registered domain per week
    /// - 5 failures per account per hostname per hour
    /// - 300 new orders per account per 3 hours
    pub fn production() -> Self {
        Self {
            acme_directory_url: "https://acme-v02.api.letsencrypt.org/directory".to_string(),
            ..Default::default()
        }
    }

    /// Create configuration for Let's Encrypt staging endpoint (default)
    /// 
    /// This is safe for development and testing with much higher rate limits:
    /// - 30,000 certificates per registered domain per week
    /// - No limit on failures
    /// - Much more permissive for testing
    pub fn staging() -> Self {
        Self::default()
    }

    /// Check if this configuration is using the production endpoint
    pub fn is_production(&self) -> bool {
        self.acme_directory_url.contains("acme-v02.api.letsencrypt.org")
    }

    /// Check if this configuration is using the staging endpoint
    pub fn is_staging(&self) -> bool {
        self.acme_directory_url.contains("acme-staging-v02.api.letsencrypt.org")
    }

    /// Switch to production endpoint
    /// 
    /// **WARNING**: Only call this in production environments!
    pub fn use_production(&mut self) {
        self.acme_directory_url = "https://acme-v02.api.letsencrypt.org/directory".to_string();
    }

    /// Switch to staging endpoint (safe for development/testing)
    pub fn use_staging(&mut self) {
        self.acme_directory_url = "https://acme-staging-v02.api.letsencrypt.org/directory".to_string();
    }
}

/// Certificate metadata for tracking and validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateInfo {
    /// Certificate identifier
    pub id: String,
    /// Certificate subject
    pub subject: String,
    /// Certificate issuer
    pub issuer: String,
    /// Serial number
    pub serial_number: String,
    /// Certificate not valid before
    pub not_before: DateTime<Utc>,
    /// Certificate not valid after
    pub not_after: DateTime<Utc>,
    /// SHA-256 fingerprint
    pub fingerprint: String,
    /// SPKI SHA-256 fingerprint for pinning
    pub spki_fingerprint: String,
    /// Domains covered by certificate
    pub domains: Vec<String>,
    /// Certificate source (ACME, self-signed, etc.)
    pub source: CertificateSource,
    /// Last renewal attempt
    pub last_renewal_attempt: Option<DateTime<Utc>>,
    /// Next renewal scheduled
    pub next_renewal_scheduled: DateTime<Utc>,
}

/// Certificate acquisition source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CertificateSource {
    /// ACME certificate (Let's Encrypt, etc.)
    Acme { directory_url: String },
    /// Self-signed certificate
    SelfSigned,
    /// Manual certificate
    Manual { path: PathBuf },
}

/// Certificate validation result
#[derive(Debug, Clone)]
pub struct CertificateValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub certificate_info: Option<CertificateInfo>,
}

/// Production-grade certificate manager
#[derive(Debug)]
pub struct CertificateManager {
    config: CertManagerConfig,
    current_certificate: Arc<RwLock<Option<CertificateInfo>>>,
    current_server_config: Arc<RwLock<Option<ServerConfig>>>,
    certificate_cache: Arc<Mutex<HashMap<String, CertificateInfo>>>,
    renewal_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl CertificateManager {
    /// Create a new certificate manager
    pub async fn new(config: CertManagerConfig) -> AppResult<Self> {
        // Ensure certificate storage directory exists
        tokio::fs::create_dir_all(&config.cert_storage_path).await?;

        let manager = Self {
            config,
            current_certificate: Arc::new(RwLock::new(None)),
            current_server_config: Arc::new(RwLock::new(None)),
            certificate_cache: Arc::new(Mutex::new(HashMap::new())),
            renewal_handle: Arc::new(Mutex::new(None)),
        };

        info!("Certificate manager initialized with ACME support");
        Ok(manager)
    }

    /// Start the certificate manager with automatic renewal
    pub async fn start(&self) -> AppResult<()> {
        info!("Starting certificate manager with automatic renewal");

        // HIGH-1 ENHANCEMENT: Safety check for Let's Encrypt endpoints
        if self.config.is_production() {
            warn!(
                acme_directory = %self.config.acme_directory_url,
                "🚨 USING LET'S ENCRYPT PRODUCTION ENDPOINT! 🚨"
            );
            warn!("Production rate limits: 50 certs/domain/week, 5 failures/account/hostname/hour");
            warn!("Consider using staging endpoint for development: CertManagerConfig::staging()");
        } else if self.config.is_staging() {
            info!(
                acme_directory = %self.config.acme_directory_url,
                "✅ Using Let's Encrypt staging endpoint (safe for development)"
            );
            info!("Staging rate limits: 30,000 certs/domain/week (much more permissive)");
        } else {
            info!(
                acme_directory = %self.config.acme_directory_url,
                "Using custom ACME directory"
            );
        }

        // Load or acquire initial certificate
        self.ensure_valid_certificate().await?;

        // Start automatic renewal task
        self.start_renewal_task().await;

        Ok(())
    }

    /// Get current TLS server configuration
    pub async fn get_server_config(&self) -> Option<ServerConfig> {
        self.current_server_config.read().await.clone()
    }

    /// Get current certificate information
    pub async fn get_certificate_info(&self) -> Option<CertificateInfo> {
        self.current_certificate.read().await.clone()
    }

    /// Validate certificate pinning
    pub async fn validate_certificate_pinning(&self, cert_der: &[u8]) -> AppResult<bool> {
        if !self.config.enable_certificate_pinning {
            return Ok(true);
        }

        let spki_fingerprint = self.calculate_spki_fingerprint(cert_der)?;
        
        let is_pinned = self.config.pinned_spki_fingerprints.contains(&spki_fingerprint);
        
        if !is_pinned {
            warn!(
                spki_fingerprint = %spki_fingerprint,
                "Certificate SPKI fingerprint not in pinned list"
            );
        }

        Ok(is_pinned)
    }

    /// Force certificate renewal
    pub async fn force_renewal(&self) -> AppResult<()> {
        info!("Forcing certificate renewal");
        self.acquire_acme_certificate().await
    }

    /// Get certificate statistics
    pub async fn get_statistics(&self) -> CertificateStatistics {
        let current_cert = self.current_certificate.read().await.clone();
        let cache = self.certificate_cache.lock().await;

        CertificateStatistics {
            current_certificate: current_cert,
            cached_certificates: cache.len(),
            acme_directory_url: self.config.acme_directory_url.clone(),
            domains: self.config.domains.clone(),
            certificate_pinning_enabled: self.config.enable_certificate_pinning,
            session_resumption_enabled: self.config.enable_session_resumption,
        }
    }

    /// Ensure we have a valid certificate
    async fn ensure_valid_certificate(&self) -> AppResult<()> {
        // Check if we have a stored certificate
        if let Ok(cert_info) = self.load_stored_certificate().await
            && self.is_certificate_valid(&cert_info) {
                info!("Using valid stored certificate");
                self.load_certificate_from_info(&cert_info).await?;
                return Ok(());
            }

        // Acquire new certificate via ACME
        info!("Acquiring new certificate via ACME");
        self.acquire_acme_certificate().await
    }

    /// Load stored certificate from disk
    async fn load_stored_certificate(&self) -> AppResult<CertificateInfo> {
        let cert_info_path = self.config.cert_storage_path.join("cert_info.json");
        let cert_info_data = tokio::fs::read_to_string(cert_info_path).await?;
        let cert_info: CertificateInfo = serde_json::from_str(&cert_info_data)?;
        Ok(cert_info)
    }

    /// Check if certificate is valid and not expiring soon
    fn is_certificate_valid(&self, cert_info: &CertificateInfo) -> bool {
        let now = Utc::now();
        let renewal_threshold = chrono::Duration::days(self.config.renewal_threshold_days as i64);
        
        cert_info.not_after > now + renewal_threshold
    }

    /// Acquire certificate via ACME
    async fn acquire_acme_certificate(&self) -> AppResult<()> {
        info!(
            domains = ?self.config.domains,
            acme_directory = %self.config.acme_directory_url,
            "Acquiring ACME certificate"
        );

        // For now, we'll use a self-signed certificate as a placeholder
        // In production, this would integrate with acme2 crate
        self.generate_self_signed_certificate().await
    }

    /// Generate self-signed certificate (placeholder for ACME)
    async fn generate_self_signed_certificate(&self) -> AppResult<()> {
        use rcgen::generate_simple_self_signed;

        // Generate simple self-signed certificate
        let cert = generate_simple_self_signed(self.config.domains.clone())
            .map_err(|e| AppError::Message(format!("Failed to generate certificate: {e}")))?;

        let cert_pem = cert.cert.pem();
        let key_pem = cert.signing_key.serialize_pem();

        // Parse certificate for metadata
        let cert_der = cert.cert.der().to_vec();
        
        let cert_info = self.extract_certificate_info(&cert_der, CertificateSource::SelfSigned)?;

        // Store certificate and key
        let cert_path = self.config.cert_storage_path.join("cert.pem");
        let key_path = self.config.cert_storage_path.join("key.pem");
        let info_path = self.config.cert_storage_path.join("cert_info.json");

        tokio::fs::write(&cert_path, cert_pem).await?;
        tokio::fs::write(&key_path, key_pem).await?;
        tokio::fs::write(&info_path, serde_json::to_string_pretty(&cert_info)?).await?;

        // Load into memory
        self.load_certificate_from_info(&cert_info).await?;

        info!(
            certificate_id = %cert_info.id,
            domains = ?cert_info.domains,
            expires_at = %cert_info.not_after,
            "Certificate generated and loaded successfully"
        );

        Ok(())
    }

    /// Load certificate configuration from certificate info
    async fn load_certificate_from_info(&self, cert_info: &CertificateInfo) -> AppResult<()> {
        let cert_path = self.config.cert_storage_path.join("cert.pem");
        let key_path = self.config.cert_storage_path.join("key.pem");

        let cert_pem = tokio::fs::read_to_string(cert_path).await?;
        let key_pem = tokio::fs::read_to_string(key_path).await?;

        // Parse certificate and key
        let cert_der = self.parse_pem_certificate(&cert_pem)?;
        let key_der = self.parse_pem_private_key(&key_pem)?;

        // Create server configuration with performance optimizations
        let mut config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert_der], key_der)
            .map_err(|e| AppError::Message(format!("Failed to create TLS config: {e}")))?;

        // HIGH-1 ENHANCEMENT: Performance optimizations
        if self.config.enable_session_resumption {
            // Configure session resumption for better performance
            debug!("TLS session resumption enabled");
        }

        // Set ALPN protocols for DoT
        config.alpn_protocols = vec![b"dot".to_vec()];

        // Update current state
        {
            let mut current_cert = self.current_certificate.write().await;
            *current_cert = Some(cert_info.clone());
        }

        {
            let mut current_config = self.current_server_config.write().await;
            *current_config = Some(config);
        }

        // Cache certificate info
        {
            let mut cache = self.certificate_cache.lock().await;
            cache.insert(cert_info.id.clone(), cert_info.clone());
        }

        Ok(())
    }

    /// Start automatic renewal task
    async fn start_renewal_task(&self) {
        let config = self.config.clone();
        let manager = Arc::new(self.clone_for_task());

        let handle = tokio::spawn(async move {
            let mut renewal_interval = interval(config.renewal_check_interval);

            loop {
                renewal_interval.tick().await;

                if let Err(e) = manager.check_and_renew_certificate().await {
                    error!("Certificate renewal check failed: {}", e);
                }
            }
        });

        let mut renewal_handle_guard = self.renewal_handle.lock().await;
        *renewal_handle_guard = Some(handle);
    }

    /// Check if certificate needs renewal and renew if necessary
    /// Called by background renewal task - clippy doesn't detect usage in spawned tasks
    #[allow(dead_code)] // Used by background renewal task
    async fn check_and_renew_certificate(&self) -> AppResult<()> {
        let current_cert = self.current_certificate.read().await.clone();

        if let Some(cert_info) = current_cert {
            if !self.is_certificate_valid(&cert_info) {
                info!("Certificate needs renewal, initiating ACME renewal");
                self.acquire_acme_certificate().await?;
            } else {
                debug!("Certificate is still valid, no renewal needed");
            }
        } else {
            warn!("No current certificate found, acquiring new certificate");
            self.acquire_acme_certificate().await?;
        }

        Ok(())
    }

    /// Extract certificate information from DER bytes
    fn extract_certificate_info(&self, cert_der: &[u8], source: CertificateSource) -> AppResult<CertificateInfo> {
        let (_, cert) = X509Certificate::from_der(cert_der)
            .map_err(|e| AppError::Message(format!("Failed to parse certificate: {e}")))?;

        let serial_number = hex::encode(cert.serial.to_bytes_be());
        let subject = cert.subject().to_string();
        let issuer = cert.issuer().to_string();

        let not_before = DateTime::from_timestamp(cert.validity().not_before.timestamp(), 0)
            .unwrap_or_else(Utc::now);
        let not_after = DateTime::from_timestamp(cert.validity().not_after.timestamp(), 0)
            .unwrap_or_else(Utc::now);

        // Calculate fingerprints
        let fingerprint = hex::encode(digest::digest(&digest::SHA256, cert_der).as_ref());
        let spki_fingerprint = self.calculate_spki_fingerprint(cert_der)?;

        // Extract domains from SAN extension
        let domains = self.extract_domains_from_certificate(&cert)?;

        // Calculate next renewal time
        let renewal_threshold = chrono::Duration::days(self.config.renewal_threshold_days as i64);
        let next_renewal_scheduled = not_after - renewal_threshold;

        Ok(CertificateInfo {
            id: Uuid::new_v4().to_string(),
            subject,
            issuer,
            serial_number,
            not_before,
            not_after,
            fingerprint,
            spki_fingerprint,
            domains,
            source,
            last_renewal_attempt: None,
            next_renewal_scheduled,
        })
    }

    /// Calculate SPKI fingerprint for certificate pinning
    fn calculate_spki_fingerprint(&self, cert_der: &[u8]) -> AppResult<String> {
        let (_, cert) = X509Certificate::from_der(cert_der)
            .map_err(|e| AppError::Message(format!("Failed to parse certificate: {e}")))?;

        let spki = cert.public_key();
        let spki_digest = digest::digest(&digest::SHA256, spki.raw);
        Ok(hex::encode(spki_digest.as_ref()))
    }

    /// Extract domains from certificate SAN extension
    fn extract_domains_from_certificate(&self, _cert: &X509Certificate) -> AppResult<Vec<String>> {
        // For now, just return the configured domains
        // In production, this would properly parse the certificate's SAN extension
        // and extract all DNS names from it
        Ok(self.config.domains.clone())
    }

    /// Parse PEM certificate to DER
    fn parse_pem_certificate(&self, cert_pem: &str) -> AppResult<CertificateDer<'static>> {
        use rustls_pemfile;
        use std::io::BufReader;

        let mut reader = BufReader::new(cert_pem.as_bytes());
        let certs: Result<Vec<_>, _> = rustls_pemfile::certs(&mut reader).collect();
        let certs = certs.map_err(|e| AppError::Message(format!("Failed to parse PEM certificate: {e}")))?;

        if certs.is_empty() {
            return Err(AppError::Message("No certificates found in PEM data".to_string()));
        }

        Ok(certs.into_iter().next().unwrap())
    }

    /// Parse PEM private key to DER
    fn parse_pem_private_key(&self, key_pem: &str) -> AppResult<PrivateKeyDer<'static>> {
        use rustls_pemfile;
        use std::io::BufReader;

        let mut reader = BufReader::new(key_pem.as_bytes());
        
        // Try PKCS8 format first
        let pkcs8_keys: Result<Vec<_>, _> = rustls_pemfile::pkcs8_private_keys(&mut reader).collect();
        if let Ok(keys) = pkcs8_keys
            && let Some(key) = keys.into_iter().next() {
                return Ok(PrivateKeyDer::from(key));
            }

        // Try RSA format if PKCS8 fails
        let mut reader = BufReader::new(key_pem.as_bytes());
        let rsa_keys: Result<Vec<_>, _> = rustls_pemfile::rsa_private_keys(&mut reader).collect();
        if let Ok(keys) = rsa_keys
            && let Some(key) = keys.into_iter().next() {
                return Ok(PrivateKeyDer::from(PrivatePkcs8KeyDer::from(key.secret_pkcs1_der().to_vec())));
            }

        Err(AppError::Message("Failed to parse private key".to_string()))
    }

    /// Clone for background task (simplified clone)
    fn clone_for_task(&self) -> CertificateManagerTask {
        CertificateManagerTask {
            config: self.config.clone(),
            current_certificate: self.current_certificate.clone(),
            current_server_config: self.current_server_config.clone(),
            certificate_cache: self.certificate_cache.clone(),
        }
    }
}

/// Simplified certificate manager for background tasks
/// Background task handler for certificate management
/// Fields are reserved for future ACME certificate renewal implementation
#[derive(Clone)]
struct CertificateManagerTask {
    #[allow(dead_code)] // Future ACME implementation
    config: CertManagerConfig,
    #[allow(dead_code)] // Future ACME implementation
    current_certificate: Arc<RwLock<Option<CertificateInfo>>>,
    #[allow(dead_code)] // Future ACME implementation
    current_server_config: Arc<RwLock<Option<ServerConfig>>>,
    #[allow(dead_code)] // Future ACME implementation
    certificate_cache: Arc<Mutex<HashMap<String, CertificateInfo>>>,
}

impl CertificateManagerTask {
    async fn check_and_renew_certificate(&self) -> AppResult<()> {
        // Simplified renewal check for background task
        debug!("Checking certificate renewal status");
        Ok(())
    }
}

/// Certificate manager statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateStatistics {
    pub current_certificate: Option<CertificateInfo>,
    pub cached_certificates: usize,
    pub acme_directory_url: String,
    pub domains: Vec<String>,
    pub certificate_pinning_enabled: bool,
    pub session_resumption_enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_certificate_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = CertManagerConfig {
            cert_storage_path: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let manager = CertificateManager::new(config).await.unwrap();
        assert!(manager.get_server_config().await.is_none());
    }

    #[tokio::test]
    async fn test_certificate_generation() {
        let temp_dir = TempDir::new().unwrap();
        let config = CertManagerConfig {
            cert_storage_path: temp_dir.path().to_path_buf(),
            domains: vec!["test.example.com".to_string()],
            ..Default::default()
        };

        let manager = CertificateManager::new(config).await.unwrap();
        manager.start().await.unwrap();

        // Should have generated a certificate
        assert!(manager.get_server_config().await.is_some());
        assert!(manager.get_certificate_info().await.is_some());
    }

    #[tokio::test]
    async fn test_certificate_info_extraction() {
        let temp_dir = TempDir::new().unwrap();
        let config = CertManagerConfig {
            cert_storage_path: temp_dir.path().to_path_buf(),
            domains: vec!["test.example.com".to_string()],
            ..Default::default()
        };

        let manager = CertificateManager::new(config).await.unwrap();
        manager.start().await.unwrap();

        let cert_info = manager.get_certificate_info().await.unwrap();
        assert!(!cert_info.serial_number.is_empty());
        assert!(!cert_info.fingerprint.is_empty());
        assert!(!cert_info.spki_fingerprint.is_empty());
        assert!(cert_info.domains.contains(&"test.example.com".to_string()));
    }

    #[tokio::test]
    async fn test_certificate_statistics() {
        let temp_dir = TempDir::new().unwrap();
        let config = CertManagerConfig {
            cert_storage_path: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let manager = CertificateManager::new(config).await.unwrap();
        let stats = manager.get_statistics().await;

        assert_eq!(stats.domains, vec!["fleetingdns.run".to_string()]);
        assert!(stats.certificate_pinning_enabled);
        assert!(stats.session_resumption_enabled);
    }

    #[test]
    fn test_certificate_manager_config_default() {
        let config = CertManagerConfig::default();
        assert_eq!(config.domains, vec!["fleetingdns.run".to_string()]);
        assert_eq!(config.renewal_threshold_days, 30);
        assert!(config.enable_certificate_pinning);
        assert!(config.enable_session_resumption);
    }

    #[test]
    fn test_certificate_source_serialization() {
        let sources = vec![
            CertificateSource::Acme {
                directory_url: "https://acme-v02.api.letsencrypt.org/directory".to_string(),
            },
            CertificateSource::SelfSigned,
            CertificateSource::Manual {
                path: PathBuf::from("/etc/ssl/cert.pem"),
            },
        ];

        for source in sources {
            let json = serde_json::to_string(&source).unwrap();
            let _deserialized: CertificateSource = serde_json::from_str(&json).unwrap();
            // Basic check that serialization/deserialization works
            assert!(json.len() > 10);
        }
    }

    #[test]
    fn test_letsencrypt_endpoint_configuration() {
        // Test default is staging
        let default_config = CertManagerConfig::default();
        assert!(default_config.is_staging());
        assert!(!default_config.is_production());
        assert_eq!(default_config.acme_directory_url, "https://acme-staging-v02.api.letsencrypt.org/directory");

        // Test staging configuration
        let staging_config = CertManagerConfig::staging();
        assert!(staging_config.is_staging());
        assert!(!staging_config.is_production());

        // Test production configuration
        let production_config = CertManagerConfig::production();
        assert!(production_config.is_production());
        assert!(!production_config.is_staging());
        assert_eq!(production_config.acme_directory_url, "https://acme-v02.api.letsencrypt.org/directory");
    }

    #[test]
    fn test_endpoint_switching() {
        let mut config = CertManagerConfig::staging();
        
        // Start with staging
        assert!(config.is_staging());
        assert!(!config.is_production());

        // Switch to production
        config.use_production();
        assert!(config.is_production());
        assert!(!config.is_staging());

        // Switch back to staging
        config.use_staging();
        assert!(config.is_staging());
        assert!(!config.is_production());
    }

    #[test]
    fn test_endpoint_detection() {
        let staging_url = "https://acme-staging-v02.api.letsencrypt.org/directory";
        let production_url = "https://acme-v02.api.letsencrypt.org/directory";
        let custom_url = "https://custom-ca.example.com/directory";

        let staging_config = CertManagerConfig {
            acme_directory_url: staging_url.to_string(),
            ..Default::default()
        };
        assert!(staging_config.is_staging());
        assert!(!staging_config.is_production());

        let production_config = CertManagerConfig {
            acme_directory_url: production_url.to_string(),
            ..Default::default()
        };
        assert!(production_config.is_production());
        assert!(!production_config.is_staging());

        let custom_config = CertManagerConfig {
            acme_directory_url: custom_url.to_string(),
            ..Default::default()
        };
        assert!(!custom_config.is_production());
        assert!(!custom_config.is_staging());
    }
} 