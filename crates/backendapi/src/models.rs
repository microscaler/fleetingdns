use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Tunnel information stored in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tunnel {
    /// Unique tunnel identifier
    pub id: Uuid,

    /// GitHub user ID who owns this tunnel
    pub github_user_id: String,

    /// GitHub username for display
    pub github_username: String,

    /// Subdomain assigned to this tunnel
    pub subdomain: String,

    /// Full FQDN for the tunnel
    pub fqdn: String,

    /// Local port being forwarded
    pub local_port: u16,

    /// SSH server slot/port assigned
    pub slot: u16,

    /// Certificate serial number for this tunnel
    pub certificate_serial: String,

    /// When the tunnel was created
    pub created_at: DateTime<Utc>,

    /// When the tunnel expires
    pub expires_at: DateTime<Utc>,

    /// Current tunnel status
    pub status: TunnelStatus,

    /// Number of bytes transferred through this tunnel
    pub bytes_transferred: u64,

    /// Number of requests processed
    pub request_count: u64,
}

/// Tunnel status enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum TunnelStatus {
    /// Tunnel is being created
    #[default]
    Creating,

    /// Tunnel is active and ready
    Active,

    /// Tunnel is being destroyed
    Destroying,

    /// Tunnel has expired
    Expired,

    /// Tunnel encountered an error
    Error,
}

/// Authentication token information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    /// JWT token string
    pub token: String,

    /// Token type (always "Bearer")
    pub token_type: String,

    /// Token expiration time
    pub expires_at: DateTime<Utc>,

    /// GitHub user information
    pub user: GitHubUser,
}

/// GitHub user information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubUser {
    /// GitHub user ID
    pub id: String,

    /// GitHub username
    pub login: String,

    /// Display name
    pub name: Option<String>,

    /// Email address
    pub email: Option<String>,

    /// Avatar URL
    pub avatar_url: String,
}

/// Certificate information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateInfo {
    /// Certificate serial number
    pub serial: String,

    /// PEM-encoded certificate
    pub certificate: String,

    /// PEM-encoded private key
    pub private_key: String,

    /// Certificate fingerprint (SHA-256)
    pub fingerprint: String,

    /// When the certificate was issued
    pub issued_at: DateTime<Utc>,

    /// When the certificate expires
    pub expires_at: DateTime<Utc>,

    /// Subject common name
    pub subject: String,
}

/// SSH key pair for tunnel authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshKeyPair {
    /// PEM-encoded private key
    pub private_key: String,

    /// Public key in OpenSSH format
    pub public_key: String,

    /// Key fingerprint
    pub fingerprint: String,
}

/// Statistics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiStats {
    /// Total number of active tunnels
    pub active_tunnels: u64,

    /// Total tunnels created today
    pub tunnels_created_today: u64,

    /// Total bytes transferred today
    pub bytes_transferred_today: u64,

    /// Certificate authority statistics
    pub ca_stats: CaStats,

    /// System uptime in seconds
    pub uptime_seconds: u64,
}

/// Certificate Authority statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaStats {
    /// Total certificates issued
    pub certificates_issued: u64,

    /// Active certificates
    pub active_certificates: u64,

    /// Expired certificates cleaned up
    pub expired_certificates: u64,

    /// Certificate issuance rate (per hour)
    pub issuance_rate: f64,
}

