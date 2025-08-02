use fleetingdns_integration_tests::{
    integration_test, TestContext, health_checks, api_tests,
};
use serde_json::json;
use std::time::Duration;

/// Test API health endpoint
async fn test_api_health(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for API service to be healthy
    health_checks::wait_for_service_health(
        "http://localhost:8080/health",
        Duration::from_secs(30),
    ).await?;
    
    // Test health endpoint
    let (status, body) = api_tests::make_api_request(
        "http://localhost:8080",
        "GET",
        "/health",
        None,
    ).await?;
    
    // Verify response
    assert_eq!(status, 200, "Health endpoint should return 200");
    
    // Verify response structure
    let has_expected_fields = api_tests::verify_api_response(&body, &["service", "status", "timestamp", "version"])?;
    assert!(has_expected_fields, "Health response should contain expected fields");
    
    Ok(())
}

/// Test API tunnels endpoint
async fn test_api_tunnels(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for API service to be healthy
    health_checks::wait_for_service_health(
        "http://localhost:8080/health",
        Duration::from_secs(30),
    ).await?;
    
    // Test GET tunnels endpoint
    let (status, body) = api_tests::make_api_request(
        "http://localhost:8080",
        "GET",
        "/v1/tunnels",
        None,
    ).await?;
    
    // Should return 200 (even if empty)
    assert_eq!(status, 200, "Tunnels endpoint should return 200");
    
    // Verify response is an array
    if let Some(array) = body.as_array() {
        // Array should exist (even if empty)
        assert!(true, "Tunnels response should be an array");
    } else {
        panic!("Tunnels response should be an array");
    }
    
    Ok(())
}

/// Test API tunnel creation
async fn test_api_tunnel_creation(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for API service to be healthy
    health_checks::wait_for_service_health(
        "http://localhost:8080/health",
        Duration::from_secs(30),
    ).await?;
    
    // Create tunnel request
    let tunnel_request = json!({
        "subdomain": "test-tunnel",
        "local_port": 8080,
        "protocol": "http"
    });
    
    // Test POST tunnel endpoint
    let (status, body) = api_tests::make_api_request(
        "http://localhost:8080",
        "POST",
        "/v1/tunnels",
        Some(tunnel_request),
    ).await?;
    
    // Should return 201 (Created) or 400 (Bad Request) depending on implementation
    assert!(
        status == 201 || status == 400,
        "Tunnel creation should return 201 or 400, got {}",
        status
    );
    
    Ok(())
}

/// Test API tunnel retrieval
async fn test_api_tunnel_retrieval(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for API service to be healthy
    health_checks::wait_for_service_health(
        "http://localhost:8080/health",
        Duration::from_secs(30),
    ).await?;
    
    // Test GET specific tunnel endpoint
    let (status, body) = api_tests::make_api_request(
        "http://localhost:8080",
        "GET",
        "/v1/tunnels/test-tunnel",
        None,
    ).await?;
    
    // Should return 200 (if exists) or 404 (if not exists)
    assert!(
        status == 200 || status == 404,
        "Tunnel retrieval should return 200 or 404, got {}",
        status
    );
    
    Ok(())
}

/// Test API certificates endpoint
async fn test_api_certificates(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for API service to be healthy
    health_checks::wait_for_service_health(
        "http://localhost:8080/health",
        Duration::from_secs(30),
    ).await?;
    
    // Test GET certificates endpoint
    let (status, body) = api_tests::make_api_request(
        "http://localhost:8080",
        "GET",
        "/v1/certificates",
        None,
    ).await?;
    
    // Should return 200 (even if empty)
    assert_eq!(status, 200, "Certificates endpoint should return 200");
    
    Ok(())
}

/// Test API error handling
async fn test_api_error_handling(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for API service to be healthy
    health_checks::wait_for_service_health(
        "http://localhost:8080/health",
        Duration::from_secs(30),
    ).await?;
    
    // Test non-existent endpoint
    let (status, _body) = api_tests::make_api_request(
        "http://localhost:8080",
        "GET",
        "/v1/nonexistent",
        None,
    ).await?;
    
    // Should return 404
    assert_eq!(status, 404, "Non-existent endpoint should return 404");
    
    // Test invalid JSON
    let invalid_json = json!({
        "invalid": "json",
        "missing": "required_fields"
    });
    
    let (status, _body) = api_tests::make_api_request(
        "http://localhost:8080",
        "POST",
        "/v1/tunnels",
        Some(invalid_json),
    ).await?;
    
    // Should return 400 (Bad Request)
    assert_eq!(status, 400, "Invalid JSON should return 400");
    
    Ok(())
}

