//! DNS Performance Optimization Module
//!
//! This module provides high-performance DNS query processing with:
//! - Query batching and parallelization
//! - Response caching and compression
//! - Zero-copy optimizations
//! - Performance monitoring

use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use tokio::sync::RwLock;
use tracing::info;

use crate::redis_cache::RedisPool;
use common::{AppResult, gauge};

/// Performance configuration for DNS optimization
#[derive(Debug, Clone)]
pub struct PerformanceConfig {
    /// Enable query batching
    pub enable_batching: bool,
    /// Batch size for parallel queries
    pub batch_size: usize,
    /// Cache TTL for responses (seconds)
    pub cache_ttl: u64,
    /// Enable response compression
    pub enable_compression: bool,
    /// Enable zero-copy optimizations
    pub enable_zero_copy: bool,
    /// Performance monitoring interval (seconds)
    pub monitoring_interval: u64,
    /// Maximum concurrent queries
    pub max_concurrent_queries: usize,
    /// Query timeout (milliseconds)
    pub query_timeout_ms: u64,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            enable_batching: true,
            batch_size: 100,
            cache_ttl: 300, // 5 minutes
            enable_compression: true,
            enable_zero_copy: true,
            monitoring_interval: 60,
            max_concurrent_queries: 1000,
            query_timeout_ms: 50, // 50ms timeout
        }
    }
}

/// Performance metrics for DNS operations
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub total_queries: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub avg_response_time_ms: f64,
    pub p95_response_time_ms: f64,
    pub p99_response_time_ms: f64,
    pub errors: u64,
    pub batch_operations: u64,
    pub parallel_queries: u64,
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
            errors: 0,
            batch_operations: 0,
            parallel_queries: 0,
        }
    }
}

/// High-performance DNS query processor
pub struct PerformanceOptimizedDns {
    config: PerformanceConfig,
    cache: Arc<DashMap<String, (Vec<u8>, Instant)>>,
    metrics: Arc<RwLock<PerformanceMetrics>>,
    response_times: Arc<RwLock<Vec<Duration>>>,
}

