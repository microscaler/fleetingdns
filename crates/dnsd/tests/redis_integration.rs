use dnsd::dns_handler::{DnsHandler, PerformanceConfig};
use common::redis::RedisPool;
mod redis_test_utils;
use redis_test_utils::{with_redis_container, with_shared_redis_container};

#[tokio::test]
async fn test_invalid_data_handling() {
    let result = with_redis_container(|pool| async move {
        let handler = DnsHandler::new(PerformanceConfig::default());

        // Test empty domain
        let result = handler.lookup_slot_in_redis("".to_string(), &pool).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        // Test malformed domain
        let result = handler
            .lookup_slot_in_redis("invalid..domain".to_string(), &pool)
            .await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        // Test domain with invalid characters
        let result = handler
            .lookup_slot_in_redis("test@domain.com".to_string(), &pool)
            .await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        "success"
    })
    .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_different_domain_formats() {
    let result = with_redis_container(|pool| async move {
        let handler = DnsHandler::new(PerformanceConfig::default());

        // Test various domain formats
        let domains = vec![
            "simple.com",
            "sub.domain.com",
            "deep.sub.domain.com",
            "domain-with-dashes.com",
            "domain_with_underscores.com",
            "domain123.com",
            "123domain.com",
        ];

        for domain in domains {
            let result = handler
                .lookup_slot_in_redis(domain.to_string(), &pool)
                .await;
            assert!(result.is_ok());
            assert!(result.unwrap().is_none());
        }

        "success"
    })
    .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_error_propagation() {
    let result = with_redis_container(|pool| async move {
        let handler = DnsHandler::new(PerformanceConfig::default());

        // Test that invalid queries return valid DNS error responses
        let result = handler.process_dns_query(b"invalid-query", &pool).await;
        assert!(result.is_ok()); // Should return a valid DNS error response

        let response = result.unwrap();
        assert!(!response.is_empty()); // Should contain a DNS error response

        "success"
    })
    .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_pool_statistics() {
    let result = with_redis_container(|pool| async move {
        // Test connection acquisition
        let conn = pool.get().await;
        assert!(conn.is_ok());

        // Test connection release
        drop(conn);

        // Test that we can get another connection
        let conn2 = pool.get().await;
        assert!(conn2.is_ok());

        "success"
    })
    .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_redis_test_utils() {
    let result = with_shared_redis_container(|pool| async move {
        // Test that the shared container approach works
        let mut conn = pool.get().await.unwrap();

        // Test basic Redis operations
                  let _: () = bb8_redis::redis::cmd("SET")
            .arg("shared_test_key")
            .arg("shared_test_value")
            .query_async(&mut *conn)
            .await
            .unwrap();
                  let result: String = bb8_redis::redis::cmd("GET")
            .arg("shared_test_key")
            .query_async(&mut *conn)
            .await
            .unwrap();
        assert_eq!(result, "shared_test_value");

        "success"
    })
    .await;

    assert!(result.is_ok());
}
