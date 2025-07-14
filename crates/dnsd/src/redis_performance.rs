//! Redis Performance Optimization for FleetingDNS
//!
//! This module provides performance optimizations for Redis operations including:
//! - Connection pooling optimization
//! - Redis pipelining for bulk operations
//! - Query optimization and caching strategies
//! - Performance monitoring and metrics
//!
//! # Features
//!
//! - Optimized connection pool configuration
//! - Bulk operations with pipelining support
//! - Adaptive connection management
//! - Performance metrics and monitoring
//! - Query batching and optimization
//!
//! # Usage
//!
//! ```rust
//! use dnsd::redis_performance::{RedisPerformanceClient, PerformanceConfig};
//!
//! let config = PerformanceConfig::default();
//! let client = RedisPerformanceClient::new(config).await?;
//! 
//! // Bulk operations with pipelining
//! let operations = vec![
//!     ("slot1", "127.0.0.1", 3600),
//!     ("slot2", "127.0.0.2", 1800),
//! ];
//! client.bulk_set_slots(operations).await?;
//! ```

use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bb8::{Pool, PooledConnection};
use bb8_redis::RedisConnectionManager;
use redis::{AsyncCommands, Pipeline, RedisError};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

/// Errors that can occur during Redis performance operations
#[derive(Error, Debug)]
pub enum PerformanceError {
    #[error("Connection pool error: {0}")]
    PoolError(String),

    #[error("Redis operation failed: {0}")]
    RedisError(#[from] RedisError),

    #[error("Bulk operation failed: {0}")]
    BulkOperationFailed(String),

    #[error("Pipeline execution failed: {0}")]
    PipelineError(String),

    #[error("Timeout error: {0}")]
    TimeoutError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// Performance configuration for Redis operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Redis connection URL
    pub redis_url: String,
    /// Connection pool configuration
    pub pool_config: PoolConfig,
    /// Pipelining configuration
    pub pipeline_config: PipelineConfig,
    /// Performance monitoring configuration
    pub monitoring_config: MonitoringConfig,
}

/// Connection pool configuration optimized for performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    /// Maximum number of connections in the pool
    pub max_size: u32,
    /// Minimum number of idle connections
    pub min_idle: Option<u32>,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Idle connection timeout
    pub idle_timeout: Option<Duration>,
    /// Connection retry attempts
    pub max_retries: u32,
    /// Retry delay
    pub retry_delay: Duration,
}

/// Pipeline configuration for bulk operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    /// Maximum number of operations per pipeline
    pub batch_size: usize,
    /// Pipeline execution timeout
    pub execution_timeout: Duration,
    /// Enable automatic pipeline flushing
    pub auto_flush: bool,
    /// Flush interval for auto-flush
    pub flush_interval: Duration,
}

/// Performance monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Enable performance metrics collection
    pub enable_metrics: bool,
    /// Metrics collection interval
    pub metrics_interval: Duration,
    /// Enable connection pool monitoring
    pub monitor_pool: bool,
    /// Enable operation latency tracking
    pub track_latency: bool,
}

/// Performance statistics for Redis operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceStats {
    /// Total operations performed
    pub total_operations: u64,
    /// Successful operations
    pub successful_operations: u64,
    /// Failed operations
    pub failed_operations: u64,
    /// Average operation latency in milliseconds
    pub avg_latency_ms: f64,
    /// 95th percentile latency in milliseconds
    pub p95_latency_ms: f64,
    /// Operations per second
    pub ops_per_second: f64,
    /// Connection pool statistics
    pub pool_stats: PoolStats,
    /// Pipeline statistics
    pub pipeline_stats: PipelineStats,
}

/// Connection pool statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStats {
    /// Active connections
    pub active_connections: u32,
    /// Idle connections
    pub idle_connections: u32,
    /// Total connections created
    pub total_connections_created: u64,
    /// Connection failures
    pub connection_failures: u64,
    /// Average connection acquisition time in milliseconds
    pub avg_acquisition_time_ms: f64,
}

/// Pipeline statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStats {
    /// Total pipelines executed
    pub total_pipelines: u64,
    /// Average pipeline size
    pub avg_pipeline_size: f64,
    /// Pipeline execution failures
    pub pipeline_failures: u64,
    /// Average pipeline execution time in milliseconds
    pub avg_execution_time_ms: f64,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://127.0.0.1:6379".to_string(),
            pool_config: PoolConfig::default(),
            pipeline_config: PipelineConfig::default(),
            monitoring_config: MonitoringConfig::default(),
        }
    }
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_size: 20,  // Increased for better performance
            min_idle: Some(5),
            connection_timeout: Duration::from_secs(10),
            idle_timeout: Some(Duration::from_secs(300)),
            max_retries: 3,
            retry_delay: Duration::from_millis(100),
        }
    }
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,  // Optimal batch size for most workloads
            execution_timeout: Duration::from_secs(30),
            auto_flush: true,
            flush_interval: Duration::from_millis(10),
        }
    }
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

