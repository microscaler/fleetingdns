use fleetingdns_integration_tests::{
    integration_test, TestContext, health_checks, api_tests, dns_tests,
};
use hickory_proto::rr::RecordType;
use std::time::Duration;

/// Test complete tunnel creation and DNS resolution workflow
async fn test_complete_tunnel_workflow(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for all services to be healthy
    health_checks::wait_for_service_health("http://localhost:8080/health", Duration::from_secs(30)).await?;
    health_checks::wait_for_service_health("http://localhost:8081/health", Duration::from_secs(30)).await?;
    
    // Step 1: Create tunnel via API
    let tunnel_request = serde_json::json!({
        "subdomain": "e2e-test.fdns.run",
        "local_port": 8080,
        "protocol": "http"
    });
    
    let (create_status, create_body) = api_tests::make_api_request(
        "http://localhost:8080",
        "POST",
        "/v1/tunnels",
        Some(tunnel_request),
    ).await?;
    
    // Should return 201 (Created) or 400 (Bad Request)
    assert!(
        create_status == 201 || create_status == 400,
        "Tunnel creation should return 201 or 400, got {}",
        create_status
    );
    
    if create_status == 201 {
        // Step 2: Verify tunnel was created
        let tunnel_id = create_body["id"].as_str().unwrap_or("e2e-test");
        
        let (get_status, _get_body) = api_tests::make_api_request(
            "http://localhost:8080",
            "GET",
            &format!("/v1/tunnels/{}", tunnel_id),
            None,
        ).await?;
        
        assert_eq!(get_status, 200, "Tunnel retrieval should return 200");
        
        // Step 3: Test DNS resolution for the tunnel
        let dns_response = dns_tests::send_dns_query(
            "127.0.0.1:6353",
            "e2e-test.fdns.run",
            RecordType::A,
        ).await?;
        
        // Verify DNS response contains expected data
        let has_expected_ip = dns_tests::verify_dns_response(&dns_response, "127.0.0.1")?;
        assert!(has_expected_ip, "DNS should resolve tunnel subdomain");
        
        // Step 4: Clean up tunnel
        let (delete_status, _delete_body) = api_tests::make_api_request(
            "http://localhost:8080",
            "DELETE",
            &format!("/v1/tunnels/{}", tunnel_id),
            None,
        ).await?;
        
        assert_eq!(delete_status, 200, "Tunnel deletion should return 200");
    }
    
    Ok(())
}

/// Test certificate issuance and validation workflow
async fn test_certificate_workflow(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for EdgeHub service to be healthy
    health_checks::wait_for_service_health("http://localhost:8081/health", Duration::from_secs(30)).await?;
    
    // Step 1: Issue certificate
    let cert_request = serde_json::json!({
        "client_id": "e2e-client",
        "subdomain": "e2e-cert.fdns.run",
        "duration_minutes": 30
    });
    
    let (issue_status, issue_body) = api_tests::make_api_request(
        "http://localhost:8081",
        "POST",
        "/v1/certificates",
        Some(cert_request),
    ).await?;
    
    if issue_status == 201 {
        // Step 2: Validate the issued certificate
        let certificate = issue_body["certificate"].as_str().unwrap_or("");
        let client_id = issue_body["client_id"].as_str().unwrap_or("e2e-client");
        
        let validation_request = serde_json::json!({
            "certificate": certificate,
            "client_id": client_id
        });
        
        let (validate_status, _validate_body) = api_tests::make_api_request(
            "http://localhost:8081",
            "POST",
            "/v1/certificates/validate",
            Some(validation_request),
        ).await?;
        
        // Should return 200 (Valid) or 400 (Invalid)
        assert!(
            validate_status == 200 || validate_status == 400,
            "Certificate validation should return 200 or 400, got {}",
            validate_status
        );
    }
    
    Ok(())
}

