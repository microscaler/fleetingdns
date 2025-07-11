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
