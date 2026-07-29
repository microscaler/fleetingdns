//! Wildcard certificate manager for EdgeHub.
//!
//! FR-EDGE-1: the HTTPS router presents a **single wildcard** certificate for
//! `*.{root_domain}`, so every ephemeral tunnel subdomain validates against one
//! certificate. Per-subdomain certificates are deliberately NOT issued here —
//! they would publish every live tunnel FQDN to Certificate Transparency logs,
//! defeating the unguessable-link security model.
//!
//! This manager owns that one certificate: it issues it from the FleetingDNS
//! certificate authority ([`edf_ca`]), caches it, and re-issues it before
//! expiry so short-lived certificates rotate without restarting the router.
//!
//! Relationship to [`common::tls`]: `generate_wildcard_tls_config` produces an
//! equivalent *self-signed* wildcard with no rotation, and
//! `load_tls_config_from_files` is the production path (a real wildcard cert
//! mounted as a k8s secret). This manager sits between them — CA-signed and
//! self-renewing — for environments that trust the FleetingDNS CA.

use anyhow::{Result, anyhow, bail};
use chrono::{DateTime, Duration, Utc};
use edf_ca::{CaConfig, CertificateAuthority};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info};

/// An issued wildcard certificate and its matching private key.
#[derive(Debug, Clone)]
pub struct CertificateInfo {
    /// Every DNS name the certificate is valid for.
    pub subject_alt_names: Vec<String>,
    /// Certificate chain, PEM encoded.
    pub certificate: Vec<u8>,
    /// Private key corresponding to `certificate`, PEM encoded.
    pub private_key: Vec<u8>,
    pub expires_at: DateTime<Utc>,
    pub serial_number: String,
}

/// Configuration for wildcard certificate issuance.
#[derive(Debug, Clone)]
pub struct CertificateConfig {
    /// Root domain; the certificate covers `*.{root_domain}` and the apex.
    pub root_domain: String,
    /// How long each issued certificate is valid for.
    pub validity_duration: Duration,
    /// Re-issue once the certificate has less than this left before expiry.
    /// Should be comfortably larger than the interval at which the router asks
    /// for the certificate, so rotation never races an expiry.
    pub renew_before: Duration,
    /// Include `localhost` as a SAN (convenient for local development).
    pub include_localhost: bool,
}

impl Default for CertificateConfig {
    fn default() -> Self {
        Self {
            root_domain: "fleetingdns.run".to_string(),
            validity_duration: Duration::hours(1), // Short-lived for security
            renew_before: Duration::minutes(10),
            include_localhost: true,
        }
    }
}

/// Issues, caches and rotates the router's wildcard certificate.
pub struct CertificateManager {
    config: CertificateConfig,
    ca: Arc<CertificateAuthority>,
    /// The current certificate. `None` until first issuance.
    cached: Arc<Mutex<Option<CertificateInfo>>>,
}

impl CertificateManager {
    /// Create a manager backed by a freshly initialised CA.
    pub async fn new(config: CertificateConfig) -> Result<Self> {
        let defaults = CaConfig::default();
        let ca_config = CaConfig {
            default_ttl: config.validity_duration,
            // Never let the CA's cap reject our own configured validity.
            max_ttl: config.validity_duration.max(defaults.max_ttl),
            ..defaults
        };
        let ca = CertificateAuthority::new(ca_config)
            .await
            .map_err(|e| anyhow!("failed to initialise certificate authority: {e}"))?;
        Ok(Self::with_ca(config, Arc::new(ca)))
    }

    /// Create a manager over an existing certificate authority, so several
    /// components can share one trust root.
    pub fn with_ca(config: CertificateConfig, ca: Arc<CertificateAuthority>) -> Self {
        Self {
            config,
            ca,
            cached: Arc::new(Mutex::new(None)),
        }
    }

    /// The DNS names the wildcard certificate covers: `*.{root}`, the apex, and
    /// optionally `localhost`.
    pub fn subject_alt_names(&self) -> Vec<String> {
        let mut sans = vec![
            format!("*.{}", self.config.root_domain),
            self.config.root_domain.clone(),
        ];
        if self.config.include_localhost {
            sans.push("localhost".to_string());
        }
        sans
    }