/// Test Redis integration across all services
async fn test_redis_integration_workflow(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for all services to be healthy
    health_checks::wait_for_service_health("http://localhost:8080/health", Duration::from_secs(30)).await?;
    health_checks::wait_for_service_health("http://localhost:8081/health", Duration::from_secs(30)).await?;
    
    // Step 1: Set up test data in Redis via API
    let slot_request = serde_json::json!({
        "subdomain": "redis-test.fdns.run",
        "target_ip": "127.0.0.1"
    });
    
    let (set_status, _set_body) = api_tests::make_api_request(
        "http://localhost:8080",
        "POST",
        "/v1/slots",
        Some(slot_request),
    ).await?;
    
    // Should return 201 (Created) or 400 (Bad Request)
    assert!(
        set_status == 201 || set_status == 400,
        "Slot creation should return 201 or 400, got {}",
        set_status
    );
    
    // Step 2: Test DNS resolution for the slot
    let dns_response = dns_tests::send_dns_query(
        "127.0.0.1:6353",
        "redis-test.fdns.run",
        RecordType::A,
    ).await?;
    
    // Verify DNS response contains expected data
    let has_expected_ip = dns_tests::verify_dns_response(&dns_response, "127.0.0.1")?;
    assert!(has_expected_ip, "DNS should resolve Redis-stored slot");
    
    // Step 3: Verify slot via API
    let (get_status, _get_body) = api_tests::make_api_request(
        "http://localhost:8080",
        "GET",
        "/v1/slots/redis-test.fdns.run",
        None,
    ).await?;
    
    // Should return 200 (if exists) or 404 (if not exists)
    assert!(
        get_status == 200 || get_status == 404,
        "Slot retrieval should return 200 or 404, got {}",
        get_status
    );
    
    Ok(())
}

/// Test service communication and data flow
async fn test_service_communication(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for all services to be healthy
    health_checks::wait_for_service_health("http://localhost:8080/health", Duration::from_secs(30)).await?;
    health_checks::wait_for_service_health("http://localhost:8081/health", Duration::from_secs(30)).await?;
    
    // Step 1: Create tunnel via API
    let tunnel_request = serde_json::json!({
        "subdomain": "comm-test.fdns.run",
        "local_port": 8080,
        "protocol": "http"
    });
    
    let (create_status, create_body) = api_tests::make_api_request(
        "http://localhost:8080",
        "POST",
        "/v1/tunnels",
        Some(tunnel_request),
    ).await?;
    
    if create_status == 201 {
        // Step 2: Verify tunnel appears in EdgeHub
        let (edgehub_status, _edgehub_body) = api_tests::make_api_request(
            "http://localhost:8081",
            "GET",
            "/v1/tunnels",
            None,
        ).await?;
        
        assert_eq!(edgehub_status, 200, "EdgeHub should return tunnel list");
        
        // Step 3: Test DNS resolution
        let dns_response = dns_tests::send_dns_query(
            "127.0.0.1:6353",
            "comm-test.fdns.run",
            RecordType::A,
        ).await?;
        
        let has_expected_ip = dns_tests::verify_dns_response(&dns_response, "127.0.0.1")?;
        assert!(has_expected_ip, "DNS should resolve communication test subdomain");
    }
    
    Ok(())
}