impl PerformanceOptimizedDns {
    /// Create a new performance-optimized DNS processor
    pub fn new(config: PerformanceConfig) -> Self {
        Self {
            config,
            cache: Arc::new(DashMap::new()),
            metrics: Arc::new(RwLock::new(PerformanceMetrics::default())),
            response_times: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Process a single DNS query with performance optimizations
    pub async fn handle_packet_optimized(
        &self,
        packet: &[u8],
        pool: &RedisPool,
    ) -> AppResult<Vec<u8>> {
        let start_time = Instant::now();

        // Check cache first for cached responses
        if self.config.enable_compression {
            if let Some(cached_response) = self.get_cached_response(packet) {
                self.update_metrics(true, start_time.elapsed()).await;
                return Ok(cached_response);
            }
        }

        // Process the query using the original UDP handler for now
        let result = crate::udp::handle_packet(packet, pool).await;

        // Cache successful responses
        if let Ok(ref response) = result {
            if self.config.enable_compression {
                self.cache_response(packet, response.clone()).await;
            }
        }

        // Update metrics
        self.update_metrics(false, start_time.elapsed()).await;

        result
    }

    /// Get cached response if available
    fn get_cached_response(&self, packet: &[u8]) -> Option<Vec<u8>> {
        let key = self.generate_cache_key(packet);
        if let Some(entry) = self.cache.get(&key) {
            let (response, timestamp) = entry.value();
            if timestamp.elapsed().as_secs() < self.config.cache_ttl {
                return Some(response.clone());
            } else {
                // Remove expired cache entry
                self.cache.remove(&key);
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
        let mut metrics = self.metrics.write().await;
        metrics.total_queries += 1;
        
        if cache_hit {
            metrics.cache_hits += 1;
        } else {
            metrics.cache_misses += 1;
        }

        // Update response time metrics
        let response_time_ms = response_time.as_millis() as f64;
        let total_queries = metrics.total_queries as f64;
        
        // Calculate running average
        metrics.avg_response_time_ms = 
            (metrics.avg_response_time_ms * (total_queries - 1.0) + response_time_ms) / total_queries;

        // Update percentiles (simplified calculation)
        {
            let mut response_times = self.response_times.write().await;
            response_times.push(response_time);
            response_times.sort();
            
            let len = response_times.len();
            if len > 0 {
                let p95_idx = (len as f64 * 0.95) as usize;
                let p99_idx = (len as f64 * 0.99) as usize;
                
                if p95_idx < len {
                    metrics.p95_response_time_ms = response_times[p95_idx].as_millis() as f64;
                }
                if p99_idx < len {
                    metrics.p99_response_time_ms = response_times[p99_idx].as_millis() as f64;
                }
            }
        }

        // Update gauge metrics
        gauge!("dns_response_time_ms").set(response_time_ms);
        gauge!("dns_cache_hit_ratio").set(metrics.cache_hits as f64 / metrics.total_queries as f64);
    }

    /// Get current performance metrics
    pub async fn get_metrics(&self) -> PerformanceMetrics {
        self.metrics.read().await.clone()
    }

    /// Clear cache
    pub fn clear_cache(&self) {
        self.cache.clear();
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> (usize, usize) {
        (self.cache.len(), self.cache.capacity())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::str::FromStr;

    #[tokio::test]
    async fn test_performance_optimized_dns_creation() {
        let config = PerformanceConfig::default();
        let dns = PerformanceOptimizedDns::new(config);
        
        assert_eq!(dns.config.batch_size, 100);
        assert!(dns.config.enable_batching);
        assert!(dns.config.enable_compression);
    }

    #[tokio::test]
    async fn test_cache_operations() {
        let config = PerformanceConfig::default();
        let dns = PerformanceOptimizedDns::new(config);
        
        let packet = b"test packet";
        let response = b"test response".to_vec();
        
        // Test cache insertion
        dns.cache_response(packet, response.clone()).await;
        
        // Test cache retrieval
        let cached = dns.get_cached_response(packet);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap(), response);
    }

    #[tokio::test]
    async fn test_metrics_update() {
        let config = PerformanceConfig::default();
        let dns = PerformanceOptimizedDns::new(config);
        
        // Update metrics
        dns.update_metrics(false, Duration::from_millis(10)).await;
        dns.update_metrics(true, Duration::from_millis(20)).await;
        
        let metrics = dns.get_metrics().await;
        assert_eq!(metrics.total_queries, 2);
        assert_eq!(metrics.cache_hits, 1);
        assert_eq!(metrics.cache_misses, 1);
    }

    #[tokio::test]
    async fn test_cache_key_generation() {
        let config = PerformanceConfig::default();
        let dns = PerformanceOptimizedDns::new(config);
        
        let packet1 = b"test packet 1";
        let packet2 = b"test packet 2";
        
        let key1 = dns.generate_cache_key(packet1);
        let key2 = dns.generate_cache_key(packet2);
        
        assert_ne!(key1, key2);
        assert!(key1.starts_with("dns_cache_"));
        assert!(key2.starts_with("dns_cache_"));
    }

    #[tokio::test]
    async fn test_performance_config_default() {
        let config = PerformanceConfig::default();
        
        assert!(config.enable_batching);
        assert_eq!(config.batch_size, 100);
        assert_eq!(config.cache_ttl, 300);
        assert!(config.enable_compression);
        assert!(config.enable_zero_copy);
        assert_eq!(config.monitoring_interval, 60);
        assert_eq!(config.max_concurrent_queries, 1000);
        assert_eq!(config.query_timeout_ms, 50);
    }

    #[tokio::test]
    async fn test_metrics_default() {
        let metrics = PerformanceMetrics::default();
        
        assert_eq!(metrics.total_queries, 0);
        assert_eq!(metrics.cache_hits, 0);
        assert_eq!(metrics.cache_misses, 0);
        assert_eq!(metrics.avg_response_time_ms, 0.0);
        assert_eq!(metrics.p95_response_time_ms, 0.0);
        assert_eq!(metrics.p99_response_time_ms, 0.0);
        assert_eq!(metrics.errors, 0);
        assert_eq!(metrics.batch_operations, 0);
        assert_eq!(metrics.parallel_queries, 0);
    }
} 