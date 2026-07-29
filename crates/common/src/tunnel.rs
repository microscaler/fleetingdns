use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
}

impl TunnelData {
    /// Create a new tunnel data instance
    #[allow(clippy::too_many_arguments)] // plain data carrier; builder is overkill
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
#[cfg(test)]
mod tests {
    use super::*;

    fn sample(expires_at: DateTime<Utc>) -> TunnelData {
        TunnelData::new(
            Uuid::nil(),
            "gh-1".to_string(),
            "octocat".to_string(),
            "abc123".to_string(),
            "abc123.fleetingdns.run".to_string(),
            3000,
            41234,
            "serial-1".to_string(),
            expires_at,
        )
    }

    #[test]
    fn new_populates_defaults() {
        let t = sample(Utc::now() + chrono::Duration::minutes(30));
        assert_eq!(t.id, Uuid::nil().to_string());
        assert_eq!(t.status, "creating");
        assert_eq!(t.local_port, 3000);
        assert_eq!(t.slot, 41234);
        assert!(!t.created_at.is_empty());
    }

    #[test]
    fn tunnel_id_roundtrips() {
        let t = sample(Utc::now());
        assert_eq!(t.tunnel_id().unwrap(), Uuid::nil());
        let mut broken = t;
        broken.id = "not-a-uuid".to_string();
        assert!(broken.tunnel_id().is_err());
    }

    #[test]
    fn expiry_and_ttl() {
        let live = sample(Utc::now() + chrono::Duration::minutes(30));
        assert!(!live.is_expired());
        let ttl = live.ttl_seconds();
        assert!(ttl > 0 && ttl <= 30 * 60, "ttl was {ttl}");

        let dead = sample(Utc::now() - chrono::Duration::minutes(1));
        assert!(dead.is_expired());
        assert_eq!(dead.ttl_seconds(), 0);
    }

    #[test]
    fn unparseable_expiry_is_expired_with_zero_ttl() {
        let mut t = sample(Utc::now());
        t.expires_at = "garbage".to_string();
        assert!(t.is_expired());
        assert_eq!(t.ttl_seconds(), 0);
    }

    #[test]
    fn serde_roundtrip() {
        let t = sample(Utc::now() + chrono::Duration::minutes(5));
        let json = serde_json::to_string(&t).unwrap();
        let back: TunnelData = serde_json::from_str(&json).unwrap();
        assert_eq!(back.fqdn, t.fqdn);
        assert_eq!(back.slot, t.slot);

        let lookup = UserTunnelLookup {
            github_user_id: "gh-1".to_string(),
            github_username: "octocat".to_string(),
            tunnels: vec![t.id.clone()],
        };
        let json = serde_json::to_string(&lookup).unwrap();
        let back: UserTunnelLookup = serde_json::from_str(&json).unwrap();
        assert_eq!(back.tunnels.len(), 1);
    }
}