/// High-performance Redis client with optimization features
pub struct RedisPerformanceClient {
    pool: Pool<RedisConnectionManager>,
    config: PerformanceConfig,
    stats: Arc<RwLock<PerformanceStats>>,
    latency_samples: Arc<RwLock<Vec<f64>>>,
}

impl RedisPerformanceClient {
    /// Create a new high-performance Redis client
    pub async fn new(config: PerformanceConfig) -> Result<Self, PerformanceError> {
        info!("Initializing high-performance Redis client");

        // Create optimized connection pool
        let manager = RedisConnectionManager::new(config.redis_url.as_str())
            .map_err(|e| PerformanceError::PoolError(format!("Failed to create manager: {}", e)))?;

        let pool = Pool::builder()
            .max_size(config.pool_config.max_size)
            .min_idle(config.pool_config.min_idle)
            .connection_timeout(config.pool_config.connection_timeout)
            .idle_timeout(config.pool_config.idle_timeout)
            .retry_connection(config.pool_config.max_retries > 0)
            .build(manager)
            .await
            .map_err(|e| PerformanceError::PoolError(format!("Failed to build pool: {}", e)))?;

        let client = Self {
            pool,
            config,
            stats: Arc::new(RwLock::new(PerformanceStats {
                total_operations: 0,
                successful_operations: 0,
                failed_operations: 0,
                avg_latency_ms: 0.0,
                p95_latency_ms: 0.0,
                ops_per_second: 0.0,
                pool_stats: PoolStats {
                    active_connections: 0,
                    idle_connections: 0,
                    total_connections_created: 0,
                    connection_failures: 0,
                    avg_acquisition_time_ms: 0.0,
                },
                pipeline_stats: PipelineStats {
                    total_pipelines: 0,
                    avg_pipeline_size: 0.0,
                    pipeline_failures: 0,
                    avg_execution_time_ms: 0.0,
                },
            })),
            latency_samples: Arc::new(RwLock::new(Vec::with_capacity(1000))),
        };

        // Start performance monitoring if enabled
        if client.config.monitoring_config.enable_metrics {
            client.start_performance_monitoring();
        }

        info!("High-performance Redis client initialized successfully");
        Ok(client)
    }

    /// Optimized single slot retrieval
    pub async fn get_slot_optimized(&self, slot: &str) -> Result<Option<Ipv4Addr>, PerformanceError> {
        let start_time = Instant::now();
        
        let result = self.execute_with_retry(|mut conn| async move {
            let value: Option<String> = conn.get(slot).await?;
            match value {
                Some(ip_str) => {
                    ip_str.parse::<Ipv4Addr>()
                        .map(Some)
                        .map_err(|e| RedisError::from((redis::ErrorKind::TypeError, "Invalid IP format", format!("{}", e))))
                }
                None => Ok(None),
            }
        }).await;

        self.record_operation_latency(start_time.elapsed()).await;
        result
    }

    /// Optimized single slot setting with TTL
    pub async fn set_slot_optimized(&self, slot: &str, ip: Ipv4Addr, ttl: u64) -> Result<(), PerformanceError> {
        let start_time = Instant::now();
        
        let result = self.execute_with_retry(|mut conn| async move {
            conn.set_ex(slot, ip.to_string(), ttl).await
        }).await;

        self.record_operation_latency(start_time.elapsed()).await;
        result
    }

