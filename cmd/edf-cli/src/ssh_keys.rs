use chrono::{DateTime, Utc};
use common::error::{CommonResult, FleetingDnsError};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::info;

// Remove the old SshKeyError enum and replace with type alias
pub type SshKeyError = FleetingDnsError;
#[allow(dead_code)] // public alias kept for downstream callers
pub type SshKeyResult<T> = CommonResult<T>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshKeyPair {
    pub public_key: String,
    pub private_key: String,
    pub key_type: String,
    pub fingerprint: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub session_id: String,
    /// R5: API-issued slot (allocated port) — not fabricated client-side.
    pub slot: u16,
    /// TDP-14: API-issued public FQDN — never assembled client-side.
    /// Optional with a serde default so pre-TDP-14 session files still load.
    #[serde(default)]
    pub fqdn: Option<String>,
    /// TDP-14: API-issued subdomain (routing key).
    #[serde(default)]
    pub subdomain: Option<String>,
}

pub struct SshKeyManager {
    pub key_directory: PathBuf,
    pub api_client: Client,
    pub api_base_url: String,
    pub auth_token: Option<String>,
}

impl SshKeyManager {
    pub fn new() -> Result<Self, SshKeyError> {
        let key_dir = Self::get_key_directory()?;
        fs::create_dir_all(&key_dir)
            .map_err(|e| FleetingDnsError::Io(format!("Failed to create key directory: {}", e)))?;

        let api_base_url =
            std::env::var("EDF_API_URL").unwrap_or_else(|_| "https://api.edf.run".to_string());

        Ok(Self {
            key_directory: key_dir,
            api_client: Client::new(),
            api_base_url,
            auth_token: None,
        })
    }

    #[allow(dead_code)] // used once auth tokens are required (TDP-13)
    pub fn with_auth_token(mut self, token: String) -> Self {
        self.auth_token = Some(token);
        self
    }

    /// TDP-14: the tunnel request carries the CLI's REAL local port and
    /// requested subdomain — previously hard-coded to 8080/"dev-tunnel".
    pub async fn request_key_pair(
        &self,
        session_ttl: u32,
        local_port: u16,
        subdomain: Option<&str>,
    ) -> Result<SshKeyPair, SshKeyError> {
        info!("Requesting SSH key pair from remote API");

        let mut tunnel_request = serde_json::json!({
            "port": local_port,
            "ttl": session_ttl,
        });
        if let Some(sub) = subdomain {
            tunnel_request["custom_subdomain"] = serde_json::json!(sub);
        }

        let mut req = self
            .api_client
            .post(format!("{}/v1/tunnels", self.api_base_url))
            .json(&tunnel_request);

        // Add development bypass header for localhost endpoints
        if self.api_base_url.contains("localhost") || self.api_base_url.contains("127.0.0.1") {
            req = req.header("x-development-bypass", "true");
            info!("🔧 Development bypass header added for localhost API");
        }

        if let Some(token) = &self.auth_token {
            req = req.header("Authorization", format!("Bearer {}", token));
        }

        let response = req.send().await.map_err(|e| {
            FleetingDnsError::ExternalService(format!("Failed to send request: {}", e))
        })?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(FleetingDnsError::ExternalService(format!(
                "API error: {}",
                error_text
            )));
        }

        // Parse the tunnel response to extract SSH key information
        let tunnel_response: serde_json::Value = response.json().await.map_err(|e| {
            FleetingDnsError::ExternalService(format!("Failed to parse response: {}", e))
        })?;

        // Extract SSH key from tunnel response
        let ssh_key = tunnel_response.get("ssh_key").ok_or_else(|| {
            FleetingDnsError::ExternalService("No SSH key in tunnel response".to_string())
        })?;

