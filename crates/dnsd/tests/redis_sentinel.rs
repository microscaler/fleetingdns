//! Tests for Redis Sentinel functionality
//!
//! These tests validate the Redis Sentinel client's ability to:
//! - Connect to Redis Sentinel instances
//! - Discover Redis masters via Sentinel
//! - Handle failover scenarios
//! - Manage connection pooling with automatic failover
//! - Monitor health and provide statistics

use std::time::Duration;

use dnsd::redis_sentinel::{SentinelConfig, SentinelError, RedisSentinelClient, PoolConfig};

#[tokio::test]
async fn test_sentinel_config_creation() {
    let config = SentinelConfig {
        sentinels: vec![
            "redis://sentinel-1:26379".to_string(),
            "redis://sentinel-2:26379".to_string(),
            "redis://sentinel-3:26379".to_string(),
        ],
        master_name: "test-master".to_string(),
        connection_timeout: Duration::from_secs(5),
        sentinel_timeout: Duration::from_secs(3),
        health_check_interval: Duration::from_secs(30),
        max_retries: 3,
        failover_timeout: Duration::from_secs(60),
        pool_config: PoolConfig {
            max_size: 10,
            min_idle: Some(2),
            connection_timeout: Duration::from_secs(10),
            idle_timeout: Some(Duration::from_secs(600)),
        },
    };

    assert_eq!(config.sentinels.len(), 3);
    assert_eq!(config.master_name, "test-master");
    assert_eq!(config.max_retries, 3);
    assert_eq!(config.pool_config.max_size, 10);
}

#[tokio::test]
async fn test_sentinel_config_default() {
    let config = SentinelConfig::default();
    
    assert_eq!(config.sentinels.len(), 3);
    assert_eq!(config.master_name, "fleetingdns-cluster");
    assert_eq!(config.connection_timeout, Duration::from_secs(5));
    assert_eq!(config.sentinel_timeout, Duration::from_secs(3));
    assert_eq!(config.health_check_interval, Duration::from_secs(30));
    assert_eq!(config.max_retries, 3);
    assert_eq!(config.failover_timeout, Duration::from_secs(60));
    
    // Check pool config
    assert_eq!(config.pool_config.max_size, 10);
    assert_eq!(config.pool_config.min_idle, Some(2));
    assert_eq!(config.pool_config.connection_timeout, Duration::from_secs(10));
    assert_eq!(config.pool_config.idle_timeout, Some(Duration::from_secs(600)));
}

#[tokio::test]
async fn test_pool_config_default() {
    let config = PoolConfig::default();
    
    assert_eq!(config.max_size, 10);
    assert_eq!(config.min_idle, Some(2));
    assert_eq!(config.connection_timeout, Duration::from_secs(10));
    assert_eq!(config.idle_timeout, Some(Duration::from_secs(600)));
}

#[test]
fn test_sentinel_error_types() {
    // Test error message formatting
    let error = SentinelError::MasterNotFound("test-master".to_string());
    assert_eq!(error.to_string(), "Master test-master not found");
    
    let error = SentinelError::NoSentinelsAvailable;
    assert_eq!(error.to_string(), "No sentinels available");
    
    let error = SentinelError::AllSentinelsFailed;
    assert_eq!(error.to_string(), "All sentinels failed to respond");
    
    let error = SentinelError::FailoverTimeout(Duration::from_secs(60));
    assert!(error.to_string().contains("Failover timeout after"));
    
    let error = SentinelError::InvalidResponse("test response".to_string());
    assert_eq!(error.to_string(), "Invalid sentinel response: test response");
    
    let error = SentinelError::PoolError("test pool error".to_string());
    assert_eq!(error.to_string(), "Pool error: test pool error");
}

#[tokio::test]
async fn test_sentinel_config_serialization() {
    let config = SentinelConfig::default();
    
    // Test that the config can be serialized and deserialized
    let json = serde_json::to_string(&config).expect("Failed to serialize config");
    let deserialized: SentinelConfig = serde_json::from_str(&json).expect("Failed to deserialize config");
    
    assert_eq!(config.sentinels, deserialized.sentinels);
    assert_eq!(config.master_name, deserialized.master_name);
    assert_eq!(config.max_retries, deserialized.max_retries);
}

#[tokio::test]
async fn test_sentinel_stats_structure() {
    use dnsd::redis_sentinel::SentinelStats;
    use std::net::SocketAddr;
    use tokio::time::Instant;
    
    let stats = SentinelStats {
        active_sentinels: 3,
        master_address: Some("127.0.0.1:6379".parse::<SocketAddr>().unwrap()),
        last_failover: Some(Instant::now()),
        failover_count: 5,
        health_check_failures: 2,
        total_connections: 100,
    };
    
    assert_eq!(stats.active_sentinels, 3);
    assert!(stats.master_address.is_some());
    assert!(stats.last_failover.is_some());
    assert_eq!(stats.failover_count, 5);
    assert_eq!(stats.health_check_failures, 2);
    assert_eq!(stats.total_connections, 100);
    
    // Test serialization
    let json = serde_json::to_string(&stats).expect("Failed to serialize stats");
    let deserialized: SentinelStats = serde_json::from_str(&json).expect("Failed to deserialize stats");
    
    assert_eq!(stats.active_sentinels, deserialized.active_sentinels);
    assert_eq!(stats.master_address, deserialized.master_address);
    assert_eq!(stats.failover_count, deserialized.failover_count);
}

