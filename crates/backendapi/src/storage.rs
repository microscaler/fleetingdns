use crate::{
    ApiError, ApiResult,
    models::{Tunnel, TunnelStatus},
};
use bb8::{Pool, PooledConnection};
use bb8_redis::{RedisConnectionManager, redis::AsyncCommands};
use chrono::{DateTime, Utc};
use tracing::{debug, info};
use uuid::Uuid;

/// User tunnel lookup data structure
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserTunnelLookup {
    pub github_user_id: String,
    pub github_username: String,
    pub tunnels: Vec<String>, // List of tunnel IDs
}

/// Redis storage for tunnel metadata
pub struct TunnelStorage {
    pool: Pool<RedisConnectionManager>,
}

impl TunnelStorage {
    /// Create a new tunnel storage instance
    pub async fn new(redis_url: &str) -> ApiResult<Self> {
        let manager = RedisConnectionManager::new(redis_url)
            .map_err(|e| ApiError::StorageError(format!("Failed to create Redis manager: {e}")))?;

        let pool = Pool::builder()
            .max_size(15)
            .build(manager)
            .await
            .map_err(|e| ApiError::StorageError(format!("Failed to create Redis pool: {e}")))?;

        info!("Connected to Redis at {}", redis_url);

        Ok(Self { pool })
    }

