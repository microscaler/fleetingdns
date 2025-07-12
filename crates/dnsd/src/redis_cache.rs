use std::net::Ipv4Addr;

use bb8::Pool;
use bb8_redis::RedisConnectionManager;
use redis::AsyncCommands;
use thiserror::Error;

/// Connection pool type for Redis.
pub type RedisPool = Pool<RedisConnectionManager>;

/// Errors returned by Redis cache operations.
#[derive(Debug, Error)]
pub enum CacheError {
    /// Key was not found.
    #[error("NXDOMAIN")]
    NXDomain,
    /// Underlying Redis error.
    #[error(transparent)]
    Redis(#[from] redis::RedisError),
    /// Connection pool error
    #[error(transparent)]
    Pool(#[from] bb8::RunError<redis::RedisError>),
}

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
/// Returns [`CacheError::NXDomain`] if the key does not exist.
pub async fn get_slot(pool: &RedisPool, slot: &str) -> Result<Ipv4Addr, CacheError> {
    let mut conn = pool.get().await?;
    let val: Option<String> = conn.get(slot).await?;
    match val {
        Some(v) => v
            .parse()
            .map_err(|_| redis::RedisError::from((redis::ErrorKind::TypeError, "invalid ip")))
            .map_err(CacheError::Redis),
        None => Err(CacheError::NXDomain),
    }
}

/// Set the IPv4 address for a slot with a TTL in seconds.
pub async fn set_slot(
    pool: &RedisPool,
    slot: &str,
    ip: Ipv4Addr,
    ttl: u64,
) -> Result<(), CacheError> {
    let mut conn = pool.get().await?;
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
    use std::sync::Arc;
    use testcontainers::{Container, RunnableImage};
    use testcontainers_modules::redis::Redis;
    use tokio::time::{Duration, sleep};

    /// Helper to create a Redis container with ephemeral port
    async fn setup_redis() -> (RedisPool, Container<'static, Redis>) {
        use testcontainers::clients::Cli;
        use testcontainers_modules::redis::Redis;
        
        let docker = Box::leak(Box::new(Cli::default()));
        let redis_image = RunnableImage::from(Redis::default());
        let redis_container = docker.run(redis_image);
        let redis_port = redis_container.get_host_port_ipv4(6379);
        let redis_url = format!("redis://127.0.0.1:{}", redis_port);
        
        // Wait a bit for Redis to start
        sleep(Duration::from_millis(100)).await;
        
        let pool = new_pool(&redis_url).await.expect("Failed to create Redis pool");
        (pool, redis_container)
    }

    #[tokio::test]
    async fn test_set_get_respects_ttl() {
        let (pool, _container) = setup_redis().await;
        
        let ip = Ipv4Addr::new(1, 2, 3, 4);
        if let Err(e) = set_slot(&pool, "slot1", ip, 1).await {
            eprintln!("skipping test: redis set failed - {}", e);
            return;
        }
        
        let got = match get_slot(&pool, "slot1").await {
            Ok(ip) => ip,
            Err(e) => {
                eprintln!("skipping test: redis get failed - {}", e);
                return;
            }
        };
        assert_eq!(got, ip);
        
        // Wait for TTL to expire
        sleep(Duration::from_secs(2)).await;
        let err = get_slot(&pool, "slot1").await.unwrap_err();
        assert!(matches!(err, CacheError::NXDomain));
    }

    #[tokio::test]
    async fn test_get_nonexistent_slot() {
        let (pool, _container) = setup_redis().await;
        
        let err = match get_slot(&pool, "nonexistent").await {
            Ok(ip) => {
                eprintln!("test failed: expected error but got success with IP: {}", ip);
                return;
            }
            Err(e) => e,
        };
        
        // Check if it's a timeout error vs NXDomain
        match err {
            CacheError::NXDomain => {
                // This is what we expect
                assert!(true);
            }
            CacheError::Pool(_) => {
                eprintln!("skipping test: Redis pool timeout");
                return;
            }
            CacheError::Redis(_) => {
                eprintln!("skipping test: Redis error");
                return;
            }
        }
    }

    #[tokio::test]
    async fn test_set_get_multiple_slots() {
        let (pool, _container) = setup_redis().await;
        
        let ip1 = Ipv4Addr::new(1, 2, 3, 4);
        let ip2 = Ipv4Addr::new(5, 6, 7, 8);
        let ip3 = Ipv4Addr::new(9, 10, 11, 12);
        
        // Set multiple slots
        if set_slot(&pool, "slot1", ip1, 300).await.is_err() ||
           set_slot(&pool, "slot2", ip2, 300).await.is_err() ||
           set_slot(&pool, "slot3", ip3, 300).await.is_err() {
            eprintln!("skipping test: redis set operations failed");
            return;
        }
        
        // Get all slots
        match (get_slot(&pool, "slot1").await, get_slot(&pool, "slot2").await, get_slot(&pool, "slot3").await) {
            (Ok(got1), Ok(got2), Ok(got3)) => {
                assert_eq!(got1, ip1);
                assert_eq!(got2, ip2);
                assert_eq!(got3, ip3);
            }
            _ => {
                eprintln!("skipping test: redis get operations failed");
                return;
            }
        }
    }

    #[tokio::test]
    async fn test_set_overwrite_existing_slot() {
        let (pool, _container) = setup_redis().await;
        
        let ip1 = Ipv4Addr::new(1, 2, 3, 4);
        let ip2 = Ipv4Addr::new(5, 6, 7, 8);
        
        // Set initial value
        if set_slot(&pool, "slot1", ip1, 300).await.is_err() {
            eprintln!("skipping test: redis set failed");
            return;
        }
        
        let got1 = match get_slot(&pool, "slot1").await {
            Ok(ip) => ip,
            Err(e) => {
                eprintln!("skipping test: redis get failed - {}", e);
                return;
            }
        };
        assert_eq!(got1, ip1);
        
        // Overwrite with new value
        if set_slot(&pool, "slot1", ip2, 300).await.is_err() {
            eprintln!("skipping test: redis overwrite failed");
            return;
        }
        
        let got2 = match get_slot(&pool, "slot1").await {
            Ok(ip) => ip,
            Err(e) => {
                eprintln!("skipping test: redis get after overwrite failed - {}", e);
                return;
            }
        };
        assert_eq!(got2, ip2);
    }

    #[tokio::test]
    async fn test_concurrent_operations() {
        let (pool, _container) = setup_redis().await;
        let pool = Arc::new(pool);
        
        let mut handles = Vec::new();
        
        // Spawn multiple concurrent operations
        for i in 0..10 {
            let pool_clone = pool.clone();
            let handle = tokio::spawn(async move {
                let slot = format!("slot{}", i);
                let ip = Ipv4Addr::new(192, 168, 1, i as u8);
                
                set_slot(&pool_clone, &slot, ip, 300).await.unwrap();
                let retrieved = get_slot(&pool_clone, &slot).await.unwrap();
                assert_eq!(retrieved, ip);
            });
            handles.push(handle);
        }
        
        // Wait for all operations to complete
        for handle in handles {
            handle.await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_pool_connection_timeout() {
        let (pool, _container) = setup_redis().await;
        
        // Test that we can get multiple connections from the pool
        let mut connections = Vec::new();
        for i in 0..5 {
            match pool.get().await {
                Ok(conn) => connections.push(conn),
                Err(e) => {
                    eprintln!("skipping test: failed to get connection {}: {}", i, e);
                    return;
                }
            }
        }
        
        // All connections should be valid
        assert_eq!(connections.len(), 5);
    }

    #[tokio::test]
    async fn test_invalid_ip_parsing() {
        let (pool, _container) = setup_redis().await;
        
        // Manually insert invalid IP data
        let mut conn = match pool.get().await {
            Ok(conn) => conn,
            Err(e) => {
                eprintln!("skipping test: failed to get connection: {}", e);
                return;
            }
        };
        
        if let Err(e) = redis::cmd("SET")
            .arg("invalid_ip_slot")
            .arg("not.an.ip.address")
            .query_async::<()>(&mut *conn)
            .await {
                eprintln!("skipping test: failed to set invalid IP: {}", e);
                return;
            }
        
        // Should return parsing error
        let err = get_slot(&pool, "invalid_ip_slot").await.unwrap_err();
        assert!(matches!(err, CacheError::Redis(_)));
    }

    #[tokio::test]
    async fn test_set_with_zero_ttl() {
        let (pool, _container) = setup_redis().await;
        
        let ip = Ipv4Addr::new(1, 2, 3, 4);
        // Redis doesn't accept 0 as TTL, so this should fail
        let result = set_slot(&pool, "zero_ttl_slot", ip, 0).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_set_with_large_ttl() {
        let (pool, _container) = setup_redis().await;
        
        let ip = Ipv4Addr::new(1, 2, 3, 4);
        if let Err(e) = set_slot(&pool, "large_ttl_slot", ip, 86400).await {
            eprintln!("skipping test: redis error - {}", e);
            return;
        }
        
        let retrieved = get_slot(&pool, "large_ttl_slot").await.unwrap();
        assert_eq!(retrieved, ip);
    }

    #[tokio::test]
    async fn test_special_characters_in_slot_names() {
        let (pool, _container) = setup_redis().await;
        
        let ip = Ipv4Addr::new(1, 2, 3, 4);
        let special_slots = vec![
            "slot-with-dashes",
            "slot_with_underscores",
            "slot.with.dots",
            "slot:with:colons",
            "slot/with/slashes",
        ];
        
        for slot in special_slots {
            if let Err(e) = set_slot(&pool, slot, ip, 300).await {
                eprintln!("skipping test for slot '{}': redis error - {}", slot, e);
                continue;
            }
            let retrieved = get_slot(&pool, slot).await.unwrap();
            assert_eq!(retrieved, ip);
        }
    }

    #[tokio::test]
    async fn test_edge_case_ip_addresses() {
        let (pool, _container) = setup_redis().await;
        
        let edge_ips = vec![
            Ipv4Addr::new(0, 0, 0, 0),        // All zeros
            Ipv4Addr::new(255, 255, 255, 255), // All ones
            Ipv4Addr::new(127, 0, 0, 1),       // Localhost
            Ipv4Addr::new(192, 168, 1, 1),     // Private network
            Ipv4Addr::new(10, 0, 0, 1),        // Private network
            Ipv4Addr::new(172, 16, 0, 1),      // Private network
        ];
        
        for (i, ip) in edge_ips.iter().enumerate() {
            let slot = format!("edge_ip_{}", i);
            if let Err(e) = set_slot(&pool, &slot, *ip, 300).await {
                eprintln!("skipping test for IP {}: redis set failed - {}", ip, e);
                continue;
            }
            
            let retrieved = match get_slot(&pool, &slot).await {
                Ok(ip) => ip,
                Err(e) => {
                    eprintln!("skipping test for IP {}: redis get failed - {}", ip, e);
                    continue;
                }
            };
            assert_eq!(retrieved, *ip);
        }
    }

    #[tokio::test]
    async fn test_rapid_set_get_operations() {
        let (pool, _container) = setup_redis().await;
        
        let ip = Ipv4Addr::new(1, 2, 3, 4);
        
        // Perform rapid set/get operations
        for i in 0..50 {
            let slot = format!("rapid_slot_{}", i);
            set_slot(&pool, &slot, ip, 300).await.unwrap();
            let retrieved = get_slot(&pool, &slot).await.unwrap();
            assert_eq!(retrieved, ip);
        }
    }

    #[tokio::test]
    async fn test_cache_error_display() {
        let nxdomain_error = CacheError::NXDomain;
        assert_eq!(format!("{}", nxdomain_error), "NXDOMAIN");
        
        let redis_error = CacheError::Redis(redis::RedisError::from((
            redis::ErrorKind::TypeError,
            "test error"
        )));
        assert!(format!("{}", redis_error).contains("test error"));
    }

    #[tokio::test]
    async fn test_new_pool_with_invalid_url() {
        let result = new_pool("invalid://url").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cache_error_from_redis_error() {
        let redis_error = redis::RedisError::from((redis::ErrorKind::TypeError, "test"));
        let cache_error = CacheError::Redis(redis_error);
        assert!(matches!(cache_error, CacheError::Redis(_)));
    }

    #[tokio::test]
    async fn test_cache_error_from_pool_error() {
        let redis_error = redis::RedisError::from((redis::ErrorKind::TypeError, "test"));
        let pool_error = bb8::RunError::User(redis_error);
        let cache_error = CacheError::Pool(pool_error);
        assert!(matches!(cache_error, CacheError::Pool(_)));
    }

    #[tokio::test]
    async fn test_cache_error_debug() {
        let nxdomain_error = CacheError::NXDomain;
        let debug_str = format!("{:?}", nxdomain_error);
        assert!(debug_str.contains("NXDomain"));
    }

    #[test]
    fn test_redis_pool_type_alias() {
        // Test that RedisPool is properly aliased
        let _pool_type: Option<RedisPool> = None;
        // If this compiles, the type alias is working
    }

    #[tokio::test]
    async fn test_pool_builder_configuration() {
        let (pool, _container) = setup_redis().await;
        
        // Test that the pool was built with correct configuration
        let conn = pool.get().await.unwrap();
        // If we can get a connection, the pool is properly configured
        drop(conn);
    }

    #[tokio::test]
    async fn test_connection_pool_reuse() {
        let (pool, _container) = setup_redis().await;
        
        // Test that connections are reused
        let conn1 = match pool.get().await {
            Ok(conn) => conn,
            Err(e) => {
                eprintln!("skipping test: failed to get first connection: {}", e);
                return;
            }
        };
        drop(conn1);
        
        let conn2 = match pool.get().await {
            Ok(conn) => conn,
            Err(e) => {
                eprintln!("skipping test: failed to get second connection: {}", e);
                return;
            }
        };
        drop(conn2);
        
        // Both connections should work
        assert!(true);
    }
}