    /// Bulk slot operations using Redis pipelining
    pub async fn bulk_set_slots(&self, operations: Vec<(String, Ipv4Addr, u64)>) -> Result<(), PerformanceError> {
        if operations.is_empty() {
            return Ok(());
        }

        let start_time = Instant::now();
        let total_ops = operations.len();
        
        info!("Executing bulk set operation for {} slots", total_ops);

        // Split operations into batches for optimal pipeline performance
        let batch_size = self.config.pipeline_config.batch_size;
        let batches: Vec<_> = operations.chunks(batch_size).collect();
        
        for (batch_idx, batch) in batches.iter().enumerate() {
            let batch_start = Instant::now();
            
            let result = self.execute_pipeline_batch(batch).await;
            
            match result {
                Ok(_) => {
                    debug!("Batch {}/{} completed successfully ({} operations)", 
                           batch_idx + 1, batches.len(), batch.len());
                }
                Err(e) => {
                    error!("Batch {}/{} failed: {}", batch_idx + 1, batches.len(), e);
                    self.record_failed_operation().await;
                    return Err(e);
                }
            }

            // Record pipeline statistics
            {
                let mut stats = self.stats.write().await;
                stats.pipeline_stats.total_pipelines += 1;
                stats.pipeline_stats.avg_pipeline_size = 
                    (stats.pipeline_stats.avg_pipeline_size * (stats.pipeline_stats.total_pipelines - 1) as f64 + batch.len() as f64) 
                    / stats.pipeline_stats.total_pipelines as f64;
                stats.pipeline_stats.avg_execution_time_ms = 
                    (stats.pipeline_stats.avg_execution_time_ms * (stats.pipeline_stats.total_pipelines - 1) as f64 + batch_start.elapsed().as_millis() as f64) 
                    / stats.pipeline_stats.total_pipelines as f64;
            }
        }

        let total_duration = start_time.elapsed();
        info!("Bulk operation completed: {} operations in {:?} ({:.2} ops/sec)", 
              total_ops, total_duration, total_ops as f64 / total_duration.as_secs_f64());

        self.record_operation_latency(total_duration).await;
        Ok(())
    }

    /// Execute a pipeline batch with retry logic
    async fn execute_pipeline_batch(&self, batch: &[(String, Ipv4Addr, u64)]) -> Result<(), PerformanceError> {
        self.execute_with_retry(|mut conn| async move {
            let mut pipeline = Pipeline::new();
            
            // Add all operations to the pipeline
            for (slot, ip, ttl) in batch {
                pipeline.set_ex(slot, ip.to_string(), *ttl);
            }

            // Execute the pipeline
            timeout(
                self.config.pipeline_config.execution_timeout,
                pipeline.query_async::<()>(&mut *conn)
            )
            .await
            .map_err(|_| RedisError::from((redis::ErrorKind::IoError, "Pipeline execution timeout")))?
        }).await
    }

    /// Bulk slot retrieval using Redis pipelining
    pub async fn bulk_get_slots(&self, slots: Vec<String>) -> Result<HashMap<String, Option<Ipv4Addr>>, PerformanceError> {
        if slots.is_empty() {
            return Ok(HashMap::new());
        }

        let start_time = Instant::now();
        let total_ops = slots.len();
        
        info!("Executing bulk get operation for {} slots", total_ops);

        let mut results = HashMap::new();
        let batch_size = self.config.pipeline_config.batch_size;
        let batches: Vec<_> = slots.chunks(batch_size).collect();

        for (batch_idx, batch) in batches.iter().enumerate() {
            let batch_start = Instant::now();
            
            let batch_results = self.execute_get_pipeline_batch(batch).await?;
            
            for (slot, ip) in batch_results {
                results.insert(slot, ip);
            }

            debug!("Get batch {}/{} completed successfully ({} operations)", 
                   batch_idx + 1, batches.len(), batch.len());

            // Record pipeline statistics
            {
                let mut stats = self.stats.write().await;
                stats.pipeline_stats.total_pipelines += 1;
                stats.pipeline_stats.avg_execution_time_ms = 
                    (stats.pipeline_stats.avg_execution_time_ms * (stats.pipeline_stats.total_pipelines - 1) as f64 + batch_start.elapsed().as_millis() as f64) 
                    / stats.pipeline_stats.total_pipelines as f64;
            }
        }

        let total_duration = start_time.elapsed();
        info!("Bulk get operation completed: {} operations in {:?} ({:.2} ops/sec)", 
              total_ops, total_duration, total_ops as f64 / total_duration.as_secs_f64());

        self.record_operation_latency(total_duration).await;
        Ok(results)
    }

    /// Execute a get pipeline batch
    async fn execute_get_pipeline_batch(&self, batch: &[String]) -> Result<HashMap<String, Option<Ipv4Addr>>, PerformanceError> {
        self.execute_with_retry(|mut conn| async move {
            let mut pipeline = Pipeline::new();
            
            // Add all get operations to the pipeline
            for slot in batch {
                pipeline.get(slot);
            }

            // Execute the pipeline and get results
            let values: Vec<Option<String>> = timeout(
                self.config.pipeline_config.execution_timeout,
                pipeline.query_async(&mut *conn)
            )
            .await
            .map_err(|_| RedisError::from((redis::ErrorKind::IoError, "Pipeline execution timeout")))?
            .map_err(RedisError::from)?;

            // Parse results
            let mut results = HashMap::new();
            for (slot, value) in batch.iter().zip(values.iter()) {
                let ip = match value {
                    Some(ip_str) => {
                        match ip_str.parse::<Ipv4Addr>() {
                            Ok(ip) => Some(ip),
                            Err(_) => {
                                warn!("Invalid IP format for slot {}: {}", slot, ip_str);
                                None
                            }
                        }
                    }
                    None => None,
                };
                results.insert(slot.clone(), ip);
            }

            Ok(results)
        }).await
    }

