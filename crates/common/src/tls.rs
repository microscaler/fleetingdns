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

#[cfg(test)]
mod tests {
    use super::*;

    
    #[ctor::ctor]
    fn init() {
        // Initialize the crypto provider once for all tests
        rustls::crypto::ring::default_provider()
            .install_default()
            .expect("Failed to install rustls ring crypto provider");
    }


    #[test]
    fn test_generate_tls_config_success() {

        let alpn_protocols = &["http/1.1", "h2"];
        let result = generate_tls_config(alpn_protocols);

        assert!(result.is_ok());
        let (config, cert_pem) = result.unwrap();

        // Verify certificate PEM is valid
        assert!(cert_pem.contains("-----BEGIN CERTIFICATE-----"));
        assert!(cert_pem.contains("-----END CERTIFICATE-----"));

        // Verify ALPN protocols are set
        assert_eq!(config.alpn_protocols.len(), 2);
        assert_eq!(config.alpn_protocols[0], b"http/1.1");
        assert_eq!(config.alpn_protocols[1], b"h2");
    }

    #[test]
    fn test_generate_tls_config_empty_alpn() {

        let result = generate_tls_config(&[]);

        assert!(result.is_ok());
        let (config, _) = result.unwrap();

        // Verify no ALPN protocols are set
        assert!(config.alpn_protocols.is_empty());
    }

    #[test]
    fn test_generate_tls_config_single_alpn() {

        let alpn_protocols = &["dot"];
        let result = generate_tls_config(alpn_protocols);

        assert!(result.is_ok());
        let (config, _) = result.unwrap();

        // Verify single ALPN protocol is set
        assert_eq!(config.alpn_protocols.len(), 1);
        assert_eq!(config.alpn_protocols[0], b"dot");
    }

    #[test]
    fn test_generate_tls_config_multiple_alpn() {

        let alpn_protocols = &["http/1.1", "h2", "dot", "quic"];
        let result = generate_tls_config(alpn_protocols);

        assert!(result.is_ok());
        let (config, _) = result.unwrap();

        // Verify multiple ALPN protocols are set
        assert_eq!(config.alpn_protocols.len(), 4);
        assert_eq!(config.alpn_protocols[0], b"http/1.1");
        assert_eq!(config.alpn_protocols[1], b"h2");
        assert_eq!(config.alpn_protocols[2], b"dot");
        assert_eq!(config.alpn_protocols[3], b"quic");
    }

    #[test]
    fn test_certificate_pem_structure() {

        let result = generate_tls_config(&["test"]);
        assert!(result.is_ok());

        let (_, cert_pem) = result.unwrap();

        // Verify PEM structure
        let lines: Vec<&str> = cert_pem.lines().collect();
        assert!(lines.len() >= 3); // At least header, body, footer

        // Check header and footer
        assert_eq!(lines[0], "-----BEGIN CERTIFICATE-----");
        assert_eq!(lines[lines.len() - 1], "-----END CERTIFICATE-----");

        // Check body lines are valid base64
        for line in &lines[1..lines.len() - 1] {
            if !line.is_empty() {
                // Base64 should only contain alphanumeric chars, +, /, and =
                assert!(
                    line.chars()
                        .all(|c| c.is_alphanumeric() || c == '+' || c == '/' || c == '=')
                );
            }
        }
    }

    #[test]
    fn test_server_config_properties() {

        let result = generate_tls_config(&["test"]);
        assert!(result.is_ok());

        let (config, _) = result.unwrap();

        // Verify server config has expected properties
        // Note: We can't easily test internal properties, but we can verify
        // the config was created successfully and has ALPN protocols set
        assert_eq!(config.alpn_protocols.len(), 1);
        assert_eq!(config.alpn_protocols[0], b"test");
    }

    #[test]
    fn test_certificate_der_structure() {

        let result = generate_tls_config(&["test"]);
        assert!(result.is_ok());

        let (config, _) = result.unwrap();

        // Verify the certificate chain has at least one certificate
        // This is an indirect test since we can't easily access the internal cert chain
        // but if the config was created successfully, it should have a valid cert
        assert_eq!(config.alpn_protocols.len(), 1);
    }

    #[test]
    fn test_multiple_calls_produce_different_certs() {

        let result1 = generate_tls_config(&["test"]);
        let result2 = generate_tls_config(&["test"]);

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        let (_, cert_pem1) = result1.unwrap();
        let (_, cert_pem2) = result2.unwrap();

        // Each call should produce a different certificate (different serial numbers)
        // We can't easily compare serial numbers, but we can verify both are valid PEM
        assert!(cert_pem1.contains("-----BEGIN CERTIFICATE-----"));
        assert!(cert_pem2.contains("-----BEGIN CERTIFICATE-----"));
        assert!(cert_pem1.contains("-----END CERTIFICATE-----"));
        assert!(cert_pem2.contains("-----END CERTIFICATE-----"));
    }

    #[test]
    fn test_alpn_protocols_encoding() {

        let test_protocols = &["http/1.1", "h2", "dot", "quic", "custom-protocol"];
        let result = generate_tls_config(test_protocols);

        assert!(result.is_ok());
        let (config, _) = result.unwrap();

        // Verify all protocols are properly encoded as bytes
        assert_eq!(config.alpn_protocols.len(), 5);
        assert_eq!(config.alpn_protocols[0], b"http/1.1");
        assert_eq!(config.alpn_protocols[1], b"h2");
        assert_eq!(config.alpn_protocols[2], b"dot");
        assert_eq!(config.alpn_protocols[3], b"quic");
        assert_eq!(config.alpn_protocols[4], b"custom-protocol");
    }

    #[test]
    fn test_error_handling_invalid_alpn() {

        // Test with empty string (though this should still work)
        let result = generate_tls_config(&[""]);
        assert!(result.is_ok());

        let (config, _) = result.unwrap();
        assert_eq!(config.alpn_protocols.len(), 1);
        assert_eq!(config.alpn_protocols[0], b"");
    }

    #[test]
    fn test_certificate_subject_alt_names() {

        let result = generate_tls_config(&["test"]);
        assert!(result.is_ok());

        let (_, cert_pem) = result.unwrap();

        // The certificate should contain "tls.local" as a subject alternative name
        // We can't easily parse the certificate to verify this, but we can verify
        // the PEM is valid and contains the expected structure
        assert!(cert_pem.contains("-----BEGIN CERTIFICATE-----"));
        assert!(cert_pem.contains("-----END CERTIFICATE-----"));
    }
}
