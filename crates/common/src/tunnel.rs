use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Shared tunnel information structure used across all services
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelData {
    pub id: String,
    pub github_user_id: String,
    pub github_username: String,
    pub subdomain: String,
    pub fqdn: String,
    pub local_port: u16,
    pub slot: u16,
    pub certificate_serial: String,
    pub created_at: String,
    pub expires_at: String,
    pub status: String,
    pub bytes_transferred: u64,
    pub request_count: u64,
}

impl TunnelData {
    /// Create a new tunnel data instance
    pub fn new(
        id: Uuid,
        github_user_id: String,
        github_username: String,
        subdomain: String,
        fqdn: String,
        local_port: u16,
        slot: u16,
        certificate_serial: String,
        expires_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id: id.to_string(),
            github_user_id,
            github_username,
            subdomain,
            fqdn,
            local_port,
            slot,
            certificate_serial,
            created_at: Utc::now().to_rfc3339(),
            expires_at: expires_at.to_rfc3339(),
            status: "creating".to_string(),
            bytes_transferred: 0,
            request_count: 0,
        }
    }

    /// Get the tunnel ID as a UUID
    pub fn tunnel_id(&self) -> Result<Uuid, uuid::Error> {
        Uuid::parse_str(&self.id)
    }

    /// Check if the tunnel is expired
    pub fn is_expired(&self) -> bool {
        if let Ok(expires_at) = DateTime::parse_from_rfc3339(&self.expires_at) {
            Utc::now() > expires_at.with_timezone(&Utc)
        } else {
            true // If we can't parse the date, consider it expired
        }
    }

    /// Get the TTL in seconds
    pub fn ttl_seconds(&self) -> u64 {
        if let Ok(expires_at) = DateTime::parse_from_rfc3339(&self.expires_at) {
            let now = Utc::now();
            let duration = expires_at.with_timezone(&Utc) - now;
            duration.num_seconds().max(0) as u64
        } else {
            0
        }
    }
}

/// User tunnel lookup data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserTunnelLookup {
    pub github_user_id: String,
    pub github_username: String,
    pub tunnels: Vec<String>, // List of tunnel IDs
} 