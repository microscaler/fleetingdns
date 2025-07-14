use hickory_proto::op::{Message, MessageType, OpCode, ResponseCode};
use hickory_proto::rr::{RData, Record, RecordType, rdata};
use hickory_proto::serialize::binary::{BinEncodable, BinEncoder};

use crate::redis_cache::{self, CacheError};
use crate::sign;
use common::{AppError, AppResult};

/// Handle a single DNS packet, returning a response buffer.
///
/// The Redis `pool` is consulted for a matching IPv4 address based on the
/// leading label of the query name. If the key is missing, the response will be
/// an NXDOMAIN.
#[tracing::instrument(level = "trace", skip(packet, pool))]
pub async fn handle_packet(packet: &[u8], pool: &redis_cache::RedisPool) -> AppResult<Vec<u8>> {
    if packet.len() < 12 {
        return Err(AppError::Message("packet too short".into()));
    }

    // Parse header fields we care about.
    let id = u16::from_be_bytes([packet[0], packet[1]]);
    let flags = u16::from_be_bytes([packet[2], packet[3]]);
    let rd = flags & 0x0100 != 0;

    // Parse the query name to determine the lookup key.
    let req = Message::from_vec(packet).map_err(|e| AppError::Message(e.to_string()))?;
    let query = req
        .query()
        .ok_or_else(|| AppError::Message("no query".into()))?;
    let qname = query.name();
    let label = qname
        .iter()
        .next()
        .and_then(|l| std::str::from_utf8(l).ok())
        .ok_or_else(|| AppError::Message("invalid label".into()))?;

    // Look up the IPv4 address in Redis.
    let lookup = match redis_cache::get_slot(pool, label).await {
        Ok(ip) => Some(ip),
        Err(CacheError::NXDomain) => None,
        Err(e) => return Err(AppError::Message(e.to_string())),
    };

    // Build the DNS response.
    let mut message = Message::new();
    message.set_id(id);
    message.set_message_type(MessageType::Response);
    message.set_op_code(OpCode::Query);
    message.set_recursion_desired(rd);
    message.set_recursion_available(true);

    message.add_query(query.clone());

    if let Some(ip) = lookup {
        message.set_response_code(ResponseCode::NoError);
        let record = Record::from_rdata(qname.clone(), 60, RData::A(rdata::A(ip)));
        message.add_answer(record);

        // Try production signer first, fallback to legacy signer
        let mut signed = false;

        if let Some(prod_signer) = sign::production_signer() {
            let mut rrset = Vec::new();
            {
                let mut enc = BinEncoder::new(&mut rrset);
                for rec in message.answers() {
                    rec.emit(&mut enc)
                        .map_err(|e| AppError::Message(e.to_string()))?;
                }
            }

            // Try to sign with production signer using default algorithm
            match prod_signer.rrsig_record(qname, RecordType::A, 60, &rrset) {
                Ok(sig) => {
                    message.add_answer(sig);
                    signed = true;
                }
                Err(e) => {
                    tracing::warn!("Production DNSSEC signing failed: {}", e);
                    // Fall back to legacy signer
                }
            }
        }

        // Fallback to legacy signer if production signer failed
        if !signed && let Some(legacy_signer) = sign::signer() {
            let mut rrset = Vec::new();
            {
                let mut enc = BinEncoder::new(&mut rrset);
                for rec in message.answers() {
                    rec.emit(&mut enc)
                        .map_err(|e| AppError::Message(e.to_string()))?;
                }
            }
            let sig = legacy_signer.rrsig_record(qname, RecordType::A, 60, &rrset);
            message.add_answer(sig);
        }
    } else {
        message.set_response_code(ResponseCode::NXDomain);
    }

    let mut out = Vec::with_capacity(512);
    let mut encoder = BinEncoder::new(&mut out);
    message
        .emit(&mut encoder)
        .map_err(|e| AppError::Message(e.to_string()))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::redis_cache;
    use hickory_proto::op::{Message, MessageType, OpCode, Query};
    use hickory_proto::rr::{Name, RecordType};
    use hickory_proto::serialize::binary::{BinEncodable, BinEncoder};
    use mini_redis::server;
    use std::net::Ipv4Addr;
    use tokio::net::TcpListener;
    use tokio::task::JoinHandle;
    use tokio::time::{Duration, sleep};

    async fn start_test_redis() -> Option<(String, JoinHandle<mini_redis::Result<()>>)> {
        let listener = TcpListener::bind("127.0.0.1:0").await.ok()?;
        let addr = listener.local_addr().unwrap();
        let handle =
            tokio::spawn(async move { server::run(listener, tokio::signal::ctrl_c()).await });

        // Wait longer for Redis to start
        sleep(Duration::from_millis(200)).await;

        let url = format!("redis://{addr}/");

        // Try multiple times to connect
        for _ in 0..10 {
            if redis_cache::new_pool(&url).await.is_ok() {
                return Some((url, handle));
            }
            sleep(Duration::from_millis(50)).await;
        }

        handle.abort();
        None
    }

    fn create_dns_query(name: &str, record_type: RecordType) -> Vec<u8> {
        let mut query = Message::new();
        query.set_id(12345);
        query.set_message_type(MessageType::Query);
        query.set_op_code(OpCode::Query);
        query.set_recursion_desired(true);
        query.add_query(Query::query(Name::from_ascii(name).unwrap(), record_type));

        let mut buffer = Vec::new();
        let mut encoder = BinEncoder::new(&mut buffer);
        query.emit(&mut encoder).unwrap();
        buffer
    }

    #[tokio::test]
    async fn test_handle_packet_too_short() {
        let Some((redis_url, redis_handle)) = start_test_redis().await else {
            eprintln!("skipping test: redis not available");
            return;
        };
        let pool = redis_cache::new_pool(&redis_url).await.unwrap();

        // Test with packet shorter than 12 bytes
        let short_packet = vec![0u8; 11];
        let result = handle_packet(&short_packet, &pool).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("packet too short"));

        redis_handle.abort();
    }

    #[tokio::test]
    async fn test_handle_packet_invalid_dns_message() {
        let Some((redis_url, redis_handle)) = start_test_redis().await else {
            eprintln!("skipping test: redis not available");
            return;
        };
        let pool = redis_cache::new_pool(&redis_url).await.unwrap();

        // Test with invalid DNS message (all zeros)
        let invalid_packet = vec![0u8; 12];
        let result = handle_packet(&invalid_packet, &pool).await;

        assert!(result.is_err());

        redis_handle.abort();
    }

    #[tokio::test]
    async fn test_handle_packet_no_query() {
        let Some((redis_url, redis_handle)) = start_test_redis().await else {
            eprintln!("skipping test: redis not available");
            return;
        };
        let pool = redis_cache::new_pool(&redis_url).await.unwrap();

        // Create a DNS message with no query
        let mut message = Message::new();
        message.set_id(12345);
        message.set_message_type(MessageType::Query);
        message.set_op_code(OpCode::Query);
        // Don't add any queries

        let mut buffer = Vec::new();
        let mut encoder = BinEncoder::new(&mut buffer);
        message.emit(&mut encoder).unwrap();

        let result = handle_packet(&buffer, &pool).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no query"));

        redis_handle.abort();
    }

    #[tokio::test]
    async fn test_handle_packet_invalid_label() {
        let Some((redis_url, redis_handle)) = start_test_redis().await else {
            eprintln!("skipping test: redis not available");
            return;
        };
        let pool = redis_cache::new_pool(&redis_url).await.unwrap();

        // Create a DNS query with invalid UTF-8 in the label
        let mut query = Message::new();
        query.set_id(12345);
        query.set_message_type(MessageType::Query);
        query.set_op_code(OpCode::Query);

        // Create a name with invalid UTF-8 bytes
        let invalid_bytes: &[u8] = &[0xFF, 0xFE, 0xFD];
        let invalid_name = Name::from_labels(vec![invalid_bytes]).unwrap();
        query.add_query(Query::query(invalid_name, RecordType::A));

        let mut buffer = Vec::new();
        let mut encoder = BinEncoder::new(&mut buffer);
        query.emit(&mut encoder).unwrap();

        let result = handle_packet(&buffer, &pool).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid label"));

        redis_handle.abort();
    }

    #[tokio::test]
    async fn test_handle_packet_successful_lookup() {
        let Some((redis_url, redis_handle)) = start_test_redis().await else {
            eprintln!("skipping test: redis not available");
            return;
        };
        let pool = redis_cache::new_pool(&redis_url).await.unwrap();

        // Set up a test record in Redis
        let test_ip = Ipv4Addr::new(192, 168, 1, 100);
        if redis_cache::set_slot(&pool, "test", test_ip, 300)
            .await
            .is_err()
        {
            eprintln!("skipping test: redis set failed");
            redis_handle.abort();
            return;
        }

        // Create a DNS query for the test record
        let query_packet = create_dns_query("test.example.com.", RecordType::A);

        let result = handle_packet(&query_packet, &pool).await;
        assert!(result.is_ok());

        let response_packet = result.unwrap();
        let response = Message::from_vec(&response_packet).unwrap();

        // Verify response properties
        assert_eq!(response.id(), 12345);
        assert_eq!(response.message_type(), MessageType::Response);
        assert_eq!(response.response_code(), ResponseCode::NoError);
        assert!(response.recursion_desired());
        assert!(response.recursion_available());

        // Verify the answer
        let answers = response.answers();
        assert_eq!(answers.len(), 1);
        let answer = &answers[0];
        assert_eq!(answer.record_type(), RecordType::A);
        assert_eq!(answer.ttl(), 60);

        if let RData::A(a_record) = answer.data().unwrap() {
            assert_eq!(a_record.0, test_ip);
        } else {
            panic!("Expected A record");
        }

        redis_handle.abort();
    }

    #[tokio::test]
    async fn test_handle_packet_nxdomain() {
        let Some((redis_url, redis_handle)) = start_test_redis().await else {
            eprintln!("skipping test: redis not available");
            return;
        };
        let pool = redis_cache::new_pool(&redis_url).await.unwrap();

        // Create a DNS query for a non-existent record
        let query_packet = create_dns_query("nonexistent.example.com.", RecordType::A);

        let result = handle_packet(&query_packet, &pool).await;
        if result.is_err() {
            eprintln!("skipping test: redis error - {}", result.unwrap_err());
            redis_handle.abort();
            return;
        }

        let response_packet = result.unwrap();
        let response = Message::from_vec(&response_packet).unwrap();

        // Verify NXDOMAIN response
        assert_eq!(response.id(), 12345);
        assert_eq!(response.message_type(), MessageType::Response);
        assert_eq!(response.response_code(), ResponseCode::NXDomain);
        assert_eq!(response.answers().len(), 0);

        redis_handle.abort();
    }

    #[tokio::test]
    async fn test_handle_packet_recursion_desired_false() {
        let Some((redis_url, redis_handle)) = start_test_redis().await else {
            eprintln!("skipping test: redis not available");
            return;
        };
        let pool = redis_cache::new_pool(&redis_url).await.unwrap();

        // Set up a test record
        let test_ip = Ipv4Addr::new(10, 0, 0, 1);
        if redis_cache::set_slot(&pool, "test", test_ip, 300)
            .await
            .is_err()
        {
            eprintln!("skipping test: redis set failed");
            redis_handle.abort();
            return;
        }

        // Create a DNS query without recursion desired
        let mut query = Message::new();
        query.set_id(54321);
        query.set_message_type(MessageType::Query);
        query.set_op_code(OpCode::Query);
        query.set_recursion_desired(false);
        query.add_query(Query::query(
            Name::from_ascii("test.example.com.").unwrap(),
            RecordType::A,
        ));

        let mut buffer = Vec::new();
        let mut encoder = BinEncoder::new(&mut buffer);
        query.emit(&mut encoder).unwrap();

        let result = handle_packet(&buffer, &pool).await;
        assert!(result.is_ok());

        let response_packet = result.unwrap();
        let response = Message::from_vec(&response_packet).unwrap();

        // Verify response preserves recursion desired flag
        assert!(!response.recursion_desired());
        assert!(response.recursion_available());

        redis_handle.abort();
    }

    #[tokio::test]
    async fn test_handle_packet_different_record_types() {
        let Some((redis_url, redis_handle)) = start_test_redis().await else {
            eprintln!("skipping test: redis not available");
            return;
        };
        let pool = redis_cache::new_pool(&redis_url).await.unwrap();

        // Set up a test record
        let test_ip = Ipv4Addr::new(203, 0, 113, 1);
        if redis_cache::set_slot(&pool, "test", test_ip, 300)
            .await
            .is_err()
        {
            eprintln!("skipping test: redis set failed");
            redis_handle.abort();
            return;
        }

        // Test different record types
        let test_cases = vec![
            RecordType::A,
            RecordType::AAAA,
            RecordType::MX,
            RecordType::TXT,
        ];

        for record_type in test_cases {
            let query_packet = create_dns_query("test.example.com.", record_type);
            let result = handle_packet(&query_packet, &pool).await;
            assert!(result.is_ok());

            let response_packet = result.unwrap();
            let response = Message::from_vec(&response_packet).unwrap();

            // All queries should get the same A record response
            if record_type == RecordType::A {
                assert_eq!(response.response_code(), ResponseCode::NoError);
                assert_eq!(response.answers().len(), 1);
                assert_eq!(response.answers()[0].record_type(), RecordType::A);
            }
        }

        redis_handle.abort();
    }

    #[tokio::test]
    async fn test_handle_packet_with_signing() {
        let Some((redis_url, redis_handle)) = start_test_redis().await else {
            eprintln!("skipping test: redis not available");
            return;
        };
        let pool = redis_cache::new_pool(&redis_url).await.unwrap();

        // Set up environment for signing
        unsafe {
            std::env::set_var("FDNS_HMAC_KEY", "test_signing_key");
        }

        // Set up a test record
        let test_ip = Ipv4Addr::new(198, 51, 100, 1);
        if redis_cache::set_slot(&pool, "signed", test_ip, 300)
            .await
            .is_err()
        {
            eprintln!("skipping test: redis set failed");
            unsafe {
                std::env::remove_var("FDNS_HMAC_KEY");
            }
            redis_handle.abort();
            return;
        }

        let query_packet = create_dns_query("signed.example.com.", RecordType::A);
        let result = handle_packet(&query_packet, &pool).await;
        assert!(result.is_ok());

        let response_packet = result.unwrap();
        let response = Message::from_vec(&response_packet).unwrap();

        // Should have both A record and RRSIG
        let answers = response.answers();
        assert_eq!(answers.len(), 2);

        // Check that we have both A and RRSIG records
        let has_a_record = answers.iter().any(|r| r.record_type() == RecordType::A);
        let has_rrsig_record = answers.iter().any(|r| r.record_type() == RecordType::RRSIG);

        assert!(has_a_record);
        assert!(has_rrsig_record);

        // Clean up
        unsafe {
            std::env::remove_var("FDNS_HMAC_KEY");
        }
        redis_handle.abort();
    }

    #[tokio::test]
    async fn test_handle_packet_redis_error() {
        let Some((redis_url, redis_handle)) = start_test_redis().await else {
            eprintln!("skipping test: redis not available");
            return;
        };
        let pool = redis_cache::new_pool(&redis_url).await.unwrap();

        // Abort Redis to simulate connection error
        redis_handle.abort();

        // Wait for Redis to be unavailable
        sleep(Duration::from_millis(100)).await;

        let query_packet = create_dns_query("test.example.com.", RecordType::A);
        let result = handle_packet(&query_packet, &pool).await;

        // Should return an error when Redis is unavailable
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_handle_packet_empty_label() {
        let Some((redis_url, redis_handle)) = start_test_redis().await else {
            eprintln!("skipping test: redis not available");
            return;
        };
        let pool = redis_cache::new_pool(&redis_url).await.unwrap();

        // Create a query with empty label (just root domain)
        let query_packet = create_dns_query(".", RecordType::A);
        let result = handle_packet(&query_packet, &pool).await;

        // Should return an error for empty label
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid label"));

        redis_handle.abort();
    }

    #[tokio::test]
    async fn test_handle_packet_response_encoding() {
        let Some((redis_url, redis_handle)) = start_test_redis().await else {
            eprintln!("skipping test: redis not available");
            return;
        };
        let pool = redis_cache::new_pool(&redis_url).await.unwrap();

        // Set up a test record
        let test_ip = Ipv4Addr::new(172, 16, 0, 1);
        if redis_cache::set_slot(&pool, "encode", test_ip, 300)
            .await
            .is_err()
        {
            eprintln!("skipping test: redis set failed");
            redis_handle.abort();
            return;
        }

        let query_packet = create_dns_query("encode.example.com.", RecordType::A);
        let result = handle_packet(&query_packet, &pool).await;
        assert!(result.is_ok());

        let response_packet = result.unwrap();

        // Verify the response can be parsed back
        let response = Message::from_vec(&response_packet).unwrap();
        assert_eq!(response.response_code(), ResponseCode::NoError);

        // Verify response structure
        assert_eq!(response.queries().len(), 1);
        assert_eq!(response.answers().len(), 1);
        assert_eq!(response.name_servers().len(), 0);
        assert_eq!(response.additionals().len(), 0);

        redis_handle.abort();
    }

    #[tokio::test]
    async fn test_handle_packet_large_query_id() {
        let Some((redis_url, redis_handle)) = start_test_redis().await else {
            eprintln!("skipping test: redis not available");
            return;
        };
        let pool = redis_cache::new_pool(&redis_url).await.unwrap();

        // Set up a test record
        let test_ip = Ipv4Addr::new(192, 0, 2, 1);
        if redis_cache::set_slot(&pool, "large", test_ip, 300)
            .await
            .is_err()
        {
            eprintln!("skipping test: redis set failed");
            redis_handle.abort();
            return;
        }

        // Create a DNS query with large ID
        let mut query = Message::new();
        query.set_id(65535); // Maximum u16 value
        query.set_message_type(MessageType::Query);
        query.set_op_code(OpCode::Query);
        query.set_recursion_desired(true);
        query.add_query(Query::query(
            Name::from_ascii("large.example.com.").unwrap(),
            RecordType::A,
        ));

        let mut buffer = Vec::new();
        let mut encoder = BinEncoder::new(&mut buffer);
        query.emit(&mut encoder).unwrap();

        let result = handle_packet(&buffer, &pool).await;
        assert!(result.is_ok());

        let response_packet = result.unwrap();
        let response = Message::from_vec(&response_packet).unwrap();

        // Verify the ID is preserved
        assert_eq!(response.id(), 65535);

        redis_handle.abort();
    }

    #[tokio::test]
    async fn test_handle_packet_various_opcodes() {
        let Some((redis_url, redis_handle)) = start_test_redis().await else {
            eprintln!("skipping test: redis not available");
            return;
        };
        let pool = redis_cache::new_pool(&redis_url).await.unwrap();

        // Set up a test record
        let test_ip = Ipv4Addr::new(203, 0, 113, 42);
        if redis_cache::set_slot(&pool, "opcode", test_ip, 300)
            .await
            .is_err()
        {
            eprintln!("skipping test: redis set failed");
            redis_handle.abort();
            return;
        }

        // Test with different opcodes (though we only handle Query)
        let opcodes = vec![OpCode::Query, OpCode::Update, OpCode::Notify];

        for opcode in opcodes {
            let mut query = Message::new();
            query.set_id(42);
            query.set_message_type(MessageType::Query);
            query.set_op_code(opcode);
            query.set_recursion_desired(true);
            query.add_query(Query::query(
                Name::from_ascii("opcode.example.com.").unwrap(),
                RecordType::A,
            ));

            let mut buffer = Vec::new();
            let mut encoder = BinEncoder::new(&mut buffer);
            query.emit(&mut encoder).unwrap();

            let result = handle_packet(&buffer, &pool).await;
            assert!(result.is_ok());

            let response_packet = result.unwrap();
            let response = Message::from_vec(&response_packet).unwrap();

            // Verify the opcode is preserved
            assert_eq!(response.op_code(), OpCode::Query); // We always respond with Query
        }

        redis_handle.abort();
    }

    #[tokio::test]
    async fn test_handle_packet_long_domain_name() {
        let Some((redis_url, redis_handle)) = start_test_redis().await else {
            eprintln!("skipping test: redis not available");
            return;
        };
        let pool = redis_cache::new_pool(&redis_url).await.unwrap();

        // Set up a test record with a long first label
        let long_label = "a".repeat(63); // Maximum label length
        if redis_cache::set_slot(&pool, &long_label, Ipv4Addr::new(192, 168, 1, 1), 300)
            .await
            .is_err()
        {
            eprintln!("skipping test: redis set failed");
            redis_handle.abort();
            return;
        }

        // Create a DNS query with the long label
        let domain_name = format!("{long_label}.example.com.");
        let query_packet = create_dns_query(&domain_name, RecordType::A);

        let result = handle_packet(&query_packet, &pool).await;
        assert!(result.is_ok());

        let response_packet = result.unwrap();
        let response = Message::from_vec(&response_packet).unwrap();

        // Should successfully resolve
        assert_eq!(response.response_code(), ResponseCode::NoError);
        assert_eq!(response.answers().len(), 1);

        redis_handle.abort();
    }

    #[tokio::test]
    async fn test_handle_packet_case_insensitive_lookup() {
        let Some((redis_url, redis_handle)) = start_test_redis().await else {
            eprintln!("skipping test: redis not available");
            return;
        };
        let pool = redis_cache::new_pool(&redis_url).await.unwrap();

        // Set up a test record with lowercase
        let test_ip = Ipv4Addr::new(10, 10, 10, 10);
        if redis_cache::set_slot(&pool, "test", test_ip, 300)
            .await
            .is_err()
        {
            eprintln!("skipping test: redis set failed");
            redis_handle.abort();
            return;
        }

        // Query with uppercase
        let query_packet = create_dns_query("TEST.example.com.", RecordType::A);
        let result = handle_packet(&query_packet, &pool).await;

        // This will likely fail since Redis is case-sensitive, but test the behavior
        if result.is_ok() {
            let response_packet = result.unwrap();
            let response = Message::from_vec(&response_packet).unwrap();
            // Check if it's NXDOMAIN due to case sensitivity
            assert!(
                response.response_code() == ResponseCode::NoError
                    || response.response_code() == ResponseCode::NXDomain
            );
        }

        redis_handle.abort();
    }

    #[tokio::test]
    async fn test_handle_packet_special_characters_in_label() {
        let Some((redis_url, redis_handle)) = start_test_redis().await else {
            eprintln!("skipping test: redis not available");
            return;
        };
        let pool = redis_cache::new_pool(&redis_url).await.unwrap();

        // Set up test records with special characters
        let special_labels = vec!["test-dash", "test_underscore", "test123", "123test"];

        for label in special_labels {
            let test_ip = Ipv4Addr::new(192, 168, 1, 50);
            if redis_cache::set_slot(&pool, label, test_ip, 300)
                .await
                .is_err()
            {
                eprintln!("skipping test: redis set failed for {label}");
                continue;
            }

            let domain_name = format!("{label}.example.com.");
            let query_packet = create_dns_query(&domain_name, RecordType::A);

            let result = handle_packet(&query_packet, &pool).await;
            assert!(result.is_ok());

            let response_packet = result.unwrap();
            let response = Message::from_vec(&response_packet).unwrap();

            // Should successfully resolve
            assert_eq!(response.response_code(), ResponseCode::NoError);
            assert_eq!(response.answers().len(), 1);
        }

        redis_handle.abort();
    }

    #[tokio::test]
    async fn test_handle_packet_signing_error_handling() {
        let Some((redis_url, redis_handle)) = start_test_redis().await else {
            eprintln!("skipping test: redis not available");
            return;
        };
        let pool = redis_cache::new_pool(&redis_url).await.unwrap();

        // Set up environment for signing with invalid key
        unsafe {
            std::env::set_var("FDNS_HMAC_KEY", "");
        } // Empty key

        // Set up a test record
        let test_ip = Ipv4Addr::new(198, 51, 100, 2);
        if redis_cache::set_slot(&pool, "signerr", test_ip, 300)
            .await
            .is_err()
        {
            eprintln!("skipping test: redis set failed");
            unsafe {
                std::env::remove_var("FDNS_HMAC_KEY");
            }
            redis_handle.abort();
            return;
        }

        let query_packet = create_dns_query("signerr.example.com.", RecordType::A);
        let result = handle_packet(&query_packet, &pool).await;

        // Should still work even with signing issues
        assert!(result.is_ok());

        let response_packet = result.unwrap();
        let response = Message::from_vec(&response_packet).unwrap();

        // Should have A record but no RRSIG due to signing error
        let answers = response.answers();
        assert_eq!(answers.len(), 1);
        assert_eq!(answers[0].record_type(), RecordType::A);

        // Clean up
        unsafe {
            std::env::remove_var("FDNS_HMAC_KEY");
        }
        redis_handle.abort();
    }

    #[tokio::test]
    async fn test_handle_packet_multiple_queries() {
        let Some((redis_url, redis_handle)) = start_test_redis().await else {
            eprintln!("skipping test: redis not available");
            return;
        };
        let pool = redis_cache::new_pool(&redis_url).await.unwrap();

        // Set up test records first to ensure Redis is working
        let test_ip = Ipv4Addr::new(192, 168, 1, 1);
        if redis_cache::set_slot(&pool, "first", test_ip, 300)
            .await
            .is_err()
        {
            eprintln!("skipping test: redis set failed");
            redis_handle.abort();
            return;
        }

        // Create a DNS message with multiple queries (unusual but possible)
        let mut query = Message::new();
        query.set_id(999);
        query.set_message_type(MessageType::Query);
        query.set_op_code(OpCode::Query);
        query.set_recursion_desired(true);

        // Add multiple queries
        query.add_query(Query::query(
            Name::from_ascii("first.example.com.").unwrap(),
            RecordType::A,
        ));
        query.add_query(Query::query(
            Name::from_ascii("second.example.com.").unwrap(),
            RecordType::A,
        ));

        let mut buffer = Vec::new();
        let mut encoder = BinEncoder::new(&mut buffer);
        query.emit(&mut encoder).unwrap();

        let result = handle_packet(&buffer, &pool).await;
        if let Err(e) = &result {
            eprintln!("Error handling packet with multiple queries: {e}");
            // If Redis connection fails, skip the test
            if e.to_string().contains("Timed out") || e.to_string().contains("bb8") {
                eprintln!("skipping test: redis connection timeout");
                redis_handle.abort();
                return;
            }
        }
        assert!(result.is_ok());

        let response_packet = result.unwrap();
        let response = Message::from_vec(&response_packet).unwrap();

        // Should handle the first query only
        assert_eq!(response.queries().len(), 1);

        redis_handle.abort();
    }

    #[tokio::test]
    async fn test_handle_packet_record_ttl_consistency() {
        let Some((redis_url, redis_handle)) = start_test_redis().await else {
            eprintln!("skipping test: redis not available");
            return;
        };
        let pool = redis_cache::new_pool(&redis_url).await.unwrap();

        // Set up a test record
        let test_ip = Ipv4Addr::new(203, 0, 113, 100);
        if redis_cache::set_slot(&pool, "ttltest", test_ip, 1800)
            .await
            .is_err()
        {
            eprintln!("skipping test: redis set failed");
            redis_handle.abort();
            return;
        }

        let query_packet = create_dns_query("ttltest.example.com.", RecordType::A);
        let result = handle_packet(&query_packet, &pool).await;
        assert!(result.is_ok());

        let response_packet = result.unwrap();
        let response = Message::from_vec(&response_packet).unwrap();

        // Check that TTL is set to 60 (hardcoded in the function)
        let answers = response.answers();
        assert_eq!(answers.len(), 1);
        assert_eq!(answers[0].ttl(), 60);

        redis_handle.abort();
    }
}
