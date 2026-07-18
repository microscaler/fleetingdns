#![allow(dead_code)] // shared test-support harness; not every test uses every helper
use bb8_redis::{bb8, RedisConnectionManager};
use sea_orm::{Database, DatabaseConnection};
use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers::ImageExt;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::redis::Redis;

/// Test harness for integration tests
pub struct TestHarness {
    pub redis_container: testcontainers::ContainerAsync<Redis>,
    pub postgres_container: testcontainers::ContainerAsync<Postgres>,
    pub redis_pool: bb8::Pool<RedisConnectionManager>,
    pub db: DatabaseConnection,
}

impl TestHarness {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let redis_container = Redis::default().with_tag("7-alpine").start().await?;
        let redis_port = redis_container
            .get_host_port_ipv4(6379)
            .await
            .expect("Failed to get Redis port");

        let postgres_container = Postgres::default()
            .with_tag("15-alpine")
            .with_env_var("POSTGRES_DB", "testdb")
            .with_env_var("POSTGRES_USER", "postgres")
            .with_env_var("POSTGRES_PASSWORD", "postgres")
            .start()
            .await?;
        let postgres_url = format!(
            "postgresql://postgres:postgres@localhost:{}/testdb",
            postgres_container
                .get_host_port_ipv4(5432)
                .await
                .expect("Failed to get Postgres port")
        );

        tokio::time::sleep(Duration::from_secs(5)).await;

        let redis_url = format!("redis://localhost:{}", redis_port);
        let manager = RedisConnectionManager::new(redis_url)
            .map_err(|e| format!("Failed to create Redis manager: {}", e))?;
        let redis_pool = bb8::Pool::builder()
            .build(manager)
            .await
            .map_err(|e| format!("Failed to create Redis pool: {}", e))?;

        let db_connection = Database::connect(&postgres_url)
            .await
            .map_err(|e| format!("Failed to connect to PostgreSQL: {}", e))?;

        Ok(Self {
            redis_container,
            postgres_container,
            redis_pool,
            db: db_connection,
        })
    }
}

/// Test context for integration tests
pub struct TestContext {
    pub harness: TestHarness,
    pub test_name: String,
}

impl TestContext {
    pub async fn new(test_name: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let harness = TestHarness::new().await?;
        Ok(Self {
            harness,
            test_name: test_name.to_string(),
        })
    }
}

/// Health check utilities
pub mod health_checks {
    use std::time::Duration;

    pub async fn check_service_health(
        url: &str,
        timeout: Duration,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let client = reqwest::Client::new();
        let response = client.get(url).timeout(timeout).send().await?;
        Ok(response.status().is_success())
    }

    pub async fn wait_for_service_health(
        url: &str,
        max_wait: Duration,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let start = std::time::Instant::now();
        let check_interval = Duration::from_millis(100);
        while start.elapsed() < max_wait {
            if check_service_health(url, Duration::from_secs(1)).await? {
                return Ok(());
            }
            tokio::time::sleep(check_interval).await;
        }
        Err(format!(
            "Service at {} did not become healthy within {:?}",
            url, max_wait
        )
        .into())
    }
}

/// DNS testing utilities
pub mod dns_tests {
    use hickory_proto::op::{Message, Query};
    use hickory_proto::rr::{DNSClass, RecordType};
    use std::net::UdpSocket;
    use std::time::Duration;

    /// Address of the dnsd under test. Defaults to the shared kind
    /// cluster's dnsd NodePort (node 172.19.0.2, UDP 30053 — see
    /// llmwiki/concepts/nodeport-mappings-ms02.md). Override with
    /// FDNS_TEST_DNS_ADDR when testing another environment.
    pub fn dns_server_addr() -> String {
        std::env::var("FDNS_TEST_DNS_ADDR").unwrap_or_else(|_| "172.19.0.2:30053".to_string())
    }

    pub async fn send_dns_query(
        server_addr: &str,
        domain: &str,
        record_type: RecordType,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_read_timeout(Some(Duration::from_secs(5)))?;

        let mut message = Message::query();
        let mut query = Query::new();
        query.set_name(domain.parse()?);
        query.set_query_type(record_type);
        query.set_query_class(DNSClass::IN);
        message.add_query(query);

        let query_data = message.to_vec()?;
        socket.send_to(&query_data, server_addr)?;

        let mut buffer = vec![0u8; 512];
        let (len, _) = socket.recv_from(&mut buffer)?;
        buffer.truncate(len);

        Ok(buffer)
    }

    pub fn verify_dns_response(
        response_data: &[u8],
        expected_ip: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let message = Message::from_vec(response_data)?;
        if message.answers.is_empty() {
            return Ok(false);
        }
        for answer in &message.answers {
            // hickory 0.26: Record::data is a public field returning RData.
            if answer.data.to_string().contains(expected_ip) {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// API testing utilities
pub mod api_tests {
    use serde_json::Value;

    pub async fn make_api_request(
        base_url: &str,
        method: &str,
        path: &str,
        body: Option<Value>,
    ) -> Result<(u16, Value), Box<dyn std::error::Error + Send + Sync>> {
        let client = reqwest::Client::new();
        let url = format!("{}{}", base_url, path);

        let request = match method.to_uppercase().as_str() {
            "GET" => client.get(&url),
            "POST" => {
                let mut req = client.post(&url);
                if let Some(body_data) = body {
                    req = req.json(&body_data);
                }
                req
            }
            "PUT" => {
                let mut req = client.put(&url);
                if let Some(body_data) = body {
                    req = req.json(&body_data);
                }
                req
            }
            "DELETE" => client.delete(&url),
            _ => return Err("Unsupported HTTP method".into()),
        };
        // The dev-cluster API runs with DEVELOPMENT_MODE=true. The bypass
        // header authenticates as the dev-bypass user, and the Bearer form
        // lets the rate limiter recognise the bypass token (both are
        // rejected by a production-mode API).
        let request = request
            .header("x-development-bypass", "true")
            .header("authorization", "Bearer dev-bypass-token");

        let response = request.send().await?;
        let status = response.status().as_u16();
        // Not every response carries a JSON body (404 fallbacks, empty
        // bodies); treat those as Null rather than failing the request.
        let body: Value = response.json().await.unwrap_or(Value::Null);

        Ok((status, body))
    }

    pub fn verify_api_response(
        response: &Value,
        expected_fields: &[&str],
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(obj) = response.as_object() {
            for field in expected_fields {
                if !obj.contains_key(*field) {
                    return Ok(false);
                }
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
