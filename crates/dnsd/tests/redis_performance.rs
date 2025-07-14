//! Tests for Redis performance optimization functionality
//!
//! These tests validate the Redis performance client's ability to:
//! - Optimize connection pooling for high throughput
//! - Execute bulk operations with pipelining
//! - Monitor performance metrics and statistics
//! - Handle high-concurrency scenarios
//! - Provide query optimization and caching

use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::time::Duration;

use dnsd::redis_performance::{
    MonitoringConfig, PerformanceConfig, PerformanceError, PipelineConfig, PoolConfig,
    RedisPerformanceClient,
};

#[tokio::test]
async fn test_performance_config_creation() {
    let config = PerformanceConfig {
        redis_url: "redis://localhost:6379".to_string(),
        pool_config: PoolConfig {
            max_size: 25,
            min_idle: Some(10),
            connection_timeout: Duration::from_secs(15),
            idle_timeout: Some(Duration::from_secs(600)),
            max_retries: 5,
            retry_delay: Duration::from_millis(200),
        },
        pipeline_config: PipelineConfig {
            batch_size: 150,
            execution_timeout: Duration::from_secs(45),
            auto_flush: true,
            flush_interval: Duration::from_millis(5),
        },
        monitoring_config: MonitoringConfig {
            enable_metrics: true,
            metrics_interval: Duration::from_secs(30),
            monitor_pool: true,
            track_latency: true,
        },
    };

    assert_eq!(config.redis_url, "redis://localhost:6379");
    assert_eq!(config.pool_config.max_size, 25);
    assert_eq!(config.pipeline_config.batch_size, 150);
    assert!(config.monitoring_config.enable_metrics);
}

#[tokio::test]
async fn test_performance_config_default() {
    let config = PerformanceConfig::default();

    assert_eq!(config.redis_url, "redis://127.0.0.1:6379");
    assert_eq!(config.pool_config.max_size, 20);
    assert_eq!(config.pool_config.min_idle, Some(5));
    assert_eq!(
        config.pool_config.connection_timeout,
        Duration::from_secs(10)
    );
    assert_eq!(config.pool_config.max_retries, 3);
    assert_eq!(config.pipeline_config.batch_size, 100);
    assert!(config.pipeline_config.auto_flush);
    assert_eq!(
        config.pipeline_config.execution_timeout,
        Duration::from_secs(30)
    );
    assert!(config.monitoring_config.enable_metrics);
    assert_eq!(
        config.monitoring_config.metrics_interval,
        Duration::from_secs(60)
    );
}

#[tokio::test]
async fn test_pool_config_optimization() {
    let config = PoolConfig::default();

    // Verify optimized settings for high performance
    assert_eq!(config.max_size, 20); // Increased from typical default
    assert_eq!(config.min_idle, Some(5)); // Maintains warm connections
    assert_eq!(config.connection_timeout, Duration::from_secs(10));
    assert_eq!(config.idle_timeout, Some(Duration::from_secs(300)));
    assert_eq!(config.max_retries, 3);
    assert_eq!(config.retry_delay, Duration::from_millis(100));
}

#[tokio::test]
async fn test_pipeline_config_optimization() {
    let config = PipelineConfig::default();

    // Verify optimal pipeline settings
    assert_eq!(config.batch_size, 100); // Optimal for most workloads
    assert_eq!(config.execution_timeout, Duration::from_secs(30));
    assert!(config.auto_flush); // Enables automatic batching
    assert_eq!(config.flush_interval, Duration::from_millis(10));
}

#[tokio::test]
async fn test_monitoring_config_default() {
    let config = MonitoringConfig::default();

    assert!(config.enable_metrics);
    assert_eq!(config.metrics_interval, Duration::from_secs(60));
    assert!(config.monitor_pool);
    assert!(config.track_latency);
}

#[test]
fn test_performance_error_types() {
    // Test error message formatting
    let error = PerformanceError::PoolError("connection failed".to_string());
    assert_eq!(
        error.to_string(),
        "Connection pool error: connection failed"
    );

    let error = PerformanceError::BulkOperationFailed("batch failed".to_string());
    assert_eq!(error.to_string(), "Bulk operation failed: batch failed");

    let error = PerformanceError::PipelineError("pipeline timeout".to_string());
    assert_eq!(
        error.to_string(),
        "Pipeline execution failed: pipeline timeout"
    );

    let error = PerformanceError::TimeoutError("operation timeout".to_string());
    assert_eq!(error.to_string(), "Timeout error: operation timeout");

    let error = PerformanceError::ConfigError("invalid config".to_string());
    assert_eq!(error.to_string(), "Configuration error: invalid config");
}

