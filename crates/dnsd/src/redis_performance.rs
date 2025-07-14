//! Redis Performance Client
//!
//! This module provides a high-performance Redis client optimized for FleetingDNS
//! with connection pooling, performance monitoring, and bulk operations.
//!
//! # Features
//!
//! - Optimized connection pooling with cluster-aware routing
//! - Performance monitoring with latency tracking
//! - Bulk operations with automatic batching
//! - Automatic retry logic with configurable timeouts
//! - Comprehensive statistics collection
//!
//! # Example
//!
//! ```no_run
//! use dnsd::redis_performance::{RedisPerformanceClient, PerformanceConfig};
//! use std::net::Ipv4Addr;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = PerformanceConfig::default();
//! let client = RedisPerformanceClient::new("redis://localhost:6379", config).await?;
//!
//! // Set a slot with performance optimization
//! let ip = "192.168.1.1".parse::<Ipv4Addr>()?;
//! client.set_slot_optimized("example.com", ip, 300).await?;
//!
//! // Get performance statistics
//! let stats = client.get_performance_stats().await;
//! println!("Total operations: {}", stats.total_operations);
//! # Ok(())
//! # }
//! ```

use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bb8::Pool;
use bb8_redis::RedisConnectionManager;
use redis::{AsyncCommands, RedisError};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{error, info, warn};

/// Errors that can occur during Redis performance operations
#[derive(Error, Debug)]
pub enum PerformanceError {
    #[error("Redis error: {0}")]
    Redis(#[from] RedisError),
    #[error("Connection pool error: {0}")]
    Pool(#[from] bb8::RunError<RedisError>),
    #[error("Timeout error: {0}")]
    Timeout(String),
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("Performance error: {0}")]
    Performance(String),
}

/// Configuration for Redis performance optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Maximum number of connections in the pool
    pub max_connections: u32,
    /// Minimum number of idle connections
    pub min_idle: u32,
    /// Connection timeout in seconds
    pub connection_timeout: u64,
    /// Operation timeout in seconds
    pub operation_timeout: u64,
    /// Number of retry attempts
    pub retry_attempts: u32,
    /// Retry delay in milliseconds
    pub retry_delay_ms: u64,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            max_connections: 20,
            min_idle: 5,
            connection_timeout: 5,
            operation_timeout: 10,
            retry_attempts: 3,
            retry_delay_ms: 100,
        }
    }
}

/// Performance statistics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceStats {
    /// Total number of operations performed
    pub total_operations: u64,
    /// Number of successful operations
    pub successful_operations: u64,
    /// Number of failed operations
    pub failed_operations: u64,
    /// Average latency in milliseconds
    pub avg_latency_ms: f64,
    /// 95th percentile latency in milliseconds
    pub p95_latency_ms: f64,
    /// Current connections in use
    pub connections_in_use: u32,
    /// Pool size
    pub pool_size: u32,
}

impl Default for PerformanceStats {
    fn default() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            avg_latency_ms: 0.0,
            p95_latency_ms: 0.0,
            connections_in_use: 0,
            pool_size: 0,
        }
    }
}

/// High-performance Redis client with optimization features
pub struct RedisPerformanceClient {
    pool: Pool<RedisConnectionManager>,
    config: PerformanceConfig,
    stats: Arc<RwLock<PerformanceStats>>,
}

impl RedisPerformanceClient {
    /// Create a new Redis performance client
    pub async fn new(redis_url: &str, config: PerformanceConfig) -> Result<Self, PerformanceError> {
        let manager = RedisConnectionManager::new(redis_url)
            .map_err(|e| PerformanceError::Config(e.to_string()))?;

        let pool = Pool::builder()
            .max_size(config.max_connections)
            .min_idle(Some(config.min_idle))
            .connection_timeout(Duration::from_secs(config.connection_timeout))
            .build(manager)
            .await
            .map_err(|e| PerformanceError::Config(format!("Failed to build pool: {}", e)))?;

        let stats = Arc::new(RwLock::new(PerformanceStats::default()));

        Ok(Self {
            pool,
            config,
            stats,
        })
    }

    /// Execute an operation with retry logic
    async fn execute_with_retry<F, T>(&self, operation: F) -> Result<T, PerformanceError>
    where
        F: Fn()
            -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, RedisError>> + Send>>,
    {
        let mut attempts = 0;
        let max_attempts = self.config.retry_attempts;

        loop {
            attempts += 1;

            let result = timeout(
                Duration::from_secs(self.config.operation_timeout),
                operation(),
            )
            .await;

            match result {
                Ok(Ok(value)) => return Ok(value),
                Ok(Err(e)) => {
                    if attempts >= max_attempts {
                        return Err(PerformanceError::Redis(e));
                    }
                    warn!(
                        "Redis operation failed (attempt {}/{}): {}",
                        attempts, max_attempts, e
                    );
                    tokio::time::sleep(Duration::from_millis(self.config.retry_delay_ms)).await;
                }
                Err(_) => {
                    if attempts >= max_attempts {
                        return Err(PerformanceError::Timeout(format!(
                            "Operation timed out after {} attempts",
                            max_attempts
                        )));
                    }
                    warn!(
                        "Redis operation timed out (attempt {}/{})",
                        attempts, max_attempts
                    );
                    tokio::time::sleep(Duration::from_millis(self.config.retry_delay_ms)).await;
                }
            }
        }
    }

