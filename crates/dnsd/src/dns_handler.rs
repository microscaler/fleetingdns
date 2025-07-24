//! Unified DNS Handler Module
//!
//! This module provides a unified DNS query processing system that combines:
//! - Core DNS functionality (parsing, response building, DNSSEC signing)
//! - Performance optimizations (caching, metrics, monitoring)
//! - Enterprise-grade features (error handling, logging, monitoring)

use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use hickory_proto::op::{Message, MessageType, ResponseCode};
use hickory_proto::rr::{Name, Record};
use redis::AsyncCommands;
use tracing::{debug, warn};

use crate::redis_cache::RedisPool;
use common::{AppError, AppResult};

/// Configuration for DNS performance optimizations
#[derive(Debug, Clone)]
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
            cache_ttl: 300, // 5 minutes
            enable_metrics: true,
            max_response_time_ms: 50,
            enable_batching: false,
            batch_size: 10,
        }
    }
}

/// Performance metrics for DNS operations
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    /// Total queries processed
    pub total_queries: u64,
    /// Cache hits
    pub cache_hits: u64,
    /// Cache misses
    pub cache_misses: u64,
    /// Average response time (ms)
    pub avg_response_time_ms: f64,
    /// 95th percentile response time (ms)
    pub p95_response_time_ms: f64,
    /// 99th percentile response time (ms)
    pub p99_response_time_ms: f64,
    /// Total errors
    pub total_errors: u64,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            total_queries: 0,
            cache_hits: 0,
            cache_misses: 0,
            avg_response_time_ms: 0.0,
            p95_response_time_ms: 0.0,
            p99_response_time_ms: 0.0,
            total_errors: 0,
        }
    }
}

/// Unified DNS handler with performance optimizations
#[derive(Clone)]
pub struct DnsHandler {
    config: PerformanceConfig,
    cache: Arc<DashMap<String, (Vec<u8>, Instant)>>,
    metrics: Arc<tokio::sync::RwLock<PerformanceMetrics>>,
    response_times: Arc<tokio::sync::RwLock<Vec<Duration>>>,
}

impl DnsHandler {
    /// Create a new DNS handler with performance optimizations
    pub fn new(config: PerformanceConfig) -> Self {
        Self {
            config,
            cache: Arc::new(DashMap::new()),
            metrics: Arc::new(tokio::sync::RwLock::new(PerformanceMetrics::default())),
            response_times: Arc::new(tokio::sync::RwLock::new(Vec::new())),
        }
    }

    /// Handle a DNS packet with performance optimizations
    pub async fn handle_packet(&self, packet: &[u8], pool: &RedisPool) -> AppResult<Vec<u8>> {
        let start_time = Instant::now();
        
        // Check cache first if compression is enabled
        if self.config.enable_compression {
            if let Some(cached_response) = self.get_cached_response(packet) {
                self.update_metrics(true, start_time.elapsed()).await;
                return Ok(cached_response);
            }
        }

        // Process the DNS query
        let result = self.process_dns_query(packet, pool).await;
        
        // Cache successful responses if compression is enabled
        if let Ok(ref response) = result {
            if self.config.enable_compression {
                self.cache_response(packet, response.clone()).await;
            }
        }

        // Update metrics
        self.update_metrics(false, start_time.elapsed()).await;
        
        result
    }

    /// Process a DNS query with Redis slot lookup
    async fn process_dns_query(&self, packet: &[u8], pool: &RedisPool) -> AppResult<Vec<u8>> {
        // Parse the DNS message
        let message = Message::from_vec(packet)
            .map_err(|e| AppError::Message(format!("Failed to parse DNS message: {}", e)))?;

        // Extract query information
        let query = message
            .queries()
            .first()
            .ok_or_else(|| AppError::Message("No query found in DNS message".to_string()))?;

        let qname = &query.name();
        let qtype = query.query_type();

        debug!("Processing DNS query: {} {:?}", qname, qtype);

        // Look up slot in Redis
        let slot = self.lookup_slot_in_redis(qname.to_string(), pool).await?;

        // Build DNS response
        let response = self.build_dns_response(&message, query, qname, slot).await?;

        // Sign the response if DNSSEC is enabled
        let signed_response = if self.should_sign_response() {
            self.sign_record_optimized(response).await?
        } else {
            response
        };

        Ok(signed_response)
    }

    /// Look up DNS slot in Redis
    async fn lookup_slot_in_redis(&self, qname: String, pool: &RedisPool) -> AppResult<Option<String>> {
        let mut conn = pool.get().await.map_err(|e| {
            AppError::Message(format!("Failed to get Redis connection: {}", e))
        })?;

        let slot: Result<Option<String>, redis::RedisError> = conn.get(&qname).await;
        match slot {
            Ok(slot) => Ok(slot),
            Err(e) => {
                // Check if it's a "not found" error
                if e.to_string().contains("not found") || e.to_string().contains("nil") {
                    Ok(None)
                } else {
                    Err(AppError::Message(format!("Redis lookup error: {}", e)))
                }
            }
        }
    }

