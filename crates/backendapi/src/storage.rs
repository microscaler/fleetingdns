use crate::{models::*, ApiError, ApiResult};
use bb8::{Pool, PooledConnection};
use bb8_redis::{redis::AsyncCommands, RedisConnectionManager};
use tracing::{debug, info};
use uuid::Uuid;

/// Redis storage for tunnel metadata
pub struct TunnelStorage {
    pool: Pool<RedisConnectionManager>,
}

impl TunnelStorage {
    /// Create a new tunnel storage instance
    pub async fn new(redis_url: &str) -> ApiResult<Self> {
        let manager = RedisConnectionManager::new(redis_url)
            .map_err(|e| ApiError::StorageError(format!("Failed to create Redis manager: {}", e)))?;
        
        let pool = Pool::builder()
            .max_size(15)
            .build(manager)
            .await
            .map_err(|e| ApiError::StorageError(format!("Failed to create Redis pool: {}", e)))?;
        
        info!("Connected to Redis at {}", redis_url);
        
        Ok(Self { pool })
    }
    
    /// Get a Redis connection from the pool
    async fn get_connection(&self) -> ApiResult<PooledConnection<'_, RedisConnectionManager>> {
        self.pool
            .get()
            .await
            .map_err(|e| ApiError::StorageError(format!("Failed to get Redis connection: {}", e)))
    }
    
    /// Store tunnel metadata
    pub async fn store_tunnel(&self, tunnel: &Tunnel) -> ApiResult<()> {
        let mut conn = self.get_connection().await?;
        
        let tunnel_json = serde_json::to_string(tunnel)
            .map_err(|e| ApiError::StorageError(format!("Failed to serialize tunnel: {}", e)))?;
        
        let tunnel_key = format!("tunnel:{}", tunnel.id);
        let subdomain_key = format!("subdomain:{}", tunnel.subdomain);
        let user_key = format!("user:{}:tunnels", tunnel.github_user_id);
        
        // Calculate TTL in seconds - fix type conversion
        let ttl = tunnel.remaining_ttl().max(0) as u64;
        
        // Store tunnel data with expiration
        let _: () = conn
            .set_ex(&tunnel_key, &tunnel_json, ttl)
            .await
            .map_err(|e| ApiError::StorageError(format!("Failed to store tunnel: {}", e)))?;
        
        // Map subdomain to tunnel ID with expiration
        let _: () = conn
            .set_ex(&subdomain_key, tunnel.id.to_string(), ttl)
            .await
            .map_err(|e| ApiError::StorageError(format!("Failed to map subdomain: {}", e)))?;
        
        // Add tunnel to user's list
        let _: () = conn
            .sadd(&user_key, tunnel.id.to_string())
            .await
            .map_err(|e| ApiError::StorageError(format!("Failed to add to user tunnels: {}", e)))?;
        
        // Set expiration on user list (longer than tunnel to allow cleanup)
        let _: () = conn
            .expire(&user_key, (ttl + 300) as i64)
            .await
            .map_err(|e| ApiError::StorageError(format!("Failed to set user list expiry: {}", e)))?;
        
        debug!("Stored tunnel {} with TTL {} seconds", tunnel.id, ttl);
        Ok(())
    }
    
    /// Retrieve tunnel by ID
    pub async fn get_tunnel(&self, tunnel_id: &Uuid) -> ApiResult<Option<Tunnel>> {
        let mut conn = self.get_connection().await?;
        
        let tunnel_key = format!("tunnel:{}", tunnel_id);
        let tunnel_json: Option<String> = conn
            .get(&tunnel_key)
            .await
            .map_err(|e| ApiError::StorageError(format!("Failed to get tunnel: {}", e)))?;
        
        match tunnel_json {
            Some(json) => {
                let tunnel: Tunnel = serde_json::from_str(&json)
                    .map_err(|e| ApiError::StorageError(format!("Failed to deserialize tunnel: {}", e)))?;
                Ok(Some(tunnel))
            }
            None => Ok(None),
        }
    }
    
    /// Retrieve tunnel by subdomain
    pub async fn get_tunnel_by_subdomain(&self, subdomain: &str) -> ApiResult<Option<Tunnel>> {
        let mut conn = self.get_connection().await?;
        
        let subdomain_key = format!("subdomain:{}", subdomain);
        let tunnel_id: Option<String> = conn
            .get(&subdomain_key)
            .await
            .map_err(|e| ApiError::StorageError(format!("Failed to get subdomain mapping: {}", e)))?;
        
        match tunnel_id {
            Some(id) => {
                let uuid = Uuid::parse_str(&id)
                    .map_err(|e| ApiError::StorageError(format!("Invalid tunnel ID: {}", e)))?;
                self.get_tunnel(&uuid).await
            }
            None => Ok(None),
        }
    }
    
    /// List tunnels for a user
    pub async fn list_user_tunnels(&self, github_user_id: &str) -> ApiResult<Vec<Tunnel>> {
        let mut conn = self.get_connection().await?;
        
        let user_key = format!("user:{}:tunnels", github_user_id);
        let tunnel_ids: Vec<String> = conn
            .smembers(&user_key)
            .await
            .map_err(|e| ApiError::StorageError(format!("Failed to get user tunnels: {}", e)))?;
        
        let mut tunnels = Vec::new();
        for id_str in tunnel_ids {
            if let Ok(uuid) = Uuid::parse_str(&id_str) {
                if let Ok(Some(tunnel)) = self.get_tunnel(&uuid).await {
                    tunnels.push(tunnel);
                }
            }
        }
        
        Ok(tunnels)
    }
    
    /// Update tunnel status
    pub async fn update_tunnel_status(&self, tunnel_id: &Uuid, status: TunnelStatus) -> ApiResult<()> {
        if let Some(mut tunnel) = self.get_tunnel(tunnel_id).await? {
            tunnel.status = status;
            self.store_tunnel(&tunnel).await?;
        }
        Ok(())
    }
    
    /// Update tunnel statistics
    pub async fn update_tunnel_stats(&self, tunnel_id: &Uuid, bytes_transferred: u64, request_count: u64) -> ApiResult<()> {
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
        
        let tunnel_key = format!("tunnel:{}", tunnel_id);
        let subdomain_key = format!("subdomain:{}", tunnel.subdomain);
        let user_key = format!("user:{}:tunnels", tunnel.github_user_id);
        
        // Delete tunnel data
        let _: () = conn
            .del(&tunnel_key)
            .await
            .map_err(|e| ApiError::StorageError(format!("Failed to delete tunnel: {}", e)))?;
        
        // Delete subdomain mapping
        let _: () = conn
            .del(&subdomain_key)
            .await
            .map_err(|e| ApiError::StorageError(format!("Failed to delete subdomain mapping: {}", e)))?;
        
        // Remove from user's tunnel list
        let _: () = conn
            .srem(&user_key, tunnel_id.to_string())
            .await
            .map_err(|e| ApiError::StorageError(format!("Failed to remove from user tunnels: {}", e)))?;
        
        debug!("Deleted tunnel {}", tunnel_id);
        Ok(true)
    }
    
    /// Clean up expired tunnels
    pub async fn cleanup_expired_tunnels(&self) -> ApiResult<u64> {
        let mut conn = self.get_connection().await?;
        
        // Get all tunnel keys
        let tunnel_keys: Vec<String> = conn
            .keys("tunnel:*")
            .await
            .map_err(|e| ApiError::StorageError(format!("Failed to get tunnel keys: {}", e)))?;
        
        let mut cleaned_count = 0;
        
        for key in tunnel_keys {
            if let Ok(tunnel_json) = conn.get::<_, Option<String>>(&key).await {
                if let Some(json) = tunnel_json {
                    if let Ok(tunnel) = serde_json::from_str::<Tunnel>(&json) {
                        if tunnel.is_expired() {
                            if self.delete_tunnel(&tunnel.id).await.unwrap_or(false) {
                                cleaned_count += 1;
                            }
                        }
                    }
                }
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
            .map_err(|e| ApiError::StorageError(format!("Failed to get tunnel keys: {}", e)))?;
        
        Ok(tunnel_keys.len() as u64)
    }
    
    /// Check if subdomain is available
    pub async fn is_subdomain_available(&self, subdomain: &str) -> ApiResult<bool> {
        let tunnel = self.get_tunnel_by_subdomain(subdomain).await?;
        Ok(tunnel.is_none())
    }
} 