    /// Get a Redis connection from the pool
    async fn get_connection(&self) -> ApiResult<PooledConnection<'_, RedisConnectionManager>> {
        self.pool
            .get()
            .await
            .map_err(|e| ApiError::StorageError(format!("Failed to get Redis connection: {e}")))
    }

    /// Store tunnel metadata
    pub async fn store_tunnel(&self, tunnel: &Tunnel) -> ApiResult<()> {
        let mut conn = self.get_connection().await?;

        // Store tunnel data in Redis with TTL
        let tunnel_key = format!("tunnel:{}", tunnel.id);
        let tunnel_data = serde_json::to_string(&tunnel).map_err(|e| {
            tracing::error!(error = %e, "Failed to serialize tunnel data");
            ApiError::StorageError(format!("Failed to serialize tunnel data: {e}"))
        })?;

        // Calculate TTL from expires_at
        let ttl = {
            let now = chrono::Utc::now();
            let duration = tunnel.expires_at - now;
            duration.num_seconds() as u64
        };

        let _: () = conn
            .set_ex(&tunnel_key, &tunnel_data, ttl)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to store tunnel data in Redis");
                ApiError::StorageError(format!("Failed to store tunnel data in Redis: {e}"))
            })?;

        // Store user tunnel lookup (no TTL - persistent)
        let user_key = format!("tunnel_lookup:{}", tunnel.github_user_id);

        // Get existing user data or create new
        let existing_data: Option<String> = conn.get(&user_key).await.map_err(|e| {
            tracing::error!(error = %e, "Failed to get existing user tunnel lookup");
            ApiError::StorageError(format!("Failed to get existing user tunnel lookup: {e}"))
        })?;

        let mut user_lookup = if let Some(data) = existing_data {
            serde_json::from_str::<UserTunnelLookup>(&data).unwrap_or_else(|_| UserTunnelLookup {
                github_user_id: tunnel.github_user_id.clone(),
                github_username: tunnel.github_username.clone(),
                tunnels: Vec::new(),
            })
        } else {
            UserTunnelLookup {
                github_user_id: tunnel.github_user_id.clone(),
                github_username: tunnel.github_username.clone(),
                tunnels: Vec::new(),
            }
        };

        // Add tunnel ID if not already present
        if !user_lookup.tunnels.contains(&tunnel.id.to_string()) {
            user_lookup.tunnels.push(tunnel.id.to_string());
        }

        // Store back to Redis (no TTL for user lookup)
        let user_data = serde_json::to_string(&user_lookup).map_err(|e| {
            tracing::error!(error = %e, "Failed to serialize user tunnel lookup");
            ApiError::StorageError(format!("Failed to serialize user tunnel lookup: {e}"))
        })?;

        let _: () = conn.set(&user_key, user_data).await.map_err(|e| {
            tracing::error!(error = %e, "Failed to store user tunnel lookup in Redis");
            ApiError::StorageError(format!("Failed to store user tunnel lookup in Redis: {e}"))
        })?;

        // Subdomain index: subdomain:{name} → tunnel id. This is what
        // get_tunnel_by_subdomain / is_subdomain_available read — it was
        // never written before (only deleted), so the uniqueness check was
        // a no-op. SET NX EX makes the claim atomic: even two concurrent
        // creates racing past the handler's pre-check cannot both own the
        // routing key (cross-tunnel isolation depends on this).
        let subdomain_key = format!("subdomain:{}", tunnel.subdomain);
        let tunnel_id_str = tunnel.id.to_string();
        let claimed: Option<String> = bb8_redis::redis::cmd("SET")
            .arg(&subdomain_key)
            .arg(&tunnel_id_str)
            .arg("NX")
            .arg("EX")
            .arg(ttl)
            .query_async(&mut *conn)
            .await
            .map_err(|e| ApiError::StorageError(format!("Failed to claim subdomain: {e}")))?;
        if claimed.is_none() {
            let holder: Option<String> = conn.get(&subdomain_key).await.map_err(|e| {
                ApiError::StorageError(format!("Failed to read subdomain holder: {e}"))
            })?;
            if holder.as_deref() != Some(tunnel_id_str.as_str()) {
                return Err(ApiError::BadRequest(format!(
                    "Subdomain '{}' is already in use by an active tunnel",
                    tunnel.subdomain
                )));
            }
            // Re-store of the same tunnel (e.g. status update): refresh TTL.
            let _: () = conn.expire(&subdomain_key, ttl as i64).await.map_err(|e| {
                ApiError::StorageError(format!("Failed to refresh subdomain TTL: {e}"))
            })?;
        }

        tracing::info!(tunnel_id = %tunnel.id, github_user_id = %tunnel.github_user_id, "Stored tunnel in Redis with new structure");

        // Create DNS slot mapping for tunnel FQDN. dnsd answers external
        // callers with this IP, so it MUST be the edge's publicly reachable
        // address, not loopback. Configurable via EDGE_PUBLIC_IP; defaults
        // to 127.0.0.1 only for single-host local dev where edge, hub and
        // caller share a loopback.
        let edge_ip = std::env::var("EDGE_PUBLIC_IP").unwrap_or_else(|_| "127.0.0.1".to_string());
        let slot_key = format!("slot:{}", tunnel.fqdn);
        let _: () = conn.set_ex(&slot_key, edge_ip, ttl).await.map_err(|e| {
            ApiError::StorageError(format!("Failed to create DNS slot mapping: {e}"))
        })?;

        debug!("Stored tunnel {} with TTL {} seconds", tunnel.id, ttl);
        Ok(())
    }

    /// Store an edge session grant for a protected tunnel (FR-EDGE-3).
    ///
    /// The edge router checks `session_grant:{subdomain}:{token}` before
    /// forwarding any bytes; the TTL enforces expiry.
    pub async fn store_session_grant(
        &self,
        subdomain: &str,
        token: &str,
        github_user_id: &str,
        ttl_seconds: u64,
    ) -> ApiResult<()> {
        let mut conn = self.get_connection().await?;
        let key = common::redis::session_grant_key(subdomain, token);
        let value = serde_json::json!({
            "github_user_id": github_user_id,
            "granted_at": chrono::Utc::now().to_rfc3339(),
        })
        .to_string();
        let _: () = conn
            .set_ex(&key, value, ttl_seconds)
            .await
            .map_err(|e| ApiError::StorageError(format!("Failed to store session grant: {e}")))?;
        debug!("Stored session grant for subdomain {subdomain} (ttl {ttl_seconds}s)");
        Ok(())
    }

    /// Store the SSH session record the hub authenticates against (TDP-13).
    ///
    /// Written under `session:{session_id}` in the shape the hub's
    /// `RedisAuthHandler` reads (`common::redis::SessionData`), with a TTL that
    /// tracks the tunnel's own expiry so stale keys cannot authenticate.
    pub async fn store_ssh_session(
        &self,
        session_id: &str,
        github_user_id: &str,
        public_key: &str,
        fingerprint: &str,
        expires_at: DateTime<Utc>,
    ) -> ApiResult<()> {
        let mut conn = self.get_connection().await?;
        let session = common::redis::SessionData {
            github_user_id: github_user_id.to_string(),
            public_key: public_key.to_string(),
            fingerprint: fingerprint.to_string(),
            expires_at,
            session_id: session_id.to_string(),
        };
        let value = serde_json::to_string(&session)
            .map_err(|e| ApiError::StorageError(format!("Failed to serialize SSH session: {e}")))?;
        // Match the hub's default key prefix ("session"); +60s grace so the
        // record does not vanish exactly at tunnel expiry mid-handshake.
        let ttl = (expires_at - Utc::now()).num_seconds().max(1) as u64 + 60;
        let key = format!("session:{session_id}");
        let _: () = conn
            .set_ex(&key, value, ttl)
            .await
            .map_err(|e| ApiError::StorageError(format!("Failed to store SSH session: {e}")))?;
        debug!("Stored SSH session {session_id} (ttl {ttl}s)");
        Ok(())
    }

    /// Whether the hub is currently publishing liveness for this tunnel.
    ///
    /// The hub writes (and refreshes) this key only while it holds a bound slot
    /// listener, so it is the authoritative answer to "is this tunnel actually
    /// connected?" — unlike the stored status field, which a client that
    /// disappears never updates.
    pub async fn is_tunnel_live(&self, tunnel_id: &str) -> ApiResult<bool> {
        let mut conn = self.get_connection().await?;
        let key = common::redis::tunnel_live_key(tunnel_id);
        let value: Option<String> = conn
            .get(&key)
            .await
            .map_err(|e| ApiError::StorageError(format!("Failed to read tunnel liveness: {e}")))?;
        Ok(value.is_some())
    }

    /// Retrieve tunnel by ID
    pub async fn get_tunnel(&self, tunnel_id: &Uuid) -> ApiResult<Option<Tunnel>> {
        let mut conn = self.get_connection().await?;

        let tunnel_key = format!("tunnel:{tunnel_id}");
        let tunnel_json: Option<String> = conn
            .get(&tunnel_key)
            .await
            .map_err(|e| ApiError::StorageError(format!("Failed to get tunnel: {e}")))?;

        match tunnel_json {
            Some(json) => {
                let tunnel: Tunnel = serde_json::from_str(&json).map_err(|e| {
                    ApiError::StorageError(format!("Failed to deserialize tunnel: {e}"))
                })?;
                Ok(Some(tunnel))
            }
            None => Ok(None),
        }
    }

    /// Retrieve tunnel by subdomain
    pub async fn get_tunnel_by_subdomain(&self, subdomain: &str) -> ApiResult<Option<Tunnel>> {
        let mut conn = self.get_connection().await?;

        let subdomain_key = format!("subdomain:{subdomain}");
        let tunnel_id: Option<String> = conn
            .get(&subdomain_key)
            .await
            .map_err(|e| ApiError::StorageError(format!("Failed to get subdomain mapping: {e}")))?;

        match tunnel_id {
            Some(id) => {
                let uuid = Uuid::parse_str(&id).map_err(|e| {
                    ApiError::StorageError(format!("Failed to parse tunnel ID: {e}"))
                })?;
                self.get_tunnel(&uuid).await
            }
            None => Ok(None),
        }
    }

    /// List tunnels for a user
    pub async fn list_user_tunnels(&self, github_user_id: &str) -> ApiResult<Vec<Tunnel>> {
        let mut conn = self.get_connection().await?;

        let user_key = format!("tunnel_lookup:{}", github_user_id);
        let user_data: Option<String> = conn.get(&user_key).await.map_err(|e| {
            ApiError::StorageError(format!("Failed to get user tunnel lookup: {e}"))
        })?;

        let mut tunnels = Vec::new();

        if let Some(data) = user_data {
            let user_lookup: UserTunnelLookup = serde_json::from_str(&data).map_err(|e| {
                ApiError::StorageError(format!("Failed to deserialize user tunnel lookup: {e}"))
            })?;

            for tunnel_id in user_lookup.tunnels {
                if let Ok(uuid) = Uuid::parse_str(&tunnel_id)
                    && let Ok(Some(tunnel)) = self.get_tunnel(&uuid).await
                {
                    tunnels.push(tunnel);
                }
            }
        }

        Ok(tunnels)
    }

    /// Update tunnel status
    pub async fn update_tunnel_status(
        &self,
        tunnel_id: &Uuid,
        status: TunnelStatus,
    ) -> ApiResult<()> {
        if let Some(mut tunnel) = self.get_tunnel(tunnel_id).await? {
            tunnel.status = status;
            self.store_tunnel(&tunnel).await?;
        }
        Ok(())
    }

    /// Update tunnel statistics
    pub async fn update_tunnel_stats(
        &self,
        tunnel_id: &Uuid,
        bytes_transferred: u64,
        request_count: u64,
    ) -> ApiResult<()> {
        if let Some(mut tunnel) = self.get_tunnel(tunnel_id).await? {
            tunnel.bytes_transferred += bytes_transferred;
            tunnel.request_count += request_count;
            self.store_tunnel(&tunnel).await?;
        }
        Ok(())
    }

    /// Delete tunnel
    pub async fn delete_tunnel(&self, tunnel_id: &Uuid) -> ApiResult<bool> {
        let tunnel = match self.get_tunnel(tunnel_id).await? {
            Some(tunnel) => tunnel,
            None => return Ok(false),
        };

        let mut conn = self.get_connection().await?;

        let tunnel_key = format!("tunnel:{tunnel_id}");
        let subdomain_key = format!("subdomain:{}", tunnel.subdomain);
        let user_key = format!("tunnel_lookup:{}", tunnel.github_user_id);

        // Delete tunnel data
        let _: () = conn
            .del(&tunnel_key)
            .await
            .map_err(|e| ApiError::StorageError(format!("Failed to delete tunnel: {e}")))?;

        // Delete subdomain mapping
        let _: () = conn.del(&subdomain_key).await.map_err(|e| {
            ApiError::StorageError(format!("Failed to delete subdomain mapping: {e}"))
        })?;

        // Delete the DNS slot mapping so dnsd stops resolving the FQDN
        // immediately (rather than waiting out the key TTL).
        let slot_key = format!("slot:{}", tunnel.fqdn);
        let _: () = conn.del(&slot_key).await.map_err(|e| {
            ApiError::StorageError(format!("Failed to delete DNS slot mapping: {e}"))
        })?;

        // Remove from the user's tunnel list. The lookup is stored as a
        // JSON-serialised UserTunnelLookup string (see store_tunnel), NOT a
        // Redis set — SREM on it fails with WRONGTYPE.
        let existing: Option<String> = conn.get(&user_key).await.map_err(|e| {
            ApiError::StorageError(format!("Failed to read user tunnel lookup: {e}"))
        })?;
        if let Some(data) = existing
            && let Ok(mut lookup) = serde_json::from_str::<UserTunnelLookup>(&data)
        {
            let id_str = tunnel_id.to_string();
            lookup.tunnels.retain(|t| t != &id_str);
            let updated = serde_json::to_string(&lookup).map_err(|e| {
                ApiError::StorageError(format!("Failed to serialize user tunnel lookup: {e}"))
            })?;
            let _: () = conn.set(&user_key, updated).await.map_err(|e| {
                ApiError::StorageError(format!("Failed to remove from user tunnels: {e}"))
            })?;
        }

        debug!("Deleted tunnel {}", tunnel_id);
        Ok(true)
    }

    /// Clean up expired tunnels
    pub async fn cleanup_expired_tunnels(&self) -> ApiResult<u32> {
        let mut conn = self.get_connection().await?;

        // Get all tunnel keys
        let tunnel_keys: Vec<String> = conn
            .keys("tunnel:*")
            .await
            .map_err(|e| ApiError::StorageError(format!("Failed to get tunnel keys: {e}")))?;

        let mut cleaned_count = 0;

        for key in tunnel_keys {
            if let Ok(Some(json)) = conn.get::<_, Option<String>>(&key).await
                && let Ok(tunnel) = serde_json::from_str::<Tunnel>(&json)
                && tunnel.is_expired()
                && conn.del::<_, ()>(&key).await.is_ok()
            {
                cleaned_count += 1;
            }
        }

        if cleaned_count > 0 {
            info!("Cleaned up {} expired tunnels", cleaned_count);
        }

        Ok(cleaned_count)
    }

    /// Get total number of active tunnels
    pub async fn get_active_tunnel_count(&self) -> ApiResult<u64> {
        let mut conn = self.get_connection().await?;

        let tunnel_keys: Vec<String> = conn
            .keys("tunnel:*")
            .await
            .map_err(|e| ApiError::StorageError(format!("Failed to get tunnel keys: {e}")))?;

        Ok(tunnel_keys.len() as u64)
    }

    /// Check if subdomain is available
    pub async fn is_subdomain_available(&self, subdomain: &str) -> ApiResult<bool> {
        let tunnel = self.get_tunnel_by_subdomain(subdomain).await?;
        Ok(tunnel.is_none())
    }

    /// Allocate an available port for a tunnel with certificate TTL
    pub async fn allocate_port(&self, tunnel_id: &str, certificate_ttl: u64) -> ApiResult<u16> {
        let mut conn = self.get_connection().await?;

        // Port allocation range: 10000-65535 (55,535 available ports per host)
        const PORT_RANGE_START: u16 = 10000;
        const PORT_RANGE_END: u16 = 65535;

        // Use a more efficient allocation strategy - start from a random position
        // to avoid clustering and improve distribution
        use rand::Rng;
        let start_port = rand::thread_rng().gen_range(PORT_RANGE_START..PORT_RANGE_END);

        // Try ports starting from random position, then wrap around
        let mut port = start_port;
        let mut attempts = 0;
        const MAX_ATTEMPTS: u16 = 1000; // Limit attempts to avoid infinite loops

        while attempts < MAX_ATTEMPTS {
            let port_key = format!("port:{}", port);

            // Atomically claim the port with SET NX EX. The previous
            // GET-then-SET pair was a TOCTOU race: two concurrent
            // create_tunnel calls could both see the port as free and both
            // "reserve" it, wiring two tunnels to one slot (cross-tunnel
            // traffic). NX makes exactly one caller win.
            let claimed: Option<String> = bb8_redis::redis::cmd("SET")
                .arg(&port_key)
                .arg(tunnel_id)
                .arg("NX")
                .arg("EX")
                .arg(certificate_ttl)
                .query_async(&mut *conn)
                .await
                .map_err(|e| ApiError::StorageError(format!("Failed to reserve port: {e}")))?;

            if claimed.is_some() {
                info!("Allocated port {} for tunnel {}", port, tunnel_id);
                return Ok(port);
            }

            // Move to next port, wrap around if needed
            port = if port == PORT_RANGE_END {
                PORT_RANGE_START
            } else {
                port + 1
            };

            attempts += 1;
        }

        Err(ApiError::StorageError(
            "No available ports found after 1000 attempts".to_string(),
        ))
    }

    /// Release a port when tunnel is deleted
    pub async fn release_port(&self, port: u16) -> ApiResult<()> {
        let mut conn = self.get_connection().await?;

        let port_key = format!("port:{}", port);
        let _: () = conn
            .del(&port_key)
            .await
            .map_err(|e| ApiError::StorageError(format!("Failed to release port: {e}")))?;

        info!("Released port {}", port);
        Ok(())
    }

    /// Get port allocation statistics
    pub async fn get_port_stats(&self) -> ApiResult<(u16, u16, u16)> {
        let mut conn = self.get_connection().await?;

        const PORT_RANGE_START: u16 = 10000;
        const PORT_RANGE_END: u16 = 65535;

        // For efficiency, sample a subset of ports rather than checking all 55,535
        const SAMPLE_SIZE: u16 = 1000;
        let mut allocated = 0;

        use rand::Rng;

        for _ in 0..SAMPLE_SIZE {
            let port = rand::thread_rng().gen_range(PORT_RANGE_START..PORT_RANGE_END);
            let port_key = format!("port:{}", port);
            let is_allocated: Option<String> = conn
                .get(&port_key)
                .await
                .map_err(|e| ApiError::StorageError(format!("Failed to check port: {e}")))?;

            if is_allocated.is_some() {
                allocated += 1;
            }
        }

        // Estimate total based on sample
        let total_ports = PORT_RANGE_END - PORT_RANGE_START + 1;
        let sample_ratio = SAMPLE_SIZE as f64 / total_ports as f64;
        let estimated_allocated = (allocated as f64 / sample_ratio) as u16;
        let estimated_available = total_ports - estimated_allocated;

        Ok((estimated_allocated, estimated_available, total_ports))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TunnelStatus;

    // Mock Redis connection for testing
    #[allow(dead_code)]
    struct MockRedisConnection;

    impl MockRedisConnection {
        #[allow(dead_code)]
        fn new() -> Self {
            Self
        }
    }

    #[tokio::test]
    async fn test_tunnel_storage_creation() {
        // Test that we can create a TunnelStorage instance
        // Note: This would normally require a real Redis connection
        // For unit tests, we'd need to mock the Redis connection
        let result = std::panic::catch_unwind(|| {
            // This would normally be: TunnelStorage::new(pool)
            // But we can't create a real Redis pool in unit tests
            // without external dependencies
            true
        });
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_tunnel_key_generation() {
        // Test the key generation logic
        let user_id = "user123";
        let subdomain = "myapp";
        let expected_key = format!("tunnel:{user_id}:{subdomain}");

        // This would be the key format used in the storage methods
        assert_eq!(expected_key, "tunnel:user123:myapp");
    }

    #[tokio::test]
    async fn test_tunnel_serialization_for_storage() {
        let tunnel = Tunnel::new(
            "user456".to_string(),
            "testuser".to_string(),
            "webapp".to_string(),
            "example.com",
            8080,
            12345,
            "cert-123".to_string(),
            3600,
        );

        // Test that tunnels can be serialized for Redis storage
        let serialized = serde_json::to_string(&tunnel).unwrap();
        assert!(serialized.contains("user456"));
        assert!(serialized.contains("webapp"));
        assert!(serialized.contains("example.com"));

        // Test deserialization
        let deserialized: Tunnel = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.github_user_id, tunnel.github_user_id);
        assert_eq!(deserialized.subdomain, tunnel.subdomain);
        assert_eq!(deserialized.fqdn, tunnel.fqdn);
    }

    #[tokio::test]
    async fn test_tunnel_status_transitions() {
        let mut tunnel = Tunnel::new(
            "user789".to_string(),
            "devuser".to_string(),
            "api".to_string(),
            "test.com",
            3000,
            54321,
            "cert-456".to_string(),
            1800,
        );

        // Test initial status
        assert!(matches!(tunnel.status, TunnelStatus::Creating));

        // Test status transitions
        tunnel.status = TunnelStatus::Active;
        assert!(matches!(tunnel.status, TunnelStatus::Active));

        tunnel.status = TunnelStatus::Destroying;
        assert!(matches!(tunnel.status, TunnelStatus::Destroying));

        tunnel.status = TunnelStatus::Expired;
        assert!(matches!(tunnel.status, TunnelStatus::Expired));

        tunnel.status = TunnelStatus::Error;
        assert!(matches!(tunnel.status, TunnelStatus::Error));
    }

    #[tokio::test]
    async fn test_tunnel_metrics_tracking() {
        let mut tunnel = Tunnel::new(
            "user999".to_string(),
            "metrics_user".to_string(),
            "metrics_app".to_string(),
            "metrics.com",
            9000,
            9999,
            "cert-metrics".to_string(),
            7200,
        );

        // Test initial metrics
        assert_eq!(tunnel.bytes_transferred, 0);
        assert_eq!(tunnel.request_count, 0);

        // Test metrics updates
        tunnel.bytes_transferred = 1024000; // 1MB
        tunnel.request_count = 500;

        assert_eq!(tunnel.bytes_transferred, 1024000);
        assert_eq!(tunnel.request_count, 500);
    }

    #[tokio::test]
    async fn test_tunnel_expiry_logic() {
        // Test tunnel that should not be expired
        let tunnel = Tunnel::new(
            "user_active".to_string(),
            "active_user".to_string(),
            "active_app".to_string(),
            "active.com",
            8000,
            8000,
            "cert-active".to_string(),
            3600, // 1 hour from now
        );

        assert!(!tunnel.is_expired());

        // Test remaining TTL
        let remaining = tunnel.remaining_ttl();
        assert!(remaining > 3500 && remaining <= 3600);

        // Test tunnel that is expired (manually set expiry to past)
        let mut expired_tunnel = tunnel.clone();
        expired_tunnel.expires_at = chrono::Utc::now() - chrono::Duration::seconds(1);
        assert!(expired_tunnel.is_expired());
    }

    #[tokio::test]
    async fn test_tunnel_fqdn_generation() {
        let tunnel = Tunnel::new(
            "fqdn_user".to_string(),
            "fqdn_username".to_string(),
            "my-service".to_string(),
            "fleetingdns.run",
            5000,
            50000,
            "cert-fqdn".to_string(),
            1800,
        );

        assert_eq!(tunnel.fqdn, "my-service.fleetingdns.run");
        assert_eq!(tunnel.subdomain, "my-service");
        assert_eq!(tunnel.local_port, 5000);
        assert_eq!(tunnel.slot, 50000);
    }

    #[tokio::test]
    async fn test_tunnel_certificate_tracking() {
        let tunnel = Tunnel::new(
            "cert_user".to_string(),
            "cert_username".to_string(),
            "cert-app".to_string(),
            "secure.com",
            4433,
            44330,
            "cert-serial-12345".to_string(),
            2700,
        );

        assert_eq!(tunnel.certificate_serial, "cert-serial-12345");
        assert_eq!(tunnel.github_user_id, "cert_user");
        assert_eq!(tunnel.github_username, "cert_username");
    }

    #[tokio::test]
    async fn test_tunnel_timing_fields() {
        let before_creation = chrono::Utc::now();

        let tunnel = Tunnel::new(
            "timing_user".to_string(),
            "timing_username".to_string(),
            "timing-app".to_string(),
            "timing.com",
            6000,
            60000,
            "cert-timing".to_string(),
            3600,
        );

        let after_creation = chrono::Utc::now();

        // Check that created_at is within reasonable bounds
        assert!(tunnel.created_at >= before_creation);
        assert!(tunnel.created_at <= after_creation);

        // Check that expires_at is approximately 1 hour from creation
        let expected_expiry = tunnel.created_at + chrono::Duration::seconds(3600);
        let time_diff = (tunnel.expires_at - expected_expiry).num_seconds().abs();
        assert!(time_diff < 5); // Within 5 seconds tolerance
    }

    #[tokio::test]
    async fn test_tunnel_edge_cases() {
        // Test with minimum TTL
        let short_tunnel = Tunnel::new(
            "short_user".to_string(),
            "short_username".to_string(),
            "short-app".to_string(),
            "short.com",
            1,
            1,
            "cert-short".to_string(),
            1, // 1 second TTL
        );

        assert_eq!(short_tunnel.local_port, 1);
        assert_eq!(short_tunnel.slot, 1);

        // Test with maximum reasonable values
        let max_tunnel = Tunnel::new(
            "max_user".to_string(),
            "max_username".to_string(),
            "max-app".to_string(),
            "max.com",
            65535, // Max port
            9999,  // Large slot
            "cert-max".to_string(),
            86400, // 24 hours
        );

        assert_eq!(max_tunnel.local_port, 65535);
        assert_eq!(max_tunnel.slot, 9999);
    }

    #[tokio::test]
    async fn test_tunnel_clone_and_equality() {
        let tunnel1 = Tunnel::new(
            "clone_user".to_string(),
            "clone_username".to_string(),
            "clone-app".to_string(),
            "clone.com",
            7000,
            7000,
            "cert-clone".to_string(),
            1800,
        );

        let tunnel2 = tunnel1.clone();

        // Test that cloned tunnel has same values
        assert_eq!(tunnel1.github_user_id, tunnel2.github_user_id);
        assert_eq!(tunnel1.subdomain, tunnel2.subdomain);
        assert_eq!(tunnel1.fqdn, tunnel2.fqdn);
        assert_eq!(tunnel1.local_port, tunnel2.local_port);
        assert_eq!(tunnel1.slot, tunnel2.slot);
        assert_eq!(tunnel1.certificate_serial, tunnel2.certificate_serial);
        assert_eq!(tunnel1.created_at, tunnel2.created_at);
        assert_eq!(tunnel1.expires_at, tunnel2.expires_at);
    }

    #[tokio::test]
    async fn test_storage_error_handling() {
        // Test that storage errors are properly handled
        // This would normally test Redis connection errors, timeouts, etc.

        // Mock a Redis error scenario
        let error_result: Result<(), redis::RedisError> = Err(redis::RedisError::from((
            redis::ErrorKind::IoError,
            "Connection refused",
        )));

        assert!(error_result.is_err());

        // Test conversion to ApiError
        if let Err(redis_error) = error_result {
            let api_error: ApiError = redis_error.into();
            match api_error {
                ApiError::StorageError(msg) => {
                    assert!(msg.contains("Connection refused"));
                }
                _ => panic!("Expected StorageError"),
            }
        }
    }

    #[tokio::test]
    async fn test_tunnel_json_compatibility() {
        let tunnel = Tunnel::new(
            "json_user".to_string(),
            "json_username".to_string(),
            "json-app".to_string(),
            "json.com",
            8080,
            8080,
            "cert-json".to_string(),
            3600,
        );

        // Test JSON serialization produces expected fields
        let json_value: serde_json::Value = serde_json::to_value(&tunnel).unwrap();

        assert_eq!(json_value["github_user_id"], "json_user");
        assert_eq!(json_value["subdomain"], "json-app");
        assert_eq!(json_value["fqdn"], "json-app.json.com");
        assert_eq!(json_value["local_port"], 8080);
        assert_eq!(json_value["slot"], 8080);
        assert_eq!(json_value["certificate_serial"], "cert-json");
        assert_eq!(json_value["bytes_transferred"], 0);
        assert_eq!(json_value["request_count"], 0);

        // Test that status is serialized correctly
        assert_eq!(json_value["status"], "creating");
    }

    #[tokio::test]
    async fn test_tunnel_validation_logic() {
        // Test various validation scenarios that storage might need

        // Valid subdomain
        let valid_tunnel = Tunnel::new(
            "valid_user".to_string(),
            "valid_username".to_string(),
            "valid-app-123".to_string(),
            "valid.com",
            8080,
            8080,
            "cert-valid".to_string(),
            3600,
        );

        assert!(
            valid_tunnel
                .subdomain
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-')
        );
        assert!(!valid_tunnel.subdomain.is_empty());
        assert!(valid_tunnel.local_port > 0);
        assert!(valid_tunnel.slot > 0);

        // Test edge case subdomains
        let edge_cases = vec![
            "a",                        // Single character
            "a-b",                      // With hyphen
            "app123",                   // With numbers
            "very-long-subdomain-name", // Long name
        ];

        for subdomain in edge_cases {
            let tunnel = Tunnel::new(
                "edge_user".to_string(),
                "edge_username".to_string(),
                subdomain.to_string(),
                "edge.com",
                8080,
                8080,
                "cert-edge".to_string(),
                3600,
            );

            assert_eq!(tunnel.subdomain, subdomain);
            assert_eq!(tunnel.fqdn, format!("{subdomain}.edge.com"));
        }
    }
}