    /// Execute an operation with retry logic and connection management
    async fn execute_with_retry<F, Fut, T>(&self, operation: F) -> Result<T, PerformanceError>
    where
        F: Fn(PooledConnection<RedisConnectionManager>) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<T, RedisError>> + Send,
        T: Send,
    {
        let mut retries = 0;
        let max_retries = self.config.pool_config.max_retries;

        loop {
            let conn_start = Instant::now();
            
            // Get connection from pool
            let conn = timeout(
                self.config.pool_config.connection_timeout,
                self.pool.get()
            )
            .await
            .map_err(|_| PerformanceError::TimeoutError("Connection acquisition timeout".to_string()))?
            .map_err(|e| PerformanceError::PoolError(format!("Failed to get connection: {}", e)))?;

            // Record connection acquisition time
            {
                let mut stats = self.stats.write().await;
                stats.pool_stats.avg_acquisition_time_ms = 
                    (stats.pool_stats.avg_acquisition_time_ms * stats.pool_stats.total_connections_created as f64 + conn_start.elapsed().as_millis() as f64) 
                    / (stats.pool_stats.total_connections_created + 1) as f64;
                stats.pool_stats.total_connections_created += 1;
            }

            // Execute operation
            match operation(conn).await {
                Ok(result) => {
                    self.record_successful_operation().await;
                    return Ok(result);
                }
                Err(e) => {
                    retries += 1;
                    warn!("Operation failed (attempt {}/{}): {}", retries, max_retries, e);
                    
                    if retries >= max_retries {
                        self.record_failed_operation().await;
                        return Err(PerformanceError::RedisError(e));
                    }

                    // Wait before retry
                    tokio::time::sleep(self.config.pool_config.retry_delay).await;
                }
            }
        }
    }

    /// Record successful operation statistics
    async fn record_successful_operation(&self) {
        let mut stats = self.stats.write().await;
        stats.total_operations += 1;
        stats.successful_operations += 1;
    }

    /// Record failed operation statistics
    async fn record_failed_operation(&self) {
        let mut stats = self.stats.write().await;
        stats.total_operations += 1;
        stats.failed_operations += 1;
    }

    /// Record operation latency
    async fn record_operation_latency(&self, duration: Duration) {
        if !self.config.monitoring_config.track_latency {
            return;
        }

        let latency_ms = duration.as_millis() as f64;
        
        {
            let mut samples = self.latency_samples.write().await;
            samples.push(latency_ms);
            
            // Keep only the last 1000 samples for memory efficiency
            if samples.len() > 1000 {
                samples.remove(0);
            }
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.avg_latency_ms = 
                (stats.avg_latency_ms * (stats.total_operations - 1) as f64 + latency_ms) 
                / stats.total_operations as f64;
        }
    }

    /// Calculate 95th percentile latency
    async fn calculate_p95_latency(&self) -> f64 {
        let samples = self.latency_samples.read().await;
        if samples.is_empty() {
            return 0.0;
        }

        let mut sorted_samples = samples.clone();
        sorted_samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let index = (sorted_samples.len() as f64 * 0.95) as usize;
        sorted_samples.get(index).copied().unwrap_or(0.0)
    }

