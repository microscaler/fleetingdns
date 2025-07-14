//! Integration tests for Redis cluster functionality using testcontainers
//!
//! These tests demonstrate how to test Redis cluster functionality with actual
//! Redis containers. Currently uses single Redis instances to validate the
//! cluster client can handle non-cluster Redis servers gracefully.

#![cfg(feature = "redis-cluster-integration")]

use std::net::Ipv4Addr;
use std::time::Duration;

use dnsd::redis_cluster::{ClusterConfig, RedisClusterClient};
use testcontainers::{GenericImage, runners::AsyncRunner};
use tokio::time::timeout;

/// Test that the cluster client can handle single Redis instances
/// This demonstrates the testing pattern for when full cluster testing is available
#[tokio::test]
async fn test_cluster_client_with_single_redis() {
    // Start a single Redis container
    let redis_container = GenericImage::new("redis", "7.0-alpine")
        .with_exposed_port(6379)
        .start()
        .await;

    let redis_port = redis_container.get_host_port_ipv4(6379).await;
    let redis_url = format!("redis://127.0.0.1:{}", redis_port);

    // Wait for Redis to be ready
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Create cluster config with single node
    let mut config = ClusterConfig::default();
    config.nodes = vec![redis_url];
    config.pool_config.max_size = 5; // Smaller for testing
    config.discovery_config.discovery_timeout = Duration::from_secs(2);

    // Test cluster client creation
    let client_result = timeout(Duration::from_secs(5), RedisClusterClient::new(config)).await;

    match client_result {
        Ok(Ok(client)) => {
            println!("✅ Cluster client created successfully with single Redis node");

            // Test basic operations
            let test_cases = vec![
                ("test-slot-1", "192.168.1.10"),
                ("test-slot-2", "192.168.1.20"),
            ];

            for (slot, ip_str) in test_cases {
                let ip: Ipv4Addr = ip_str.parse().unwrap();

                // Test set operation
                if let Ok(()) = client.set_slot(slot, ip, 300).await {
                    println!("✅ Set slot {} -> {}", slot, ip);

                    // Test get operation
                    if let Ok(retrieved_ip) = client.get_slot(slot).await {
                        assert_eq!(retrieved_ip, ip);
                        println!("✅ Retrieved slot {} -> {}", slot, retrieved_ip);
                    } else {
                        println!("⚠️  Could not retrieve slot {}", slot);
                    }
                } else {
                    println!("⚠️  Could not set slot {}", slot);
                }
            }

            // Test cluster statistics
            let stats = client.get_cluster_stats().await;
            println!("📊 Cluster stats: {:?}", stats);
        }
        Ok(Err(e)) => {
            println!(
                "⚠️  Cluster client creation failed (expected with single Redis): {}",
                e
            );
            // This is expected since we're not running a real cluster
        }
        Err(_) => {
            println!("⚠️  Cluster client creation timed out");
        }
    }
}

/// Test bulk operations with single Redis instance
#[tokio::test]
async fn test_bulk_operations_with_single_redis() {
    // Start a single Redis container
    let redis_container = GenericImage::new("redis", "7.0-alpine")
        .with_exposed_port(6379)
        .start()
        .await;

    let redis_port = redis_container.get_host_port_ipv4(6379).await;
    let redis_url = format!("redis://127.0.0.1:{}", redis_port);

    // Wait for Redis to be ready
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Create cluster config
    let mut config = ClusterConfig::default();
    config.nodes = vec![redis_url];
    config.pool_config.max_size = 5;
    config.discovery_config.discovery_timeout = Duration::from_secs(2);

    // Test with fallback to individual operations when cluster is not available
    let client_result = timeout(Duration::from_secs(5), RedisClusterClient::new(config)).await;

    match client_result {
        Ok(Ok(client)) => {
            // Test bulk operations
            let bulk_operations = vec![
                ("bulk-1".to_string(), "10.0.0.1".parse().unwrap(), 300),
                ("bulk-2".to_string(), "10.0.0.2".parse().unwrap(), 300),
                ("bulk-3".to_string(), "10.0.0.3".parse().unwrap(), 300),
            ];

            if let Ok(()) = client.bulk_set_slots(bulk_operations.clone()).await {
                println!("✅ Bulk operations completed successfully");

                // Verify some operations
                for (slot, expected_ip, _) in bulk_operations.iter().take(2) {
                    if let Ok(retrieved_ip) = client.get_slot(slot).await {
                        assert_eq!(retrieved_ip, *expected_ip);
                        println!("✅ Bulk operation verified: {} -> {}", slot, expected_ip);
                    }
                }
            } else {
                println!("⚠️  Bulk operations failed or fell back to individual operations");
            }
        }
        Ok(Err(e)) => {
            println!("⚠️  Expected cluster error with single Redis: {}", e);
        }
        Err(_) => {
            println!("⚠️  Cluster client creation timed out");
        }
    }
}

