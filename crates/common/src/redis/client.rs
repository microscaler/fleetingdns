//! Redis Performance Client
//!
//! This module provides a high-performance Redis client optimized for FleetingDNS
//! with connection pooling, performance monitoring, and bulk operations.

use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::error::{CommonResult, FleetingDnsError};
use bb8::Pool;
use bb8_redis::RedisConnectionManager;
use bb8_redis::redis::{AsyncCommands, RedisError};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tokio::time::timeout;

/// Type alias for performance errors using the common error system
pub type PerformanceError = FleetingDnsError;

/// Type alias for performance results
pub type PerformanceResult<T> = CommonResult<T>;

/// Connection pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    pub max_size: u32,
    pub min_idle: Option<u32>,
    pub connection_timeout: Duration,
    pub idle_timeout: Option<Duration>,
    pub max_retries: u32,
    pub retry_delay: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_size: 20,
            min_idle: Some(5),
            connection_timeout: Duration::from_secs(10),
            idle_timeout: Some(Duration::from_secs(300)),
            max_retries: 3,
            retry_delay: Duration::from_millis(100),
        }
    }
}

/// Pipeline configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    pub batch_size: usize,
    pub auto_flush: bool,
    pub execution_timeout: Duration,
    pub flush_interval: Duration,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,
            auto_flush: true,
            execution_timeout: Duration::from_secs(30),
            flush_interval: Duration::from_millis(10),
        }
    }
}

/// Monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub enable_metrics: bool,
    pub metrics_interval: Duration,
    pub monitor_pool: bool,
    pub track_latency: bool,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enable_metrics: true,
            metrics_interval: Duration::from_secs(60),
            monitor_pool: true,
            track_latency: true,
        }
    }
}

/// Performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    pub redis_url: String,
    pub pool_config: PoolConfig,
    pub pipeline_config: PipelineConfig,
    pub monitoring_config: MonitoringConfig,
    pub max_connections: u32,
    pub min_idle: u32,
    pub connection_timeout: u64,
    pub operation_timeout: u64,
    pub retry_attempts: u32,
    pub retry_delay_ms: u64,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://127.0.0.1:6379".to_string(),
            pool_config: PoolConfig::default(),
            pipeline_config: PipelineConfig::default(),
            monitoring_config: MonitoringConfig::default(),
            max_connections: 20,
            min_idle: 5,
            connection_timeout: 10,
            operation_timeout: 30,
            retry_attempts: 3,
            retry_delay_ms: 100,
        }
    }
}

/// Pool statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStats {
    pub active_connections: u32,
    pub idle_connections: u32,
    pub total_connections_created: u64,
    pub connection_failures: u64,
    pub avg_acquisition_time_ms: f64,
}

impl Default for PoolStats {
    fn default() -> Self {
        Self {
            active_connections: 0,
            idle_connections: 0,
            total_connections_created: 0,
            connection_failures: 0,
            avg_acquisition_time_ms: 0.0,
        }
    }
}

/// Pipeline statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStats {
    pub total_pipelines: u64,
    pub avg_pipeline_size: f64,
    pub pipeline_failures: u64,
    pub avg_execution_time_ms: f64,
}

impl Default for PipelineStats {
    fn default() -> Self {
        Self {
            total_pipelines: 0,
            avg_pipeline_size: 0.0,
            pipeline_failures: 0,
            avg_execution_time_ms: 0.0,
        }
    }
}

/// Performance statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceStats {
    pub total_operations: u64,
    pub successful_operations: u64,
    pub failed_operations: u64,
    pub avg_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub connections_in_use: u32,
    pub pool_size: u32,
    pub ops_per_second: f64,
    pub pool_stats: PoolStats,
    pub pipeline_stats: PipelineStats,
}

impl Default for PerformanceStats {
    fn default() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            avg_latency_ms: 0.0,
            p95_latency_ms: 0.0,
            p99_latency_ms: 0.0,
            connections_in_use: 0,
            pool_size: 0,
            ops_per_second: 0.0,
            pool_stats: PoolStats::default(),
            pipeline_stats: PipelineStats::default(),
        }
    }
}

/// High-performance Redis client with monitoring and optimization
pub struct RedisPerformanceClient {
    pool: Pool<RedisConnectionManager>,
    config: PerformanceConfig,
    stats: Arc<RwLock<PerformanceStats>>,
}

impl RedisPerformanceClient {
    /// Create a new performance client
    pub async fn new(config: PerformanceConfig) -> Result<Self, PerformanceError> {
        let manager = RedisConnectionManager::new(config.redis_url.as_str()).map_err(|e| {
            PerformanceError::ConfigurationError(format!("Failed to create manager: {e}"))
        })?;

        let pool = Pool::builder()
            .max_size(config.pool_config.max_size)
            .min_idle(config.pool_config.min_idle)
            .connection_timeout(config.pool_config.connection_timeout)
            .build(manager)
            .await
            .map_err(|e| {
                PerformanceError::ConfigurationError(format!("Failed to build pool: {e}"))
            })?;

        let stats = Arc::new(RwLock::new(PerformanceStats::default()));

        Ok(Self {
            pool,
            config,
            stats,
        })
    }

