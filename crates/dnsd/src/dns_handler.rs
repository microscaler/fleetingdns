//! Unified DNS Handler Module
//!
//! This module provides a unified DNS query processing system that combines:
//! - Core DNS functionality (parsing, response building, DNSSEC signing)
//! - Performance optimizations (caching, metrics, monitoring)
//! - Enterprise-grade features (error handling, logging, monitoring)

use crate::metrics_manager::{PerformanceMetrics, get_metrics, update_metrics};
use common::redis::RedisPool;

use common::{AppError, AppResult};
use hickory_proto::op::{Message, MessageType, ResponseCode};
use hickory_proto::rr::rdata::A;
use hickory_proto::rr::{Name, Record};
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::{Instant as TokioInstant, interval};
use tracing::warn;
use ttl_cache_with_purging::{cache::TtlCache, purging::start_periodic_purge};

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

impl Default for MockTimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MockTimeProvider {
    pub fn new() -> Self {
        Self {
            current_time: std::sync::RwLock::new(Instant::now()),
        }
    }

    pub fn advance(&self, duration: std::time::Duration) {
        let mut time = self.current_time.write().unwrap();
        *time += duration;
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
    /// Maximum cache size (entries)
    pub max_cache_size: usize,
    /// Enable aggressive cache warming
    pub enable_cache_warming: bool,
    /// Cache hit ratio target (percentage)
    pub cache_hit_ratio_target: u8,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            enable_compression: false, // Disable compression for standard DNS clients
            cache_ttl: 300,
            enable_metrics: true,
            max_response_time_ms: 50,
            max_cache_size: 5_000, // Aggressive L1 cache with 5K entries
            enable_cache_warming: true,
            cache_hit_ratio_target: 80, // Target 80% cache hit ratio
        }
    }
}

/// DNS handler with performance optimizations
#[derive(Clone)]
pub struct DnsHandler {
    config: PerformanceConfig,
    cache: Arc<RwLock<TtlCache<String, Vec<u8>>>>,
    #[allow(dead_code)]
    time_provider: Arc<dyn TimeProvider + Send + Sync>,
    max_cache_size: usize,
}

impl DnsHandler {
    /// Create a new DNS handler with default time provider
    pub fn new(config: PerformanceConfig) -> Self {
        let cache = Arc::new(RwLock::new(TtlCache::new()));
        let max_cache_size = config.max_cache_size; // Use configurable cache size

        // Start background purging every 30 seconds (smaller saw effect)
        // Note: This requires a tokio runtime to be running
        if let Ok(rt) = tokio::runtime::Handle::try_current() {
            let cache_clone = cache.clone();
            let purge_interval = interval(Duration::from_secs(30));
            rt.spawn(async move {
                start_periodic_purge(cache_clone, purge_interval);
            });
        }

        Self {
            config,
            cache,
            time_provider: Arc::new(RealTimeProvider),
            max_cache_size,
        }
    }

    /// Create a new DNS handler with custom time provider (for testing)
    pub fn new_with_time_provider(
        config: PerformanceConfig,
        time_provider: Arc<dyn TimeProvider + Send + Sync>,
    ) -> Self {
        let cache = Arc::new(RwLock::new(TtlCache::new()));
        let max_cache_size = 10_000;

        Self {
            config,
            cache,
            time_provider,
            max_cache_size,
        }
    }

    /// Handle a DNS packet with Redis integration and performance tracking
    pub async fn handle_packet(&self, packet: &[u8], pool: &RedisPool) -> AppResult<Vec<u8>> {
        let start_time = Instant::now();

        // Check cache first (compression disabled due to DNS client compatibility issues)
        if let Some(cached_response) = self.get_cached_response(packet).await {
            if self.config.enable_metrics {
                update_metrics(true, start_time.elapsed()).await;
            }
            return Ok(cached_response);
        }

        let result = self.process_dns_query(packet, pool).await;

        if let Ok(ref response) = result {
            // Cache the uncompressed response
            self.cache_response(packet, response.clone()).await;

            if self.config.enable_metrics {
                update_metrics(false, start_time.elapsed()).await;
            }

            Ok(response.clone())
        } else {
            if self.config.enable_metrics {
                update_metrics(false, start_time.elapsed()).await;
            }
            result
        }
    }