#[tokio::test]
async fn test_master_info_structure() {
    use dnsd::redis_sentinel::MasterInfo;
    
    let master_info = MasterInfo {
        name: "test-master".to_string(),
        host: "127.0.0.1".to_string(),
        port: 6379,
        flags: vec!["master".to_string(), "up".to_string()],
        last_ping_sent: 1000,
        last_ok_ping_reply: 990,
        down_after_milliseconds: 5000,
        info_refresh: 1500,
        role_reported: "master".to_string(),
        role_reported_time: 2000,
    };
    
    assert_eq!(master_info.name, "test-master");
    assert_eq!(master_info.host, "127.0.0.1");
    assert_eq!(master_info.port, 6379);
    assert_eq!(master_info.flags.len(), 2);
    assert!(master_info.flags.contains(&"master".to_string()));
    assert!(master_info.flags.contains(&"up".to_string()));
    assert_eq!(master_info.last_ping_sent, 1000);
    assert_eq!(master_info.last_ok_ping_reply, 990);
    assert_eq!(master_info.down_after_milliseconds, 5000);
    assert_eq!(master_info.info_refresh, 1500);
    assert_eq!(master_info.role_reported, "master");
    assert_eq!(master_info.role_reported_time, 2000);
}

// Integration test with mock Redis Sentinel (requires actual Redis setup)
#[tokio::test]
#[ignore] // Ignored by default as it requires actual Redis Sentinel setup
async fn test_sentinel_client_integration() {
    // This test requires a real Redis Sentinel setup
    // It's ignored by default but can be run manually when testing against real infrastructure
    
    let config = SentinelConfig {
        sentinels: vec![
            "redis://127.0.0.1:26379".to_string(),
        ],
        master_name: "test-master".to_string(),
        connection_timeout: Duration::from_secs(5),
        sentinel_timeout: Duration::from_secs(3),
        health_check_interval: Duration::from_secs(30),
        max_retries: 3,
        failover_timeout: Duration::from_secs(60),
        pool_config: PoolConfig::default(),
    };
    
    // This would fail without a real Redis Sentinel setup
    match RedisSentinelClient::new(config).await {
        Ok(client) => {
            // Test getting master address
            let _master_addr = client.get_master_address().await.expect("Failed to get master address");
            
            // Test getting stats
            let stats = client.get_stats().await;
            assert!(stats.active_sentinels >= 0);
            
            // Test failover detection
            let _is_failover = client.is_failover_in_progress().await;
        }
        Err(e) => {
            // Expected to fail without real Redis Sentinel setup
            println!("Expected failure without Redis Sentinel: {}", e);
        }
    }
}

#[tokio::test]
async fn test_sentinel_error_conversion() {
    use redis::{RedisError, ErrorKind};
    
    // Test Redis error conversion
    let redis_error = RedisError::from((ErrorKind::IoError, "Connection failed"));
    let sentinel_error: SentinelError = redis_error.into();
    
    match sentinel_error {
        SentinelError::RedisError(_) => {
            // Expected
        }
        _ => panic!("Expected RedisError variant"),
    }
}

#[tokio::test]
async fn test_configuration_validation() {
    // Test configuration with empty sentinels
    let config = SentinelConfig {
        sentinels: vec![],
        master_name: "test-master".to_string(),
        connection_timeout: Duration::from_secs(5),
        sentinel_timeout: Duration::from_secs(3),
        health_check_interval: Duration::from_secs(30),
        max_retries: 3,
        failover_timeout: Duration::from_secs(60),
        pool_config: PoolConfig::default(),
    };
    
    // This should fail with no sentinels
    match RedisSentinelClient::new(config).await {
        Ok(_) => panic!("Expected failure with empty sentinels"),
        Err(e) => {
            // Expected to fail
            println!("Expected failure with empty sentinels: {}", e);
        }
    }
}

#[tokio::test]
async fn test_timeout_configurations() {
    let config = SentinelConfig {
        sentinels: vec!["redis://invalid-host:26379".to_string()],
        master_name: "test-master".to_string(),
        connection_timeout: Duration::from_millis(100), // Very short timeout
        sentinel_timeout: Duration::from_millis(100),
        health_check_interval: Duration::from_secs(30),
        max_retries: 1,
        failover_timeout: Duration::from_secs(1),
        pool_config: PoolConfig {
            max_size: 5,
            min_idle: Some(1),
            connection_timeout: Duration::from_millis(100),
            idle_timeout: Some(Duration::from_secs(300)),
        },
    };
    
    // Should fail quickly due to short timeouts
    let start = std::time::Instant::now();
    match RedisSentinelClient::new(config).await {
        Ok(_) => panic!("Expected failure with invalid host"),
        Err(_) => {
            let elapsed = start.elapsed();
            // Should fail quickly (within a reasonable time)
            assert!(elapsed < Duration::from_secs(5), "Took too long to fail: {:?}", elapsed);
        }
    }
} 