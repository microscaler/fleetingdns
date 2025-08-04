//! Redis utilities for the EdgeHub crate.
//!
//! This module provides a thin wrapper around the Redis connection pooling
//! facilities from the `dnsd` crate. It exposes helper functions to create a
//! pool, store a slot mapping with TTL, and delete the mapping when a tunnel
//! closes.

use std::net::Ipv4Addr;

use dnsd::redis_cache as dnsd_cache;
pub use dnsd::redis_cache::{CacheError, RedisPool, get_slot};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};

/// Default TTL applied to slot mappings in seconds.
pub const DEFAULT_TTL: u64 = 1800;

/// Create a new Redis connection pool using the given URL.
///
/// This simply forwards to [`dnsd_cache::new_pool`].
pub async fn new_pool(url: &str) -> Result<RedisPool, redis::RedisError> {
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
    let mut conn = pool.get().await.map_err(CacheError::Pool)?;
    let _: () = conn.del(slot).await.map_err(CacheError::Redis)?;
    Ok(())
}

/// Tunnel information for HTTPS routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelInfo {
    pub id: String,
    pub fqdn: String,
    pub local_port: u16,
    pub slot: u16,
    pub user_id: String,
    pub expires_at: String,
}

/// Get tunnel information by subdomain
#[tracing::instrument(skip_all, fields(subdomain))]
pub async fn get_tunnel_by_subdomain(
    pool: &RedisPool,
    subdomain: &str,
) -> Result<Option<TunnelInfo>, CacheError> {
    let mut conn = pool.get().await.map_err(CacheError::Pool)?;
    
    // Look up tunnel by subdomain in Redis
    let key = format!("tunnel:subdomain:{}", subdomain);
    let tunnel_data: Option<String> = conn.get(&key).await.map_err(CacheError::Redis)?;
    
    if let Some(data) = tunnel_data {
        match serde_json::from_str::<TunnelInfo>(&data) {
            Ok(tunnel) => Ok(Some(tunnel)),
            Err(_) => {
                tracing::warn!(subdomain, "Failed to deserialize tunnel data");
                Ok(None)
            }
        }
    } else {
        Ok(None)
    }
}
