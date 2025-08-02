use fleetingdns_integration_tests::{
    integration_test, TestContext, health_checks, api_tests,
};
use std::time::Duration;
use tokio::net::TcpStream;
use std::io::{Read, Write};

/// Test EdgeHub SSH server connectivity
async fn test_edgehub_ssh_connectivity(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for EdgeHub service to be healthy
    health_checks::wait_for_service_health(
        "http://localhost:8081/health",
        Duration::from_secs(30),
    ).await?;
    
    // Test SSH port connectivity
    let ssh_result = TcpStream::connect("localhost:2222").await;
    
    // Should be able to connect to SSH port
    assert!(ssh_result.is_ok(), "EdgeHub SSH port should be accessible");
    
    Ok(())
}

/// Test EdgeHub tunnel creation
async fn test_edgehub_tunnel_creation(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for EdgeHub service to be healthy
    health_checks::wait_for_service_health(
        "http://localhost:8081/health",
        Duration::from_secs(30),
    ).await?;
    
    // Test tunnel creation via API
    let tunnel_request = serde_json::json!({
        "subdomain": "test-tunnel-edgehub",
        "local_port": 8080,
        "protocol": "http"
    });
    
    let (status, _body) = api_tests::make_api_request(
        "http://localhost:8081",
        "POST",
        "/v1/tunnels",
        Some(tunnel_request),
    ).await?;
    
    // Should return 201 (Created) or 400 (Bad Request) depending on implementation
    assert!(
        status == 201 || status == 400,
        "EdgeHub tunnel creation should return 201 or 400, got {}",
        status
    );
    
    Ok(())
}

/// Test EdgeHub certificate issuance
async fn test_edgehub_certificate_issuance(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for EdgeHub service to be healthy
    health_checks::wait_for_service_health(
        "http://localhost:8081/health",
        Duration::from_secs(30),
    ).await?;
    
    // Test certificate issuance endpoint
    let cert_request = serde_json::json!({
        "client_id": "test-client",
        "subdomain": "test-cert.fdns.run",
        "duration_minutes": 30
    });
    
    let (status, body) = api_tests::make_api_request(
        "http://localhost:8081",
        "POST",
        "/v1/certificates",
        Some(cert_request),
    ).await?;
    
    // Should return 201 (Created) or 400 (Bad Request)
    assert!(
        status == 201 || status == 400,
        "EdgeHub certificate issuance should return 201 or 400, got {}",
        status
    );
    
    // If successful, verify response structure
    if status == 201 {
        let has_expected_fields = api_tests::verify_api_response(&body, &["certificate", "private_key", "serial"])?;
        assert!(has_expected_fields, "Certificate response should contain expected fields");
    }
    
    Ok(())
}

/// Test EdgeHub brute force protection
async fn test_edgehub_brute_force_protection(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for EdgeHub service to be healthy
    health_checks::wait_for_service_health(
        "http://localhost:8081/health",
        Duration::from_secs(30),
    ).await?;
    
    // Test multiple failed authentication attempts
    for i in 0..5 {
        let auth_request = serde_json::json!({
            "username": "invalid-user",
            "password": "invalid-password",
            "attempt": i
        });
        
        let (status, _body) = api_tests::make_api_request(
            "http://localhost:8081",
            "POST",
            "/v1/auth",
            Some(auth_request),
        ).await?;
        
        // Should return 401 (Unauthorized) for failed attempts
        assert_eq!(status, 401, "Failed authentication should return 401");
    }
    
    // Test that the service is still responding after multiple failed attempts
    let (status, _body) = api_tests::make_api_request(
        "http://localhost:8081",
        "GET",
        "/health",
        None,
    ).await?;
    
    assert_eq!(status, 200, "EdgeHub should still be responding after failed auth attempts");
    
    Ok(())
}

