//! Redis utilities for the EdgeHub crate.
//!
//! This module provides a thin wrapper around the Redis connection pooling
//! facilities from the `dnsd` crate. It exposes helper functions to create a
//! pool, store a slot mapping with TTL, and delete the mapping when a tunnel
//! closes.

use std::net::Ipv4Addr;

use super::cache as dnsd_cache;
pub use super::cache::{CacheError, RedisPool, get_slot};
use bb8_redis::redis::AsyncCommands;
use serde::{Deserialize, Serialize};

use crate::tunnel::UserTunnelLookup;
use tracing;

/// Default TTL applied to slot mappings in seconds.
pub const DEFAULT_TTL: u64 = 1800;

/// Tunnel information compatible with the existing Redis data format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelInfo {
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

/// Create a new Redis connection pool using the given URL.
///
/// This simply forwards to [`dnsd_cache::new_pool`].
pub async fn new_pool(url: &str) -> Result<RedisPool, bb8_redis::redis::RedisError> {
    dnsd_cache::new_pool(url).await
}

/// Store the mapping from `slot` to `ip` with the provided TTL.
///
/// If the command fails, a [`redis::RedisError`] is returned.
#[tracing::instrument(skip_all, fields(slot, ip=%ip, ttl))]
pub async fn set_slot(
    pool: &RedisPool,
    slot: &str,
    ip: Ipv4Addr,
    ttl: u64,
) -> Result<(), CacheError> {
    dnsd_cache::set_slot(pool, slot, ip, ttl).await
}

/// Delete the mapping associated with `slot`.
#[tracing::instrument(skip_all, fields(slot))]
pub async fn del_slot(pool: &RedisPool, slot: &str) -> Result<(), CacheError> {
    let mut conn = pool.get().await.map_err(|e| CacheError::ConnectionError(e.to_string()))?;
    let _: () = conn.del(slot).await.map_err(|e| CacheError::RedisError(e.to_string()))?;
    Ok(())
}

/// Store tunnel data in Redis with TTL
#[tracing::instrument(skip_all, fields(tunnel_id))]
pub async fn store_tunnel_data(
    pool: &RedisPool,
    tunnel_info: &TunnelInfo,
) -> Result<(), CacheError> {
    let mut conn = pool.get().await.map_err(|e| CacheError::ConnectionError(e.to_string()))?;
    
    let tunnel_key = format!("tunnel:{}", tunnel_info.id);
    let data = serde_json::to_string(tunnel_info).map_err(|e| {
        CacheError::SerializationError(format!("Failed to serialize tunnel info: {}", e))
    })?;
    
    // Calculate TTL from expires_at
    let ttl = if let Ok(expires) = tunnel_info.expires_at.parse::<chrono::DateTime<chrono::Utc>>() {
        let now = chrono::Utc::now();
        let duration = expires - now;
        duration.num_seconds() as u64
    } else {
        3600u64 // Default 1 hour
    };
    
    let _: () = conn.set_ex(&tunnel_key, data, ttl).await.map_err(|e| CacheError::RedisError(e.to_string()))?;
    
    tracing::info!(tunnel_id = %tunnel_info.id, ttl = %ttl, "Stored tunnel data in Redis");
    Ok(())
}