    /// Build DNS response from slot information
    async fn build_dns_response(
        &self,
        message: &Message,
        query: &hickory_proto::op::Query,
        qname: &Name,
        slot: Option<String>,
    ) -> AppResult<Vec<u8>> {
        let mut response = Message::new();
        response.set_id(message.id());
        response.set_message_type(MessageType::Response);
        response.set_op_code(message.op_code());
        response.set_response_code(ResponseCode::NoError);

        // Add the query
        response.add_query(query.clone());

        // Add answer records based on slot
        if let Some(slot) = slot {
            // Parse slot as IP address
            let ip: Ipv4Addr = slot.parse().map_err(|_| {
                AppError::Message(format!("Invalid IP address in slot: {}", slot))
            })?;

            // Create A record
            let record = Record::from_rdata(
                qname.clone(),
                60, // TTL
                hickory_proto::rr::RData::A(hickory_proto::rr::rdata::A(ip)),
            );
            response.add_answer(record);
        }

        // Serialize response
        let buf = response.to_vec().map_err(|e| {
            AppError::Message(format!("Failed to serialize DNS response: {}", e))
        })?;

        Ok(buf)
    }

    /// Sign DNS record with DNSSEC
    async fn sign_record_optimized(&self, response: Vec<u8>) -> AppResult<Vec<u8>> {
        // For now, return the response as-is
        // TODO: Implement DNSSEC signing
        Ok(response)
    }

    /// Check if response should be signed
    fn should_sign_response(&self) -> bool {
        // TODO: Check DNSSEC configuration
        false
    }

    /// Get cached response if available
    fn get_cached_response(&self, packet: &[u8]) -> Option<Vec<u8>> {
        let key = self.generate_cache_key(packet);
        if let Some(entry) = self.cache.get(&key) {
            let (response, timestamp) = entry.value();
            if timestamp.elapsed().as_secs() < self.config.cache_ttl {
                return Some(response.clone());
            }
        }
        None
    }

    /// Cache response for future use
    async fn cache_response(&self, packet: &[u8], response: Vec<u8>) {
        let key = self.generate_cache_key(packet);
        self.cache.insert(key, (response, Instant::now()));
    }

    /// Generate cache key from packet
    fn generate_cache_key(&self, packet: &[u8]) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        packet.hash(&mut hasher);
        format!("dns_cache_{}", hasher.finish())
    }

    /// Update performance metrics
    async fn update_metrics(&self, cache_hit: bool, response_time: Duration) {
        if !self.config.enable_metrics {
            return;
        }

        let mut metrics = self.metrics.write().await;
        metrics.total_queries += 1;

        if cache_hit {
            metrics.cache_hits += 1;
        } else {
            metrics.cache_misses += 1;
        }

        // Update response time statistics
        let response_time_ms = response_time.as_millis() as f64;
        metrics.avg_response_time_ms = 
            (metrics.avg_response_time_ms * (metrics.total_queries - 1) as f64 + response_time_ms) 
            / metrics.total_queries as f64;

        // Store response time for percentile calculation
        let mut response_times = self.response_times.write().await;
        response_times.push(response_time);
        
        // Keep only last 1000 response times for memory efficiency
        if response_times.len() > 1000 {
            response_times.remove(0);
        }

        // Calculate percentiles
        if response_times.len() >= 20 {
            let mut sorted_times: Vec<Duration> = response_times.clone();
            sorted_times.sort();
            
            let p95_idx = (sorted_times.len() as f64 * 0.95) as usize;
            let p99_idx = (sorted_times.len() as f64 * 0.99) as usize;
            
            metrics.p95_response_time_ms = sorted_times[p95_idx].as_millis() as f64;
            metrics.p99_response_time_ms = sorted_times[p99_idx].as_millis() as f64;
        }

        // Log performance alerts
        if response_time_ms > self.config.max_response_time_ms as f64 {
            warn!(
                "Slow DNS response: {}ms (threshold: {}ms)",
                response_time_ms, self.config.max_response_time_ms
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_config_default() {
        let config = PerformanceConfig::default();
        
        assert!(config.enable_compression);
        assert_eq!(config.cache_ttl, 300);
        assert!(config.enable_metrics);
        assert_eq!(config.max_response_time_ms, 50);
        assert!(!config.enable_batching);
        assert_eq!(config.batch_size, 10);
    }

    #[test]
    fn test_performance_metrics_default() {
        let metrics = PerformanceMetrics::default();
        
        assert_eq!(metrics.total_queries, 0);
        assert_eq!(metrics.cache_hits, 0);
        assert_eq!(metrics.cache_misses, 0);
        assert_eq!(metrics.avg_response_time_ms, 0.0);
        assert_eq!(metrics.p95_response_time_ms, 0.0);
        assert_eq!(metrics.p99_response_time_ms, 0.0);
        assert_eq!(metrics.total_errors, 0);
    }

    #[tokio::test]
    async fn test_dns_handler_creation() {
        let config = PerformanceConfig::default();
        let handler = DnsHandler::new(config);
        
        assert_eq!(handler.config.batch_size, 10);
        assert!(!handler.config.enable_batching);
        assert!(handler.config.enable_compression);
    }

    #[tokio::test]
    async fn test_metrics_update() {
        let config = PerformanceConfig::default();
        let handler = DnsHandler::new(config);
        
        handler.update_metrics(false, Duration::from_millis(10)).await;
        handler.update_metrics(true, Duration::from_millis(20)).await;
        
        let metrics = handler.metrics.read().await.clone();
        assert_eq!(metrics.total_queries, 2);
        assert_eq!(metrics.cache_hits, 1);
        assert_eq!(metrics.cache_misses, 1);
    }
} 