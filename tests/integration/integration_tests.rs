//! Integration tests against the live dev-cluster stack (Tilt on ms02).
//!
//! Assumptions about the environment:
//! - The API is reachable on `localhost:8880` (Tilt port-forward) and runs
//!   with `DEVELOPMENT_MODE=true`, so requests authenticate via the
//!   dev-bypass headers added by `test_common::api_tests::make_api_request`.
//! - The EdgeHub SSH port is reachable on `localhost:2222` (Tilt forward).
//! - dnsd is reachable at `dns_tests::dns_server_addr()` (the kind node's
//!   UDP NodePort by default; override with FDNS_TEST_DNS_ADDR).
//!
//! The assertions follow the REAL API contract (see backendapi routes):
//! - POST /v1/tunnels {"port", "ttl", "custom_subdomain"?} → 200
//! - malformed create bodies are rejected by axum's Json extractor → 422
//! - GET /v1/tunnels → 200 bare array
//! - GET/DELETE /v1/tunnels/{uuid} → 200 / 400 (bad uuid) / 404 (missing)
//! - GET /v1/tunnels/{uuid}/health → 200; POST /v1/tunnels/health/bulk → 200
//! - POST /v1/certificates {"common_name", "ttl"?} → 200
//! - GET /v1/certificates/{serial} → 200
//! - GET /v1/stats → 200

mod test_common;
use hickory_proto::rr::RecordType;
use serde_json::json;
use std::time::Duration;
use test_common::{api_tests, dns_tests, health_checks};

const API: &str = "http://localhost:8880";

async fn wait_api_healthy() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    health_checks::wait_for_service_health(&format!("{API}/health"), Duration::from_secs(30)).await
}

/// Create a tunnel with a random (API-generated) subdomain and return
/// `(status, body)`. Callers should delete the tunnel when done.
async fn create_tunnel(
    port: u16,
) -> Result<(u16, serde_json::Value), Box<dyn std::error::Error + Send + Sync>> {
    api_tests::make_api_request(
        API,
        "POST",
        "/v1/tunnels",
        Some(json!({ "port": port, "ttl": 300 })),
    )
    .await
}

async fn delete_tunnel(id: &str) -> Result<u16, Box<dyn std::error::Error + Send + Sync>> {
    let (status, _) =
        api_tests::make_api_request(API, "DELETE", &format!("/v1/tunnels/{id}"), None).await?;
    Ok(status)
}