    /// Get the current wildcard certificate, issuing or renewing as needed.
    ///
    /// Callers may invoke this per connection: it is a cache read in the common
    /// case and only re-issues once the certificate is close to expiring.
    pub async fn certificate(&self) -> Result<CertificateInfo> {
        let mut cached = self.cached.lock().await;

        if let Some(current) = cached.as_ref() {
            if !self.needs_renewal(current) {
                debug!(serial = %current.serial_number, "using cached wildcard certificate");
                return Ok(current.clone());
            }
            info!(
                serial = %current.serial_number,
                expires_at = %current.expires_at,
                "wildcard certificate near expiry; re-issuing"
            );
        }

        let fresh = self.issue().await?;
        *cached = Some(fresh.clone());
        Ok(fresh)
    }

    /// True once the certificate is within `renew_before` of expiry (or past it).
    fn needs_renewal(&self, cert: &CertificateInfo) -> bool {
        cert.expires_at - Utc::now() <= self.config.renew_before
    }

    /// Issue a fresh wildcard certificate from the CA.
    async fn issue(&self) -> Result<CertificateInfo> {
        let sans = self.subject_alt_names();
        let (primary, extra) = sans
            .split_first()
            .expect("subject_alt_names always yields at least the wildcard");

        info!(wildcard = %primary, "issuing wildcard certificate");

        let ephemeral = self
            .ca
            .issue_server_certificate(primary, extra.to_vec(), Some(self.config.validity_duration))
            .await
            .map_err(|e| anyhow!("CA failed to issue wildcard certificate for {primary}: {e}"))?;

        let info = CertificateInfo {
            subject_alt_names: sans,
            certificate: ephemeral.certificate_pem.into_bytes(),
            private_key: ephemeral.private_key_pem.into_bytes(),
            expires_at: ephemeral.metadata.expires_at,
            serial_number: ephemeral.metadata.serial_number,
        };

        info!(
            serial = %info.serial_number,
            expires_at = %info.expires_at,
            "issued wildcard certificate"
        );

        Ok(info)
    }

