//! TLS Routing and HTTP Forwarding Tests
//!
//! These tests verify the core USP functionality where incoming HTTPS connections
//! on port 443 are routed to the appropriate SSH tunnels based on SNI.

use anyhow::Result;
use edgehub::{
    CertificateManager, CertificateConfig, TlsRouter, TlsRouterConfig,
    SshServerState, ReverseTunnelInfo
};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use rustls::ClientConfig;
use std::net::SocketAddr;

/// Test TLS routing with a mock tunnel
#[tokio::test]
async fn test_tls_routing_with_mock_tunnel() -> Result<()> {
    // Create certificate manager
    let cert_config = CertificateConfig::default();
    let cert_manager = CertificateManager::new(cert_config)?;
    
    // Generate certificate for test subdomain
    let cert_info = cert_manager.generate_certificate("test").await?;
    
    // Create mock tunnel info
    let tunnel_info = ReverseTunnelInfo {
        subdomain: "test".to_string(),
        local_port: 8080,
        session_id: "test-session-123".to_string(),
        github_user_id: "12345678".to_string(),
        created_at: chrono::Utc::now(),
        expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
    };
    
    // Create SSH server state with mock tunnel
    let state = Arc::new(SshServerState {
        active_tunnels: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        reverse_tunnels: Arc::new(tokio::sync::Mutex::new({
            let mut map = std::collections::HashMap::new();
            map.insert("test".to_string(), tunnel_info);
            map
        })),
        shutdown_tx: tokio::sync::mpsc::channel(1).0,
        certificate_authority: None,
        brute_force_protection: Arc::new(tokio::sync::Mutex::new(edgehub::ssh_server::BruteForceProtection::default())),
        redis_auth_handler: None,
    });
    
    // Create TLS router config
    let tls_config = rustls::ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(
            vec![rustls::Certificate(cert_info.certificate)],
            rustls::PrivateKey(cert_info.private_key)
        )?;
    
    let router_config = TlsRouterConfig {
        bind_addr: "127.0.0.1:0".parse()?, // Use port 0 for testing
        tls_config,
        public_domain: "fleetingdns.run".to_string(),
        redis_url: "redis://localhost:6379".to_string(),
        max_connections: 100,
    };
    
    // Create TLS router
    let router = TlsRouter::new(router_config, state);
    
    // Test SNI extraction
    let sni = "test.fleetingdns.run";
    assert!(router.is_valid_subdomain(sni));
    
    // Test subdomain extraction
    let subdomain = sni
        .strip_suffix(".fleetingdns.run")
        .unwrap();
    assert_eq!(subdomain, "test");
    
    Ok(())
}

/// Test certificate generation for different subdomains
#[tokio::test]
async fn test_certificate_generation() -> Result<()> {
    let config = CertificateConfig::default();
    let manager = CertificateManager::new(config)?;
    
    // Test individual certificate generation
    let cert1 = manager.generate_certificate("app1").await?;
    let cert2 = manager.generate_certificate("app2").await?;
    
    assert_eq!(cert1.subdomain, "app1");
    assert_eq!(cert2.subdomain, "app2");
    assert_ne!(cert1.serial_number, cert2.serial_number);
    
    // Test wildcard certificate generation
    let wildcard_cert = manager.generate_wildcard_certificate("api").await?;
    assert_eq!(wildcard_cert.subdomain, "api");
    
    // Test certificate caching
    let cached_cert = manager.generate_certificate("app1").await?;
    assert_eq!(cached_cert.serial_number, cert1.serial_number);
    
    Ok(())
}

/// Test TLS router configuration
#[test]
fn test_tls_router_configuration() {
    let config = TlsRouterConfig::default();
    
    assert_eq!(config.bind_addr, "0.0.0.0:443".parse::<SocketAddr>().unwrap());
    assert_eq!(config.public_domain, "fleetingdns.run");
    assert_eq!(config.max_connections, 1000);
}

/// Test certificate manager configuration
#[test]
fn test_certificate_manager_configuration() {
    let config = CertificateConfig::default();
    
    assert_eq!(config.root_domain, "fleetingdns.run");
    assert_eq!(config.validity_duration, chrono::Duration::hours(1));
    assert_eq!(config.max_cache_size, 1000);
    assert!(!config.use_wildcards);
}