// =============================================================================
// API Integration Tests
// =============================================================================

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_api_health() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    let (status, body) = api_tests::make_api_request(API, "GET", "/health", None).await?;
    assert_eq!(status, 200, "Health endpoint should return 200");

    let has_expected_fields =
        api_tests::verify_api_response(&body, &["service", "status", "timestamp", "version"])?;
    assert!(
        has_expected_fields,
        "Health response should contain expected fields"
    );

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_api_tunnels() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    let (status, body) = api_tests::make_api_request(API, "GET", "/v1/tunnels", None).await?;
    assert_eq!(status, 200, "Tunnels endpoint should return 200");
    assert!(body.is_array(), "Tunnels response should be an array");

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_api_tunnel_creation() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    let (status, body) = create_tunnel(8080).await?;
    assert_eq!(
        status, 200,
        "Tunnel creation should return 200, got {status}: {body}"
    );

    let has_expected_fields =
        api_tests::verify_api_response(&body, &["id", "fqdn", "slot", "expires_at"])?;
    assert!(
        has_expected_fields,
        "Create response should contain id/fqdn/slot/expires_at"
    );

    let id = body["id"].as_str().expect("tunnel id");
    assert_eq!(
        delete_tunnel(id).await?,
        200,
        "Cleanup deletion should return 200"
    );

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_api_tunnel_retrieval() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    // Unknown-but-valid UUID → 404; non-UUID id → 400.
    let (status, _) = api_tests::make_api_request(
        API,
        "GET",
        "/v1/tunnels/00000000-0000-0000-0000-000000000000",
        None,
    )
    .await?;
    assert_eq!(status, 404, "Unknown tunnel should return 404");

    let (status, _) =
        api_tests::make_api_request(API, "GET", "/v1/tunnels/not-a-uuid", None).await?;
    assert_eq!(status, 400, "Malformed tunnel id should return 400");

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_api_certificates() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    let (status, body) = api_tests::make_api_request(
        API,
        "POST",
        "/v1/certificates",
        Some(json!({ "common_name": "itest-client", "ttl": 1800 })),
    )
    .await?;
    assert_eq!(
        status, 200,
        "Certificate issuance should return 200, got {status}: {body}"
    );

    let has_expected_fields = api_tests::verify_api_response(
        &body,
        &["serial", "certificate", "fingerprint", "expires_at"],
    )?;
    assert!(
        has_expected_fields,
        "Certificate response should contain expected fields"
    );

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_api_error_handling() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    // Unknown route → 404.
    let (status, _) = api_tests::make_api_request(API, "GET", "/v1/nonexistent", None).await?;
    assert_eq!(status, 404, "Non-existent endpoint should return 404");

    // Body missing required fields → axum Json extractor rejects with 422.
    let (status, _) = api_tests::make_api_request(
        API,
        "POST",
        "/v1/tunnels",
        Some(json!({ "invalid": "data" })),
    )
    .await?;
    assert_eq!(status, 422, "Malformed tunnel request should return 422");

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_api_authentication() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    // A protected endpoint without any credentials must be rejected: the
    // raw client sends no dev-bypass headers here on purpose.
    let client = reqwest::Client::new();
    let response = client.get(format!("{API}/v1/tunnels")).send().await?;
    assert_eq!(
        response.status().as_u16(),
        401,
        "Protected endpoint without credentials should return 401"
    );

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_api_rate_limiting() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    // The dev-bypass token is registered as a rate-limit bypass token in
    // development mode, so a burst of requests must all succeed.
    for _ in 0..10 {
        let (status, _) = api_tests::make_api_request(API, "GET", "/health", None).await?;
        assert_eq!(status, 200, "Health endpoint should handle burst requests");
    }

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_api_metrics() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    for endpoint in ["/health", "/v1/tunnels", "/v1/stats"] {
        let _ = api_tests::make_api_request(API, "GET", endpoint, None).await?;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let (status, _) = api_tests::make_api_request(API, "GET", "/health", None).await?;
    assert_eq!(
        status, 200,
        "API should still be responding after multiple requests"
    );

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_api_database_connectivity() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    let (status, body) = api_tests::make_api_request(API, "GET", "/v1/tunnels", None).await?;
    assert_eq!(status, 200, "Database-dependent endpoint should return 200");
    assert!(body.is_array(), "Tunnels response should be an array");

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_api_redis_connectivity() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    // Tunnel storage is Redis-backed: a successful create+delete proves
    // the API's Redis connectivity end-to-end.
    let (status, body) = create_tunnel(8080).await?;
    assert_eq!(
        status, 200,
        "Redis-backed tunnel creation should return 200"
    );
    let id = body["id"].as_str().expect("tunnel id");
    assert_eq!(delete_tunnel(id).await?, 200);

    Ok(())
}