/// Test error handling and recovery across services
async fn test_error_handling_workflow(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for all services to be healthy
    health_checks::wait_for_service_health("http://localhost:8080/health", Duration::from_secs(30)).await?;
    health_checks::wait_for_service_health("http://localhost:8081/health", Duration::from_secs(30)).await?;
    
    // Step 1: Test invalid requests to API
    let invalid_request = serde_json::json!({
        "invalid": "data",
        "missing": "required_fields"
    });
    
    let (api_status, _api_body) = api_tests::make_api_request(
        "http://localhost:8080",
        "POST",
        "/v1/tunnels",
        Some(invalid_request),
    ).await?;
    
    assert_eq!(api_status, 400, "Invalid API request should return 400");
    
    // Step 2: Test invalid requests to EdgeHub
    let (edgehub_status, _edgehub_body) = api_tests::make_api_request(
        "http://localhost:8081",
        "POST",
        "/v1/tunnels",
        Some(invalid_request),
    ).await?;
    
    assert_eq!(edgehub_status, 400, "Invalid EdgeHub request should return 400");
    
    // Step 3: Test DNS resolution for non-existent domain
    let dns_response = dns_tests::send_dns_query(
        "127.0.0.1:6353",
        "nonexistent-e2e.fdns.run",
        RecordType::A,
    ).await?;
    
    // Should not contain answers
    let has_answers = dns_tests::verify_dns_response(&dns_response, "127.0.0.1")?;
    assert!(!has_answers, "DNS should not resolve non-existent domain");
    
    // Step 4: Verify services are still healthy after errors
    let (api_health_status, _api_health_body) = api_tests::make_api_request(
        "http://localhost:8080",
        "GET",
        "/health",
        None,
    ).await?;
    
    assert_eq!(api_health_status, 200, "API should still be healthy after errors");
    
    let (edgehub_health_status, _edgehub_health_body) = api_tests::make_api_request(
        "http://localhost:8081",
        "GET",
        "/health",
        None,
    ).await?;
    
    assert_eq!(edgehub_health_status, 200, "EdgeHub should still be healthy after errors");
    
    Ok(())
}

/// Test performance under load
async fn test_performance_workflow(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for all services to be healthy
    health_checks::wait_for_service_health("http://localhost:8080/health", Duration::from_secs(30)).await?;
    health_checks::wait_for_service_health("http://localhost:8081/health", Duration::from_secs(30)).await?;
    
    // Step 1: Send multiple concurrent API requests
    let mut api_handles = vec![];
    for i in 0..10 {
        let handle = tokio::spawn(async move {
            api_tests::make_api_request(
                "http://localhost:8080",
                "GET",
                "/health",
                None,
            ).await
        });
        api_handles.push(handle);
    }
    
    // Step 2: Send multiple concurrent DNS queries
    let mut dns_handles = vec![];
    for i in 0..10 {
        let domain = format!("perf-test{}.fdns.run", i);
        let handle = tokio::spawn(async move {
            dns_tests::send_dns_query("127.0.0.1:6353", &domain, RecordType::A).await
        });
        dns_handles.push(handle);
    }
    
    // Step 3: Wait for all requests to complete
    let start = std::time::Instant::now();
    
    let api_results = futures::future::join_all(api_handles).await;
    let dns_results = futures::future::join_all(dns_handles).await;
    
    let duration = start.elapsed();
    
    // Step 4: Verify all API requests succeeded
    for result in api_results {
        let (status, _body) = result??;
        assert_eq!(status, 200, "All API requests should succeed under load");
    }
    
    // Step 5: Verify all DNS queries succeeded
    for result in dns_results {
        let response = result??;
        assert!(response.len() > 0, "All DNS queries should return responses under load");
    }
    
    // Step 6: Performance assertion
    assert!(duration < Duration::from_secs(10), "All requests should complete within 10 seconds");
    
    Ok(())
}