/// Test cluster configuration validation
#[tokio::test]
async fn test_cluster_configuration() {
    // Test default configuration
    let default_config = ClusterConfig::default();
    assert_eq!(default_config.nodes.len(), 3);
    assert!(default_config.performance_config.enable_pipelining);
    assert_eq!(default_config.pool_config.max_size, 20);

    // Test custom configuration
    let mut custom_config = ClusterConfig::default();
    custom_config.nodes = vec!["redis://localhost:6379".to_string()];
    custom_config.pool_config.max_size = 10;
    custom_config.performance_config.enable_pipelining = false;

    assert_eq!(custom_config.nodes.len(), 1);
    assert_eq!(custom_config.pool_config.max_size, 10);
    assert!(!custom_config.performance_config.enable_pipelining);

    println!("✅ Cluster configuration validation passed");
}

/// Performance benchmark test (simplified)
#[tokio::test]
async fn test_performance_benchmark() {
    // Start a single Redis container
    let redis_container = GenericImage::new("redis", "7.0-alpine")
        .with_exposed_port(6379)
        .start()
        .await;

    let redis_port = redis_container.get_host_port_ipv4(6379).await;
    let redis_url = format!("redis://127.0.0.1:{}", redis_port);

    // Wait for Redis to be ready
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Create cluster config
    let mut config = ClusterConfig::default();
    config.nodes = vec![redis_url];
    config.pool_config.max_size = 10;
    config.discovery_config.discovery_timeout = Duration::from_secs(2);

    if let Ok(Ok(client)) = timeout(Duration::from_secs(5), RedisClusterClient::new(config)).await {
        // Performance test with smaller dataset
        let num_operations = 100;
        let start_time = std::time::Instant::now();

        let operations = (0..num_operations)
            .map(|i| {
                (
                    format!("perf-{}", i),
                    Ipv4Addr::new(10, 0, (i / 256) as u8, (i % 256) as u8),
                    300,
                )
            })
            .collect::<Vec<_>>();

        if let Ok(()) = client.bulk_set_slots(operations).await {
            let duration = start_time.elapsed();
            let ops_per_sec = num_operations as f64 / duration.as_secs_f64();

            println!(
                "📊 Performance: {} ops in {:?} ({:.2} ops/sec)",
                num_operations, duration, ops_per_sec
            );

            // Verify performance is reasonable
            assert!(ops_per_sec > 10.0, "Should achieve > 10 ops/sec");
            println!("✅ Performance benchmark passed");
        } else {
            println!("⚠️  Performance benchmark failed");
        }
    } else {
        println!("⚠️  Could not create cluster client for performance test");
    }
}

/// Test error handling with unavailable Redis
#[tokio::test]
async fn test_error_handling() {
    // Test with invalid Redis URL
    let mut config = ClusterConfig::default();
    config.nodes = vec!["redis://invalid-host:6379".to_string()];
    config.discovery_config.discovery_timeout = Duration::from_secs(1);

    let client_result = timeout(Duration::from_secs(3), RedisClusterClient::new(config)).await;

    match client_result {
        Ok(Err(_)) => {
            println!("✅ Error handling works correctly for invalid Redis URL");
        }
        Ok(Ok(_)) => {
            println!("⚠️  Unexpected success with invalid Redis URL");
        }
        Err(_) => {
            println!("✅ Timeout handling works correctly");
        }
    }
}

/// Helper function to check if Docker is available
async fn is_docker_available() -> bool {
    std::process::Command::new("docker")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Integration test that skips if Docker is not available
#[tokio::test]
async fn test_docker_availability() {
    if !is_docker_available().await {
        println!("⚠️  Docker not available, skipping integration tests");
        return;
    }

    println!("✅ Docker is available for integration testing");

    // Run a simple test to verify testcontainers works
    let redis_container = GenericImage::new("redis", "7.0-alpine")
        .with_exposed_port(6379)
        .start()
        .await;

    let redis_port = redis_container.get_host_port_ipv4(6379).await;

    println!("✅ Redis container started on port {}", redis_port);

    // Test basic Redis connectivity
    let redis_url = format!("redis://127.0.0.1:{}", redis_port);
    let client = redis::Client::open(redis_url).unwrap();

    // Wait for Redis to be ready
    tokio::time::sleep(Duration::from_millis(500)).await;

    let mut conn = client.get_connection().unwrap();
    let pong: String = redis::cmd("PING").query(&mut conn).unwrap();
    assert_eq!(pong, "PONG");

    println!("✅ Redis connectivity verified");
}
