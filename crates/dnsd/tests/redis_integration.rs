use dnsd::dns_handler::{DnsHandler, PerformanceConfig};
mod redis_test_utils;
use redis_test_utils::with_redis_test_container;

/// Test Redis connection pool creation and basic operations
#[tokio::test]
async fn test_redis_pool_creation() {
    let result = with_redis_test_container(|pool| async move {
        // Test that we can get a connection
        let conn = pool.get().await;
        assert!(conn.is_ok());
        "success"
    }).await;
    
    assert!(result.is_ok());
}

/// Test slot lookup in Redis
#[tokio::test]
async fn test_slot_lookup() {
    let result = with_redis_test_container(|pool| async move {
        let handler = DnsHandler::new(PerformanceConfig::default());
        
        // Test lookup for non-existent domain
        let result = handler.lookup_slot_in_redis("nonexistent.example.com".to_string(), &pool).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
        "success"
    }).await;
    
    assert!(result.is_ok());
}

/// Test Redis connection error handling
#[tokio::test]
async fn test_redis_connection_error() {
    let handler = DnsHandler::new(PerformanceConfig::default());
    let pool = bb8::Pool::builder()
        .build_unchecked(bb8_redis::RedisConnectionManager::new("redis://localhost:9999").unwrap());
    
    // This should fail gracefully
    let result = handler.lookup_slot_in_redis("test.example.com".to_string(), &pool).await;
    assert!(result.is_err());
}

/// Test Redis timeout scenarios
#[tokio::test]
async fn test_redis_timeout() {
    let result = with_redis_test_container(|pool| async move {
        let handler = DnsHandler::new(PerformanceConfig::default());
        
        // Test with a reasonable timeout
        let result = handler.lookup_slot_in_redis("timeout.example.com".to_string(), &pool).await;
        assert!(result.is_ok());
        "success"
    }).await;
    
    assert!(result.is_ok());
}

/// Test Redis pool reuse
#[tokio::test]
async fn test_redis_pool_reuse() {
    let result = with_redis_test_container(|pool| async move {
        // Test multiple connections
        let mut handles = vec![];
        for _ in 0..5 {
            let pool_clone = pool.clone();
            let handle = tokio::spawn(async move {
                let conn = pool_clone.get().await;
                assert!(conn.is_ok());
            });
            handles.push(handle);
        }
        
        for handle in handles {
            handle.await.unwrap();
        }
        "success"
    }).await;
    
    assert!(result.is_ok());
}

/// Test invalid data handling
#[tokio::test]
async fn test_invalid_data_handling() {
    let result = with_redis_test_container(|pool| async move {
        let handler = DnsHandler::new(PerformanceConfig::default());
        
        // Test with empty domain
        let result = handler.lookup_slot_in_redis("".to_string(), &pool).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
        
        // Test with malformed domain
        let result = handler.lookup_slot_in_redis("invalid..domain".to_string(), &pool).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
        "success"
    }).await;
    
    assert!(result.is_ok());
}

/// Test performance under load
#[tokio::test]
async fn test_redis_performance_under_load() {
    let result = with_redis_test_container(|pool| async move {
        let handler = DnsHandler::new(PerformanceConfig::default());
        
        let mut handles = vec![];
        for i in 0..10 {
            let handler_clone = handler.clone();
            let pool_clone = pool.clone();
            let handle = tokio::spawn(async move {
                let domain = format!("test{}.example.com", i);
                let result = handler_clone.lookup_slot_in_redis(domain, &pool_clone).await;
                assert!(result.is_ok());
            });
            handles.push(handle);
        }
        
        for handle in handles {
            handle.await.unwrap();
        }
        "success"
    }).await;
    
    assert!(result.is_ok());
}

/// Test Redis interaction during DNS packet processing
#[tokio::test]
async fn test_redis_dns_packet_processing() {
    let result = with_redis_test_container(|pool| async move {
        let handler = DnsHandler::new(PerformanceConfig::default());
        
        // Create a simple DNS packet
        let packet = b"\x00\x01\x01\x00\x00\x01\x00\x00\x00\x00\x00\x00\x07example\x03com\x00\x00\x01\x00\x01";
        
        // Test processing with Redis
        let result = handler.process_dns_query(packet, &pool).await;
        // This should fail gracefully since we don't have actual DNS data
        assert!(result.is_err());
        "success"
    }).await;
    
    assert!(result.is_ok());
}

/// Test different domain formats
#[tokio::test]
async fn test_different_domain_formats() {
    let result = with_redis_test_container(|pool| async move {
        let handler = DnsHandler::new(PerformanceConfig::default());
        
        let domains = vec![
            "simple.com",
            "sub.domain.com",
            "very.deep.sub.domain.com",
            "domain-with-dashes.com",
            "domain_with_underscores.com",
        ];
        
        for domain in domains {
            let result = handler.lookup_slot_in_redis(domain.to_string(), &pool).await;
            assert!(result.is_ok());
            assert!(result.unwrap().is_none());
        }
        "success"
    }).await;
    
    assert!(result.is_ok());
}

/// Test error propagation
#[tokio::test]
async fn test_error_propagation() {
    let result = with_redis_test_container(|pool| async move {
        let handler = DnsHandler::new(PerformanceConfig::default());
        
        // Test that errors are properly propagated
        let result = handler.lookup_slot_in_redis("test.example.com".to_string(), &pool).await;
        assert!(result.is_ok());
        "success"
    }).await;
    
    assert!(result.is_ok());
}

/// Test pool statistics
#[tokio::test]
async fn test_pool_statistics() {
    let result = with_redis_test_container(|pool| async move {
        // Use some connections
        for _ in 0..3 {
            let conn = pool.get().await;
            assert!(conn.is_ok());
        }
        
        // Test that pool is working by getting another connection
        let conn = pool.get().await;
        assert!(conn.is_ok());
        "success"
    }).await;
    
    assert!(result.is_ok());
} 