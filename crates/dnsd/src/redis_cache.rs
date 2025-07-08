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
    use tokio::process::Command;
    use tokio::time::{Duration, sleep};

    #[tokio::test]
    async fn set_get_respects_ttl() {
        let port = 6380u16;
        if Command::new("redis-server")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .status()
            .await
            .is_err()
        {
            eprintln!("skipping test: redis-server not installed");
            return;
        }

        let mut child = Command::new("redis-server")
            .arg("--port")
            .arg(port.to_string())
            .arg("--bind")
            .arg("127.0.0.1")
            .arg("--save")
            .arg("")
            .arg("--appendonly")
            .arg("no")
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("start redis");

        sleep(Duration::from_millis(500)).await;

        let pool = new_pool(&format!("redis://127.0.0.1:{port}"))
            .await
            .unwrap();
        let ip = Ipv4Addr::new(1, 2, 3, 4);
        set_slot(&pool, "slot1", ip, 1).await.unwrap();
        let got = get_slot(&pool, "slot1").await.unwrap();
        assert_eq!(got, ip);
        sleep(Duration::from_secs(2)).await;
        let err = get_slot(&pool, "slot1").await.unwrap_err();
        assert!(matches!(err, CacheError::NXDomain));
        child.kill().await.ok();
    }
}