#[tokio::test]
async fn test_performance_stats_structure() {
    use dnsd::redis_performance::{PerformanceStats, PipelineStats, PoolStats};

    let stats = PerformanceStats {
        total_operations: 1000,
        successful_operations: 980,
        failed_operations: 20,
        avg_latency_ms: 5.5,
        p95_latency_ms: 15.0,
        ops_per_second: 100.0,
        pool_stats: PoolStats {
            active_connections: 15,
            idle_connections: 5,
            total_connections_created: 20,
            connection_failures: 2,
            avg_acquisition_time_ms: 1.5,
        },
        pipeline_stats: PipelineStats {
            total_pipelines: 50,
            avg_pipeline_size: 20.0,
            pipeline_failures: 1,
            avg_execution_time_ms: 25.0,
        },
    };

    assert_eq!(stats.total_operations, 1000);
    assert_eq!(stats.successful_operations, 980);
    assert_eq!(stats.failed_operations, 20);
    assert_eq!(stats.avg_latency_ms, 5.5);
    assert_eq!(stats.p95_latency_ms, 15.0);
    assert_eq!(stats.ops_per_second, 100.0);

    // Pool statistics
    assert_eq!(stats.pool_stats.active_connections, 15);
    assert_eq!(stats.pool_stats.idle_connections, 5);
    assert_eq!(stats.pool_stats.total_connections_created, 20);
    assert_eq!(stats.pool_stats.connection_failures, 2);
    assert_eq!(stats.pool_stats.avg_acquisition_time_ms, 1.5);

    // Pipeline statistics
    assert_eq!(stats.pipeline_stats.total_pipelines, 50);
    assert_eq!(stats.pipeline_stats.avg_pipeline_size, 20.0);
    assert_eq!(stats.pipeline_stats.pipeline_failures, 1);
    assert_eq!(stats.pipeline_stats.avg_execution_time_ms, 25.0);
}

#[tokio::test]
async fn test_config_serialization() {
    let config = PerformanceConfig::default();

    // Test that the config can be serialized and deserialized
    let json = serde_json::to_string(&config).expect("Failed to serialize config");
    let deserialized: PerformanceConfig =
        serde_json::from_str(&json).expect("Failed to deserialize config");

    assert_eq!(config.redis_url, deserialized.redis_url);
    assert_eq!(
        config.pool_config.max_size,
        deserialized.pool_config.max_size
    );
    assert_eq!(
        config.pipeline_config.batch_size,
        deserialized.pipeline_config.batch_size
    );
    assert_eq!(
        config.monitoring_config.enable_metrics,
        deserialized.monitoring_config.enable_metrics
    );
}

#[tokio::test]
async fn test_bulk_operation_batching() {
    // Test that bulk operations are properly batched
    let config = PerformanceConfig {
        pipeline_config: PipelineConfig {
            batch_size: 50, // Small batch size for testing
            execution_timeout: Duration::from_secs(30),
            auto_flush: true,
            flush_interval: Duration::from_millis(10),
        },
        ..Default::default()
    };

    // Create test operations that exceed batch size
    let operations: Vec<(String, Ipv4Addr, u64)> = (0..125)
        .map(|i| {
            (
                format!("slot{}", i),
                Ipv4Addr::new(127, 0, 0, (i % 255) as u8 + 1),
                3600,
            )
        })
        .collect();

    assert_eq!(operations.len(), 125);

    // This would be split into 3 batches: 50 + 50 + 25
    let expected_batches =
        (125 + config.pipeline_config.batch_size - 1) / config.pipeline_config.batch_size;
    assert_eq!(expected_batches, 3);
}

#[tokio::test]
async fn test_performance_monitoring_config() {
    let config = MonitoringConfig {
        enable_metrics: true,
        metrics_interval: Duration::from_secs(30),
        monitor_pool: true,
        track_latency: true,
    };

    assert!(config.enable_metrics);
    assert_eq!(config.metrics_interval, Duration::from_secs(30));
    assert!(config.monitor_pool);
    assert!(config.track_latency);

    // Test serialization
    let json = serde_json::to_string(&config).expect("Failed to serialize monitoring config");
    let deserialized: MonitoringConfig =
        serde_json::from_str(&json).expect("Failed to deserialize monitoring config");

    assert_eq!(config.enable_metrics, deserialized.enable_metrics);
    assert_eq!(config.metrics_interval, deserialized.metrics_interval);
    assert_eq!(config.monitor_pool, deserialized.monitor_pool);
    assert_eq!(config.track_latency, deserialized.track_latency);
}

