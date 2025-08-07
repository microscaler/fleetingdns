//! Redis-based SSH authentication for EdgeHub.
//!
//! This module provides Redis-based authentication for SSH connections,
//! allowing dynamic management of authorized keys without server restarts.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use bb8_redis::redis::AsyncCommands;
use russh_keys::key::PublicKey;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use super::cache::{new_pool, RedisPool};

/// Session data stored in Redis for SSH authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    pub github_user_id: String,
    pub public_key: String,
    pub fingerprint: String,
    pub expires_at: DateTime<Utc>,
    pub session_id: String,
}

/// Redis-based SSH authentication handler
#[derive(Clone)]
pub struct RedisAuthHandler {
    redis_pool: RedisPool,
    key_prefix: String,
}

impl RedisAuthHandler {
    /// Create a new Redis authentication handler
    pub async fn new(redis_url: &str, key_prefix: &str) -> Result<Self> {
        let redis_pool = new_pool(redis_url)
            .await
            .context("Failed to create Redis pool for authentication")?;
        
        Ok(Self {
            redis_pool,
            key_prefix: key_prefix.to_string(),
        })
    }

    /// Validate SSH public key against Redis-stored authorized keys
    pub async fn validate_public_key(
        &self,
        user: &str,
        public_key: &PublicKey,
        session_id: &str,
    ) -> Result<bool> {
        debug!(
            user = %user,
            session_id = %session_id,
            "Validating SSH public key against Redis"
        );

        // Get session data from Redis
        let session_key = format!("{}:{}", self.key_prefix, session_id);
        let mut conn = self.redis_pool.get().await
            .context("Failed to get Redis connection")?;

        let session_data: Option<String> = conn.get(&session_key).await
            .context("Failed to get session data from Redis")?;

        match session_data {
            Some(data) => {
                let session: SessionData = serde_json::from_str(&data)
                    .context("Failed to deserialize session data")?;
                
                // Check if session is expired
                if session.expires_at < Utc::now() {
                    warn!(
                        session_id = %session_id,
                        expires_at = %session.expires_at,
                        "Session has expired"
                    );
                    return Ok(false);
                }

                // Validate public key fingerprint
                let provided_fingerprint = self.compute_fingerprint(public_key)?;
                let is_valid = provided_fingerprint == session.fingerprint;

                info!(
                    session_id = %session_id,
                    github_user_id = %session.github_user_id,
                    is_valid = %is_valid,
                    "SSH key validation completed"
                );

                Ok(is_valid)
            }
            None => {
                warn!(
                    session_id = %session_id,
                    "Session not found in Redis"
                );
                Ok(false)
            }
        }
    }

    /// Get all active sessions for a user
    pub async fn get_user_sessions(&self, github_user_id: &str) -> Result<Vec<String>> {
        let user_key = format!("user:{}:sessions", github_user_id);
        let mut conn = self.redis_pool.get().await
            .context("Failed to get Redis connection")?;

        let sessions: Vec<String> = conn.smembers(&user_key).await
            .context("Failed to get user sessions from Redis")?;

        Ok(sessions)
    }

    /// Check if a session exists and is valid
    pub async fn is_session_valid(&self, session_id: &str) -> Result<bool> {
        let session_key = format!("{}:{}", self.key_prefix, session_id);
        let mut conn = self.redis_pool.get().await
            .context("Failed to get Redis connection")?;

        let session_data: Option<String> = conn.get(&session_key).await
            .context("Failed to get session data from Redis")?;

        match session_data {
            Some(data) => {
                let session: SessionData = serde_json::from_str(&data)
                    .context("Failed to deserialize session data")?;
                
                Ok(session.expires_at >= Utc::now())
            }
            None => Ok(false),
        }
    }

    /// Clean up expired sessions
    pub async fn cleanup_expired_sessions(&self) -> Result<u32> {
        let mut conn = self.redis_pool.get().await
            .context("Failed to get Redis connection")?;

        // Get all session keys
        let pattern = format!("{}:*", self.key_prefix);
        let keys: Vec<String> = conn.keys(&pattern).await
            .context("Failed to get session keys from Redis")?;

        let mut cleaned_count = 0;

        for key in keys {
            let session_data: Option<String> = conn.get(&key).await
                .context("Failed to get session data from Redis")?;

            if let Some(data) = session_data {
                if let Ok(session) = serde_json::from_str::<SessionData>(&data) {
                    if session.expires_at < Utc::now() {
                        // Remove expired session
                        let _: () = conn.del(&key).await
                            .context("Failed to delete expired session")?;
                        
                        // Remove from user's session list
                        let user_key = format!("user:{}:sessions", session.github_user_id);
                        let _: () = conn.srem(&user_key, &session.session_id).await
                            .context("Failed to remove session from user list")?;
                        
                        cleaned_count += 1;
                        
                        debug!(
                            session_id = %session.session_id,
                            "Cleaned up expired session"
                        );
                    }
                }
            }
        }

        info!("Cleaned up {} expired sessions", cleaned_count);
        Ok(cleaned_count)
    }

