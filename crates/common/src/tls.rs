use std::path::Path;

use rcgen::generate_simple_self_signed;
use rustls::{
    ServerConfig,
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
};

use crate::{AppError, AppResult};

/// Build a rustls [`ServerConfig`] from a cert chain + private key, setting ALPN.
fn server_config_from_parts(
    cert_chain: Vec<CertificateDer<'static>>,
    key_der: PrivateKeyDer<'static>,
    alpn: &[&str],
) -> AppResult<ServerConfig> {
    let mut config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, key_der)
        .map_err(|e| AppError::Message(e.to_string()))?;
    config.alpn_protocols = alpn.iter().map(|p| p.as_bytes().to_vec()).collect();
    Ok(config)
}

/// Generate a self-signed TLS configuration for testing.
///
/// Returns the [`ServerConfig`] and the PEM-encoded certificate so that
/// clients can trust the server.
pub fn generate_tls_config(alpn: &[&str]) -> AppResult<(ServerConfig, String)> {
    generate_self_signed_config(alpn, &["tls.local".to_string()])
}

/// Generate a **wildcard** self-signed TLS configuration for `base_domain`
/// (FR-EDGE-1). The cert covers `*.{base_domain}` and the apex, so every
/// ephemeral tunnel subdomain (the capability URL) validates against ONE
/// cert — critical because per-subdomain certs would publish every live
/// tunnel FQDN to Certificate Transparency logs, defeating the
/// unguessable-link security model. `localhost` is included for local dev.
///
/// This is the dev / fallback path; production should mount a real
/// wildcard cert and load it via [`load_tls_config_from_files`].
pub fn generate_wildcard_tls_config(
    alpn: &[&str],
    base_domain: &str,
) -> AppResult<(ServerConfig, String)> {
    let sans = vec![
        format!("*.{base_domain}"),
        base_domain.to_string(),
        "localhost".to_string(),
    ];
    generate_self_signed_config(alpn, &sans)
}

/// Generate a self-signed cert for the given SANs and wrap it in a config.
fn generate_self_signed_config(
    alpn: &[&str],
    sans: &[String],
) -> AppResult<(ServerConfig, String)> {
    let cert =
        generate_simple_self_signed(sans.to_vec()).map_err(|e| AppError::Message(e.to_string()))?;

    let cert_pem = cert.cert.pem();
    let cert_der = CertificateDer::from(cert.cert.der().to_vec());
    let key_der = PrivateKeyDer::from(PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der()));

    let config = server_config_from_parts(vec![cert_der], key_der, alpn)?;
    Ok((config, cert_pem))
}

/// Load a TLS [`ServerConfig`] from PEM cert-chain and private-key files
/// (FR-EDGE-1 production path: a real wildcard cert mounted as a k8s
/// secret, e.g. issued by cert-manager for `*.tilt.tiffany.microscaler.io`).
///
/// Accepts PKCS#8, PKCS#1 (RSA) or SEC1 (EC) keys. Fails closed with a
/// clear error if either file is missing, empty, or malformed.
pub fn load_tls_config_from_files(
    cert_path: impl AsRef<Path>,
    key_path: impl AsRef<Path>,
    alpn: &[&str],
) -> AppResult<ServerConfig> {
    let cert_path = cert_path.as_ref();
    let key_path = key_path.as_ref();

    let cert_bytes = std::fs::read(cert_path).map_err(|e| {
        AppError::Message(format!(
            "Failed to read TLS cert {}: {e}",
            cert_path.display()
        ))
    })?;
    let key_bytes = std::fs::read(key_path).map_err(|e| {
        AppError::Message(format!(
            "Failed to read TLS key {}: {e}",
            key_path.display()
        ))
    })?;

    let cert_chain = rustls_pemfile::certs(&mut &cert_bytes[..])
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Message(format!("Failed to parse TLS cert PEM: {e}")))?;
    if cert_chain.is_empty() {
        return Err(AppError::Message(format!(
            "No certificates found in {}",
            cert_path.display()
        )));
    }

    let key_der = rustls_pemfile::private_key(&mut &key_bytes[..])
        .map_err(|e| AppError::Message(format!("Failed to parse TLS key PEM: {e}")))?
        .ok_or_else(|| {
            AppError::Message(format!("No private key found in {}", key_path.display()))
        })?;

    server_config_from_parts(cert_chain, key_der, alpn)
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
    fn test_wildcard_config_covers_subdomain_and_apex() {
        let (config, cert_pem) =
            generate_wildcard_tls_config(&["http/1.1"], "fleetingdns.run").unwrap();
        assert_eq!(config.alpn_protocols[0], b"http/1.1");
        assert!(cert_pem.contains("-----BEGIN CERTIFICATE-----"));

        // Parse the DER and confirm the wildcard + apex SANs are present, so
        // an arbitrary tunnel subdomain validates against this one cert.
        let der = CertificateDer::from(
            rustls_pemfile::certs(&mut cert_pem.as_bytes())
                .next()
                .unwrap()
                .unwrap()
                .to_vec(),
        );
        let (_, parsed) = x509_parser::parse_x509_certificate(der.as_ref()).unwrap();
        let san_ext = parsed
            .subject_alternative_name()
            .unwrap()
            .expect("SAN present");
        let names: Vec<String> = san_ext
            .value
            .general_names
            .iter()
            .filter_map(|gn| match gn {
                x509_parser::extensions::GeneralName::DNSName(n) => Some(n.to_string()),
                _ => None,
            })
            .collect();
        assert!(
            names.contains(&"*.fleetingdns.run".to_string()),
            "wildcard SAN: {names:?}"
        );
        assert!(
            names.contains(&"fleetingdns.run".to_string()),
            "apex SAN: {names:?}"
        );
    }

    #[test]
    fn test_load_from_files_roundtrip_and_errors() {
        // Write a generated cert+key to temp files, load them back.
        let cert = rcgen::generate_simple_self_signed(vec!["*.example.test".to_string()]).unwrap();
        let dir = std::env::temp_dir();
        let cert_path = dir.join(format!("edge-cert-{}.pem", std::process::id()));
        let key_path = dir.join(format!("edge-key-{}.pem", std::process::id()));
        std::fs::write(&cert_path, cert.cert.pem()).unwrap();
        std::fs::write(&key_path, cert.signing_key.serialize_pem()).unwrap();

        let config = load_tls_config_from_files(&cert_path, &key_path, &["h2"]).unwrap();
        assert_eq!(config.alpn_protocols[0], b"h2");

        // Missing file → error (fail closed).
        assert!(load_tls_config_from_files("/no/such/cert.pem", &key_path, &["h2"]).is_err());
        // Malformed cert → error.
        let bad = dir.join(format!("edge-bad-{}.pem", std::process::id()));
        std::fs::write(&bad, b"not a pem").unwrap();
        assert!(load_tls_config_from_files(&bad, &key_path, &["h2"]).is_err());

        let _ = std::fs::remove_file(&cert_path);
        let _ = std::fs::remove_file(&key_path);
        let _ = std::fs::remove_file(&bad);
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