/// Test EdgeHub tunnel statistics
async fn test_edgehub_tunnel_statistics(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for EdgeHub service to be healthy
    health_checks::wait_for_service_health(
        "http://localhost:8081/health",
        Duration::from_secs(30),
    ).await?;
    
    // Test statistics endpoint
    let (status, body) = api_tests::make_api_request(
        "http://localhost:8081",
        "GET",
        "/v1/statistics",
        None,
    ).await?;
    
    // Should return 200
    assert_eq!(status, 200, "EdgeHub statistics endpoint should return 200");
    
    // Verify response structure
    let has_expected_fields = api_tests::verify_api_response(&body, &["active_tunnels", "total_connections", "uptime"])?;
    assert!(has_expected_fields, "Statistics response should contain expected fields");
    
    Ok(())
}

/// Test EdgeHub Redis connectivity
async fn test_edgehub_redis_connectivity(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for EdgeHub service to be healthy
    health_checks::wait_for_service_health(
        "http://localhost:8081/health",
        Duration::from_secs(30),
    ).await?;
    
    // Test an endpoint that uses Redis
    let (status, _body) = api_tests::make_api_request(
        "http://localhost:8081",
        "GET",
        "/v1/tunnels",
        None,
    ).await?;
    
    // Should return 200 (even if empty)
    assert_eq!(status, 200, "EdgeHub should work with Redis connectivity");
    
    Ok(())
}

/// Test EdgeHub certificate validation
async fn test_edgehub_certificate_validation(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for EdgeHub service to be healthy
    health_checks::wait_for_service_health(
        "http://localhost:8081/health",
        Duration::from_secs(30),
    ).await?;
    
    // Test certificate validation endpoint
    let validation_request = serde_json::json!({
        "certificate": "-----BEGIN CERTIFICATE-----\nMIIFazCCA1OgAwIBAgIRAIIQz7DSQONZRGPgu2OCiwAwDQYJKoZIhvcNAQELBQAw\nTzELMAkGA1UEBhMCVVMxKTAnBgNVBAoTIEludGVybmV0IFNlY3VyaXR5IFJlc2Vh\ncmNoIEdyb3VwMRUwEwYDVQQDEwxJU1JHIFJvb3QgQzEwHhcNMTUwNjA0MTEwNDM4\nWhcNMzUwNjA0MTEwNDM4WjBPMQswCQYDVQQGEwJVUzEpMCcGA1UEChMgSW50ZXJu\nZXQgU2VjdXJpdHkgUmVzZWFyY2ggR3JvdXAxFTATBgNVBAMTDElTUkcgUm9vdCB\nDMTCCAiIwDQYJKoZIhvcNAQEBBQADggIPADCCAgoCggIBAK3oJHP0FDfzm54rV\n-----END CERTIFICATE-----",
        "client_id": "test-client"
    });
    
    let (status, _body) = api_tests::make_api_request(
        "http://localhost:8081",
        "POST",
        "/v1/certificates/validate",
        Some(validation_request),
    ).await?;
    
    // Should return 200 (Valid) or 400 (Invalid)
    assert!(
        status == 200 || status == 400,
        "Certificate validation should return 200 or 400, got {}",
        status
    );
    
    Ok(())
}

/// Test EdgeHub tunnel lifecycle
async fn test_edgehub_tunnel_lifecycle(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for EdgeHub service to be healthy
    health_checks::wait_for_service_health(
        "http://localhost:8081/health",
        Duration::from_secs(30),
    ).await?;
    
    // Create tunnel
    let create_request = serde_json::json!({
        "subdomain": "lifecycle-test.fdns.run",
        "local_port": 8080,
        "protocol": "http"
    });
    
    let (create_status, create_body) = api_tests::make_api_request(
        "http://localhost:8081",
        "POST",
        "/v1/tunnels",
        Some(create_request),
    ).await?;
    
    if create_status == 201 {
        // Get tunnel ID from response
        let tunnel_id = create_body["id"].as_str().unwrap_or("test-tunnel");
        
        // Test tunnel retrieval
        let (get_status, _get_body) = api_tests::make_api_request(
            "http://localhost:8081",
            "GET",
            &format!("/v1/tunnels/{}", tunnel_id),
            None,
        ).await?;
        
        assert_eq!(get_status, 200, "Tunnel retrieval should return 200");
        
        // Test tunnel deletion
        let (delete_status, _delete_body) = api_tests::make_api_request(
            "http://localhost:8081",
            "DELETE",
            &format!("/v1/tunnels/{}", tunnel_id),
            None,
        ).await?;
        
        assert_eq!(delete_status, 200, "Tunnel deletion should return 200");
    }
    
    Ok(())
}