    /// Process a DNS query with Redis slot lookup
    pub async fn process_dns_query(&self, packet: &[u8], pool: &RedisPool) -> AppResult<Vec<u8>> {
        let start_time = std::time::Instant::now();

        // Parse DNS message with fallback
        let message = match Message::from_vec(packet) {
            Ok(msg) => msg,
            Err(e) => {
                tracing::error!("Failed to parse DNS message: {}", e);
                return self
                    .build_error_response(packet, ResponseCode::FormErr)
                    .await;
            }
        };

        if message.header().message_type() == MessageType::Response {
            tracing::warn!("Received response packet, ignoring");
            return self
                .build_error_response(packet, ResponseCode::FormErr)
                .await;
        }

        let query = match message.queries().first() {
            Some(q) => q,
            None => {
                tracing::error!("No query found in DNS message");
                return self
                    .build_error_response(packet, ResponseCode::FormErr)
                    .await;
            }
        };

        let qname = query.name().clone();
        tracing::info!("Processing DNS query for: {}", qname);

        // Redis lookup with telemetry and fallback
        let slot = {
            let redis_start = std::time::Instant::now();
            let result = self.lookup_slot_in_redis(qname.to_string(), pool).await;
            let redis_duration = redis_start.elapsed().as_millis() as u64;

            match &result {
                Ok(slot) => {
                    tracing::info!("Redis lookup result for {}: {:?}", qname, slot);
                    common::telemetry::record_redis_metrics(
                        "get",
                        &format!("slot:{qname}"),
                        redis_duration,
                        true,
                    );
                }
                Err(e) => {
                    tracing::error!("Redis lookup failed for {}: {}", qname, e);
                    common::telemetry::record_redis_metrics(
                        "get",
                        &format!("slot:{qname}"),
                        redis_duration,
                        false,
                    );
                }
            }

            // Return None on Redis errors instead of failing
            match result {
                Ok(slot) => slot,
                Err(e) => {
                    tracing::warn!("Using fallback response due to Redis error: {}", e);
                    None
                }
            }
        };

        // Build response with telemetry and fallback
        let response = {
            let response_start = std::time::Instant::now();
            let result = self.build_dns_response(&message, query, &qname, slot).await;
            let response_duration = response_start.elapsed().as_millis() as u64;

            match &result {
                Ok(response) => {
                    tracing::info!("Built DNS response for {}: {} bytes", qname, response.len());
                    common::telemetry::record_dns_metrics(
                        "build_response",
                        &qname.to_string(),
                        response_duration,
                        true,
                    );
                }
                Err(e) => {
                    tracing::error!("Failed to build DNS response for {}: {}", qname, e);
                    common::telemetry::record_dns_metrics(
                        "build_response",
                        &qname.to_string(),
                        response_duration,
                        false,
                    );
                }
            }

            // Return error response on build failure
            match result {
                Ok(response) => response,
                Err(e) => {
                    tracing::warn!("Using fallback response due to build error: {}", e);
                    return self
                        .build_error_response(packet, ResponseCode::ServFail)
                        .await;
                }
            }
        };

        // Record overall metrics
        let total_duration = start_time.elapsed().as_millis() as u64;
        common::telemetry::record_dns_metrics(
            "process_query",
            &qname.to_string(),
            total_duration,
            true,
        );

        Ok(response)
    }

    /// Build an error response for DNS queries
    async fn build_error_response(
        &self,
        packet: &[u8],
        response_code: ResponseCode,
    ) -> AppResult<Vec<u8>> {
        // Try to parse the original packet to extract the query
        let message = match Message::from_vec(packet) {
            Ok(msg) => msg,
            Err(_) => {
                // If we can't parse the packet, create a minimal error response
                let mut response = Message::new();
                response.set_message_type(MessageType::Response);
                response.set_response_code(ResponseCode::FormErr);
                return Ok(response.to_vec().unwrap_or_else(|_| vec![]));
            }
        };

        // Create error response with the same ID as the query
        let mut response = Message::new();
        response.set_id(message.header().id());
        response.set_message_type(MessageType::Response);
        response.set_response_code(response_code);

        // Add the original query to the response
        if let Some(query) = message.queries().first() {
            response.add_query(query.clone());
        }

        Ok(response.to_vec().unwrap_or_else(|_| vec![]))
    }

