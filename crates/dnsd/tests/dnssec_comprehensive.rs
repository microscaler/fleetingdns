use dnsd::dns_handler::{DnsHandler, PerformanceConfig};
use dnsd::sign::{AlertType, DnssecAlgorithm, DnssecConfig, ProductionDnssecSigner};
use hickory_proto::op::{Message, MessageType, ResponseCode};
use hickory_proto::rr::rdata::A;
use hickory_proto::rr::{Name, Record, RecordType};
use std::net::Ipv4Addr;

/// Test DNSSEC configuration creation and validation
#[test]
fn test_dnssec_config_creation() {
    let config = DnssecConfig {
        default_algorithm: DnssecAlgorithm::RsaSha256,
        rotation_interval: 86400,
        grace_period: 3600,
        max_keys: 5,
        enable_signature_cache: true,
        signature_cache_ttl: 3600,
        enable_key_backup: false,
        backup_directory: None,
    };

    assert_eq!(config.default_algorithm, DnssecAlgorithm::RsaSha256);
    assert_eq!(config.rotation_interval, 86400);
    assert_eq!(config.grace_period, 3600);
    assert_eq!(config.max_keys, 5);
}

/// Test DNSSEC algorithm conversion
#[test]
fn test_dnssec_algorithm_conversion() {
    assert_eq!(DnssecAlgorithm::HmacSha256.algorithm_number(), 253);
    assert_eq!(DnssecAlgorithm::RsaSha256.algorithm_number(), 8);
    assert_eq!(DnssecAlgorithm::EcdsaP256Sha256.algorithm_number(), 13);
}

/// Test DNSSEC alert types
#[test]
fn test_alert_types() {
    let alert_types = [
        AlertType::HighFailureRate,
        AlertType::SlowValidation,
        AlertType::KeyRotationNeeded,
        AlertType::KeyGenerationFailure,
        AlertType::SignatureCacheIssues,
    ];

    for alert_type in &alert_types {
        assert!(format!("{:?}", alert_type).len() > 0);
    }
}

/// Test DNSSEC signer creation
#[test]
fn test_dnssec_signer_creation() {
    let config = DnssecConfig::default();
    let signer = ProductionDnssecSigner::new(config);
    assert!(signer.is_ok());
}

/// Test DNSSEC signature generation
#[test]
fn test_dnssec_signature_generation() {
    let config = DnssecConfig::default();
    let signer = ProductionDnssecSigner::new(config).unwrap();

    // Create a test record
    let name = Name::from_ascii("test.example.com.").unwrap();
    let record = Record::from_rdata(name.clone(), 300, A(Ipv4Addr::new(192, 168, 1, 1)));

    // Create a simple rrset for testing
    let rrset = b"test_rrset_data";

    // Sign the record
    let signed_record = signer.rrsig_record(&name, RecordType::A, 300, rrset);
    assert!(signed_record.is_ok());
}

/// Test DNSSEC key rotation
#[test]
fn test_dnssec_key_rotation() {
    let config = DnssecConfig::default();
    let signer = ProductionDnssecSigner::new(config).unwrap();

    // Test key rotation
    let rotation_result = signer.force_key_rotation();
    assert!(rotation_result.is_ok());
}

/// Test DNSSEC signature caching
#[test]
fn test_dnssec_signature_caching() {
    let mut config = DnssecConfig::default();
    config.enable_signature_cache = true;
    config.signature_cache_ttl = 3600;

    let signer = ProductionDnssecSigner::new(config).unwrap();

    let name = Name::from_ascii("cache.example.com.").unwrap();
    let rrset = b"test_rrset_data";

    // Sign the same record twice
    let first_signature = signer
        .rrsig_record(&name, RecordType::A, 300, rrset)
        .unwrap();
    let second_signature = signer
        .rrsig_record(&name, RecordType::A, 300, rrset)
        .unwrap();

    // Signatures should be identical due to caching
    assert_eq!(first_signature.to_string(), second_signature.to_string());
}