    /// Parse a certificate into the rustls DER types, returning the chain and
    /// its matching private key.
    pub fn to_rustls_certificate(
        &self,
        cert_info: &CertificateInfo,
    ) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)> {
        let mut cert_reader = std::io::BufReader::new(cert_info.certificate.as_slice());
        let certs = rustls_pemfile::certs(&mut cert_reader)
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| anyhow!("certificate PEM parse error: {e}"))?;
        if certs.is_empty() {
            bail!("no certificate found in issued PEM");
        }

        let mut key_reader = std::io::BufReader::new(cert_info.private_key.as_slice());
        let key = rustls_pemfile::private_key(&mut key_reader)
            .map_err(|e| anyhow!("private key PEM parse error: {e}"))?
            .ok_or_else(|| anyhow!("no private key found in issued PEM"))?;

        Ok((certs, key))
    }

    /// Build a rustls [`rustls::ServerConfig`] presenting the current wildcard
    /// certificate, with the given ALPN protocols.
    ///
    /// `with_single_cert` verifies the key matches the certificate, so a
    /// mismatched pair fails here rather than at handshake time.
    pub async fn server_config(&self, alpn: &[&str]) -> Result<rustls::ServerConfig> {
        let info = self.certificate().await?;
        let (chain, key) = self.to_rustls_certificate(&info)?;

        let mut config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(chain, key)
            .map_err(|e| anyhow!("failed to build rustls server config: {e}"))?;
        config.alpn_protocols = alpn.iter().map(|p| p.as_bytes().to_vec()).collect();
        Ok(config)
    }

    /// The CA certificate (PEM), so clients can trust certificates issued here.
    pub fn ca_certificate_pem(&self) -> Result<String> {
        self.ca
            .get_ca_certificate_pem()
            .map_err(|e| anyhow!("failed to read CA certificate: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Building a rustls config needs a process-level crypto provider. The
    /// binary installs one at startup (see `edgehub-bin`); tests must do the
    /// same. Both the `ring` and `aws-lc-rs` features are enabled in this
    /// workspace, so rustls cannot pick one automatically. The result is
    /// ignored: another test in this binary may have installed it first.
    fn ensure_crypto_provider() {
        let _ = rustls::crypto::CryptoProvider::install_default(
            rustls::crypto::ring::default_provider(),
        );
    }

    /// Extract DNS SANs from a PEM certificate.
    fn sans_of(cert_pem: &[u8]) -> Vec<String> {
        let (_, pem) = x509_parser::pem::parse_x509_pem(cert_pem).expect("valid PEM");
        let x509 = pem.parse_x509().expect("valid X.509");
        let mut names = Vec::new();
        if let Ok(Some(san)) = x509.subject_alternative_name() {
            for gn in &san.value.general_names {
                if let x509_parser::extensions::GeneralName::DNSName(dns) = gn {
                    names.push((*dns).to_string());
                }
            }
        }
        names
    }

    #[test]
    fn test_certificate_config_default() {
        let config = CertificateConfig::default();
        assert_eq!(config.root_domain, "fleetingdns.run");
        assert_eq!(config.validity_duration, Duration::hours(1));
        assert!(config.renew_before < config.validity_duration);
    }

    #[tokio::test]
    async fn test_subject_alt_names_cover_wildcard_and_apex() {
        let manager = CertificateManager::new(CertificateConfig::default())
            .await
            .unwrap();

        let sans = manager.subject_alt_names();
        assert_eq!(sans[0], "*.fleetingdns.run");
        assert!(sans.contains(&"fleetingdns.run".to_string()));
        assert!(sans.contains(&"localhost".to_string()));
    }

    /// FR-EDGE-1: one certificate must cover every tunnel subdomain, so it has
    /// to carry the wildcard SAN and the apex — never a per-subdomain name.
    #[tokio::test]
    async fn test_issued_certificate_is_wildcard() {
        let manager = CertificateManager::new(CertificateConfig::default())
            .await
            .unwrap();

        let cert = manager.certificate().await.unwrap();
        let sans = sans_of(&cert.certificate);

        assert!(
            sans.contains(&"*.fleetingdns.run".to_string()),
            "wildcard SAN missing: {sans:?}"
        );
        assert!(
            sans.contains(&"fleetingdns.run".to_string()),
            "apex SAN missing: {sans:?}"
        );
    }

    /// The certificate and key must form a usable TLS identity.
    /// `with_single_cert` rejects a mismatched pair, so a successful build is
    /// the proof.
    #[tokio::test]
    async fn test_server_config_builds_with_matching_key() {
        ensure_crypto_provider();
        let manager = CertificateManager::new(CertificateConfig::default())
            .await
            .unwrap();

        let config = manager.server_config(&["http/1.1", "h2"]).await.unwrap();
        assert_eq!(
            config.alpn_protocols,
            vec![b"http/1.1".to_vec(), b"h2".to_vec()]
        );
    }

    /// A second call must reuse the cached certificate rather than burning CA
    /// issuance (and rate limit) on every connection.
    #[tokio::test]
    async fn test_certificate_is_cached() {
        let manager = CertificateManager::new(CertificateConfig::default())
            .await
            .unwrap();

        let first = manager.certificate().await.unwrap();
        let second = manager.certificate().await.unwrap();

        assert_eq!(first.serial_number, second.serial_number);
    }

    /// Once inside the renewal window the manager must issue a NEW certificate.
    /// A `renew_before` exceeding the validity makes every certificate instantly
    /// due for renewal, exercising rotation deterministically.
    #[tokio::test]
    async fn test_certificate_rotates_when_near_expiry() {
        let config = CertificateConfig {
            validity_duration: Duration::minutes(30),
            renew_before: Duration::hours(2),
            ..CertificateConfig::default()
        };
        let manager = CertificateManager::new(config).await.unwrap();

        let first = manager.certificate().await.unwrap();
        let second = manager.certificate().await.unwrap();

        assert_ne!(
            first.serial_number, second.serial_number,
            "certificate inside the renewal window must be re-issued"
        );
    }

    #[tokio::test]
    async fn test_ca_certificate_pem_available() {
        let manager = CertificateManager::new(CertificateConfig::default())
            .await
            .unwrap();

        let pem = manager.ca_certificate_pem().unwrap();
        assert!(pem.contains("BEGIN CERTIFICATE"));
    }
}
