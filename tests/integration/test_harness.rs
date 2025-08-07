use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use testcontainers::{clients::Cli, images::redis::Redis, Container, Docker};
use bb8_redis::{bb8, RedisConnectionManager};
use sea_orm::{Database, DatabaseConnection};
use std::time::Duration;

/// Test harness for FleetingDNS integration tests
pub struct FleetingDnsTestHarness {
    /// Redis container for testing
    pub redis_container: Container<'static, Redis>,
    /// PostgreSQL container for testing
    pub postgres_container: Container<'static, postgres::Postgres>,
    /// Redis connection pool
    pub redis_pool: bb8::Pool<RedisConnectionManager>,
    /// Database connection
    pub db_connection: DatabaseConnection,
    /// Test data storage
    pub test_data: Arc<Mutex<HashMap<String, String>>>,
}

impl FleetingDnsTestHarness {
    /// Create a new test harness with all required services
    pub async fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let docker = Cli::default();
        
        // Start Redis container
        let redis_container = docker.run(Redis::default());
        let redis_url = format!("redis://localhost:{}", redis_container.get_host_port_ipv4(6379));
        
        // Start PostgreSQL container
        let postgres_container = docker.run(postgres::Postgres::default());
        let postgres_url = format!(
            "postgresql://postgres:postgres@localhost:{}/testdb",
            postgres_container.get_host_port_ipv4(5432)
        );
        
        // Create Redis connection pool
        let manager = RedisConnectionManager::new(redis_url.clone())?;
        let redis_pool = bb8::Pool::builder()
            .connection_timeout(Duration::from_secs(5))
            .max_size(10)
            .build(manager)
            .await?;
        
        // Create database connection
        let db_connection = Database::connect(&postgres_url).await?;
        
        // Wait for services to be ready
        Self::wait_for_redis_ready(&redis_pool).await?;
        Self::wait_for_postgres_ready(&db_connection).await?;
        
        Ok(Self {
            redis_container,
            postgres_container,
            redis_pool,
            db_connection,
            test_data: Arc::new(Mutex::new(HashMap::new())),
        })
    }
    
    /// Wait for Redis to be ready
    async fn wait_for_redis_ready(pool: &bb8::Pool<RedisConnectionManager>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut retries = 0;
        while retries < 30 {
            match pool.get().await {
                Ok(_) => return Ok(()),
                Err(_) => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    retries += 1;
                }
            }
        }
        Err("Redis failed to become ready".into())
    }
    
    /// Wait for PostgreSQL to be ready
    async fn wait_for_postgres_ready(db: &DatabaseConnection) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut retries = 0;
        while retries < 30 {
            match db.ping().await {
                Ok(_) => return Ok(()),
                Err(_) => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    retries += 1;
                }
            }
        }
        Err("PostgreSQL failed to become ready".into())
    }
    
    /// Set up test data in Redis
    pub async fn setup_test_slots(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = self.redis_pool.get().await?;
        
        // Set up test DNS slots
        let test_slots = vec![
            ("slot:test.fdns.run", "127.0.0.1"),
            ("slot:dev.fdns.run", "127.0.0.1"),
            ("slot:staging.fdns.run", "127.0.0.1"),
        ];
        
        for (key, value) in test_slots {
            redis::cmd("SET").arg(key).arg(value).execute_async(&mut *conn).await?;
        }
        
        Ok(())
    }
    
    /// Clean up test data
    pub async fn cleanup_test_data(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = self.redis_pool.get().await?;
        
        // Clean up test slots
        let test_keys = vec![
            "slot:test.fdns.run",
            "slot:dev.fdns.run", 
            "slot:staging.fdns.run",
        ];
        
        for key in test_keys {
            let _: Result<(), _> = redis::cmd("DEL").arg(key).execute_async(&mut *conn).await;
        }
        
        Ok(())
    }
    
    /// Verify Redis connectivity
    pub async fn verify_redis_connectivity(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = self.redis_pool.get().await?;
        
        // Test basic operations
        redis::cmd("PING").execute_async(&mut *conn).await?;
        redis::cmd("SET").arg("test:key").arg("test:value").execute_async(&mut *conn).await?;
        let value: String = redis::cmd("GET").arg("test:key").query_async(&mut *conn).await?;
        assert_eq!(value, "test:value");
        
        // Clean up test key
        redis::cmd("DEL").arg("test:key").execute_async(&mut *conn).await?;
        
        Ok(())
    }
    
    /// Verify PostgreSQL connectivity
    pub async fn verify_postgres_connectivity(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Test basic database operations
        let result = self.db_connection.ping().await;
        assert!(result.is_ok(), "PostgreSQL ping failed");
        
        Ok(())
    }
    
    /// Get Redis connection pool for testing
    pub async fn get_redis_pool(&self) -> bb8::Pool<RedisConnectionManager> {
        self.redis_pool.clone()
    }
    
    /// Get database connection for testing
    pub fn get_db_connection(&self) -> DatabaseConnection {
        self.db_connection.clone()
    }
}

