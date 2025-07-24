use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use std::collections::VecDeque;
use tracing::warn;

/// Singleton metrics manager for DNS performance tracking
pub struct MetricsManager {
    total_queries: u64,
    cache_hits: u64,
    cache_misses: u64,
    avg_response_time_ms: f64,
    p95_response_time_ms: f64,
    p99_response_time_ms: f64,
    total_errors: u64,
    response_times: VecDeque<Duration>,
    max_response_time_ms: u64,
    max_response_times: usize,
}

impl MetricsManager {
    pub fn new(max_response_time_ms: u64) -> Self {
        Self {
            total_queries: 0,
            cache_hits: 0,
            cache_misses: 0,
            avg_response_time_ms: 0.0,
            p95_response_time_ms: 0.0,
            p99_response_time_ms: 0.0,
            total_errors: 0,
            response_times: VecDeque::with_capacity(1000),
            max_response_time_ms,
            max_response_times: 1000,
        }
    }

    pub fn update_metrics(&mut self, cache_hit: bool, response_time: Duration) {
        self.total_queries += 1;

        if cache_hit {
            self.cache_hits += 1;
        } else {
            self.cache_misses += 1;
        }

        // Update response time statistics
        let response_time_ms = response_time.as_millis() as f64;
        self.avg_response_time_ms = 
            (self.avg_response_time_ms * (self.total_queries - 1) as f64 + response_time_ms) 
            / self.total_queries as f64;

        // Store response time for percentile calculation
        self.response_times.push_back(response_time);
        
        // Keep only last N response times for memory efficiency
        if self.response_times.len() > self.max_response_times {
            self.response_times.pop_front();
        }

        // Calculate percentiles
        if self.response_times.len() >= 20 {
            let mut sorted_times: Vec<Duration> = self.response_times.iter().copied().collect();
            sorted_times.sort();
            
            let p95_idx = (sorted_times.len() as f64 * 0.95) as usize;
            let p99_idx = (sorted_times.len() as f64 * 0.99) as usize;
            
            self.p95_response_time_ms = sorted_times[p95_idx].as_millis() as f64;
            self.p99_response_time_ms = sorted_times[p99_idx].as_millis() as f64;
        }

        // Log performance alerts
        if response_time_ms > self.max_response_time_ms as f64 {
            warn!(
                "Slow DNS response: {}ms (threshold: {}ms)",
                response_time_ms, self.max_response_time_ms
            );
        }
    }

    pub fn get_metrics(&self) -> PerformanceMetrics {
        PerformanceMetrics {
            total_queries: self.total_queries,
            cache_hits: self.cache_hits,
            cache_misses: self.cache_misses,
            avg_response_time_ms: self.avg_response_time_ms,
            p95_response_time_ms: self.p95_response_time_ms,
            p99_response_time_ms: self.p99_response_time_ms,
            total_errors: self.total_errors,
        }
    }

    pub fn reset(&mut self) {
        self.total_queries = 0;
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.avg_response_time_ms = 0.0;
        self.p95_response_time_ms = 0.0;
        self.p99_response_time_ms = 0.0;
        self.total_errors = 0;
        self.response_times.clear();
    }
}

/// Global singleton instance
static METRICS_MANAGER: once_cell::sync::Lazy<Arc<RwLock<MetricsManager>>> = 
    once_cell::sync::Lazy::new(|| {
        Arc::new(RwLock::new(MetricsManager::new(50))) // Default 50ms threshold
    });

/// Initialize the metrics manager with custom settings
pub fn init_metrics_manager(max_response_time_ms: u64) {
    let mut manager = METRICS_MANAGER.blocking_write();
    *manager = MetricsManager::new(max_response_time_ms);
}

/// Update metrics safely using the singleton
pub async fn update_metrics(cache_hit: bool, response_time: Duration) {
    let mut manager = METRICS_MANAGER.write().await;
    manager.update_metrics(cache_hit, response_time);
}

/// Get current metrics safely
pub async fn get_metrics() -> PerformanceMetrics {
    let manager = METRICS_MANAGER.read().await;
    manager.get_metrics()
}

/// Reset metrics safely
pub async fn reset_metrics() {
    let mut manager = METRICS_MANAGER.write().await;
    manager.reset();
}

/// Performance metrics structure
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub total_queries: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub avg_response_time_ms: f64,
    pub p95_response_time_ms: f64,
    pub p99_response_time_ms: f64,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_manager_singleton() {
        // Reset metrics before test
        reset_metrics().await;
        
        // Test metrics update
        update_metrics(true, Duration::from_millis(50)).await;
        update_metrics(false, Duration::from_millis(100)).await;
        
        let metrics = get_metrics().await;
        assert_eq!(metrics.total_queries, 2);
        assert_eq!(metrics.cache_hits, 1);
        assert_eq!(metrics.cache_misses, 1);
        assert!(metrics.avg_response_time_ms > 0.0);
    }

    #[tokio::test]
    async fn test_metrics_reset() {
        // Add some metrics
        update_metrics(false, Duration::from_millis(100)).await;
        update_metrics(true, Duration::from_millis(50)).await;
        
        // Reset
        reset_metrics().await;
        
        let metrics = get_metrics().await;
        assert_eq!(metrics.total_queries, 0);
        assert_eq!(metrics.cache_hits, 0);
        assert_eq!(metrics.cache_misses, 0);
    }

    #[tokio::test]
    async fn test_concurrent_metrics_updates() {
        reset_metrics().await;
        
        // Test concurrent updates
        let mut handles = vec![];
        for i in 0..10 {
            let handle = tokio::spawn(async move {
                update_metrics(false, Duration::from_millis(i * 10)).await;
            });
            handles.push(handle);
        }
        
        for handle in handles {
            handle.await.unwrap();
        }
        
        let metrics = get_metrics().await;
        assert_eq!(metrics.total_queries, 10);
    }
} 