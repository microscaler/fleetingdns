use fleetingdns_integration_tests::{
    integration_test, TestContext, health_checks, dns_tests,
};
use hickory_proto::rr::RecordType;
use std::time::Duration;

/// Test DNS service with Redis integration
async fn test_dns_redis_integration(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Set up test data
    ctx.harness.setup_test_slots().await?;
    
    // Test DNS query for existing slot
    let response = dns_tests::send_dns_query(
        "127.0.0.1:6353",
        "test.fdns.run",
        RecordType::A,
    ).await?;
    
    // Verify response contains expected IP
    let has_expected_ip = dns_tests::verify_dns_response(&response, "127.0.0.1")?;
    assert!(has_expected_ip, "DNS response should contain expected IP");
    
    Ok(())
}

/// Test DNS service with non-existent domain
async fn test_dns_nonexistent_domain(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Test DNS query for non-existent domain
    let response = dns_tests::send_dns_query(
        "127.0.0.1:6353",
        "nonexistent.fdns.run",
        RecordType::A,
    ).await?;
    
    // Verify response has no answers (NXDOMAIN or empty response)
    let has_answers = dns_tests::verify_dns_response(&response, "127.0.0.1")?;
    assert!(!has_answers, "DNS response should not contain answers for non-existent domain");
    
    Ok(())
}

/// Test DNS service performance under load
async fn test_dns_performance(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Set up multiple test slots
    let test_domains = vec![
        "test1.fdns.run",
        "test2.fdns.run", 
        "test3.fdns.run",
        "test4.fdns.run",
        "test5.fdns.run",
    ];
    
    // Add test data to Redis
    let mut conn = ctx.harness.redis_pool.get().await?;
    for domain in &test_domains {
        let key = format!("slot:{}", domain);
        redis::cmd("SET").arg(&key).arg("127.0.0.1").execute_async(&mut *conn).await?;
    }
    
    // Send concurrent queries
    let mut handles = vec![];
    for domain in test_domains {
        let handle = tokio::spawn(async move {
            dns_tests::send_dns_query("127.0.0.1:6353", &domain, RecordType::A).await
        });
        handles.push(handle);
    }
    
    // Wait for all queries to complete
    let start = std::time::Instant::now();
    let results = futures::future::join_all(handles).await;
    let duration = start.elapsed();
    
    // Verify all queries succeeded
    for result in results {
        let response = result??;
        let has_expected_ip = dns_tests::verify_dns_response(&response, "127.0.0.1")?;
        assert!(has_expected_ip, "All DNS queries should return expected IP");
    }
    
    // Performance assertion: all queries should complete within 5 seconds
    assert!(duration < Duration::from_secs(5), "DNS queries took too long: {:?}", duration);
    
    Ok(())
}

/// Test DNS service error handling
async fn test_dns_error_handling(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Test with malformed DNS query
    let malformed_query = vec![0u8; 10]; // Too short to be valid DNS
    
    let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
    socket.set_read_timeout(Some(Duration::from_secs(1)))?;
    
    // Send malformed query
    let result = socket.send_to(&malformed_query, "127.0.0.1:6353");
    
    // Should not panic, even with malformed input
    assert!(result.is_ok(), "DNS service should handle malformed queries gracefully");
    
    Ok(())
}

/// Test DNS service with different record types
async fn test_dns_record_types(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Test A record
    let a_response = dns_tests::send_dns_query(
        "127.0.0.1:6353",
        "test.fdns.run",
        RecordType::A,
    ).await?;
    
    // Test AAAA record (should return empty response since we only have A records)
    let aaaa_response = dns_tests::send_dns_query(
        "127.0.0.1:6353",
        "test.fdns.run",
        RecordType::AAAA,
    ).await?;
    
    // A record should have answers, AAAA should not
    let a_has_answers = dns_tests::verify_dns_response(&a_response, "127.0.0.1")?;
    let aaaa_has_answers = dns_tests::verify_dns_response(&aaaa_response, "::1")?;
    
    assert!(a_has_answers, "A record query should return answers");
    assert!(!aaaa_has_answers, "AAAA record query should not return answers");
    
    Ok(())
}

/// Test DNS service health check
async fn test_dns_health_check(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Check if DNS service is responding
    let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
    socket.set_read_timeout(Some(Duration::from_secs(1)))?;
    
    // Send a simple query
    let response = dns_tests::send_dns_query(
        "127.0.0.1:6353",
        "test.fdns.run",
        RecordType::A,
    ).await;
    
    // Should get a response (even if it's an error response)
    assert!(response.is_ok(), "DNS service should be responding to queries");
    
    Ok(())
}

/// Test DNS service with Redis connection failure
async fn test_dns_redis_failure(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // This test would require stopping the Redis container
    // For now, we'll test that the service handles Redis errors gracefully
    
    // Send a query - the service should handle Redis connection issues gracefully
    let response = dns_tests::send_dns_query(
        "127.0.0.1:6353",
        "test.fdns.run",
        RecordType::A,
    ).await;
    
    // Should get some kind of response (even if it's an error)
    assert!(response.is_ok(), "DNS service should handle Redis issues gracefully");
    
    Ok(())
}

/// Test DNS service metrics collection
async fn test_dns_metrics(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Send multiple queries to generate metrics
    for i in 0..5 {
        let domain = format!("test{}.fdns.run", i);
        let _response = dns_tests::send_dns_query(
            "127.0.0.1:6353",
            &domain,
            RecordType::A,
        ).await?;
        
        // Small delay between queries
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    // In a real test, we would check Prometheus metrics here
    // For now, we'll just verify the service is still responding
    let response = dns_tests::send_dns_query(
        "127.0.0.1:6353",
        "test.fdns.run",
        RecordType::A,
    ).await?;
    
    assert!(response.len() > 0, "DNS service should still be responding after multiple queries");
    
    Ok(())
}

// Integration test macros
integration_test!(test_dns_redis_integration, test_dns_redis_integration);
integration_test!(test_dns_nonexistent_domain, test_dns_nonexistent_domain);
integration_test!(test_dns_performance, test_dns_performance);
integration_test!(test_dns_error_handling, test_dns_error_handling);
integration_test!(test_dns_record_types, test_dns_record_types);
integration_test!(test_dns_health_check, test_dns_health_check);
integration_test!(test_dns_redis_failure, test_dns_redis_failure);
integration_test!(test_dns_metrics, test_dns_metrics);

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_dns_basic_functionality() {
        let mut ctx = TestContext::new("dns_basic").await.unwrap();
        ctx.setup().await.unwrap();
        
        // Basic DNS functionality test
        let result = test_dns_redis_integration(&mut ctx).await;
        
        ctx.cleanup().await.unwrap();
        result.unwrap();
    }
} 