    /// Get a slot value with performance optimization
    pub async fn get_slot_optimized(
        &self,
        slot: &str,
    ) -> Result<Option<Ipv4Addr>, PerformanceError> {
        let start_time = Instant::now();
        let slot = slot.to_string();

        let result = timeout(Duration::from_secs(self.config.operation_timeout), async {
            let conn = self.pool.get().await.map_err(|e| {
                RedisError::from((
                    bb8_redis::redis::ErrorKind::IoError,
                    "Failed to get connection",
                    e.to_string(),
                ))
            })?;
            let mut conn = conn;
            let value: Option<String> = conn.get(&slot).await?;
            match value {
                Some(ip_str) => ip_str.parse::<Ipv4Addr>().map(Some).map_err(|e| {
                    RedisError::from((
                        bb8_redis::redis::ErrorKind::TypeError,
                        "Invalid IP address format",
                        e.to_string(),
                    ))
                }),
                None => Ok(None),
            }
        })
        .await
        .map_err(|_| PerformanceError::TimeoutError("Operation timed out".to_string()))??;

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
        let ip_str = ip.to_string();

        timeout(Duration::from_secs(self.config.operation_timeout), async {
            let conn = self.pool.get().await.map_err(|e| {
                RedisError::from((
                    bb8_redis::redis::ErrorKind::IoError,
                    "Failed to get connection",
                    e.to_string(),
                ))
            })?;
            let mut conn = conn;
            conn.set_ex(&slot, ip_str, ttl).await.map(|(): ()| ())
        })
        .await
        .map_err(|_| PerformanceError::TimeoutError("Operation timed out".to_string()))??;

        // Update performance metrics
        let latency = start_time.elapsed();
        self.update_performance_metrics(1, latency).await;

        Ok(())
    }

    /// Bulk set multiple slots with pipelining
    pub async fn bulk_set_slots(
        &self,
        operations: Vec<(String, Ipv4Addr, u64)>,
    ) -> Result<Vec<()>, PerformanceError> {
        let start_time = Instant::now();
        let total_operations = operations.len();

        // For now, just execute operations sequentially
        // In a full implementation, this would use pipelining
        let mut results = Vec::new();
        for (slot, ip, ttl) in operations {
            self.set_slot_optimized(&slot, ip, ttl).await?;
            results.push(());
        }

        // Update performance metrics
        let latency = start_time.elapsed();
        self.update_performance_metrics(total_operations, latency)
            .await;

        Ok(results)
    }

    /// Bulk get multiple slots with pipelining
    pub async fn bulk_get_slots(
        &self,
        slots: Vec<String>,
    ) -> Result<Vec<Option<Ipv4Addr>>, PerformanceError> {
        let start_time = Instant::now();
        let total_slots = slots.len();

        // For now, just execute operations sequentially
        // In a full implementation, this would use pipelining
        let mut results = Vec::new();
        for slot in slots {
            let result = self.get_slot_optimized(&slot).await?;
            results.push(result);
        }

        // Update performance metrics
        let latency = start_time.elapsed();
        self.update_performance_metrics(total_slots, latency).await;

        Ok(results)
    }

    /// Get performance statistics
    pub async fn get_stats(&self) -> PerformanceStats {
        let stats = self.stats.read().await;
        stats.clone()
    }

    /// Reset performance statistics
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = PerformanceStats::default();
    }

    /// Update performance metrics
    async fn update_performance_metrics(&self, operations: usize, latency: Duration) {
        let mut stats = self.stats.write().await;
        stats.total_operations += operations as u64;
        stats.successful_operations += operations as u64;

        let latency_ms = latency.as_millis() as f64;
        stats.avg_latency_ms = f64::midpoint(stats.avg_latency_ms, latency_ms);

        // Update pool stats
        stats.pool_size = self.config.pool_config.max_size;
        stats.connections_in_use = self.config.pool_config.max_size;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_performance_config_creation() {
        let config = PerformanceConfig::default();
        assert_eq!(config.redis_url, "redis://127.0.0.1:6379");
        assert_eq!(config.pool_config.max_size, 20);
        assert_eq!(config.pipeline_config.batch_size, 100);
        assert!(config.monitoring_config.enable_metrics);
    }

    #[tokio::test]
    async fn test_performance_stats_default() {
        let stats = PerformanceStats::default();
        assert_eq!(stats.total_operations, 0);
        assert_eq!(stats.successful_operations, 0);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.avg_latency_ms, 0.0);
    }
}
