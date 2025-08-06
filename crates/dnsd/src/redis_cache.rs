use std::net::Ipv4Addr;

use bb8::Pool;
use bb8_redis::RedisConnectionManager;
use redis::AsyncCommands;
use common::error::{FleetingDnsError, CommonResult};

/// Connection pool type for Redis.
pub type RedisPool = Pool<RedisConnectionManager>;

/// Type alias for cache errors using the common error system
pub type CacheError = FleetingDnsError;
pub type CacheResult<T> = CommonResult<T>;

/// Create a new Redis connection pool from the given URL.
pub async fn new_pool(url: &str) -> Result<RedisPool, redis::RedisError> {
    let manager = RedisConnectionManager::new(url)?;
    Pool::builder()
        .connection_timeout(std::time::Duration::from_secs(5))
        .build(manager)
        .await
}

/// Fetch the IPv4 address for a slot.
///
/// Returns [`FleetingDnsError::NotFound`] if the key does not exist.
pub async fn get_slot(pool: &RedisPool, slot: &str) -> Result<Ipv4Addr, CacheError> {
    let mut conn = pool.get().await
        .map_err(|e| FleetingDnsError::ConnectionError(e.to_string()))?;
    let val: Option<String> = conn.get(slot).await?;
    match val {
        Some(v) => v
            .parse()
            .map_err(|_| FleetingDnsError::ValidationError("Invalid IP address format".to_string())),
        None => Err(FleetingDnsError::NotFound(format!("DNS record not found for slot: {}", slot))),
    }
}

/// Set the IPv4 address for a slot with a TTL in seconds.
pub async fn set_slot(
    pool: &RedisPool,
    slot: &str,
    ip: Ipv4Addr,
    ttl: u64,
) -> Result<(), CacheError> {
    let mut conn = pool.get().await
        .map_err(|e| FleetingDnsError::ConnectionError(e.to_string()))?;
    let _: () = redis::cmd("SET")
        .arg(slot)
        .arg(ip.to_string())
        .arg("EX")
        .arg(ttl)
        .query_async(&mut *conn)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    // Note: Testcontainer tests temporarily disabled due to API compatibility issues
    // These will be re-enabled after updating testcontainers dependencies

    #[test]
    fn test_cache_error_display() {
        let error = FleetingDnsError::NotFound("DNS record not found".to_string());
        assert!(format!("{error}").contains("not found"));
    }

    #[test]
    fn test_cache_error_debug() {
        let error = FleetingDnsError::NotFound("DNS record not found".to_string());
        assert!(!format!("{error:?}").is_empty());
    }

    #[test]
    fn test_redis_pool_type_alias() {
        // Ensure our type alias compiles correctly
        let _pool_type: Option<RedisPool> = None;
    }
}
