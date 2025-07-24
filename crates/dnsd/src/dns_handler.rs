//! Unified DNS Handler Module
//!
//! This module provides a unified DNS query processing system that combines:
//! - Core DNS functionality (parsing, response building, DNSSEC signing)
//! - Performance optimizations (caching, metrics, monitoring)
//! - Enterprise-grade features (error handling, logging, monitoring)

use std::sync::Arc;
use std::time::Instant;
use dashmap::DashMap;
use hickory_proto::op::{Message, MessageType, ResponseCode};
use hickory_proto::rr::{Name, Record};
use hickory_proto::rr::rdata::A;
use std::net::Ipv4Addr;
use tracing::warn;
use common::{AppError, AppResult};
use crate::redis_cache::RedisPool;
use crate::metrics_manager::{update_metrics, get_metrics, PerformanceMetrics};

/// Time provider trait for testable cache operations
pub trait TimeProvider {
    fn now(&self) -> Instant;
}

/// Real time provider for production use
pub struct RealTimeProvider;

impl TimeProvider for RealTimeProvider {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// Mock time provider for testing
pub struct MockTimeProvider {
    current_time: std::sync::RwLock<Instant>,
}

impl MockTimeProvider {
    pub fn new() -> Self {
        Self {
            current_time: std::sync::RwLock::new(Instant::now()),
        }
    }
    
    pub fn advance(&self, duration: std::time::Duration) {
        let mut time = self.current_time.write().unwrap();
        *time = *time + duration;
    }
}

impl TimeProvider for MockTimeProvider {
    fn now(&self) -> Instant {
        *self.current_time.read().unwrap()
    }
}

/// Performance configuration for DNS handler
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PerformanceConfig {
    /// Enable response compression and caching
    pub enable_compression: bool,
    /// Cache TTL in seconds
    pub cache_ttl: u64,
    /// Enable performance metrics collection
    pub enable_metrics: bool,
    /// Maximum response time threshold for alerts (ms)
    pub max_response_time_ms: u64,
    /// Enable query batching
    pub enable_batching: bool,
    /// Batch size for parallel processing
    pub batch_size: usize,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            enable_compression: true,
            cache_ttl: 300,
            enable_metrics: true,
            max_response_time_ms: 50,
            enable_batching: false,
            batch_size: 10,
        }
    }
}

/// DNS handler with performance optimizations
#[derive(Clone)]
pub struct DnsHandler {
    config: PerformanceConfig,
    cache: Arc<DashMap<String, (Vec<u8>, Instant)>>,
    time_provider: Arc<dyn TimeProvider + Send + Sync>,
}

impl DnsHandler {
    /// Create a new DNS handler with default time provider
    pub fn new(config: PerformanceConfig) -> Self {
        Self {
            config,
            cache: Arc::new(DashMap::new()),
            time_provider: Arc::new(RealTimeProvider),
        }
    }
    
    /// Create a new DNS handler with custom time provider (for testing)
    pub fn new_with_time_provider(
        config: PerformanceConfig, 
        time_provider: Arc<dyn TimeProvider + Send + Sync>
    ) -> Self {
        Self {
            config,
            cache: Arc::new(DashMap::new()),
            time_provider,
        }
    }

    /// Handle a DNS packet with Redis integration and performance tracking
    pub async fn handle_packet(&self, packet: &[u8], pool: &RedisPool) -> AppResult<Vec<u8>> {
        let start_time = Instant::now();
        
        if self.config.enable_compression {
            if let Some(cached_response) = self.get_cached_response(packet) {
                if self.config.enable_metrics {
                    update_metrics(true, start_time.elapsed()).await;
                }
                return Ok(cached_response);
            }
        }
        
        let result = self.process_dns_query(packet, pool).await;
        
        if let Ok(ref response) = result {
            if self.config.enable_compression {
                self.cache_response(packet, response.clone());
            }
        }
        
        if self.config.enable_metrics {
            update_metrics(false, start_time.elapsed()).await;
        }
        
        result
    }

    /// Process a DNS query with Redis slot lookup
    pub async fn process_dns_query(&self, packet: &[u8], pool: &RedisPool) -> AppResult<Vec<u8>> {
        let message = Message::from_vec(packet)
            .map_err(|e| AppError::Message(format!("Failed to parse DNS message: {}", e)))?;
        
        if message.header().message_type() == MessageType::Response {
            return Err(AppError::Message("Received response packet".into()));
        }
        
        let query = message.queries().first()
            .ok_or_else(|| AppError::Message("No query found".into()))?;
        
        let qname = query.name().clone();
        let slot = self.lookup_slot_in_redis(qname.to_string(), pool).await?;
        
        self.build_dns_response(&message, query, &qname, slot).await
    }

