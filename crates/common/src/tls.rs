use rcgen::generate_simple_self_signed;
use rustls::{
    ServerConfig,
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
};

use crate::{AppError, AppResult};

/// Generate a self-signed TLS configuration for testing.
///
/// Returns the [`ServerConfig`] and the PEM-encoded certificate so that
/// clients can trust the server.
pub fn generate_tls_config(alpn: &[&str]) -> AppResult<(ServerConfig, String)> {
    let subject_alt_names = vec!["tls.local".to_string()];
    let cert = generate_simple_self_signed(subject_alt_names)
        .map_err(|e| AppError::Message(e.to_string()))?;

    let cert_pem = cert.cert.pem();
    let cert_der = CertificateDer::from(cert.cert.der().to_vec());
    let key_der = PrivateKeyDer::from(PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der()));

    let mut config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)
        .map_err(|e| AppError::Message(e.to_string()))?;

    config.alpn_protocols = alpn.iter().map(|p| p.as_bytes().to_vec()).collect();
    Ok((config, cert_pem))
}
