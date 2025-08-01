use std::process::Command;
use std::time::Duration;
use tokio::time::sleep;
use crate::redis_cache::RedisPool;

pub struct RedisTestContainer {
    container_name: String,
}

impl RedisTestContainer {
    pub fn new() -> Self {
        let container_name = format!("redis-test-{}", std::process::id());
        Self { container_name }
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Stop any existing container with the same name
        let _ = Command::new("docker")
            .args(["stop", &self.container_name])
            .output();

        let _ = Command::new("docker")
            .args(["rm", &self.container_name])
            .output();

        // Start new Redis container
        let output = Command::new("docker")
            .args([
                "run",
                "-d",
                "--name", &self.container_name,
                "-p", "6379:6379",
                "redis:7-alpine",
            ])
            .output()?;

        if !output.status.success() {
            return Err(format!(
                "Failed to start Redis container: {}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }

        // Wait for Redis to be ready
        for _ in 0..30 {
            sleep(Duration::from_millis(100)).await;
            
            // Test connection
            let pool = self.create_pool();
            if let Ok(conn) = pool.get().await {
                drop(conn);
                return Ok(());
            }
        }

        Err("Redis container failed to start within 3 seconds".into())
    }

    pub fn stop(&self) -> Result<(), Box<dyn std::error::Error>> {
        let output = Command::new("docker")
            .args(["stop", &self.container_name])
            .output()?;

        if !output.status.success() {
            return Err(format!(
                "Failed to stop Redis container: {}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }

        let output = Command::new("docker")
            .args(["rm", &self.container_name])
            .output()?;

        if !output.status.success() {
            return Err(format!(
                "Failed to remove Redis container: {}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }

        Ok(())
    }

    pub fn create_pool(&self) -> RedisPool {
        bb8::Pool::builder()
            .build_unchecked(bb8_redis::RedisConnectionManager::new("redis://localhost:6379").unwrap())
    }
}

impl Drop for RedisTestContainer {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

pub async fn with_redis_test_container<F, Fut, T>(test_fn: F) -> Result<T, Box<dyn std::error::Error>>
where
    F: FnOnce(RedisPool) -> Fut,
    Fut: std::future::Future<Output = T>,
{
    let container = RedisTestContainer::new();
    container.start().await?;
    
    let pool = container.create_pool();
    let result = test_fn(pool).await;
    
    container.stop()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_redis_container_lifecycle() {
        let container = RedisTestContainer::new();
        
        // Start container
        assert!(container.start().await.is_ok());
        
        // Test connection
        let pool = container.create_pool();
        let conn = pool.get().await;
        assert!(conn.is_ok());
        
        // Stop container
        assert!(container.stop().is_ok());
    }

    #[tokio::test]
    async fn test_with_redis_test_container() {
        let result = with_redis_test_container(|pool| async move {
            let conn = pool.get().await;
            assert!(conn.is_ok());
            "success"
        }).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
    }
} 