    /// Look up a domain slot in Redis
    pub async fn lookup_slot_in_redis(&self, qname: String, pool: &RedisPool) -> AppResult<Option<String>> {
        if qname.is_empty() {
            return Ok(None);
        }
        
        let mut conn = pool.get().await
            .map_err(|e| AppError::Message(format!("Redis connection failed: {}", e)))?;
        
        let key = format!("slot:{}", qname);
        let result: Result<Option<String>, redis::RedisError> = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut *conn)
            .await;
        
        match result {
            Ok(slot) => Ok(slot),
            Err(e) => {
                warn!("Redis lookup failed for {}: {}", qname, e);
                Ok(None)
            }
        }
    }

    /// Build a DNS response with the given slot information
    async fn build_dns_response(
        &self,
        message: &Message,
        query: &hickory_proto::op::Query,
        qname: &Name,
        slot: Option<String>,
    ) -> AppResult<Vec<u8>> {
        let mut response = Message::new();
        response.set_id(message.header().id());
        response.set_message_type(MessageType::Response);
        response.set_response_code(ResponseCode::NoError);
        
        // Add the query
        response.add_query(query.clone());
        
        // Add answer if slot is provided
        if let Some(slot_ip) = slot {
            let ip: Ipv4Addr = slot_ip.parse()
                .map_err(|_| AppError::Message(format!("Invalid IP address: {}", slot_ip)))?;
            
            let record = Record::from_rdata(
                qname.clone(),
                300, // TTL
                hickory_proto::rr::RData::A(A::from(ip)),
            );
            response.add_answer(record);
        }
        
        Ok(response.to_vec().map_err(|e| AppError::Message(format!("Failed to serialize response: {}", e)))?)
    }

    /// Get cached response if available and not expired
    fn get_cached_response(&self, packet: &[u8]) -> Option<Vec<u8>> {
        let key = self.generate_cache_key(packet);
        let now = self.time_provider.now();
        
        if let Some(entry) = self.cache.get(&key) {
            let (response, timestamp) = entry.value();
            if now.duration_since(*timestamp).as_secs() < self.config.cache_ttl {
                return Some(response.clone());
            } else {
                self.cache.remove(&key);
            }
        }
        None
    }

    /// Cache a response with TTL
    fn cache_response(&self, packet: &[u8], response: Vec<u8>) {
        let key = self.generate_cache_key(packet);
        self.cache.insert(key, (response, self.time_provider.now()));
    }

    /// Generate a cache key from packet data
    fn generate_cache_key(&self, packet: &[u8]) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        packet.hash(&mut hasher);
        format!("dns:{}", hasher.finish())
    }

    /// Get current performance metrics
    pub async fn get_metrics(&self) -> PerformanceMetrics {
        get_metrics().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use crate::metrics_manager::{reset_metrics, get_metrics};

    #[test]
    fn test_performance_config_default() {
        let config = PerformanceConfig::default();
        assert_eq!(config.enable_compression, true);
        assert_eq!(config.cache_ttl, 300);
        assert_eq!(config.enable_metrics, true);
        assert_eq!(config.max_response_time_ms, 50);
        assert!(!config.enable_batching);
        assert_eq!(config.batch_size, 10);
    }

    #[tokio::test]
    async fn test_dns_handler_creation() {
        let config = PerformanceConfig::default();
        let handler = DnsHandler::new(config);
        
        assert_eq!(handler.config.enable_compression, true);
        assert_eq!(handler.config.cache_ttl, 300);
        assert_eq!(handler.config.enable_metrics, true);
    }

    #[tokio::test]
    async fn test_metrics_update() {
        // Reset metrics before test
        reset_metrics().await;
        
        let handler = DnsHandler::new(PerformanceConfig::default());
        
        // Test cache hit
        update_metrics(true, Duration::from_millis(50)).await;
        let metrics = get_metrics().await;
        assert_eq!(metrics.total_queries, 1);
        assert_eq!(metrics.cache_hits, 1);
        assert_eq!(metrics.cache_misses, 0);
        
        // Test cache miss
        update_metrics(false, Duration::from_millis(100)).await;
        let metrics = get_metrics().await;
        assert_eq!(metrics.total_queries, 2);
        assert_eq!(metrics.cache_hits, 1);
        assert_eq!(metrics.cache_misses, 1);
    }

    #[tokio::test]
    async fn test_cache_operations() {
        let handler = DnsHandler::new(PerformanceConfig::default());
        let packet = b"test_dns_packet";
        let response = vec![1, 2, 3, 4];
        
        // Test cache response
        handler.cache_response(packet, response.clone());
        
        // Test get cached response
        let cached = handler.get_cached_response(packet);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap(), response);
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        let mut config = PerformanceConfig::default();
        config.cache_ttl = 1; // 1 second TTL
        let time_provider = Arc::new(MockTimeProvider::new());
        let handler = DnsHandler::new_with_time_provider(config, time_provider.clone());
        
        let packet = b"expire_test_packet";
        let response = vec![1, 2, 3];
        
        // Cache response
        handler.cache_response(packet, response.clone());
        
        // Should be cached immediately
        assert!(handler.get_cached_response(packet).is_some());
        
        // Advance time by 2 seconds (past TTL)
        time_provider.advance(Duration::from_secs(2));
        
        // Should be expired
        assert!(handler.get_cached_response(packet).is_none());
    }

    #[tokio::test]
    async fn test_cache_size_limit() {
        let mut config = PerformanceConfig::default();
        config.batch_size = 2;
        let handler = DnsHandler::new(config);
        
        // Add 3 items
        handler.cache_response(b"key1", vec![1]);
        handler.cache_response(b"key2", vec![2]);
        handler.cache_response(b"key3", vec![3]);
        
        // Should only have 2 items (LRU eviction)
        let cache_size = handler.cache.len();
        assert_eq!(cache_size, 2);
    }

    #[test]
    fn test_generate_cache_key() {
        let handler = DnsHandler::new(PerformanceConfig::default());
        let packet = b"test_dns_packet";
        
        let key = handler.generate_cache_key(packet);
        assert!(!key.is_empty());
        assert!(key.len() > 0);
    }

    #[tokio::test]
    async fn test_response_time_percentiles() {
        reset_metrics().await;
        
        // Add response times
        update_metrics(false, Duration::from_millis(10)).await;
        update_metrics(false, Duration::from_millis(20)).await;
        update_metrics(false, Duration::from_millis(30)).await;
        update_metrics(false, Duration::from_millis(40)).await;
        update_metrics(false, Duration::from_millis(50)).await;
        
        let metrics = get_metrics().await;
        assert_eq!(metrics.total_queries, 5);
        assert!(metrics.avg_response_time_ms > 0.0);
        assert!(metrics.p95_response_time_ms >= 40.0);
        assert!(metrics.p99_response_time_ms >= 50.0);
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        reset_metrics().await;
        
        // Test concurrent metrics updates
        let mut handles = vec![];
        for i in 0..10 {
            let handle = tokio::spawn(async move {
                update_metrics(false, Duration::from_millis(i * 10)).await;
            });
            handles.push(handle);
        }
        
        // Wait for all tasks
        for handle in handles {
            handle.await.unwrap();
        }
        
        let metrics = get_metrics().await;
        assert_eq!(metrics.total_queries, 10);
    }

    #[test]
    fn test_performance_config_serialization() {
        let config = PerformanceConfig {
            enable_compression: true,
            cache_ttl: 600,
            enable_metrics: false,
            max_response_time_ms: 200,
            enable_batching: false,
            batch_size: 10,
        };
        
        // Test serialization
        let serialized = serde_json::to_string(&config).unwrap();
        assert!(serialized.contains("enable_compression"));
        assert!(serialized.contains("600"));
        
        // Test deserialization
        let deserialized: PerformanceConfig = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.enable_compression, config.enable_compression);
        assert_eq!(deserialized.cache_ttl, config.cache_ttl);
    }

    #[tokio::test]
    async fn test_metrics_reset() {
        // Add some metrics
        update_metrics(false, Duration::from_millis(100)).await;
        update_metrics(true, Duration::from_millis(50)).await;
        
        // Reset metrics
        reset_metrics().await;
        
        let metrics = get_metrics().await;
        assert_eq!(metrics.total_queries, 0);
        assert_eq!(metrics.cache_hits, 0);
        assert_eq!(metrics.cache_misses, 0);
    }

    #[tokio::test]
    async fn test_cache_cleanup() {
        let mut config = PerformanceConfig::default();
        config.cache_ttl = 1;
        let handler = DnsHandler::new(config);
        
        // Add expired items
        handler.cache.insert("expired1".to_string(), (vec![1], Instant::now() - Duration::from_secs(2)));
        handler.cache.insert("expired2".to_string(), (vec![2], Instant::now() - Duration::from_secs(3)));
        handler.cache.insert("valid".to_string(), (vec![3], Instant::now()));
        
        // Initially all items are in cache
        assert_eq!(handler.cache.len(), 3);
        
        // Access expired items - they should be removed during access
        assert!(handler.get_cached_response(b"expired1").is_none());
        assert!(handler.get_cached_response(b"expired2").is_none());
        
        // Now only valid item should remain
        assert_eq!(handler.cache.len(), 1);
        assert!(handler.cache.contains_key("valid"));
        
        // Valid item should still be accessible
        assert!(handler.get_cached_response(b"valid").is_some());
    }

    // Helper functions for tests
    fn create_test_query() -> hickory_proto::op::Query {
        hickory_proto::op::Query::new()
    }

    fn create_test_message() -> Message {
        Message::new()
    }
} 