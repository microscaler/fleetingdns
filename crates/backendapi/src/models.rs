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
pub enum TunnelStatus {
    /// Tunnel is being created
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
            fqdn: format!("{}.{}", subdomain, base_domain),
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

impl Default for TunnelStatus {
    fn default() -> Self {
        TunnelStatus::Creating
    }
} 