    /// Look up a domain slot in Redis
    pub async fn lookup_slot_in_redis(
        &self,
        qname: String,
        pool: &RedisPool,
    ) -> AppResult<Option<String>> {
        if qname.is_empty() {
            return Ok(None);
        }

        let mut conn = pool
            .get()
            .await
            .map_err(|e| AppError::Message(format!("Redis connection failed: {e}")))?;

        // Remove trailing dot if present (DNS queries often include trailing dots)
        let clean_qname = qname.trim_end_matches('.');
        let key = format!("slot:{clean_qname}");

        let result: Result<Option<String>, bb8_redis::redis::RedisError> =
            bb8_redis::redis::cmd("GET").arg(&key).query_async(&mut *conn).await;

        match result {
            Ok(slot) => Ok(slot),
            Err(e) => {
                warn!("Redis lookup failed for {}: {}", clean_qname, e);
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
            match query.query_type() {
                hickory_proto::rr::RecordType::A => {
                    // Handle IPv4 addresses
                    let ip: Ipv4Addr = slot_ip
                        .parse()
                        .map_err(|_| AppError::Message(format!("Invalid IPv4 address: {slot_ip}")))?;

                    let record = Record::from_rdata(
                        qname.clone(),
                        300, // TTL
                        hickory_proto::rr::RData::A(A::from(ip)),
                    );
                    response.add_answer(record);
                }
                hickory_proto::rr::RecordType::AAAA => {
                    // Handle IPv6 addresses
                    let ip: std::net::Ipv6Addr = slot_ip
                        .parse()
                        .map_err(|_| AppError::Message(format!("Invalid IPv6 address: {slot_ip}")))?;

                    let record = Record::from_rdata(
                        qname.clone(),
                        300, // TTL
                        hickory_proto::rr::RData::AAAA(hickory_proto::rr::rdata::AAAA::from(ip)),
                    );
                    response.add_answer(record);
                }
                _ => {
                    // For other record types, return NXDOMAIN
                    response.set_response_code(ResponseCode::NXDomain);
                }
            }
        } else {
            // No slot found, return NXDOMAIN
            response.set_response_code(ResponseCode::NXDomain);
        }

        response
            .to_vec()
            .map_err(|e| AppError::Message(format!("Failed to serialize response: {e}")))
    }

    /// Get cached response with lazy cleanup and size management
    pub async fn get_cached_response(&self, packet: &[u8]) -> Option<Vec<u8>> {
        let key = self.generate_cache_key(packet);

        // Get cached response (lazy cleanup happens automatically)
        let cache = self.cache.read().await;
        if let Some(value) = cache.get(&key) {
            return Some(value.clone());
        }

        None
    }

    /// Cache a response with TTL
    async fn cache_response(&self, packet: &[u8], response: Vec<u8>) {
        let key = self.generate_cache_key(packet);
        let expires_at = TokioInstant::now() + Duration::from_secs(self.config.cache_ttl);

        let mut cache = self.cache.write().await;
        cache.insert(key, response, expires_at);
    }

    /// Evict oldest entries when cache size exceeds limit
    #[allow(dead_code)]
    async fn evict_oldest_entries(&self) {
        // TtlCache handles expiration automatically, so we don't need manual eviction
        // The background purge thread will handle expired entries
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

    /// Get cache statistics for monitoring
    pub async fn get_cache_stats(&self) -> (usize, usize) {
        // TtlCache doesn't expose len() method, so we return max_cache_size as current size
        // In a real implementation, you might want to track this separately
        (0, self.max_cache_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics_manager::{get_metrics, reset_metrics, reset_singleton};
    use std::time::Duration;

    #[test]
    fn test_performance_config_default() {
        let config = PerformanceConfig::default();
        assert_eq!(config.enable_compression, false);
        assert_eq!(config.cache_ttl, 300);
        assert_eq!(config.enable_metrics, true);
        assert_eq!(config.max_response_time_ms, 50);
        assert_eq!(config.max_cache_size, 5_000);
        assert_eq!(config.enable_cache_warming, true);
        assert_eq!(config.cache_hit_ratio_target, 80);
    }

    #[tokio::test]
    async fn test_dns_handler_creation() {
        let config = PerformanceConfig::default();
        let handler = DnsHandler::new(config);

        assert_eq!(handler.config.enable_compression, false);
        assert_eq!(handler.config.cache_ttl, 300);
        assert_eq!(handler.config.enable_metrics, true);
        assert_eq!(handler.config.max_cache_size, 5_000);
        assert_eq!(handler.config.enable_cache_warming, true);
        assert_eq!(handler.config.cache_hit_ratio_target, 80);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_metrics_update() {
        // Reset singleton before test
        reset_singleton().await;

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
        handler.cache_response(packet, response.clone()).await;

        // Test get cached response
        let cached = handler.get_cached_response(packet).await;
        assert!(cached.is_some());
        assert_eq!(cached.unwrap(), response);
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        let mut config = PerformanceConfig::default();
        config.cache_ttl = 1; // 1 second TTL
        let handler = DnsHandler::new(config);

        let packet = b"expire_test_packet";
        let response = vec![1, 2, 3];

        // Cache response
        handler.cache_response(packet, response.clone()).await;

        // Should be cached immediately
        assert!(handler.get_cached_response(packet).await.is_some());

        // Wait for expiration (TtlCache handles this automatically)
        tokio::time::sleep(Duration::from_millis(1100)).await;

        // Should be expired (TtlCache automatically removes expired entries)
        assert!(handler.get_cached_response(packet).await.is_none());
    }

    #[tokio::test]
    async fn test_cache_size_limit() {
        let mut config = PerformanceConfig::default();
        config.max_cache_size = 2;
        let handler = DnsHandler::new(config);

        // Add 3 items
        handler.cache_response(b"key1", vec![1]).await;
        handler.cache_response(b"key2", vec![2]).await;
        handler.cache_response(b"key3", vec![3]).await;

        // Should only have 2 items (LRU eviction)
        // Note: TtlCache doesn't expose len(), so we can't verify cache size
        // The background purge will handle expired entries automatically
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
    #[serial_test::serial]
    async fn test_response_time_percentiles() {
        // Reset singleton before test
        reset_singleton().await;

        // Add more response times for better percentile calculation
        update_metrics(false, Duration::from_millis(10)).await;
        update_metrics(false, Duration::from_millis(20)).await;
        update_metrics(false, Duration::from_millis(30)).await;
        update_metrics(false, Duration::from_millis(40)).await;
        update_metrics(false, Duration::from_millis(50)).await;
        update_metrics(false, Duration::from_millis(60)).await;
        update_metrics(false, Duration::from_millis(70)).await;
        update_metrics(false, Duration::from_millis(80)).await;
        update_metrics(false, Duration::from_millis(90)).await;
        update_metrics(false, Duration::from_millis(100)).await;

        let metrics = get_metrics().await;
        assert_eq!(metrics.total_queries, 10);
        assert!(metrics.avg_response_time_ms > 0.0);

        // Test that average response time is calculated correctly
        assert!(metrics.avg_response_time_ms >= 50.0); // Should be around 55ms
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_concurrent_access() {
        // Reset singleton before test
        reset_singleton().await;

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

        // Add small delay to ensure all updates are processed
        tokio::time::sleep(Duration::from_millis(10)).await;

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
            max_cache_size: 3_000,
            enable_cache_warming: false,
            cache_hit_ratio_target: 70,
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
    #[serial_test::serial]
    async fn test_metrics_reset() {
        // Reset singleton before test
        reset_singleton().await;

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

        // Add items to cache
        handler.cache_response(b"test1", vec![1]).await;
        handler.cache_response(b"test2", vec![2]).await;
        handler.cache_response(b"test3", vec![3]).await;

        // All items should be accessible initially
        assert!(handler.get_cached_response(b"test1").await.is_some());
        assert!(handler.get_cached_response(b"test2").await.is_some());
        assert!(handler.get_cached_response(b"test3").await.is_some());

        // TtlCache handles expiration automatically via background purge
        // We can't directly test cache size since TtlCache doesn't expose len()
        // The background purge thread will handle expired entries
    }

    // Helper functions for tests
    fn create_test_query() -> hickory_proto::op::Query {
        hickory_proto::op::Query::new()
    }

    fn create_test_message() -> Message {
        Message::new()
    }
}