impl Tunnel {
    /// Create a new tunnel
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        github_user_id: String,
        github_username: String,
        subdomain: String,
        base_domain: &str,
        local_port: u16,
        slot: u16,
        certificate_serial: String,
        ttl_seconds: u64,
    ) -> Self {
        let now = Utc::now();
        let expires_at = now + chrono::Duration::seconds(ttl_seconds as i64);

        Self {
            id: Uuid::new_v4(),
            github_user_id,
            github_username,
            subdomain: subdomain.clone(),
            fqdn: format!("{subdomain}.{base_domain}"),
            local_port,
            slot,
            certificate_serial,
            created_at: now,
            expires_at,
            status: TunnelStatus::Creating,
            bytes_transferred: 0,
            request_count: 0,
        }
    }

    /// Check if the tunnel has expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Get remaining TTL in seconds
    pub fn remaining_ttl(&self) -> i64 {
        (self.expires_at - Utc::now()).num_seconds()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tunnel_creation() {
        let tunnel = Tunnel::new(
            "user123".to_string(),
            "testuser".to_string(),
            "test-subdomain".to_string(),
            "fdns.run",
            8080,
            12345,
            "cert-serial-123".to_string(),
            3600,
        );

        assert_eq!(tunnel.github_user_id, "user123");
        assert_eq!(tunnel.github_username, "testuser");
        assert_eq!(tunnel.subdomain, "test-subdomain");
        assert_eq!(tunnel.fqdn, "test-subdomain.fdns.run");
        assert_eq!(tunnel.local_port, 8080);
        assert_eq!(tunnel.slot, 12345);
        assert_eq!(tunnel.certificate_serial, "cert-serial-123");
        assert!(matches!(tunnel.status, TunnelStatus::Creating));
        assert_eq!(tunnel.bytes_transferred, 0);
        assert_eq!(tunnel.request_count, 0);
    }

    #[test]
    fn test_tunnel_expiry() {
        let mut tunnel = Tunnel::new(
            "user123".to_string(),
            "testuser".to_string(),
            "test-subdomain".to_string(),
            "fdns.run",
            8080,
            12345,
            "cert-serial-123".to_string(),
            1, // 1 second TTL
        );

        // Initially should not be expired
        assert!(!tunnel.is_expired());

        // Manually set expiry to past
        tunnel.expires_at = Utc::now() - chrono::Duration::seconds(1);
        assert!(tunnel.is_expired());
    }

    #[test]
    fn test_tunnel_remaining_ttl() {
        let tunnel = Tunnel::new(
            "user123".to_string(),
            "testuser".to_string(),
            "test-subdomain".to_string(),
            "fdns.run",
            8080,
            12345,
            "cert-serial-123".to_string(),
            3600,
        );

        let remaining = tunnel.remaining_ttl();
        // Should be approximately 3600 seconds (within 5 seconds tolerance)
        assert!((3595..=3600).contains(&remaining));
    }

    #[test]
    fn test_tunnel_status_default() {
        let status = TunnelStatus::default();
        assert!(matches!(status, TunnelStatus::Creating));
    }

    #[test]
    fn test_tunnel_status_serialization() {
        let status = TunnelStatus::Active;
        let serialized = serde_json::to_string(&status).unwrap();
        assert_eq!(serialized, "\"active\"");

        let deserialized: TunnelStatus = serde_json::from_str(&serialized).unwrap();
        assert!(matches!(deserialized, TunnelStatus::Active));
    }

    #[test]
    fn test_tunnel_status_variants() {
        let statuses = vec![
            TunnelStatus::Creating,
            TunnelStatus::Active,
            TunnelStatus::Destroying,
            TunnelStatus::Expired,
            TunnelStatus::Error,
        ];

        for status in statuses {
            // Test that all variants can be serialized and deserialized
            let serialized = serde_json::to_string(&status).unwrap();
            let deserialized: TunnelStatus = serde_json::from_str(&serialized).unwrap();
            
            // Use Debug format for comparison since TunnelStatus doesn't implement PartialEq
            assert_eq!(format!("{status:?}"), format!("{deserialized:?}"));
        }
    }

    #[test]
    fn test_auth_token_creation() {
        let user = GitHubUser {
            id: "123".to_string(),
            login: "testuser".to_string(),
            name: Some("Test User".to_string()),
            email: Some("test@example.com".to_string()),
            avatar_url: "https://avatar.url".to_string(),
        };

        let token = AuthToken {
            token: "jwt-token-here".to_string(),
            token_type: "Bearer".to_string(),
            expires_at: Utc::now() + chrono::Duration::hours(1),
            user: user.clone(),
        };

        assert_eq!(token.token, "jwt-token-here");
        assert_eq!(token.token_type, "Bearer");
        assert_eq!(token.user.id, "123");
        assert_eq!(token.user.login, "testuser");
    }

    #[test]
    fn test_certificate_info_creation() {
        let cert_info = CertificateInfo {
            serial: "cert-123".to_string(),
            certificate: "-----BEGIN CERTIFICATE-----\n...\n-----END CERTIFICATE-----".to_string(),
            private_key: "-----BEGIN PRIVATE KEY-----\n...\n-----END PRIVATE KEY-----".to_string(),
            fingerprint: "sha256:abcd1234".to_string(),
            issued_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(1),
            subject: "CN=test.example.com".to_string(),
        };

        assert_eq!(cert_info.serial, "cert-123");
        assert!(cert_info.certificate.starts_with("-----BEGIN CERTIFICATE-----"));
        assert!(cert_info.private_key.starts_with("-----BEGIN PRIVATE KEY-----"));
        assert_eq!(cert_info.fingerprint, "sha256:abcd1234");
        assert_eq!(cert_info.subject, "CN=test.example.com");
    }

    #[test]
    fn test_ssh_key_pair_creation() {
        let key_pair = SshKeyPair {
            private_key: "-----BEGIN OPENSSH PRIVATE KEY-----\n...\n-----END OPENSSH PRIVATE KEY-----".to_string(),
            public_key: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5... test@example.com".to_string(),
            fingerprint: "SHA256:abcd1234...".to_string(),
        };

        assert!(key_pair.private_key.starts_with("-----BEGIN OPENSSH PRIVATE KEY-----"));
        assert!(key_pair.public_key.starts_with("ssh-ed25519"));
        assert!(key_pair.fingerprint.starts_with("SHA256:"));
    }

    #[test]
    fn test_api_stats_creation() {
        let ca_stats = CaStats {
            certificates_issued: 100,
            active_certificates: 50,
            expired_certificates: 25,
            issuance_rate: 10.5,
        };

        let api_stats = ApiStats {
            active_tunnels: 15,
            tunnels_created_today: 25,
            bytes_transferred_today: 1024000,
            ca_stats: ca_stats.clone(),
            uptime_seconds: 3600,
        };

        assert_eq!(api_stats.active_tunnels, 15);
        assert_eq!(api_stats.tunnels_created_today, 25);
        assert_eq!(api_stats.bytes_transferred_today, 1024000);
        assert_eq!(api_stats.ca_stats.certificates_issued, 100);
        assert_eq!(api_stats.uptime_seconds, 3600);
    }

    #[test]
    fn test_ca_stats_creation() {
        let ca_stats = CaStats {
            certificates_issued: 100,
            active_certificates: 50,
            expired_certificates: 25,
            issuance_rate: 10.5,
        };

        assert_eq!(ca_stats.certificates_issued, 100);
        assert_eq!(ca_stats.active_certificates, 50);
        assert_eq!(ca_stats.expired_certificates, 25);
        assert_eq!(ca_stats.issuance_rate, 10.5);
    }

    #[test]
    fn test_tunnel_serialization() {
        let tunnel = Tunnel::new(
            "user123".to_string(),
            "testuser".to_string(),
            "test-subdomain".to_string(),
            "fdns.run",
            8080,
            12345,
            "cert-serial-123".to_string(),
            3600,
        );

        // Test serialization
        let serialized = serde_json::to_string(&tunnel).unwrap();
        assert!(serialized.contains("user123"));
        assert!(serialized.contains("test-subdomain"));

        // Test deserialization
        let deserialized: Tunnel = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.github_user_id, tunnel.github_user_id);
        assert_eq!(deserialized.subdomain, tunnel.subdomain);
        assert_eq!(deserialized.local_port, tunnel.local_port);
    }

    #[test]
    fn test_github_user_optional_fields() {
        let user_with_name = GitHubUser {
            id: "123".to_string(),
            login: "testuser".to_string(),
            name: Some("Test User".to_string()),
            email: Some("test@example.com".to_string()),
            avatar_url: "https://avatar.url".to_string(),
        };

        let user_without_name = GitHubUser {
            id: "456".to_string(),
            login: "anotheruser".to_string(),
            name: None,
            email: None,
            avatar_url: "https://avatar.url".to_string(),
        };

        assert!(user_with_name.name.is_some());
        assert!(user_with_name.email.is_some());
        assert!(user_without_name.name.is_none());
        assert!(user_without_name.email.is_none());
    }
}
