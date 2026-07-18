use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{error, info, warn};

use crate::ssh_client::SshClientHandler;
use crate::ssh_client::{TunnelClient, TunnelClientConfig};
use crate::ssh_keys::SshKeyManager;
use std::time::Duration;

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

    pub async fn forward(
        &mut self,
        port: u16,
        ttl: u32,
        subdomain: Option<String>,
        hub_url: Option<String>,
        hub_port: Option<u16>,
    ) -> Result<()> {
        info!("Starting tunnel for port {} with TTL {}", port, ttl);

        // TDP-14: pass the REAL local port and requested subdomain to the API
        // (previously hard-coded to 8080/"dev-tunnel" in the key request).
        let key_pair = self
            .ssh_key_manager
            .get_or_request_key_pair(ttl, port, subdomain.as_deref())
            .await?;
        info!(
            "Using SSH key pair with fingerprint: {}",
            key_pair.fingerprint
        );

        // The API's tunnel id is the session identity — don't mint a second one.
        let session_id = key_pair.session_id.clone();

        // TDP-14: the FQDN comes from the API response. Only if the API did
        // not return one (older API) fall back to subdomain + configured
        // domain — never a hard-coded domain that differs from the hub's.
        let subdomain_name = key_pair
            .subdomain
            .clone()
            .or(subdomain)
            .unwrap_or_else(|| "test".to_string());
        let fqdn = key_pair.fqdn.clone().unwrap_or_else(|| {
            let domain = std::env::var("EDF_PUBLIC_DOMAIN")
                .unwrap_or_else(|_| "fleetingdns.run".to_string());
            format!("{subdomain_name}.{domain}")
        });

        // R5: Use the API-issued slot (allocated_port) from the key pair response.
        // The API already allocated this port via POST /v1/tunnels.
        let allocated_port = key_pair.slot;

        let session = TunnelSession {
            id: session_id.clone(),
            fqdn: fqdn.clone(),
            slot: allocated_port,
            pubkey: key_pair.public_key.clone(),
            cert: String::new(),
            private_key: key_pair.private_key.clone(),
            expires_at: key_pair.expires_at,
            github_id: String::new(),
            local_port: port,
            status: TunnelStatus::Creating,
        };

        self.active_sessions
            .insert(session.id.clone(), session.clone());

        info!("Tunnel session created: {}", session.id);
        println!("🚀 Tunnel created successfully!");
        println!("   ID: {}", session.id);
        println!("   FQDN: {}", session.fqdn);
        println!("   Local Port: {}", session.local_port);
        println!("   Expires: {}", session.expires_at);
        println!("   SSH Key Fingerprint: {}", key_pair.fingerprint);

        // Use provided hub configuration or fall back to defaults
        let hub_url = hub_url.unwrap_or_else(|| "localhost".to_string());
        let hub_port = hub_port.unwrap_or(2222); // Changed from 8443 to 2222 for plain SSH

        // Create tunnel client and establish connection
        let mut tunnel_client = TunnelClient::new(
            TunnelClientConfig {
                hub_url: hub_url.split(':').next().unwrap_or(&hub_url).to_string(),
                hub_port,
                local_port: port,
                subdomain: subdomain_name.clone(),
                connection_timeout: Duration::from_secs(30),
                keep_alive_interval: Duration::from_secs(60),
            },
            session_id.clone(),
        );

        // Create the SSH handler
        let handler = SshClientHandler::new(session_id.clone(), subdomain_name.clone(), port);

        // TDP-12: decode the API-issued private key for SSH auth. If it does
        // not parse we FAIL — no silent fallback to a throwaway key.
        let ssh_key = russh::keys::decode_secret_key(&key_pair.private_key, None)
            .map_err(|e| anyhow::anyhow!("API-issued private key failed to parse: {e}"))?;

        // Establish SSH tunnel
        match tunnel_client.establish_tunnel(handler, ssh_key).await {
            Ok(()) => {
                info!(
                    "SSH connection established successfully for session: {}",
                    session_id
                );
                println!("✅ SSH connection established successfully!");

                // Request reverse tunnel (reverse port forwarding) using the allocated port
                let allocated_port = session.slot; // API-issued slot (R5)
                match tunnel_client.request_reverse_tunnel(allocated_port).await {
                    Ok(()) => {
                        info!(
                            "Reverse tunnel established successfully for session: {}",
                            session_id
                        );
                        println!("✅ Reverse tunnel established successfully!");
                        println!("   Your service is now accessible at: https://{}", fqdn);
                        println!("   Press Ctrl+C to close the tunnel");

                        // Update session status to active
                        if let Some(session) = self.active_sessions.get_mut(&session_id) {
                            session.status = TunnelStatus::Active;
                        }

                        // TDP-11: hold the tunnel open by watching the REAL
                        // SSH session (keepalives + disconnect detection),
                        // not a local HashMap. Ctrl+C closes cleanly.
                        let expires_at = session.expires_at;
                        let result = tokio::select! {
                            r = tunnel_client.run_until_expiry(expires_at) => r,
                            _ = tokio::signal::ctrl_c() => {
                                println!("\n⏹  Closing tunnel...");
                                tunnel_client.close_tunnel().await
                            }
                        };

                        if let Some(session) = self.active_sessions.get_mut(&session_id) {
                            session.status = match &result {
                                Ok(()) => TunnelStatus::Closed,
                                Err(e) => TunnelStatus::Error(e.to_string()),
                            };
                        }
                        if let Err(e) = result {
                            error!("Tunnel ended abnormally: {}", e);
                            println!("❌ Tunnel lost: {e}");
                            return Err(e);
                        }
                        println!("✅ Tunnel closed");
                    }
                    Err(e) => {
                        error!("Failed to establish reverse tunnel: {}", e);

                        // Update session status to error
                        if let Some(session) = self.active_sessions.get_mut(&session_id) {
                            session.status = TunnelStatus::Error(e.to_string());
                        }

                        return Err(anyhow::anyhow!(
                            "Reverse tunnel establishment failed: {}",
                            e
                        ));
                    }
                }
            }
            Err(e) => {
                error!("Failed to establish SSH connection: {}", e);

                // Update session status to error
                if let Some(session) = self.active_sessions.get_mut(&session_id) {
                    session.status = TunnelStatus::Error(e.to_string());
                }

                return Err(anyhow::anyhow!("SSH connection failed: {}", e));
            }
        }

        Ok(())
    }

    // TDP-11: the old `keep_tunnel_alive` was deleted. It ticked every
    // second over a local HashMap that nothing else updated, so it would
    // happily report an active tunnel long after the SSH session died.
    // Liveness now lives in `TunnelClient::run_until_expiry`.

    pub async fn list_tunnels(&self) -> Result<()> {
        if self.active_sessions.is_empty() {
            println!("No active tunnels found.");
            return Ok(());
        }

        println!("Active Tunnels:");
        println!(
            "{:<36} {:<20} {:<10} {:<20}",
            "ID", "FQDN", "Port", "Status"
        );
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

    #[allow(dead_code)] // exercised in tests
    pub fn get_session(&self, id: &str) -> Option<&TunnelSession> {
        self.active_sessions.get(id)
    }

    #[allow(dead_code)] // exercised in tests
    pub fn cleanup_expired_sessions(&mut self) {
        let now = Utc::now();
        let expired_ids: Vec<String> = self
            .active_sessions
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

    fn manager_with(sessions: Vec<TunnelSession>) -> TunnelManager {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let dir = temp_dir.keep();
        let ssh_key_manager = SshKeyManager {
            key_directory: dir,
            api_client: reqwest::Client::new(),
            api_base_url: "https://test.api.invalid".to_string(),
            auth_token: None,
        };
        let mut active_sessions = HashMap::new();
        for s in sessions {
            active_sessions.insert(s.id.clone(), s);
        }
        TunnelManager {
            ssh_key_manager,
            active_sessions,
        }
    }

    fn session(id: &str, expires_at: DateTime<Utc>) -> TunnelSession {
        TunnelSession {
            id: id.to_string(),
            fqdn: format!("{id}.fleetingdns.run"),
            slot: 40001,
            pubkey: "pk".to_string(),
            cert: String::new(),
            private_key: "sk".to_string(),
            expires_at,
            github_id: String::new(),
            local_port: 3000,
            status: TunnelStatus::Active,
        }
    }

    #[tokio::test]
    async fn list_tunnels_empty_and_populated() {
        let mgr = manager_with(vec![]);
        mgr.list_tunnels().await.unwrap();
        let mgr = manager_with(vec![session(
            "t1",
            Utc::now() + chrono::Duration::minutes(5),
        )]);
        mgr.list_tunnels().await.unwrap();
    }

    #[tokio::test]
    async fn close_tunnel_removes_session() {
        let mut mgr = manager_with(vec![session(
            "t1",
            Utc::now() + chrono::Duration::minutes(5),
        )]);
        assert!(mgr.get_session("t1").is_some());
        mgr.close_tunnel("t1").await.unwrap();
        assert!(mgr.get_session("t1").is_none());
        // closing an unknown tunnel is not an error
        mgr.close_tunnel("nope").await.unwrap();
    }

    #[test]
    fn cleanup_removes_only_expired() {
        let mut mgr = manager_with(vec![
            session("live", Utc::now() + chrono::Duration::minutes(5)),
            session("dead", Utc::now() - chrono::Duration::minutes(5)),
        ]);
        mgr.cleanup_expired_sessions();
        assert!(mgr.get_session("live").is_some());
        assert!(mgr.get_session("dead").is_none());
    }

    #[test]
    fn tunnel_status_error_roundtrip() {
        let status = TunnelStatus::Error("boom".to_string());
        let json = serde_json::to_string(&status).unwrap();
        let back: TunnelStatus = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, TunnelStatus::Error(m) if m == "boom"));
    }

    #[test]
    fn test_tunnel_status_serialization() {
        let status = TunnelStatus::Active;
        let serialized = serde_json::to_string(&status).unwrap();
        let deserialized: TunnelStatus = serde_json::from_str(&serialized).unwrap();

        assert!(matches!(deserialized, TunnelStatus::Active));
    }
}