        let key_pair = SshKeyPair {
            public_key: ssh_key
                .get("public_key")
                .and_then(|v| v.as_str())
                .ok_or_else(|| FleetingDnsError::ExternalService("Missing public_key".to_string()))?
                .to_string(),
            private_key: ssh_key
                .get("private_key")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    FleetingDnsError::ExternalService("Missing private_key".to_string())
                })?
                .to_string(),
            key_type: ssh_key
                .get("key_type")
                .and_then(|v| v.as_str())
                .unwrap_or("ed25519")
                .to_string(),
            fingerprint: ssh_key
                .get("fingerprint")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    FleetingDnsError::ExternalService("Missing fingerprint".to_string())
                })?
                .to_string(),
            created_at: Utc::now(),
            expires_at: tunnel_response
                .get("expires_at")
                .and_then(|v| v.as_str())
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map_or_else(
                    || Utc::now() + chrono::Duration::seconds(session_ttl as i64),
                    |dt| dt.with_timezone(&Utc),
                ),
            session_id: tunnel_response
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| FleetingDnsError::ExternalService("Missing tunnel ID".to_string()))?
                .to_string(),
            // R5: Extract the API-issued slot (allocated port) from the response.
            slot: tunnel_response
                .get("slot")
                .and_then(serde_json::Value::as_u64)
                .map(|v| v as u16)
                .ok_or_else(|| {
                    FleetingDnsError::ExternalService("Missing slot in tunnel response".to_string())
                })?,
            // TDP-14: the API is the single source of truth for the FQDN.
            fqdn: tunnel_response
                .get("fqdn")
                .and_then(|v| v.as_str())
                .map(std::string::ToString::to_string),
            subdomain: tunnel_response
                .get("subdomain")
                .and_then(|v| v.as_str())
                .map(std::string::ToString::to_string),
        };

        // Store the private key locally with proper naming
        let private_key_path = self.store_private_key(&key_pair)?;

        info!(
            "SSH key pair received from API with fingerprint: {}",
            key_pair.fingerprint
        );
        info!("Private key stored at: {}", private_key_path.display());

        Ok(key_pair)
    }

    pub fn load_existing_key_pair(&self) -> Result<SshKeyPair, SshKeyError> {
        let session_info_path = self.key_directory.join("session_info.json");

        if !session_info_path.exists() {
            return Err(FleetingDnsError::NotFound(format!(
                "No active session found in {}",
                self.key_directory.display()
            )));
        }

        let session_info = fs::read_to_string(&session_info_path)
            .map_err(|e| FleetingDnsError::Io(format!("Failed to read session info: {}", e)))?;

        let session_data: SshKeyPair = serde_json::from_str(&session_info).map_err(|e| {
            FleetingDnsError::ValidationError(format!("Failed to parse session info: {}", e))
        })?;

        // Check if session is expired
        if session_data.expires_at < Utc::now() {
            self.cleanup_session()?;
            return Err(FleetingDnsError::NotFound(
                "Session has expired".to_string(),
            ));
        }

        // Find the private key file in ~/.ssh
        let ssh_dir = dirs::home_dir()
            .ok_or_else(|| FleetingDnsError::Io("Could not determine home directory".to_string()))?
            .join(".ssh");

        let expiry_str = session_data.expires_at.format("%Y%m%d-%H%M%S").to_string();
        let filename = format!("edf-cli-{}-{}.priv", expiry_str, session_data.session_id);
        let private_key_path = ssh_dir.join(&filename);

        if !private_key_path.exists() {
            return Err(FleetingDnsError::NotFound(format!(
                "Private key file not found: {}",
                private_key_path.display()
            )));
        }

        let private_key = fs::read_to_string(&private_key_path)
            .map_err(|e| FleetingDnsError::Io(format!("Failed to read private key: {}", e)))?;

        Ok(SshKeyPair {
            private_key,
            ..session_data
        })
    }

    pub async fn get_or_request_key_pair(
        &self,
        session_ttl: u32,
        local_port: u16,
        subdomain: Option<&str>,
    ) -> Result<SshKeyPair, SshKeyError> {
        match self.load_existing_key_pair() {
            Ok(key_pair) => {
                info!(
                    "Using existing SSH key pair with fingerprint: {}",
                    key_pair.fingerprint
                );
                Ok(key_pair)
            }
            Err(FleetingDnsError::NotFound(_)) => {
                info!("No existing SSH keys found, requesting from API");
                self.request_key_pair(session_ttl, local_port, subdomain)
                    .await
            }
            Err(e) => Err(e),
        }
    }

    pub fn cleanup_session(&self) -> Result<(), SshKeyError> {
        let session_info_path = self.key_directory.join("session_info.json");

        // Clean up session info
        if session_info_path.exists() {
            // Read session info to get the private key path
            let session_info = fs::read_to_string(&session_info_path)
                .map_err(|e| FleetingDnsError::Io(format!("Failed to read session info: {}", e)))?;

            let session_data: SshKeyPair = serde_json::from_str(&session_info).map_err(|e| {
                FleetingDnsError::ValidationError(format!("Failed to parse session info: {}", e))
            })?;

            // Clean up private key in ~/.ssh
            let ssh_dir = dirs::home_dir()
                .ok_or_else(|| {
                    FleetingDnsError::Io("Could not determine home directory".to_string())
                })?
                .join(".ssh");

            let expiry_str = session_data.expires_at.format("%Y%m%d-%H%M%S").to_string();
            let filename = format!("edf-cli-{}-{}.priv", expiry_str, session_data.session_id);
            let private_key_path = ssh_dir.join(&filename);

            if private_key_path.exists() {
                fs::remove_file(&private_key_path).map_err(|e| {
                    FleetingDnsError::Io(format!("Failed to remove private key: {}", e))
                })?;
                info!("Removed private key: {}", private_key_path.display());
            }

            // Remove session info
            fs::remove_file(&session_info_path).map_err(|e| {
                FleetingDnsError::Io(format!("Failed to remove session info: {}", e))
            })?;
            info!("Removed session info: {}", session_info_path.display());
        }

        Ok(())
    }

    fn store_private_key(&self, key_pair: &SshKeyPair) -> Result<PathBuf, SshKeyError> {
        // Create SSH directory if it doesn't exist
        let ssh_dir = dirs::home_dir()
            .ok_or_else(|| FleetingDnsError::Io("Could not determine home directory".to_string()))?
            .join(".ssh");

        fs::create_dir_all(&ssh_dir)
            .map_err(|e| FleetingDnsError::Io(format!("Failed to create SSH directory: {}", e)))?;

        // Generate filename with expiry time and UUID
        let expiry_str = key_pair.expires_at.format("%Y%m%d-%H%M%S").to_string();
        let filename = format!("edf-cli-{}-{}.priv", expiry_str, key_pair.session_id);
        let private_key_path = ssh_dir.join(&filename);

        // Store private key
        fs::write(&private_key_path, &key_pair.private_key)
            .map_err(|e| FleetingDnsError::Io(format!("Failed to write private key: {}", e)))?;

        // Set proper permissions on private key (600)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&private_key_path)?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&private_key_path, perms).map_err(|e| {
                FleetingDnsError::Io(format!("Failed to set key permissions: {}", e))
            })?;
        }

        // Store session info in .edf directory
        let session_info_path = self.key_directory.join("session_info.json");
        let session_info = SshKeyPair {
            private_key: String::new(), // Don't store private key in JSON
            public_key: key_pair.public_key.clone(),
            key_type: key_pair.key_type.clone(),
            fingerprint: key_pair.fingerprint.clone(),
            created_at: key_pair.created_at,
            expires_at: key_pair.expires_at,
            session_id: key_pair.session_id.clone(),
            slot: key_pair.slot,
            fqdn: key_pair.fqdn.clone(),
            subdomain: key_pair.subdomain.clone(),
        };

        let session_json = serde_json::to_string_pretty(&session_info).map_err(|e| {
            FleetingDnsError::Io(format!("Failed to serialize session info: {}", e))
        })?;

        fs::write(&session_info_path, session_json)
            .map_err(|e| FleetingDnsError::Io(format!("Failed to write session info: {}", e)))?;

        Ok(private_key_path)
    }

    fn get_key_directory() -> Result<PathBuf, SshKeyError> {
        let home_dir = dirs::home_dir().ok_or_else(|| {
            FleetingDnsError::Io("Could not determine home directory".to_string())
        })?;

        Ok(home_dir.join(".edf").join("keys"))
    }

    #[allow(dead_code)] // exercised in tests; CLI validation entrypoint
    pub fn validate_key_pair(&self, key_pair: &SshKeyPair) -> Result<bool, SshKeyError> {
        // Basic validation
        if key_pair.public_key.is_empty() || key_pair.private_key.is_empty() {
            return Ok(false);
        }

        if !key_pair.public_key.starts_with("ssh-ed25519") {
            return Ok(false);
        }

        if !key_pair
            .private_key
            .starts_with("-----BEGIN OPENSSH PRIVATE KEY-----")
        {
            return Ok(false);
        }

        // Check if session is expired
        if key_pair.expires_at < Utc::now() {
            return Ok(false);
        }

        Ok(true)
    }

    pub async fn test_key_storage(&self) -> Result<SshKeyPair, SshKeyError> {
        // Create a mock key pair for testing
        let key_pair = SshKeyPair {
            public_key: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI... test-key".to_string(),
            private_key: "[REDACTED PRIVATE KEY]".to_string(),
            key_type: "ed25519".to_string(),
            fingerprint: "SHA256:test-fingerprint-1234567890".to_string(),
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(1),
            session_id: "test-session-123".to_string(),
            slot: 50000, // mock slot for test
            fqdn: Some("test.fleetingdns.run".to_string()),
            subdomain: Some("test".to_string()),
        };

        // Store the private key locally with proper naming
        let private_key_path = self.store_private_key(&key_pair)?;

        info!(
            "Test SSH key pair created with fingerprint: {}",
            key_pair.fingerprint
        );
        info!("Private key stored at: {}", private_key_path.display());

        Ok(key_pair)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_ssh_key_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let key_manager = SshKeyManager {
            key_directory: temp_dir.path().to_path_buf(),
            api_client: Client::new(),
            api_base_url: "https://test.api.edf.run".to_string(),
            auth_token: None,
        };

        assert_eq!(key_manager.api_base_url, "https://test.api.edf.run");
    }

    #[test]
    fn validate_key_pair_paths() {
        let temp_dir = TempDir::new().unwrap();
        let km = SshKeyManager {
            key_directory: temp_dir.path().to_path_buf(),
            api_client: Client::new(),
            api_base_url: "https://test.api.invalid".to_string(),
            auth_token: None,
        };
        let good = SshKeyPair {
            public_key: "ssh-ed25519 AAAA test".to_string(),
            private_key:
                "-----BEGIN OPENSSH PRIVATE KEY-----\nx\n-----END OPENSSH PRIVATE KEY-----"
                    .to_string(),
            key_type: "ed25519".to_string(),
            fingerprint: "SHA256:x".to_string(),
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(1),
            session_id: "s".to_string(),
            slot: 40001,
            fqdn: Some("s.fleetingdns.run".to_string()),
            subdomain: Some("s".to_string()),
        };
        assert!(km.validate_key_pair(&good).unwrap());

        // wrong key type prefix
        let mut bad = good.clone();
        bad.public_key = "ssh-rsa AAAA test".to_string();
        assert!(!km.validate_key_pair(&bad).unwrap());

        // wrong private key armor
        let mut bad = good.clone();
        bad.private_key = "-----BEGIN RSA PRIVATE KEY-----".to_string();
        assert!(!km.validate_key_pair(&bad).unwrap());

        // expired session
        let mut bad = good.clone();
        bad.expires_at = Utc::now() - chrono::Duration::hours(1);
        assert!(!km.validate_key_pair(&bad).unwrap());
    }

    #[test]
    fn load_existing_key_pair_reports_missing_session() {
        let temp_dir = TempDir::new().unwrap();
        let km = SshKeyManager {
            key_directory: temp_dir.path().to_path_buf(),
            api_client: Client::new(),
            api_base_url: "https://test.api.invalid".to_string(),
            auth_token: None,
        };
        let err = km.load_existing_key_pair().unwrap_err();
        assert!(matches!(err, FleetingDnsError::NotFound(_)));
        // cleanup of a non-existent session is a no-op, not an error
        km.cleanup_session().unwrap();
    }

    #[test]
    fn session_info_serde_defaults_accept_pre_tdp14_files() {
        // Old session files have no fqdn/subdomain keys (TDP-14 serde defaults).
        let old_json = serde_json::json!({
            "public_key": "ssh-ed25519 AAAA",
            "private_key": "",
            "key_type": "ed25519",
            "fingerprint": "SHA256:x",
            "created_at": Utc::now().to_rfc3339(),
            "expires_at": (Utc::now() + chrono::Duration::hours(1)).to_rfc3339(),
            "session_id": "old",
            "slot": 40100
        });
        let parsed: SshKeyPair = serde_json::from_value(old_json).unwrap();
        assert_eq!(parsed.slot, 40100);
        assert!(parsed.fqdn.is_none());
        assert!(parsed.subdomain.is_none());
    }

    #[test]
    fn test_key_pair_validation() {
        let temp_dir = TempDir::new().unwrap();
        let key_manager = SshKeyManager {
            key_directory: temp_dir.path().to_path_buf(),
            api_client: Client::new(),
            api_base_url: "https://test.api.edf.run".to_string(),
            auth_token: None,
        };

        let invalid_key_pair = SshKeyPair {
            public_key: String::new(),
            private_key: String::new(),
            key_type: "ed25519".to_string(),
            fingerprint: String::new(),
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(1),
            session_id: "test-session".to_string(),
            slot: 50000,
            fqdn: None,
            subdomain: None,
        };

        assert!(!key_manager.validate_key_pair(&invalid_key_pair).unwrap());
    }
}