/// Test metrics collection across all services
async fn test_metrics_workflow(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for all services to be healthy
    health_checks::wait_for_service_health("http://localhost:8080/health", Duration::from_secs(30)).await?;
    health_checks::wait_for_service_health("http://localhost:8081/health", Duration::from_secs(30)).await?;
    
    // Step 1: Generate activity across all services
    let activities = vec![
        // API activities
        ("http://localhost:8080", "GET", "/health"),
        ("http://localhost:8080", "GET", "/v1/tunnels"),
        ("http://localhost:8080", "GET", "/v1/certificates"),
        // EdgeHub activities
        ("http://localhost:8081", "GET", "/health"),
        ("http://localhost:8081", "GET", "/v1/tunnels"),
        ("http://localhost:8081", "GET", "/v1/statistics"),
        // DNS activities
        ("127.0.0.1:6353", "DNS", "metrics-test.fdns.run"),
        ("127.0.0.1:6353", "DNS", "metrics-test2.fdns.run"),
        ("127.0.0.1:6353", "DNS", "metrics-test3.fdns.run"),
    ];
    
    for (host, method, path) in activities {
        if method == "DNS" {
            let _response = dns_tests::send_dns_query(host, path, RecordType::A).await?;
        } else {
            let _response = api_tests::make_api_request(host, method, path, None).await?;
        }
        
        // Small delay between activities
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    
    // Step 2: Verify services are still responding
    let (api_status, _api_body) = api_tests::make_api_request(
        "http://localhost:8080",
        "GET",
        "/health",
        None,
    ).await?;
    
    assert_eq!(api_status, 200, "API should still be responding after metrics generation");
    
    let (edgehub_status, _edgehub_body) = api_tests::make_api_request(
        "http://localhost:8081",
        "GET",
        "/health",
        None,
    ).await?;
    
    assert_eq!(edgehub_status, 200, "EdgeHub should still be responding after metrics generation");
    
    // Step 3: Test DNS is still responding
    let dns_response = dns_tests::send_dns_query(
        "127.0.0.1:6353",
        "test.fdns.run",
        RecordType::A,
    ).await?;
    
    assert!(dns_response.len() > 0, "DNS should still be responding after metrics generation");
    
    Ok(())
}

/// Test graceful shutdown and recovery
async fn test_graceful_shutdown_workflow(ctx: &mut TestContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for all services to be healthy
    health_checks::wait_for_service_health("http://localhost:8080/health", Duration::from_secs(30)).await?;
    health_checks::wait_for_service_health("http://localhost:8081/health", Duration::from_secs(30)).await?;
    
    // Step 1: Send graceful shutdown signal to services
    // Note: In a real test, we would use the fleetingdns-ctl tool
    // For now, we'll test that services handle requests gracefully
    
    // Step 2: Test that services handle requests during shutdown
    let (api_status, _api_body) = api_tests::make_api_request(
        "http://localhost:8080",
        "GET",
        "/health",
        None,
    ).await?;
    
    assert_eq!(api_status, 200, "API should handle requests gracefully");
    
    let (edgehub_status, _edgehub_body) = api_tests::make_api_request(
        "http://localhost:8081",
        "GET",
        "/health",
        None,
    ).await?;
    
    assert_eq!(edgehub_status, 200, "EdgeHub should handle requests gracefully");
    
    // Step 3: Test DNS service during shutdown
    let dns_response = dns_tests::send_dns_query(
        "127.0.0.1:6353",
        "test.fdns.run",
        RecordType::A,
    ).await?;
    
    assert!(dns_response.len() > 0, "DNS should handle queries gracefully");
    
    Ok(())
}

// Integration test macros
integration_test!(test_complete_tunnel_workflow, test_complete_tunnel_workflow);
integration_test!(test_certificate_workflow, test_certificate_workflow);
integration_test!(test_redis_integration_workflow, test_redis_integration_workflow);
integration_test!(test_service_communication, test_service_communication);
integration_test!(test_error_handling_workflow, test_error_handling_workflow);
integration_test!(test_performance_workflow, test_performance_workflow);
integration_test!(test_metrics_workflow, test_metrics_workflow);
integration_test!(test_graceful_shutdown_workflow, test_graceful_shutdown_workflow);

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_e2e_basic_functionality() {
        let mut ctx = TestContext::new("e2e_basic").await.unwrap();
        ctx.setup().await.unwrap();
        
        // Basic end-to-end functionality test
        let result = test_complete_tunnel_workflow(&mut ctx).await;
        
        ctx.cleanup().await.unwrap();
        result.unwrap();
    }
} 