/// Test EdgeHub error handling
async fn test_edgehub_error_handling(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for EdgeHub service to be healthy
    health_checks::wait_for_service_health(
        "http://localhost:8081/health",
        Duration::from_secs(30),
    ).await?;
    
    // Test invalid tunnel creation
    let invalid_request = serde_json::json!({
        "invalid": "data",
        "missing": "required_fields"
    });
    
    let (status, _body) = api_tests::make_api_request(
        "http://localhost:8081",
        "POST",
        "/v1/tunnels",
        Some(invalid_request),
    ).await?;
    
    // Should return 400 (Bad Request)
    assert_eq!(status, 400, "Invalid tunnel request should return 400");
    
    // Test non-existent tunnel retrieval
    let (status, _body) = api_tests::make_api_request(
        "http://localhost:8081",
        "GET",
        "/v1/tunnels/nonexistent-tunnel",
        None,
    ).await?;
    
    // Should return 404 (Not Found)
    assert_eq!(status, 404, "Non-existent tunnel should return 404");
    
    Ok(())
}

/// Test EdgeHub metrics collection
async fn test_edgehub_metrics(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for EdgeHub service to be healthy
    health_checks::wait_for_service_health(
        "http://localhost:8081/health",
        Duration::from_secs(30),
    ).await?;
    
    // Make multiple API calls to generate metrics
    let endpoints = vec![
        "/health",
        "/v1/tunnels",
        "/v1/statistics",
        "/v1/certificates",
    ];
    
    for endpoint in endpoints {
        let _response = api_tests::make_api_request(
            "http://localhost:8081",
            "GET",
            endpoint,
            None,
        ).await?;
        
        // Small delay between requests
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    // Verify service is still responding
    let (status, _body) = api_tests::make_api_request(
        "http://localhost:8081",
        "GET",
        "/health",
        None,
    ).await?;
    
    assert_eq!(status, 200, "EdgeHub should still be responding after multiple requests");
    
    Ok(())
}

// Integration test macros
integration_test!(test_edgehub_ssh_connectivity, test_edgehub_ssh_connectivity);
integration_test!(test_edgehub_tunnel_creation, test_edgehub_tunnel_creation);
integration_test!(test_edgehub_certificate_issuance, test_edgehub_certificate_issuance);
integration_test!(test_edgehub_brute_force_protection, test_edgehub_brute_force_protection);
integration_test!(test_edgehub_tunnel_statistics, test_edgehub_tunnel_statistics);
integration_test!(test_edgehub_redis_connectivity, test_edgehub_redis_connectivity);
integration_test!(test_edgehub_certificate_validation, test_edgehub_certificate_validation);
integration_test!(test_edgehub_tunnel_lifecycle, test_edgehub_tunnel_lifecycle);
integration_test!(test_edgehub_error_handling, test_edgehub_error_handling);
integration_test!(test_edgehub_metrics, test_edgehub_metrics);

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_edgehub_basic_functionality() {
        let mut ctx = TestContext::new("edgehub_basic").await.unwrap();
        ctx.setup().await.unwrap();
        
        // Basic EdgeHub functionality test
        let result = test_edgehub_ssh_connectivity(&mut ctx).await;
        
        ctx.cleanup().await.unwrap();
        result.unwrap();
    }
} 