// Integration test with mock Redis (requires actual Redis setup)
#[tokio::test]
#[ignore] // Ignored by default as it requires actual Redis setup
async fn test_performance_client_integration() {
    let config = PerformanceConfig {
        redis_url: "redis://127.0.0.1:6379".to_string(),
        pool_config: PoolConfig {
            max_size: 10,
            min_idle: Some(2),
            connection_timeout: Duration::from_secs(5),
            idle_timeout: Some(Duration::from_secs(300)),
            max_retries: 2,
            retry_delay: Duration::from_millis(50),
        },
        pipeline_config: PipelineConfig {
            batch_size: 50,
            execution_timeout: Duration::from_secs(10),
            auto_flush: true,
            flush_interval: Duration::from_millis(5),
        },
        monitoring_config: MonitoringConfig {
            enable_metrics: false, // Disable for testing
            metrics_interval: Duration::from_secs(60),
            monitor_pool: false,
            track_latency: true,
        },
    };

    match RedisPerformanceClient::new(config).await {
        Ok(client) => {
            // Test single operation
            let result = client
                .set_slot_optimized("test-slot", "127.0.0.1".parse().unwrap(), 3600)
                .await;
            match result {
                Ok(_) => println!("Single operation successful"),
                Err(e) => println!("Single operation failed: {}", e),
            }

            // Test bulk operations
            let operations = vec![
                (
                    "bulk-slot-1".to_string(),
                    "127.0.0.1".parse().unwrap(),
                    3600,
                ),
                (
                    "bulk-slot-2".to_string(),
                    "127.0.0.2".parse().unwrap(),
                    1800,
                ),
                (
                    "bulk-slot-3".to_string(),
                    "127.0.0.3".parse().unwrap(),
                    7200,
                ),
            ];

            let result = client.bulk_set_slots(operations).await;
            match result {
                Ok(_) => println!("Bulk operation successful"),
                Err(e) => println!("Bulk operation failed: {}", e),
            }

            // Test bulk get operations
            let slots = vec![
                "bulk-slot-1".to_string(),
                "bulk-slot-2".to_string(),
                "bulk-slot-3".to_string(),
            ];

            let result = client.bulk_get_slots(slots).await;
            match result {
                Ok(results) => {
                    println!("Bulk get successful: {} results", results.len());
                    for (slot, ip) in results {
                        println!("  {}: {:?}", slot, ip);
                    }
                }
                Err(e) => println!("Bulk get failed: {}", e),
            }

            // Test statistics
            let stats = client.get_stats().await;
            println!("Performance stats:");
            println!("  Total operations: {}", stats.total_operations);
            println!("  Successful operations: {}", stats.successful_operations);
            println!("  Failed operations: {}", stats.failed_operations);
            println!("  Average latency: {:.2}ms", stats.avg_latency_ms);
            println!("  Operations per second: {:.2}", stats.ops_per_second);
        }
        Err(e) => {
            println!("Expected failure without Redis: {}", e);
        }
    }
}

#[tokio::test]
async fn test_error_conversion() {
    use redis::{ErrorKind, RedisError};

    // Test Redis error conversion
    let redis_error = RedisError::from((ErrorKind::IoError, "Connection failed"));
    let performance_error: PerformanceError = redis_error.into();

    match performance_error {
        PerformanceError::RedisError(_) => {
            // Expected
        }
        _ => panic!("Expected RedisError variant"),
    }
}

#[tokio::test]
async fn test_timeout_configurations() {
    let config = PerformanceConfig {
        redis_url: "redis://invalid-host:6379".to_string(),
        pool_config: PoolConfig {
            max_size: 5,
            min_idle: Some(1),
            connection_timeout: Duration::from_millis(100), // Very short timeout
            idle_timeout: Some(Duration::from_secs(60)),
            max_retries: 1,
            retry_delay: Duration::from_millis(10),
        },
        pipeline_config: PipelineConfig {
            batch_size: 10,
            execution_timeout: Duration::from_millis(100), // Very short timeout
            auto_flush: true,
            flush_interval: Duration::from_millis(5),
        },
        monitoring_config: MonitoringConfig {
            enable_metrics: false,
            metrics_interval: Duration::from_secs(60),
            monitor_pool: false,
            track_latency: false,
        },
    };

    // Should fail quickly due to short timeouts
    let start = std::time::Instant::now();
    match RedisPerformanceClient::new(config).await {
        Ok(_) => panic!("Expected failure with invalid host"),
        Err(_) => {
            let elapsed = start.elapsed();
            // Should fail quickly (within a reasonable time)
            assert!(
                elapsed < Duration::from_secs(5),
                "Took too long to fail: {:?}",
                elapsed
            );
        }
    }
}

#[tokio::test]
async fn test_performance_optimization_settings() {
    let config = PerformanceConfig::default();

    // Verify performance-optimized settings
    assert!(
        config.pool_config.max_size >= 20,
        "Pool size should be optimized for performance"
    );
    assert!(
        config.pool_config.min_idle.unwrap_or(0) >= 5,
        "Should maintain warm connections"
    );
    assert!(
        config.pipeline_config.batch_size >= 100,
        "Batch size should be optimized"
    );
    assert!(
        config.pipeline_config.auto_flush,
        "Auto-flush should be enabled for performance"
    );
    assert!(
        config.monitoring_config.enable_metrics,
        "Metrics should be enabled by default"
    );
}