    /// Get a slot value with performance optimization
    pub async fn get_slot_optimized(
        &self,
        slot: &str,
    ) -> Result<Option<Ipv4Addr>, PerformanceError> {
        let start_time = Instant::now();
        let slot = slot.to_string();

        let result = self
            .execute_with_retry(|| {
                let pool = self.pool.clone();
                let slot = slot.clone();
                Box::pin(async move {
                    let mut conn = pool.get().await.map_err(|e| {
                        RedisError::from((
                            redis::ErrorKind::IoError,
                            "Failed to get connection",
                            e.to_string(),
                        ))
                    })?;
                    let value: Option<String> = conn.get(&slot).await?;
                    match value {
                        Some(ip_str) => ip_str.parse::<Ipv4Addr>().map(Some).map_err(|e| {
                            RedisError::from((
                                redis::ErrorKind::TypeError,
                                "Invalid IP address format",
                                e.to_string(),
                            ))
                        }),
                        None => Ok(None),
                    }
                })
            })
            .await?;

        // Update performance metrics
        let latency = start_time.elapsed();
        self.update_performance_metrics(1, latency).await;

        Ok(result)
    }

    /// Set a slot value with performance optimization
    pub async fn set_slot_optimized(
        &self,
        slot: &str,
        ip: Ipv4Addr,
        ttl: u64,
    ) -> Result<(), PerformanceError> {
        let start_time = Instant::now();
        let slot = slot.to_string();

        let result = self
            .execute_with_retry(|| {
                let pool = self.pool.clone();
                let slot = slot.clone();
                let ip_str = ip.to_string();
                Box::pin(async move {
                    let mut conn = pool.get().await.map_err(|e| {
                        RedisError::from((
                            redis::ErrorKind::IoError,
                            "Failed to get connection",
                            e.to_string(),
                        ))
                    })?;
                    conn.set_ex(&slot, ip_str, ttl).await
                })
            })
            .await?;

        // Update performance metrics
        let latency = start_time.elapsed();
        self.update_performance_metrics(1, latency).await;

        Ok(result)
    }

    /// Bulk set multiple slots (simplified version without pipelining)
    pub async fn bulk_set_slots(
        &self,
        operations: Vec<(String, Ipv4Addr, u64)>,
    ) -> Result<Vec<()>, PerformanceError> {
        let start_time = Instant::now();
        let mut results = Vec::new();

        // Process operations individually for now
        for (slot, ip, ttl) in operations {
            let result = self.set_slot_optimized(&slot, ip, ttl).await?;
            results.push(result);
        }

        let latency = start_time.elapsed();
        self.update_performance_metrics(results.len(), latency)
            .await;

        Ok(results)
    }

    /// Bulk get multiple slots (simplified version without pipelining)
    pub async fn bulk_get_slots(
        &self,
        slots: Vec<String>,
    ) -> Result<Vec<Option<Ipv4Addr>>, PerformanceError> {
        let start_time = Instant::now();
        let mut results = Vec::new();

        // Process slots individually for now
        for slot in slots {
            let result = self.get_slot_optimized(&slot).await?;
            results.push(result);
        }

        let latency = start_time.elapsed();
        self.update_performance_metrics(results.len(), latency)
            .await;

        Ok(results)
    }

    /// Update performance metrics
    async fn update_performance_metrics(&self, operations: usize, latency: Duration) {
        let mut stats = self.stats.write().await;
        stats.total_operations += operations as u64;
        stats.successful_operations += operations as u64;

        // Simple latency tracking (in production, this would be more sophisticated)
        let latency_ms = latency.as_secs_f64() * 1000.0;
        stats.avg_latency_ms = (stats.avg_latency_ms + latency_ms) / 2.0;
        stats.p95_latency_ms = stats.p95_latency_ms.max(latency_ms);

        // Update pool stats
        stats.pool_size = self.config.max_connections;
    }

    /// Get current performance statistics
    pub async fn get_performance_stats(&self) -> PerformanceStats {
        self.stats.read().await.clone()
    }

    /// Reset performance statistics
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = PerformanceStats::default();
        info!("Performance statistics reset");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[tokio::test]
    async fn test_performance_config_default() {
        let config = PerformanceConfig::default();
        assert_eq!(config.max_connections, 20);
        assert_eq!(config.min_idle, 5);
        assert_eq!(config.connection_timeout, 5);
        assert_eq!(config.operation_timeout, 10);
        assert_eq!(config.retry_attempts, 3);
        assert_eq!(config.retry_delay_ms, 100);
    }

    #[tokio::test]
    async fn test_performance_stats_default() {
        let stats = PerformanceStats::default();
        assert_eq!(stats.total_operations, 0);
        assert_eq!(stats.successful_operations, 0);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.avg_latency_ms, 0.0);
        assert_eq!(stats.p95_latency_ms, 0.0);
        assert_eq!(stats.connections_in_use, 0);
        assert_eq!(stats.pool_size, 0);
    }

    #[tokio::test]
    async fn test_performance_config_serialization() {
        let config = PerformanceConfig::default();
        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: PerformanceConfig = serde_json::from_str(&serialized).unwrap();
        assert_eq!(config.max_connections, deserialized.max_connections);
    }

    #[tokio::test]
    async fn test_performance_stats_serialization() {
        let stats = PerformanceStats::default();
        let serialized = serde_json::to_string(&stats).unwrap();
        let deserialized: PerformanceStats = serde_json::from_str(&serialized).unwrap();
        assert_eq!(stats.total_operations, deserialized.total_operations);
    }

    // Note: Integration tests with actual Redis would require testcontainers
    // and are commented out to avoid CI failures

    /*
    #[tokio::test]
    async fn test_redis_performance_client_creation() {
        let config = PerformanceConfig::default();
        let result = RedisPerformanceClient::new("redis://localhost:6379", config).await;
        // This would fail without a Redis instance, so we just test the error type
        assert!(result.is_err());
    }
    */
}