/// Store user tunnel lookup data in Redis
#[tracing::instrument(skip_all, fields(github_user_id))]
pub async fn store_user_tunnel_lookup(
    pool: &RedisPool,
    github_user_id: &str,
    github_username: &str,
    tunnel_id: &str,
) -> Result<(), CacheError> {
    let mut conn = pool.get().await.map_err(|e| CacheError::ConnectionError(e.to_string()))?;
    
    let key = format!("tunnel_lookup:{}", github_user_id);
    
    // Get existing user data or create new
    let existing_data: Option<String> = conn.get(&key).await.map_err(|e| CacheError::RedisError(e.to_string()))?;
    
    let mut user_lookup = if let Some(data) = existing_data {
        serde_json::from_str::<UserTunnelLookup>(&data).unwrap_or_else(|_| UserTunnelLookup {
            github_user_id: github_user_id.to_string(),
            github_username: github_username.to_string(),
            tunnels: Vec::new(),
        })
    } else {
        UserTunnelLookup {
            github_user_id: github_user_id.to_string(),
            github_username: github_username.to_string(),
            tunnels: Vec::new(),
        }
    };
    
    // Add tunnel ID if not already present
    if !user_lookup.tunnels.contains(&tunnel_id.to_string()) {
        user_lookup.tunnels.push(tunnel_id.to_string());
    }
    
    // Store back to Redis (no TTL for user lookup)
    let data = serde_json::to_string(&user_lookup).map_err(|e| {
        CacheError::SerializationError(format!("Failed to serialize user lookup: {}", e))
    })?;
    
    let _: () = conn.set(&key, data).await.map_err(|e| CacheError::RedisError(e.to_string()))?;
    
    tracing::info!(github_user_id, tunnel_id, "Updated user tunnel lookup in Redis");
    Ok(())
}

/// Get tunnel information by subdomain from Redis
#[tracing::instrument(skip_all, fields(subdomain))]
pub async fn get_tunnel_by_subdomain(
    pool: &RedisPool,
    subdomain: &str,
) -> Result<Option<TunnelInfo>, CacheError> {
    let mut conn = pool.get().await.map_err(|e| CacheError::ConnectionError(e.to_string()))?;
    
    // Get all user tunnel lookups
    let pattern = "tunnel_lookup:*";
    let keys: Vec<String> = conn.keys(pattern).await.map_err(|e| CacheError::RedisError(e.to_string()))?;
    
    tracing::info!(subdomain, user_count = keys.len(), "Scanning user tunnel lookups");
    
    for key in keys {
        tracing::info!(subdomain, key = %key, "Checking user lookup key");
        let user_data: Option<String> = conn.get(&key).await.map_err(|e| CacheError::RedisError(e.to_string()))?;
        
        if let Some(data) = user_data {
            tracing::info!(subdomain, key = %key, "Found user data, parsing...");
            match serde_json::from_str::<UserTunnelLookup>(&data) {
                Ok(user_lookup) => {
                    tracing::info!(subdomain, key = %key, tunnel_count = user_lookup.tunnels.len(), "Parsed user lookup");
                    // Check each tunnel ID for this user
                    for tunnel_id in &user_lookup.tunnels {
                        let tunnel_key = format!("tunnel:{}", tunnel_id);
                        tracing::info!(subdomain, tunnel_id = %tunnel_id, tunnel_key = %tunnel_key, "Checking tunnel");
                        let tunnel_data: Option<String> = conn.get(&tunnel_key).await.map_err(|e| CacheError::RedisError(e.to_string()))?;
                        
                        if let Some(data) = tunnel_data {
                            tracing::info!(subdomain, tunnel_id = %tunnel_id, "Found tunnel data, parsing...");
                            match serde_json::from_str::<TunnelInfo>(&data) {
                                Ok(tunnel_info) => {
                                    tracing::info!(subdomain, tunnel_id = %tunnel_info.id, tunnel_subdomain = %tunnel_info.subdomain, "Parsed tunnel info");
                                    if tunnel_info.subdomain == subdomain {
                                        tracing::info!(subdomain, tunnel_id = %tunnel_info.id, "Found tunnel in Redis");
                                        return Ok(Some(tunnel_info));
                                    } else {
                                        tracing::info!(subdomain, tunnel_id = %tunnel_info.id, tunnel_subdomain = %tunnel_info.subdomain, "Subdomain mismatch");
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!(tunnel_id, error = %e, "Failed to deserialize tunnel data");
                                }
                            }
                        } else {
                            tracing::info!(subdomain, tunnel_id = %tunnel_id, "No tunnel data found");
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(key = %key, error = %e, "Failed to deserialize user tunnel lookup");
                }
            }
        } else {
            tracing::info!(subdomain, key = %key, "No user data found");
        }
    }
    
    tracing::info!(subdomain, "No tunnel found for subdomain");
    Ok(None)
}
