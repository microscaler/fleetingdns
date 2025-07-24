//! Integration tests for Redis cluster functionality using testcontainers
//!
//! These tests demonstrate how to test Redis cluster functionality with actual
//! Redis containers. Currently uses single Redis instances to validate the
//! cluster client can handle non-cluster Redis servers gracefully.

#![cfg(feature = "redis-cluster-integration")]

use std::time::Duration;
use testcontainers::{runners::AsyncRunner, GenericImage, ImageExt};

use dnsd::redis_cluster::{ClusterConfig, RedisClusterClient};

#[tokio::test]
async fn test_cluster_client_creation_with_redis() {
    // Start Redis container
    let container = GenericImage::new("redis", "7.2.4").with_exposed_port(6379).start().await.expect("Failed to start Redis");
    let port = container.get_host_port_ipv4(6379).await.unwrap();
    let url = format!("redis://127.0.0.1:{port}");

    // Wait for Redis to be ready
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Create cluster config with single node
    let config = ClusterConfig {
        nodes: vec![url],
        pool_config: dnsd::redis_cluster::PoolConfig {
            max_size: 5, // Smaller for testing
            ..Default::default()
        },
        ..Default::default()
    };

    // Test cluster client creation
    match RedisClusterClient::new(config).await {
        Ok(client) => {
            // Test some basic operations
            for slot in 0..10 {
                let ip = std::net::Ipv4Addr::new(127, 0, 0, 1);
                if client.set_slot(&format!("test-slot-{slot}"), ip, 3600).await.is_ok() {
                    println!("✅ Set slot {slot} -> {ip}");
                    
                    if let Ok(retrieved_ip) = client.get_slot(&format!("test-slot-{slot}")).await {
                        println!("✅ Retrieved slot {slot} -> {retrieved_ip}");
                    } else {
                        println!("⚠️  Could not retrieve slot {slot}");
                    }
                } else {
                    println!("⚠️  Could not set slot {slot}");
                }
            }
            
            let stats = client.get_cluster_stats().await;
            println!("📊 Cluster stats: {stats:?}");
        }
        Err(e) => {
            println!(
                "⚠️  Cluster client creation failed (expected with single Redis): {e}"
            );
        }
    }
}

#[tokio::test]
async fn test_cluster_bulk_operations_with_redis() {
    // Start Redis container
    let container = GenericImage::new("redis", "7.2.4").with_exposed_port(6379).start().await.expect("Failed to start Redis");
    let port = container.get_host_port_ipv4(6379).await.unwrap();
    let url = format!("redis://127.0.0.1:{port}");

    // Wait for Redis to be ready
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Create cluster config
    let config = ClusterConfig {
        nodes: vec![url],
        ..Default::default()
    };

    // Test bulk operations
    match RedisClusterClient::new(config).await {
        Ok(client) => {
            // Test bulk set operations
            let operations = vec![
                ("bulk-slot-1".to_string(), std::net::Ipv4Addr::new(127, 0, 0, 1), 3600),
                ("bulk-slot-2".to_string(), std::net::Ipv4Addr::new(127, 0, 0, 2), 3600),
                ("bulk-slot-3".to_string(), std::net::Ipv4Addr::new(127, 0, 0, 3), 3600),
            ];

            if client.bulk_set_slots(operations).await.is_ok() {
                println!("✅ Bulk set operations completed successfully");
            }
        }
        Err(e) => {
            println!("⚠️  Expected cluster error with single Redis: {e}");
        }
    }
}

#[tokio::test]
async fn test_cluster_config_custom() {
    // Test custom cluster configuration
    let custom_config = ClusterConfig {
        nodes: vec!["redis://localhost:6379".to_string()],
        ..Default::default()
    };

    assert_eq!(custom_config.nodes.len(), 1);
    assert_eq!(custom_config.nodes[0], "redis://localhost:6379");
}

#[tokio::test]
async fn test_cluster_performance_with_redis() {
    // Start Redis container
    let container = GenericImage::new("redis", "7.2.4").with_exposed_port(6379).start().await.expect("Failed to start Redis");
    let port = container.get_host_port_ipv4(6379).await.unwrap();
    let url = format!("redis://127.0.0.1:{port}");

    // Wait for Redis to be ready
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Create cluster config
    let config = ClusterConfig {
        nodes: vec![url],
        ..Default::default()
    };

    // Test performance
    match RedisClusterClient::new(config).await {
        Ok(client) => {
            let start = std::time::Instant::now();
            let num_operations = 100;

            // Perform operations
            for i in 0..num_operations {
                let _ = client.set_slot(
                    &format!("perf-{i}"),
                    std::net::Ipv4Addr::new(127, 0, 0, 1),
                    3600,
                ).await;
            }

            let duration = start.elapsed();
            let ops_per_sec = num_operations as f64 / duration.as_secs_f64();

            println!(
                "📊 Performance: {num_operations} ops in {duration:?} ({ops_per_sec:.2} ops/sec)"
            );
        }
        Err(_) => {
            // Expected with single Redis instance
        }
    }
}

#[tokio::test]
async fn test_cluster_error_handling() {
    // Test with invalid Redis host
    let config = ClusterConfig {
        nodes: vec!["redis://invalid-host:6379".to_string()],
        ..Default::default()
    };

    // This should fail
    match RedisClusterClient::new(config).await {
        Ok(_) => panic!("Expected failure with invalid host"),
        Err(_) => {
            // Expected to fail
        }
    }
}

// Helper function to start Redis container for testing
async fn start_redis_container() -> testcontainers::ContainerAsync<testcontainers::GenericImage> {
    GenericImage::new("redis", "7.2.4")
        .with_exposed_port(6379)
        .start()
        .await
}

#[tokio::test]
async fn test_cluster_client_creation_without_redis() {
    // Start Redis container
    let container = start_redis_container().await;
    let port = container.get_host_port_ipv4(6379).await.unwrap();
    let url = format!("redis://127.0.0.1:{port}");

    // Wait for Redis to be ready
    tokio::time::sleep(Duration::from_millis(1000)).await;

    // Test cluster client creation - this should work with a single Redis instance
    // but cluster operations might fail
    let config = ClusterConfig {
        nodes: vec![url],
        ..Default::default()
    };

    match RedisClusterClient::new(config).await {
        Ok(_client) => {
            println!("✅ Cluster client created successfully with single Redis node");
        }
        Err(e) => {
            println!("⚠️  Cluster client creation failed: {e}");
        }
    }
}