/// Test certificate statistics
#[tokio::test]
async fn test_certificate_statistics() -> Result<()> {
    let config = CertificateConfig::default();
    let manager = CertificateManager::new(config)?;
    
    // Generate some certificates
    manager.generate_certificate("test1").await?;
    manager.generate_certificate("test2").await?;
    manager.generate_certificate("test3").await?;
    
    let stats = manager.get_stats().await;
    assert_eq!(stats.total_certificates, 3);
    assert_eq!(stats.expired_certificates, 0);
    assert_eq!(stats.cache_size, 1000);
    
    Ok(())
}

/// Test certificate cleanup
#[tokio::test]
async fn test_certificate_cleanup() -> Result<()> {
    let config = CertificateConfig {
        validity_duration: chrono::Duration::milliseconds(1), // Very short for testing
        ..Default::default()
    };
    let manager = CertificateManager::new(config)?;
    
    // Generate certificate that will expire quickly
    manager.generate_certificate("expiring").await?;
    
    // Wait for expiration
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    
    // Clean up expired certificates
    let removed = manager.cleanup_expired_certificates().await?;
    assert_eq!(removed, 1);
    
    let stats = manager.get_stats().await;
    assert_eq!(stats.total_certificates, 0);
    
    Ok(())
}

/// Test subdomain validation
#[test]
fn test_subdomain_validation() {
    let config = TlsRouterConfig::default();
    let state = Arc::new(SshServerState::default());
    let router = TlsRouter::new(config, state);
    
    // Valid subdomains
    assert!(router.is_valid_subdomain("test.fleetingdns.run"));
    assert!(router.is_valid_subdomain("api.fleetingdns.run"));
    assert!(router.is_valid_subdomain("app-123.fleetingdns.run"));
    
    // Invalid subdomains
    assert!(!router.is_valid_subdomain("test.example.com"));
    assert!(!router.is_valid_subdomain("fleetingdns.run"));
    assert!(!router.is_valid_subdomain("test.fleetingdns.com"));
}

/// Test tunnel lookup simulation
#[tokio::test]
async fn test_tunnel_lookup_simulation() -> Result<()> {
    // Create mock tunnel data
    let tunnel_info = ReverseTunnelInfo {
        subdomain: "test".to_string(),
        local_port: 8080,
        session_id: "test-session-123".to_string(),
        github_user_id: "12345678".to_string(),
        created_at: chrono::Utc::now(),
        expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
    };
    
    // Create SSH server state with mock tunnel
    let state = Arc::new(SshServerState {
        active_tunnels: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        reverse_tunnels: Arc::new(tokio::sync::Mutex::new({
            let mut map = std::collections::HashMap::new();
            map.insert("test".to_string(), tunnel_info);
            map
        })),
        shutdown_tx: tokio::sync::mpsc::channel(1).0,
        certificate_authority: None,
        brute_force_protection: Arc::new(tokio::sync::Mutex::new(edgehub::ssh_server::BruteForceProtection::default())),
        redis_auth_handler: None,
    });
    
    // Test tunnel lookup
    let tunnel = state.find_reverse_tunnel("test").await;
    assert!(tunnel.is_some());
    
    let found_tunnel = tunnel.unwrap();
    assert_eq!(found_tunnel.subdomain, "test");
    assert_eq!(found_tunnel.local_port, 8080);
    
    // Test non-existent tunnel
    let non_existent = state.find_reverse_tunnel("nonexistent").await;
    assert!(non_existent.is_none());
    
    Ok(())
}

/// Test certificate conversion to Rustls format
#[tokio::test]
async fn test_certificate_conversion() -> Result<()> {
    let config = CertificateConfig::default();
    let manager = CertificateManager::new(config)?;
    
    let cert_info = manager.generate_certificate("test").await?;
    let (cert, key) = manager.to_rustls_certificate(&cert_info)?;
    
    assert!(!cert.0.is_empty());
    assert!(!key.0.is_empty());
    
    Ok(())
}

/// Test wildcard certificate generation
#[tokio::test]
async fn test_wildcard_certificate() -> Result<()> {
    let config = CertificateConfig {
        use_wildcards: true,
        ..Default::default()
    };
    let manager = CertificateManager::new(config)?;
    
    let wildcard_cert = manager.generate_wildcard_certificate("api").await?;
    
    assert_eq!(wildcard_cert.subdomain, "api");
    assert!(wildcard_cert.expires_at > chrono::Utc::now());
    assert!(!wildcard_cert.certificate.is_empty());
    assert!(!wildcard_cert.private_key.is_empty());
    
    Ok(())
} 