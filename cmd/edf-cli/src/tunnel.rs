use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use anyhow::Result;

use crate::ssh_keys::SshKeyManager;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TunnelSession {
    pub id: String,
    pub fqdn: String,
    pub slot: u16,
    pub pubkey: String,
    pub cert: String,
    pub private_key: String,
    pub expires_at: DateTime<Utc>,
    pub github_id: String,
    pub local_port: u16,
    pub status: TunnelStatus,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum TunnelStatus {
    Creating,
    Active,
    Closing,
    Closed,
    Error(String),
}

pub struct TunnelManager {
    ssh_key_manager: SshKeyManager,
    active_sessions: HashMap<String, TunnelSession>,
}

impl TunnelManager {
    pub fn new() -> Result<Self> {
        let ssh_key_manager = SshKeyManager::new()?;
        
        Ok(Self {
            ssh_key_manager,
            active_sessions: HashMap::new(),
        })
    }
    
    pub async fn forward(&mut self, port: u16, ttl: u32, subdomain: Option<String>) -> Result<()> {
        info!("Starting tunnel for port {} with TTL {}", port, ttl);
        
        // Request or load SSH key pair from API
        let key_pair = self.ssh_key_manager.get_or_request_key_pair(ttl).await?;
        info!("Using SSH key pair with fingerprint: {}", key_pair.fingerprint);
        
        // TODO: Implement API call to edf-api for tunnel creation
        // This would involve:
        // 1. GitHub OAuth authentication
        // 2. API call to create tunnel
        // 3. Receiving ephemeral certificate
        // 4. Establishing TLS-wrapped SSH connection
        
        // For now, create a mock session
        let session = TunnelSession {
            id: uuid::Uuid::new_v4().to_string(),
            fqdn: subdomain.unwrap_or_else(|| "test".to_string()) + ".edf.run",
            slot: 1,
            pubkey: key_pair.public_key,
            cert: "mock-cert".to_string(),
            private_key: key_pair.private_key,
            expires_at: Utc::now() + chrono::Duration::seconds(ttl as i64),
            github_id: "mock-github-id".to_string(),
            local_port: port,
            status: TunnelStatus::Creating,
        };
        
        self.active_sessions.insert(session.id.clone(), session.clone());
        
        info!("Tunnel session created: {}", session.id);
        println!("🚀 Tunnel created successfully!");
        println!("   ID: {}", session.id);
        println!("   FQDN: {}", session.fqdn);
        println!("   Local Port: {}", session.local_port);
        println!("   Expires: {}", session.expires_at);
        println!("   SSH Key Fingerprint: {}", key_pair.fingerprint);
        
        // TODO: Implement actual tunnel establishment
        // This would involve:
        // 1. TLS handshake with edf-hub
        // 2. SSH handshake with ephemeral key
        // 3. Reverse port forwarding setup
        // 4. Keep-alive monitoring
        // 5. Graceful shutdown on expiry
        
        Ok(())
    }
    
    pub async fn list_tunnels(&self) -> Result<()> {
        if self.active_sessions.is_empty() {
            println!("No active tunnels found.");
            return Ok(());
        }
        
        println!("Active Tunnels:");
        println!("{:<36} {:<20} {:<10} {:<20}", "ID", "FQDN", "Port", "Status");
        println!("{}", "-".repeat(90));
        
        for session in self.active_sessions.values() {
            println!(
                "{:<36} {:<20} {:<10} {:<20}",
                session.id,
                session.fqdn,
                session.local_port,
                format!("{:?}", session.status)
            );
        }
        
        Ok(())
    }
    
    pub async fn close_tunnel(&mut self, id: &str) -> Result<()> {
        if let Some(session) = self.active_sessions.get_mut(id) {
            session.status = TunnelStatus::Closing;
            info!("Closing tunnel: {}", id);
            
            // TODO: Implement actual tunnel teardown
            // This would involve:
            // 1. Sending SSH close signal
            // 2. Closing TLS connection
            // 3. API call to edf-api to mark tunnel as closed
            // 4. Cleaning up local resources
            
            self.active_sessions.remove(id);
            println!("✅ Tunnel {} closed successfully", id);
        } else {
            println!("❌ Tunnel {} not found", id);
        }
        
        Ok(())
    }
    
    pub fn get_session(&self, id: &str) -> Option<&TunnelSession> {
        self.active_sessions.get(id)
    }
    
    pub fn cleanup_expired_sessions(&mut self) {
        let now = Utc::now();
        let expired_ids: Vec<String> = self.active_sessions
            .iter()
            .filter(|(_, session)| session.expires_at < now)
            .map(|(id, _)| id.clone())
            .collect();
        
        for id in expired_ids {
            if let Some(session) = self.active_sessions.remove(&id) {
                warn!("Removing expired tunnel session: {}", session.id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_tunnel_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let ssh_key_manager = SshKeyManager {
            key_directory: temp_dir.path().to_path_buf(),
            api_client: reqwest::Client::new(),
            api_base_url: "https://test.api.edf.run".to_string(),
            auth_token: None,
        };
        
        let tunnel_manager = TunnelManager {
            ssh_key_manager,
            active_sessions: HashMap::new(),
        };
        
        assert_eq!(tunnel_manager.active_sessions.len(), 0);
    }
    
    #[test]
    fn test_tunnel_status_serialization() {
        let status = TunnelStatus::Active;
        let serialized = serde_json::to_string(&status).unwrap();
        let deserialized: TunnelStatus = serde_json::from_str(&serialized).unwrap();
        
        assert!(matches!(deserialized, TunnelStatus::Active));
    }
} 