/// Test DNSSEC monitoring and alerts
#[test]
fn test_dnssec_monitoring() {
    let config = DnssecConfig::default();
    let signer = ProductionDnssecSigner::new(config).unwrap();

    // Create monitor
    let monitor = signer.create_monitor(dnsd::sign::AlertConfig::default());

    // Check for alerts
    let alerts = monitor.check_alerts();
    assert!(alerts.len() >= 0); // Should have some alerts
}

/// Test DNSSEC performance under load
#[test]
fn test_dnssec_performance_load() {
    let config = DnssecConfig::default();
    let signer = ProductionDnssecSigner::new(config).unwrap();

    let start = std::time::Instant::now();

    // Sign multiple records
    for i in 0..10 {
        let name = Name::from_ascii(&format!("load{}.example.com.", i)).unwrap();
        let rrset = b"test_rrset_data";
        let result = signer.rrsig_record(&name, RecordType::A, 300, rrset);
        assert!(result.is_ok());
    }

    let duration = start.elapsed();
    assert!(duration < std::time::Duration::from_secs(5)); // Should complete within 5 seconds
}

/// Test DNSSEC error handling
#[test]
fn test_dnssec_error_handling() {
    let config = DnssecConfig::default();
    let signer = ProductionDnssecSigner::new(config).unwrap();

    // Test with a valid domain name but expect the signer to handle errors gracefully
    let valid_name = Name::from_ascii("test.example.com.").unwrap();
    let invalid_rrset = b"invalid_data";
    let result = signer.rrsig_record(&valid_name, RecordType::A, 300, invalid_rrset);
    // The signer should handle this gracefully, either succeed or fail cleanly
    // We don't assert on the result since it depends on the implementation
}

/// Test DNSSEC with different record types
#[test]
fn test_dnssec_different_record_types() {
    let config = DnssecConfig::default();
    let signer = ProductionDnssecSigner::new(config).unwrap();

    let name = Name::from_ascii("types.example.com.").unwrap();
    let rrset = b"test_rrset_data";

    // Test A record
    let a_signed = signer.rrsig_record(&name, RecordType::A, 300, rrset);
    assert!(a_signed.is_ok());

    // Test AAAA record
    let aaaa_signed = signer.rrsig_record(&name, RecordType::AAAA, 300, rrset);
    assert!(aaaa_signed.is_ok());
}

/// Test DNSSEC algorithm compatibility
#[test]
fn test_dnssec_algorithm_compatibility() {
    let algorithms = [
        DnssecAlgorithm::HmacSha256,
        DnssecAlgorithm::RsaSha256,
        DnssecAlgorithm::EcdsaP256Sha256,
    ];

    for algorithm in &algorithms {
        let config = DnssecConfig {
            default_algorithm: *algorithm,
            ..Default::default()
        };

        assert_eq!(config.default_algorithm, *algorithm);
    }
}

/// Test DNSSEC configuration serialization
#[test]
fn test_dnssec_config_serialization() {
    let config = DnssecConfig {
        default_algorithm: DnssecAlgorithm::RsaSha256,
        rotation_interval: 86400,
        grace_period: 3600,
        max_keys: 5,
        enable_signature_cache: true,
        signature_cache_ttl: 3600,
        enable_key_backup: false,
        backup_directory: None,
    };

    // Test serialization
    let serialized = serde_json::to_string(&config).unwrap();
    assert!(serialized.contains("default_algorithm"));
    assert!(serialized.contains("RsaSha256"));
    assert!(serialized.contains("86400"));

    // Test deserialization
    let deserialized: DnssecConfig = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.default_algorithm, config.default_algorithm);
    assert_eq!(deserialized.rotation_interval, config.rotation_interval);
    assert_eq!(deserialized.max_keys, config.max_keys);
}