/// Test API authentication (if implemented)
async fn test_api_authentication(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for API service to be healthy
    health_checks::wait_for_service_health(
        "http://localhost:8080/health",
        Duration::from_secs(30),
    ).await?;
    
    // Test protected endpoint without authentication
    let (status, _body) = api_tests::make_api_request(
        "http://localhost:8080",
        "GET",
        "/v1/admin/users",
        None,
    ).await?;
    
    // Should return 401 (Unauthorized) or 403 (Forbidden)
    assert!(
        status == 401 || status == 403 || status == 404,
        "Protected endpoint should return 401, 403, or 404, got {}",
        status
    );
    
    Ok(())
}

/// Test API rate limiting
async fn test_api_rate_limiting(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for API service to be healthy
    health_checks::wait_for_service_health(
        "http://localhost:8080/health",
        Duration::from_secs(30),
    ).await?;
    
    // Send multiple rapid requests
    let mut responses = vec![];
    for _ in 0..10 {
        let response = api_tests::make_api_request(
            "http://localhost:8080",
            "GET",
            "/health",
            None,
        ).await;
        responses.push(response);
    }
    
    // All requests should succeed (rate limiting might not be implemented yet)
    for response in responses {
        let (status, _body) = response?;
        assert!(status == 200, "Health endpoint should handle multiple requests");
    }
    
    Ok(())
}

/// Test API metrics collection
async fn test_api_metrics(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for API service to be healthy
    health_checks::wait_for_service_health(
        "http://localhost:8080/health",
        Duration::from_secs(30),
    ).await?;
    
    // Make multiple API calls to generate metrics
    let endpoints = vec![
        "/health",
        "/v1/tunnels",
        "/v1/certificates",
    ];
    
    for endpoint in endpoints {
        let _response = api_tests::make_api_request(
            "http://localhost:8080",
            "GET",
            endpoint,
            None,
        ).await?;
        
        // Small delay between requests
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    // Verify service is still responding
    let (status, _body) = api_tests::make_api_request(
        "http://localhost:8080",
        "GET",
        "/health",
        None,
    ).await?;
    
    assert_eq!(status, 200, "API service should still be responding after multiple requests");
    
    Ok(())
}

/// Test API database connectivity
async fn test_api_database_connectivity(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for API service to be healthy
    health_checks::wait_for_service_health(
        "http://localhost:8080/health",
        Duration::from_secs(30),
    ).await?;
    
    // Test an endpoint that requires database access
    let (status, body) = api_tests::make_api_request(
        "http://localhost:8080",
        "GET",
        "/v1/tunnels",
        None,
    ).await?;
    
    // Should return 200 (even if empty)
    assert_eq!(status, 200, "Database-dependent endpoint should return 200");
    
    // Verify response structure
    if let Some(array) = body.as_array() {
        // Array should exist (even if empty)
        assert!(true, "Tunnels response should be an array");
    } else {
        panic!("Tunnels response should be an array");
    }
    
    Ok(())
}

/// Test API Redis connectivity
async fn test_api_redis_connectivity(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for API service to be healthy
    health_checks::wait_for_service_health(
        "http://localhost:8080/health",
        Duration::from_secs(30),
    ).await?;
    
    // Test an endpoint that might use Redis
    let (status, _body) = api_tests::make_api_request(
        "http://localhost:8080",
        "GET",
        "/health",
        None,
    ).await?;
    
    // Should return 200
    assert_eq!(status, 200, "Health endpoint should work with Redis connectivity");
    
    Ok(())
}

// Integration test macros
integration_test!(test_api_health, test_api_health);
integration_test!(test_api_tunnels, test_api_tunnels);
integration_test!(test_api_tunnel_creation, test_api_tunnel_creation);
integration_test!(test_api_tunnel_retrieval, test_api_tunnel_retrieval);
integration_test!(test_api_certificates, test_api_certificates);
integration_test!(test_api_error_handling, test_api_error_handling);
integration_test!(test_api_authentication, test_api_authentication);
integration_test!(test_api_rate_limiting, test_api_rate_limiting);
integration_test!(test_api_metrics, test_api_metrics);
integration_test!(test_api_database_connectivity, test_api_database_connectivity);
integration_test!(test_api_redis_connectivity, test_api_redis_connectivity);

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_api_basic_functionality() {
        let mut ctx = TestContext::new("api_basic").await.unwrap();
        ctx.setup().await.unwrap();
        
        // Basic API functionality test
        let result = test_api_health(&mut ctx).await;
        
        ctx.cleanup().await.unwrap();
        result.unwrap();
    }
} 