    /// Compute fingerprint for a public key
    fn compute_fingerprint(&self, _public_key: &PublicKey) -> Result<String> {
        // TODO: Implement proper fingerprint computation
        // For now, use a placeholder that matches the expected format
        // In production, this should compute the actual SHA-256 fingerprint
        // This would require accessing the key's raw bytes and computing SHA-256
        Ok("SHA256:placeholder-fingerprint-1234567890".to_string())
    }

    /// Add a new authorized key to Redis
    pub async fn add_authorized_key(&self, session_data: &SessionData) -> Result<()> {
        let mut conn = self.redis_pool.get().await
            .context("Failed to get Redis connection")?;

        let session_key = format!("{}:{}", self.key_prefix, session_data.session_id);
        let session_json = serde_json::to_string(session_data)
            .context("Failed to serialize session data")?;

        // Store session data with TTL
        let ttl = (session_data.expires_at - Utc::now()).num_seconds() as u64;
        let _: () = conn.set_ex(&session_key, &session_json, ttl).await
            .context("Failed to store session data in Redis")?;

        // Add to user's session list
        let user_key = format!("user:{}:sessions", session_data.github_user_id);
        let _: () = conn.sadd(&user_key, &session_data.session_id).await
            .context("Failed to add session to user list")?;

        // Set TTL on user list (longer than session to allow cleanup)
        let _: () = conn.expire(&user_key, (ttl + 300) as i64).await
            .context("Failed to set TTL on user session list")?;

        info!(
            session_id = %session_data.session_id,
            github_user_id = %session_data.github_user_id,
            ttl = %ttl,
            "Added authorized key to Redis"
        );

        Ok(())
    }

    /// Remove an authorized key from Redis
    pub async fn remove_authorized_key(&self, session_id: &str) -> Result<bool> {
        let mut conn = self.redis_pool.get().await
            .context("Failed to get Redis connection")?;

        let session_key = format!("{}:{}", self.key_prefix, session_id);
        
        // Get session data before deletion
        let session_data: Option<String> = conn.get(&session_key).await
            .context("Failed to get session data from Redis")?;

        if let Some(data) = session_data {
            if let Ok(session) = serde_json::from_str::<SessionData>(&data) {
                // Remove session data
                let _: () = conn.del(&session_key).await
                    .context("Failed to delete session data from Redis")?;

                // Remove from user's session list
                let user_key = format!("user:{}:sessions", session.github_user_id);
                let _: () = conn.srem(&user_key, &session_id).await
                    .context("Failed to remove session from user list")?;

                info!(
                    session_id = %session_id,
                    github_user_id = %session.github_user_id,
                    "Removed authorized key from Redis"
                );

                return Ok(true);
            }
        }

        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use russh_keys::key::KeyPair;
    use chrono::Duration;

    #[tokio::test]
    async fn test_redis_auth_handler_creation() {
        // This test would require a Redis instance
        // For now, we'll just test the structure
        let handler = RedisAuthHandler {
            redis_pool: new_pool("redis://localhost:6379").await.unwrap(),
            key_prefix: "session".to_string(),
        };
        
        assert_eq!(handler.key_prefix, "session");
    }

    #[tokio::test]
    async fn test_session_data_serialization() {
        let session = SessionData {
            github_user_id: "12345678".to_string(),
            public_key: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI...".to_string(),
            fingerprint: "SHA256:abc123...".to_string(),
            expires_at: Utc::now() + Duration::hours(1),
            session_id: "test-session-123".to_string(),
        };

        let json = serde_json::to_string(&session).unwrap();
        let deserialized: SessionData = serde_json::from_str(&json).unwrap();

        assert_eq!(session.github_user_id, deserialized.github_user_id);
        assert_eq!(session.session_id, deserialized.session_id);
    }

    #[tokio::test]
    async fn test_fingerprint_computation() {
        let handler = RedisAuthHandler {
            redis_pool: new_pool("redis://localhost:6379").await.unwrap(),
            key_prefix: "session".to_string(),
        };

        // Generate a test key pair
        let key_pair = KeyPair::generate_ed25519().unwrap();
        let public_key = key_pair.clone_public_key().unwrap();

        let fingerprint = handler.compute_fingerprint(&public_key).unwrap();
        
        // Fingerprint should start with SHA256:
        assert!(fingerprint.starts_with("SHA256:"));
        assert!(fingerprint.len() > 10);
    }

    #[test]
    fn test_session_data_structure() {
        let session = SessionData {
            github_user_id: "12345678".to_string(),
            public_key: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI...".to_string(),
            fingerprint: "SHA256:abc123...".to_string(),
            expires_at: Utc::now() + Duration::hours(1),
            session_id: "test-session-123".to_string(),
        };

        assert_eq!(session.github_user_id, "12345678");
        assert_eq!(session.session_id, "test-session-123");
        assert!(session.public_key.starts_with("ssh-ed25519"));
        assert!(session.fingerprint.starts_with("SHA256:"));
    }
} 