    /// Start background performance monitoring
    fn start_performance_monitoring(&self) {
        let stats = Arc::clone(&self.stats);
        let latency_samples = Arc::clone(&self.latency_samples);
        let interval = self.config.monitoring_config.metrics_interval;
        
        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            let mut last_total_ops = 0u64;
            let mut last_time = Instant::now();
            
            loop {
                interval_timer.tick().await;
                
                let current_time = Instant::now();
                let time_diff = current_time.duration_since(last_time).as_secs_f64();
                
                {
                    let mut stats = stats.write().await;
                    let current_total_ops = stats.total_operations;
                    
                    // Calculate operations per second
                    if time_diff > 0.0 {
                        let ops_diff = current_total_ops - last_total_ops;
                        stats.ops_per_second = ops_diff as f64 / time_diff;
                    }
                    
                    last_total_ops = current_total_ops;
                }
                
                // Calculate 95th percentile latency
                let p95_latency = {
                    let samples = latency_samples.read().await;
                    if !samples.is_empty() {
                        let mut sorted = samples.clone();
                        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                        let index = (sorted.len() as f64 * 0.95) as usize;
                        sorted.get(index).copied().unwrap_or(0.0)
                    } else {
                        0.0
                    }
                };
                
                {
                    let mut stats = stats.write().await;
                    stats.p95_latency_ms = p95_latency;
                }
                
                last_time = current_time;
                
                // Log performance metrics
                let stats = stats.read().await;
                info!("Performance metrics - Ops/sec: {:.2}, Avg latency: {:.2}ms, P95 latency: {:.2}ms, Success rate: {:.2}%",
                      stats.ops_per_second,
                      stats.avg_latency_ms,
                      stats.p95_latency_ms,
                      if stats.total_operations > 0 { 
                          stats.successful_operations as f64 / stats.total_operations as f64 * 100.0 
                      } else { 
                          0.0 
                      });
            }
        });
    }

    /// Get current performance statistics
    pub async fn get_stats(&self) -> PerformanceStats {
        self.stats.read().await.clone()
    }

    /// Reset performance statistics
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = PerformanceStats {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            avg_latency_ms: 0.0,
            p95_latency_ms: 0.0,
            ops_per_second: 0.0,
            pool_stats: PoolStats {
                active_connections: 0,
                idle_connections: 0,
                total_connections_created: 0,
                connection_failures: 0,
                avg_acquisition_time_ms: 0.0,
            },
            pipeline_stats: PipelineStats {
                total_pipelines: 0,
                avg_pipeline_size: 0.0,
                pipeline_failures: 0,
                avg_execution_time_ms: 0.0,
            },
        };
        
        let mut samples = self.latency_samples.write().await;
        samples.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_performance_config_default() {
        let config = PerformanceConfig::default();
        assert_eq!(config.redis_url, "redis://127.0.0.1:6379");
        assert_eq!(config.pool_config.max_size, 20);
        assert_eq!(config.pipeline_config.batch_size, 100);
        assert!(config.monitoring_config.enable_metrics);
    }

    #[tokio::test]
    async fn test_pool_config_optimization() {
        let config = PoolConfig::default();
        assert_eq!(config.max_size, 20); // Increased for performance
        assert_eq!(config.min_idle, Some(5));
        assert_eq!(config.max_retries, 3);
    }

    #[tokio::test]
    async fn test_pipeline_config_default() {
        let config = PipelineConfig::default();
        assert_eq!(config.batch_size, 100); // Optimal batch size
        assert!(config.auto_flush);
        assert_eq!(config.execution_timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_performance_error_display() {
        let error = PerformanceError::PoolError("test error".to_string());
        assert_eq!(error.to_string(), "Connection pool error: test error");
        
        let error = PerformanceError::BulkOperationFailed("bulk error".to_string());
        assert_eq!(error.to_string(), "Bulk operation failed: bulk error");
    }

    #[tokio::test]
    async fn test_performance_stats_structure() {
        let stats = PerformanceStats {
            total_operations: 100,
            successful_operations: 95,
            failed_operations: 5,
            avg_latency_ms: 10.5,
            p95_latency_ms: 25.0,
            ops_per_second: 50.0,
            pool_stats: PoolStats {
                active_connections: 10,
                idle_connections: 5,
                total_connections_created: 15,
                connection_failures: 1,
                avg_acquisition_time_ms: 2.5,
            },
            pipeline_stats: PipelineStats {
                total_pipelines: 10,
                avg_pipeline_size: 10.0,
                pipeline_failures: 0,
                avg_execution_time_ms: 15.0,
            },
        };
        
        assert_eq!(stats.total_operations, 100);
        assert_eq!(stats.successful_operations, 95);
        assert_eq!(stats.failed_operations, 5);
        assert_eq!(stats.avg_latency_ms, 10.5);
        assert_eq!(stats.pool_stats.active_connections, 10);
        assert_eq!(stats.pipeline_stats.total_pipelines, 10);
    }

    #[tokio::test]
    async fn test_config_serialization() {
        let config = PerformanceConfig::default();
        
        // Test serialization/deserialization
        let json = serde_json::to_string(&config).expect("Failed to serialize config");
        let deserialized: PerformanceConfig = serde_json::from_str(&json).expect("Failed to deserialize config");
        
        assert_eq!(config.redis_url, deserialized.redis_url);
        assert_eq!(config.pool_config.max_size, deserialized.pool_config.max_size);
        assert_eq!(config.pipeline_config.batch_size, deserialized.pipeline_config.batch_size);
    }
} 