/// Test context for integration tests
pub struct TestContext {
    pub harness: FleetingDnsTestHarness,
    pub test_name: String,
}

impl TestContext {
    /// Create a new test context
    pub async fn new(test_name: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let harness = FleetingDnsTestHarness::new().await?;
        
        Ok(Self {
            harness,
            test_name: test_name.to_string(),
        })
    }
    
    /// Set up test environment
    pub async fn setup(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Verify service connectivity
        self.harness.verify_redis_connectivity().await?;
        self.harness.verify_postgres_connectivity().await?;
        
        // Set up test data
        self.harness.setup_test_slots().await?;
        
        Ok(())
    }
    
    /// Clean up test environment
    pub async fn cleanup(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Clean up test data
        self.harness.cleanup_test_data().await?;
        
        Ok(())
    }
}

/// Macro for running integration tests with proper setup/teardown
#[macro_export]
macro_rules! integration_test {
    ($test_name:ident, $test_fn:expr) => {
        #[tokio::test]
        async fn $test_name() {
            let mut ctx = TestContext::new(stringify!($test_name))
                .await
                .expect("Failed to create test context");
            
            // Set up test environment
            ctx.setup()
                .await
                .expect("Failed to set up test environment");
            
            // Run the test
            let result = $test_fn(&mut ctx).await;
            
            // Clean up test environment
            ctx.cleanup()
                .await
                .expect("Failed to clean up test environment");
            
            // Return test result
            result.expect("Test failed");
        }
    };
}

/// Health check utilities for integration tests
pub mod health_checks {
    use super::*;
    use std::time::Duration;
    
    /// Check if a service is healthy by making a health request
    pub async fn check_service_health(url: &str, timeout: Duration) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let client = reqwest::Client::new();
        let response = client
            .get(url)
            .timeout(timeout)
            .send()
            .await?;
        
        Ok(response.status().is_success())
    }
    
    /// Wait for a service to become healthy
    pub async fn wait_for_service_health(url: &str, max_wait: Duration) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let start = std::time::Instant::now();
        let check_interval = Duration::from_millis(100);
        
        while start.elapsed() < max_wait {
            if check_service_health(url, Duration::from_secs(1)).await? {
                return Ok(());
            }
            tokio::time::sleep(check_interval).await;
        }
        
        Err(format!("Service at {} did not become healthy within {:?}", url, max_wait).into())
    }
}

/// DNS testing utilities
pub mod dns_tests {
    use super::*;
    use hickory_proto::op::{Message, Query};
    use hickory_proto::rr::{DNSClass, RecordType};
    use std::net::UdpSocket;
    
    /// Send a DNS query and get the response
    pub async fn send_dns_query(
        server_addr: &str,
        domain: &str,
        record_type: RecordType,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_read_timeout(Some(Duration::from_secs(5)))?;
        
        // Create DNS query
        let mut message = Message::new();
        let mut query = Query::new();
        query.set_name(domain.parse()?);
        query.set_query_type(record_type);
        query.set_query_class(DNSClass::IN);
        message.add_query(query);
        
        // Send query
        let query_data = message.to_vec()?;
        socket.send_to(&query_data, server_addr)?;
        
        // Receive response
        let mut buffer = vec![0u8; 512];
        let (len, _) = socket.recv_from(&mut buffer)?;
        buffer.truncate(len);
        
        Ok(buffer)
    }
    
    /// Verify DNS response contains expected data
    pub fn verify_dns_response(response_data: &[u8], expected_ip: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let message = Message::from_vec(response_data)?;
        
        // Check if response has answers
        if message.answer_count() == 0 {
            return Ok(false);
        }
        
        // Check if any answer contains the expected IP
        for answer in message.answers() {
            if let Some(rdatatype) = answer.data() {
                if rdatatype.to_string().contains(expected_ip) {
                    return Ok(true);
                }
            }
        }
        
        Ok(false)
    }
}

/// API testing utilities
pub mod api_tests {
    use super::*;
    use serde_json::Value;
    
    /// Make an API request and return the response
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
        
        let response = request.send().await?;
        let status = response.status().as_u16();
        let body: Value = response.json().await?;
        
        Ok((status, body))
    }
    
    /// Verify API response structure
    pub fn verify_api_response(response: &Value, expected_fields: &[&str]) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
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

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_harness_creation() {
        let harness = FleetingDnsTestHarness::new().await.unwrap();
        
        // Verify Redis connectivity
        harness.verify_redis_connectivity().await.unwrap();
        
        // Verify PostgreSQL connectivity
        harness.verify_postgres_connectivity().await.unwrap();
    }
    
    #[tokio::test]
    async fn test_test_context() {
        let mut ctx = TestContext::new("test_context").await.unwrap();
        
        // Set up and clean up
        ctx.setup().await.unwrap();
        ctx.cleanup().await.unwrap();
    }
    
    #[tokio::test]
    async fn test_health_checks() {
        // Test health check utilities
        let result = health_checks::check_service_health("http://localhost:8080/health", Duration::from_secs(1)).await;
        // This might fail if API is not running, but the function should work
        assert!(result.is_ok());
    }
} 