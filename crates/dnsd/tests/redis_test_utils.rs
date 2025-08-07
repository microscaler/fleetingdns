use bb8_redis::RedisConnectionManager;
use common::redis::RedisPool;
use once_cell::sync::Lazy;
use std::sync::Arc;
use testcontainers::{ImageExt, runners::AsyncRunner};
use testcontainers_modules::redis::Redis;

/// Redis test fixture using testcontainers
pub struct RedisTestFixture {
    container: testcontainers::ContainerAsync<Redis>,
    pool: RedisPool,
}

impl RedisTestFixture {
    /// Create a new Redis test fixture
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Start Redis container
        let redis_container = Redis::default()
            .with_tag("7.2-alpine")
            .start()
            .await
            .map_err(|e| format!("Failed to start Redis container: {:?}", e))?;

        // Get the port that Redis is running on
        let port = redis_container
            .get_host_port_ipv4(6379)
            .await
            .map_err(|e| format!("Failed to get Redis port: {:?}", e))?;

        // Create connection pool
        let url = format!("redis://localhost:{}", port);
        let pool = Self::create_pool(&url)?;

        // Wait for Redis to be ready
        Self::wait_for_redis_ready(&pool).await?;

        Ok(Self {
            container: redis_container,
            pool,
        })
    }

    /// Create a Redis connection pool
    fn create_pool(url: &str) -> Result<RedisPool, Box<dyn std::error::Error>> {
        let manager = RedisConnectionManager::new(url.to_string())
            .map_err(|e| format!("Failed to create Redis connection manager: {}", e))?;

        let pool = bb8::Pool::builder()
            .max_size(10)
            .min_idle(Some(2))
            .build_unchecked(manager);

        Ok(pool)
    }

    /// Wait for Redis to be ready
    async fn wait_for_redis_ready(pool: &RedisPool) -> Result<(), Box<dyn std::error::Error>> {
        for i in 0..30 {
            match pool.get().await {
                Ok(conn) => {
                    // Test the connection with a simple PING
                    let mut conn = conn;
                    match bb8_redis::redis::cmd("PING").query_async::<String>(&mut *conn).await {
                        Ok(pong) if pong == "PONG" => {
                            return Ok(());
                        }
                        _ => {
                            // Connection works but PING failed, wait a bit more
                            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        }
                    }
                }
                Err(_) => {
                    if i == 29 {
                        return Err("Redis failed to start within 3 seconds".into());
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }
        }
        Err("Redis failed to start within 3 seconds".into())
    }

    /// Get the Redis connection pool
    pub fn get_pool(&self) -> RedisPool {
        self.pool.clone()
    }

    /// Get container info for debugging
    pub async fn get_container_info(&self) -> String {
        let port = self.container.get_host_port_ipv4(6379).await.unwrap_or(0);
        format!("Redis container running on port {}", port)
    }
}

/// Run a test with a Redis container
pub async fn with_redis_container<F, Fut, T>(test_fn: F) -> Result<T, Box<dyn std::error::Error>>
where
    F: FnOnce(RedisPool) -> Fut,
    Fut: std::future::Future<Output = T>,
{
    let fixture = RedisTestFixture::new().await?;
    let pool = fixture.get_pool();

    // Run the test
    let result = test_fn(pool).await;

    // Container is automatically cleaned up when fixture goes out of scope
    Ok(result)
}

/// Run multiple tests with the same Redis container (for performance)
pub async fn with_shared_redis_container<F, Fut, T>(
    test_fn: F,
) -> Result<T, Box<dyn std::error::Error>>
where
    F: FnOnce(RedisPool) -> Fut,
    Fut: std::future::Future<Output = T>,
{
    // For now, just use the regular container approach to avoid runtime conflicts
    // TODO: Implement proper shared container when needed
    with_redis_container(test_fn).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_redis_container_lifecycle() {
        let fixture = RedisTestFixture::new().await.unwrap();
        let pool = fixture.get_pool();

        // Test connection
        let conn = pool.get().await;
        assert!(conn.is_ok());

        // Test basic Redis operations
        let mut conn = conn.unwrap();
                  let result: Result<String, _> = bb8_redis::redis::cmd("PING").query_async(&mut *conn).await;
        assert_eq!(result.unwrap(), "PONG");
    }

    #[tokio::test]
    async fn test_with_redis_container() {
        let result = with_redis_container(|pool| async move {
            let conn = pool.get().await;
            assert!(conn.is_ok());

            // Test basic operations
            let mut conn = conn.unwrap();
                          let result: Result<String, _> = bb8_redis::redis::cmd("SET")
                .arg("test_key")
                .arg("test_value")
                .query_async(&mut *conn)
                .await;
            assert!(result.is_ok());

                          let result: Result<String, _> = bb8_redis::redis::cmd("GET")
                .arg("test_key")
                .query_async(&mut *conn)
                .await;
            assert_eq!(result.unwrap(), "test_value");

            "success"
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
    }

    #[tokio::test]
    async fn test_redis_operations() {
        let result = with_redis_container(|pool| async move {
            let mut conn = pool.get().await.unwrap();

            // Test various Redis operations
                          let _: () = bb8_redis::redis::cmd("SET")
                .arg("key1")
                .arg("value1")
                .query_async(&mut *conn)
                .await
                .unwrap();
                          let _: () = bb8_redis::redis::cmd("SET")
                .arg("key2")
                .arg("value2")
                .query_async(&mut *conn)
                .await
                .unwrap();

                          let value1: String = bb8_redis::redis::cmd("GET")
                .arg("key1")
                .query_async(&mut *conn)
                .await
                .unwrap();
                          let value2: String = bb8_redis::redis::cmd("GET")
                .arg("key2")
                .query_async(&mut *conn)
                .await
                .unwrap();

            assert_eq!(value1, "value1");
            assert_eq!(value2, "value2");

            "success"
        })
        .await;

        assert!(result.is_ok());
    }
}