// =============================================================================
// EdgeHub Integration Tests
// =============================================================================

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_edgehub_ssh_connectivity() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    let ssh_result = tokio::net::TcpStream::connect("localhost:2222").await;
    assert!(ssh_result.is_ok(), "EdgeHub SSH port should be accessible");

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_edgehub_tunnel_creation() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    let (status, body) = create_tunnel(8080).await?;
    assert_eq!(
        status, 200,
        "Tunnel creation should return 200, got {status}: {body}"
    );

    // The response must carry the API-issued slot the CLI will forward to
    // the hub (the CLI must never fabricate one).
    let slot = body["slot"].as_u64().expect("slot in response");
    assert!(
        slot >= 1024,
        "Slot should be a non-privileged port, got {slot}"
    );

    let id = body["id"].as_str().expect("tunnel id");
    assert_eq!(delete_tunnel(id).await?, 200);

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_edgehub_certificate_issuance() -> Result<(), Box<dyn std::error::Error + Send + Sync>>
{
    wait_api_healthy().await?;

    let (status, body) = api_tests::make_api_request(
        API,
        "POST",
        "/v1/certificates",
        Some(json!({ "common_name": "tunnel-client-itest", "ttl": 1800 })),
    )
    .await?;
    assert_eq!(
        status, 200,
        "Certificate issuance should return 200, got {status}: {body}"
    );

    let has_expected_fields = api_tests::verify_api_response(&body, &["serial", "certificate"])?;
    assert!(
        has_expected_fields,
        "Certificate response should contain serial and certificate"
    );

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_edgehub_brute_force_protection(
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    // Hammer a protected endpoint with bogus Bearer tokens; every attempt
    // must be rejected and the service must stay healthy afterwards.
    let client = reqwest::Client::new();
    for i in 0..5 {
        let response = client
            .get(format!("{API}/v1/tunnels"))
            .header("authorization", format!("Bearer bogus-token-{i}"))
            .send()
            .await?;
        assert_eq!(
            response.status().as_u16(),
            401,
            "Bogus credentials should return 401"
        );
    }

    let (status, _) = api_tests::make_api_request(API, "GET", "/health", None).await?;
    assert_eq!(
        status, 200,
        "API should still be responding after failed auth attempts"
    );

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_edgehub_tunnel_statistics() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    let (status, body) = api_tests::make_api_request(API, "GET", "/v1/stats", None).await?;
    assert_eq!(
        status, 200,
        "Stats endpoint should return 200, got {status}: {body}"
    );

    let has_expected_fields = api_tests::verify_api_response(&body, &["api_stats", "system_info"])?;
    assert!(
        has_expected_fields,
        "Stats response should contain api_stats and system_info"
    );

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_edgehub_redis_connectivity() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    let (status, _) = api_tests::make_api_request(API, "GET", "/v1/tunnels", None).await?;
    assert_eq!(
        status, 200,
        "Tunnel listing (Redis-backed) should return 200"
    );

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_edgehub_certificate_validation(
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    // Issue a certificate, then fetch it back by serial.
    let (issue_status, issue_body) = api_tests::make_api_request(
        API,
        "POST",
        "/v1/certificates",
        Some(json!({ "common_name": "validate-itest", "ttl": 1800 })),
    )
    .await?;
    assert_eq!(issue_status, 200, "Certificate issuance should return 200");

    let serial = issue_body["serial"].as_str().expect("serial in response");
    let (get_status, get_body) =
        api_tests::make_api_request(API, "GET", &format!("/v1/certificates/{serial}"), None)
            .await?;
    assert_eq!(
        get_status, 200,
        "Certificate lookup by serial should return 200, got {get_status}: {get_body}"
    );

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_edgehub_tunnel_lifecycle() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    let (create_status, create_body) = create_tunnel(8080).await?;
    assert_eq!(create_status, 200, "Tunnel creation should return 200");
    let tunnel_id = create_body["id"].as_str().expect("tunnel id").to_string();

    let (get_status, _) =
        api_tests::make_api_request(API, "GET", &format!("/v1/tunnels/{tunnel_id}"), None).await?;
    assert_eq!(get_status, 200, "Tunnel retrieval should return 200");

    assert_eq!(
        delete_tunnel(&tunnel_id).await?,
        200,
        "Tunnel deletion should return 200"
    );

    let (get_deleted, _) =
        api_tests::make_api_request(API, "GET", &format!("/v1/tunnels/{tunnel_id}"), None).await?;
    assert_eq!(get_deleted, 404, "Deleted tunnel should return 404");

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_edgehub_error_handling() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    let (status, _) = api_tests::make_api_request(
        API,
        "POST",
        "/v1/tunnels",
        Some(json!({ "invalid": "data" })),
    )
    .await?;
    assert_eq!(status, 422, "Malformed tunnel request should return 422");

    let (status, _) =
        api_tests::make_api_request(API, "GET", "/v1/tunnels/nonexistent-tunnel", None).await?;
    assert_eq!(status, 400, "Non-UUID tunnel id should return 400");

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_edgehub_metrics() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    for endpoint in ["/health", "/v1/tunnels", "/v1/stats"] {
        let _ = api_tests::make_api_request(API, "GET", endpoint, None).await?;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let (status, _) = api_tests::make_api_request(API, "GET", "/health", None).await?;
    assert_eq!(
        status, 200,
        "API should still be responding after multiple requests"
    );

    Ok(())
}

// =============================================================================
// End-to-End Tests
// =============================================================================

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_complete_tunnel_workflow() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    let (create_status, create_body) = create_tunnel(8080).await?;
    assert_eq!(create_status, 200, "Tunnel creation should return 200");

    let tunnel_id = create_body["id"].as_str().expect("tunnel id").to_string();
    let fqdn = create_body["fqdn"].as_str().expect("fqdn").to_string();

    let (get_status, _) =
        api_tests::make_api_request(API, "GET", &format!("/v1/tunnels/{tunnel_id}"), None).await?;
    assert_eq!(get_status, 200, "Tunnel retrieval should return 200");

    // The tunnel FQDN must resolve through dnsd.
    let dns_response =
        dns_tests::send_dns_query(&dns_tests::dns_server_addr(), &fqdn, RecordType::A).await?;
    let has_answer = dns_tests::verify_dns_response(&dns_response, "127.0.0.1")?;
    assert!(has_answer, "DNS should resolve tunnel subdomain {fqdn}");

    assert_eq!(
        delete_tunnel(&tunnel_id).await?,
        200,
        "Tunnel deletion should return 200"
    );

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_certificate_workflow() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    let (issue_status, issue_body) = api_tests::make_api_request(
        API,
        "POST",
        "/v1/certificates",
        Some(json!({ "common_name": "e2e-cert-client", "ttl": 1800 })),
    )
    .await?;
    assert_eq!(issue_status, 200, "Certificate issuance should return 200");

    let serial = issue_body["serial"].as_str().expect("serial");
    let (get_status, get_body) =
        api_tests::make_api_request(API, "GET", &format!("/v1/certificates/{serial}"), None)
            .await?;
    assert_eq!(
        get_status, 200,
        "Certificate lookup should return 200, got {get_status}: {get_body}"
    );

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_redis_integration_workflow() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    // The tunnel record and its subdomain index live in Redis; DNS reads
    // the same data. Create → resolve → delete exercises the full loop.
    let (create_status, create_body) = create_tunnel(8080).await?;
    assert_eq!(create_status, 200, "Tunnel creation should return 200");

    let tunnel_id = create_body["id"].as_str().expect("tunnel id").to_string();
    let fqdn = create_body["fqdn"].as_str().expect("fqdn").to_string();

    let dns_response =
        dns_tests::send_dns_query(&dns_tests::dns_server_addr(), &fqdn, RecordType::A).await?;
    assert!(!dns_response.is_empty(), "DNS should answer for {fqdn}");

    assert_eq!(delete_tunnel(&tunnel_id).await?, 200);

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_service_communication() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    let (create_status, create_body) = create_tunnel(8080).await?;
    assert_eq!(create_status, 200, "Tunnel creation should return 200");
    let tunnel_id = create_body["id"].as_str().expect("tunnel id").to_string();
    let fqdn = create_body["fqdn"].as_str().expect("fqdn").to_string();

    let (list_status, list_body) =
        api_tests::make_api_request(API, "GET", "/v1/tunnels", None).await?;
    assert_eq!(list_status, 200, "Tunnel list should return 200");
    let listed = list_body
        .as_array()
        .is_some_and(|arr| arr.iter().any(|t| t["id"] == tunnel_id.as_str()));
    assert!(listed, "Created tunnel should appear in the list");

    let dns_response =
        dns_tests::send_dns_query(&dns_tests::dns_server_addr(), &fqdn, RecordType::A).await?;
    let has_answer = dns_tests::verify_dns_response(&dns_response, "127.0.0.1")?;
    assert!(has_answer, "DNS should resolve {fqdn}");

    assert_eq!(delete_tunnel(&tunnel_id).await?, 200);

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_error_handling_workflow() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    // Invalid API requests are rejected without harming the service.
    let (status, _) = api_tests::make_api_request(
        API,
        "POST",
        "/v1/tunnels",
        Some(json!({ "invalid": "data" })),
    )
    .await?;
    assert_eq!(status, 422, "Malformed tunnel request should return 422");

    // dnsd answers queries for unknown names without falling over (the
    // response may or may not carry answers depending on zone config).
    let dns_response = dns_tests::send_dns_query(
        &dns_tests::dns_server_addr(),
        "nonexistent-e2e.fleetingdns.run",
        RecordType::A,
    )
    .await?;
    assert!(
        !dns_response.is_empty(),
        "DNS should respond to unknown-name queries"
    );

    let (health_status, _) = api_tests::make_api_request(API, "GET", "/health", None).await?;
    assert_eq!(
        health_status, 200,
        "API should still be healthy after errors"
    );

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_performance_workflow() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    let mut api_handles = vec![];
    for _ in 0..10 {
        api_handles.push(tokio::spawn(async {
            api_tests::make_api_request(API, "GET", "/health", None).await
        }));
    }

    let mut dns_handles = vec![];
    for i in 0..10 {
        let domain = format!("perf-test{i}.fleetingdns.run");
        dns_handles.push(tokio::spawn(async move {
            dns_tests::send_dns_query(&dns_tests::dns_server_addr(), &domain, RecordType::A).await
        }));
    }

    let start = std::time::Instant::now();
    let api_results = futures::future::join_all(api_handles).await;
    let dns_results = futures::future::join_all(dns_handles).await;
    let duration = start.elapsed();

    for result in api_results {
        let (status, _) = result??;
        assert_eq!(status, 200, "All API requests should succeed under load");
    }
    for result in dns_results {
        let response = result??;
        assert!(
            !response.is_empty(),
            "All DNS queries should return responses under load"
        );
    }

    assert!(
        duration < Duration::from_secs(10),
        "All requests should complete within 10 seconds"
    );

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_metrics_workflow() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    for endpoint in [
        "/health",
        "/v1/tunnels",
        "/v1/stats",
        "/health",
        "/v1/tunnels",
    ] {
        let _ = api_tests::make_api_request(API, "GET", endpoint, None).await?;
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    for name in ["metrics-test", "metrics-test2", "metrics-test3"] {
        let domain = format!("{name}.fleetingdns.run");
        let _ = dns_tests::send_dns_query(&dns_tests::dns_server_addr(), &domain, RecordType::A)
            .await?;
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let (api_status, _) = api_tests::make_api_request(API, "GET", "/health", None).await?;
    assert_eq!(
        api_status, 200,
        "API should still be responding after metrics generation"
    );

    let dns_response = dns_tests::send_dns_query(
        &dns_tests::dns_server_addr(),
        "test.fleetingdns.run",
        RecordType::A,
    )
    .await?;
    assert!(
        !dns_response.is_empty(),
        "DNS should still be responding after metrics generation"
    );

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_graceful_shutdown_workflow() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    let (api_status, _) = api_tests::make_api_request(API, "GET", "/health", None).await?;
    assert_eq!(api_status, 200, "API should handle requests gracefully");

    let dns_response = dns_tests::send_dns_query(
        &dns_tests::dns_server_addr(),
        "test.fleetingdns.run",
        RecordType::A,
    )
    .await?;
    assert!(
        !dns_response.is_empty(),
        "DNS should handle queries gracefully"
    );

    Ok(())
}

// =============================================================================
// Tunnel Lifecycle Tests (from tunnel_lifecycle_tests.rs)
// =============================================================================

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_complete_tunnel_lifecycle() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    let (create_status, create_body) = create_tunnel(8080).await?;
    assert_eq!(create_status, 200, "Tunnel creation should return 200");
    let tunnel_id = create_body["id"].as_str().expect("tunnel id").to_string();

    let (get_status, get_body) =
        api_tests::make_api_request(API, "GET", &format!("/v1/tunnels/{tunnel_id}"), None).await?;
    assert_eq!(get_status, 200, "Tunnel retrieval should return 200");
    assert_eq!(
        get_body["id"].as_str().unwrap(),
        tunnel_id,
        "Tunnel ID should match"
    );

    let (health_status, health_body) =
        api_tests::make_api_request(API, "GET", &format!("/v1/tunnels/{tunnel_id}/health"), None)
            .await?;
    assert_eq!(health_status, 200, "Health check should return 200");
    assert!(
        health_body["status"].is_string(),
        "Health status should be present"
    );
    assert!(
        health_body["connection_status"].is_string(),
        "Connection status should be present"
    );
    assert!(
        health_body["details"].is_object(),
        "Health details should be present"
    );

    let (bulk_status, bulk_body) = api_tests::make_api_request(
        API,
        "POST",
        "/v1/tunnels/health/bulk",
        Some(json!({ "tunnel_ids": [tunnel_id] })),
    )
    .await?;
    assert_eq!(bulk_status, 200, "Bulk health check should return 200");
    assert_eq!(
        bulk_body["total_tunnels"].as_u64().unwrap(),
        1,
        "Should check 1 tunnel"
    );
    assert_eq!(
        bulk_body["successful_checks"].as_u64().unwrap(),
        1,
        "Should have 1 successful check"
    );
    assert_eq!(
        bulk_body["failed_checks"].as_u64().unwrap(),
        0,
        "Should have 0 failed checks"
    );

    let (list_status, list_body) =
        api_tests::make_api_request(API, "GET", "/v1/tunnels", None).await?;
    assert_eq!(list_status, 200, "Tunnel listing should return 200");
    assert!(list_body.is_array(), "Should return tunnels array");

    assert_eq!(
        delete_tunnel(&tunnel_id).await?,
        200,
        "Tunnel deletion should return 200"
    );

    let (get_deleted, _) =
        api_tests::make_api_request(API, "GET", &format!("/v1/tunnels/{tunnel_id}"), None).await?;
    assert_eq!(get_deleted, 404, "Deleted tunnel should return 404");

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_tunnel_creation_scenarios() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    let (basic_status, basic_body) = create_tunnel(8080).await?;
    assert_eq!(basic_status, 200, "Basic tunnel creation should return 200");
    let basic_tunnel_id = basic_body["id"].as_str().expect("tunnel id").to_string();

    // Missing the required `port` field → 422 from the Json extractor.
    let (invalid_status, _) = api_tests::make_api_request(
        API,
        "POST",
        "/v1/tunnels",
        Some(json!({ "custom_subdomain": "invalid-test" })),
    )
    .await?;
    assert_eq!(
        invalid_status, 422,
        "Invalid tunnel creation should return 422"
    );

    assert_eq!(
        delete_tunnel(&basic_tunnel_id).await?,
        200,
        "Tunnel deletion should return 200"
    );

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_tunnel_health_scenarios() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    let (create_status, create_body) = create_tunnel(8080).await?;
    assert_eq!(create_status, 200, "Tunnel creation should return 200");
    let tunnel_id = create_body["id"].as_str().expect("tunnel id").to_string();

    let (health_status, health_body) =
        api_tests::make_api_request(API, "GET", &format!("/v1/tunnels/{tunnel_id}/health"), None)
            .await?;
    assert_eq!(health_status, 200, "Health check should return 200");
    for field in ["status", "connection_status", "last_check"] {
        assert!(
            health_body[field].is_string(),
            "Health response should contain string field: {field}"
        );
    }
    assert!(
        health_body["details"].is_object(),
        "Health details should be present"
    );
    assert!(
        health_body["response_time_ms"].is_number(),
        "response_time_ms should be a number"
    );

    // Bulk check with one live and one missing tunnel.
    let missing_id = "00000000-0000-0000-0000-000000000000";
    let (bulk_status, bulk_body) = api_tests::make_api_request(
        API,
        "POST",
        "/v1/tunnels/health/bulk",
        Some(json!({ "tunnel_ids": [tunnel_id, missing_id] })),
    )
    .await?;
    assert_eq!(bulk_status, 200, "Bulk health check should return 200");
    assert_eq!(
        bulk_body["total_tunnels"].as_u64().unwrap(),
        2,
        "Should check 2 tunnels"
    );
    assert_eq!(
        bulk_body["successful_checks"].as_u64().unwrap(),
        1,
        "Should have 1 successful check"
    );
    assert_eq!(
        bulk_body["failed_checks"].as_u64().unwrap(),
        1,
        "Should have 1 failed check"
    );

    let (not_found_status, _) = api_tests::make_api_request(
        API,
        "GET",
        &format!("/v1/tunnels/{missing_id}/health"),
        None,
    )
    .await?;
    assert_eq!(
        not_found_status, 404,
        "Health check for non-existent tunnel should return 404"
    );

    assert_eq!(
        delete_tunnel(&tunnel_id).await?,
        200,
        "Tunnel deletion should return 200"
    );

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_tunnel_error_scenarios() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    let (invalid_id_status, _) =
        api_tests::make_api_request(API, "GET", "/v1/tunnels/invalid-uuid-format", None).await?;
    assert_eq!(
        invalid_id_status, 400,
        "Invalid tunnel ID should return 400"
    );

    let (not_found_status, _) = api_tests::make_api_request(
        API,
        "GET",
        "/v1/tunnels/00000000-0000-0000-0000-000000000000",
        None,
    )
    .await?;
    assert_eq!(
        not_found_status, 404,
        "Non-existent tunnel should return 404"
    );

    let (empty_bulk_status, _) = api_tests::make_api_request(
        API,
        "POST",
        "/v1/tunnels/health/bulk",
        Some(json!({ "tunnel_ids": [] })),
    )
    .await?;
    assert_eq!(
        empty_bulk_status, 400,
        "Empty tunnel list should return 400"
    );

    let too_many: Vec<String> = (0..101).map(|i| format!("tunnel-{i}")).collect();
    let (too_many_status, _) = api_tests::make_api_request(
        API,
        "POST",
        "/v1/tunnels/health/bulk",
        Some(json!({ "tunnel_ids": too_many })),
    )
    .await?;
    assert_eq!(too_many_status, 400, "Too many tunnels should return 400");

    Ok(())
}

#[tokio::test]
#[ignore = "requires a running FleetingDNS stack (localhost:8880); run via the integration-deploy job or `cargo test -- --ignored` against a live stack"]
async fn test_tunnel_performance_load() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    wait_api_healthy().await?;

    let mut tunnel_ids = Vec::new();
    for _ in 0..5 {
        let (create_status, create_body) = create_tunnel(8080).await?;
        assert_eq!(create_status, 200, "Tunnel creation should return 200");
        tunnel_ids.push(create_body["id"].as_str().expect("tunnel id").to_string());
    }

    let start_time = std::time::Instant::now();
    let (bulk_status, bulk_body) = api_tests::make_api_request(
        API,
        "POST",
        "/v1/tunnels/health/bulk",
        Some(json!({ "tunnel_ids": tunnel_ids.clone() })),
    )
    .await?;
    let response_time = start_time.elapsed();

    assert_eq!(bulk_status, 200, "Bulk health check should return 200");
    assert_eq!(
        bulk_body["total_tunnels"].as_u64().unwrap(),
        tunnel_ids.len() as u64,
        "Should check all tunnels"
    );
    assert_eq!(
        bulk_body["successful_checks"].as_u64().unwrap(),
        tunnel_ids.len() as u64,
        "All checks should succeed"
    );
    assert!(
        response_time < Duration::from_secs(10),
        "Bulk health check should complete within 10 seconds"
    );

    for tunnel_id in &tunnel_ids {
        assert_eq!(
            delete_tunnel(tunnel_id).await?,
            200,
            "Tunnel deletion should return 200"
        );
    }

    Ok(())
}
