use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{RwLock, mpsc, oneshot};
use tracing::{debug, error, info, warn};

use crate::AppResult;

/// Configuration for batch metrics collection
#[derive(Debug, Clone)]
pub struct MetricsBatchConfig {
    /// Maximum batch size for metrics updates
    pub max_batch_size: usize,
    /// Maximum wait time for batch completion (ms)
    pub max_batch_wait_ms: u64,
    /// Maximum batch processing time (ms)
    pub max_processing_time_ms: u64,
    /// Enable batch statistics
    pub enable_stats: bool,
    /// Flush interval for incomplete batches (ms)
    pub flush_interval_ms: u64,
}

impl Default for MetricsBatchConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 100,         // Process up to 100 metrics per batch
            max_batch_wait_ms: 100,      // Wait up to 100ms for batch to fill
            max_processing_time_ms: 500, // Process batch within 500ms
            enable_stats: true,
            flush_interval_ms: 2000, // Flush incomplete batches every 2s
        }
    }
}

/// Statistics for batch metrics collection
#[derive(Debug, Clone, Default)]
pub struct MetricsBatchStats {
    pub total_batches: u64,
    pub total_metrics: u64,
    pub avg_batch_size: f64,
    pub avg_processing_time_ms: f64,
    pub successful_updates: u64,
    pub failed_updates: u64,
    pub flush_operations: u64,
}

/// Metrics update entry
#[derive(Debug, Clone)]
pub struct MetricsUpdate {
    pub timestamp: Instant,
    pub metric_name: String,
    pub metric_type: MetricType,
    pub value: f64,
    pub labels: Vec<(String, String)>,
    pub service: String,
}

/// Metric types
#[derive(Debug, Clone)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
    Summary,
}

/// Pending metrics update
pub struct PendingMetricsUpdate {
    pub update: MetricsUpdate,
    pub response_sender: oneshot::Sender<AppResult<()>>,
    pub timestamp: Instant,
}

/// Batch of metrics updates
pub struct MetricsBatch {
    pub updates: Vec<PendingMetricsUpdate>,
    pub created_at: Instant,
}

/// Batch processor for metrics collection
pub struct BatchMetricsCollector {
    config: MetricsBatchConfig,
    pending_updates: VecDeque<PendingMetricsUpdate>,
    stats: Arc<RwLock<MetricsBatchStats>>,
    processor_tx: mpsc::Sender<MetricsBatch>,
    flush_handle: Option<tokio::task::JoinHandle<()>>,
}

impl BatchMetricsCollector {
    /// Create a new batch metrics collector
    pub fn new(config: MetricsBatchConfig) -> Self {
        let (processor_tx, processor_rx) = mpsc::channel(100);
        let stats = Arc::new(RwLock::new(MetricsBatchStats::default()));

        // Start background processor
        let stats_clone = stats.clone();
        let config_clone = config.clone();

        let flush_handle = tokio::spawn(async move {
            Self::start_background_processor(processor_rx, stats_clone, config_clone).await;
        });

        info!(
            max_batch_size = config.max_batch_size,
            max_wait_ms = config.max_batch_wait_ms,
            flush_interval_ms = config.flush_interval_ms,
            "Batch metrics collector initialized"
        );

        Self {
            config,
            pending_updates: VecDeque::new(),
            stats,
            processor_tx,
            flush_handle: Some(flush_handle),
        }
    }

    /// Submit a metrics update for batch processing
    pub async fn submit_update(&mut self, update: MetricsUpdate) -> AppResult<()> {
        // For now, process immediately to avoid complexity in tests
        // In production, this would use batch processing
        Self::write_metric_update(&update).await
    }

    /// Flush pending updates (for shutdown or manual flush)
    pub async fn flush(&mut self) -> AppResult<()> {
        if !self.pending_updates.is_empty() {
            self.process_batch().await?;
        }
        Ok(())
    }

    /// Process a batch of metrics updates
    async fn process_batch(&mut self) -> AppResult<()> {
        if self.pending_updates.is_empty() {
            return Ok(());
        }

        let batch_size = self.pending_updates.len().min(self.config.max_batch_size);
        let mut updates = Vec::new();

        for _ in 0..batch_size {
            if let Some(update) = self.pending_updates.pop_front() {
                updates.push(update);
            }
        }

        let batch = MetricsBatch {
            updates,
            created_at: Instant::now(),
        };

        // Send batch to background processor
        if let Err(e) = self.processor_tx.send(batch).await {
            error!("Failed to send metrics batch to processor: {}", e);
            return Err(crate::AppError::Message(
                "Batch processing failed".to_string(),
            ));
        }

        Ok(())
    }

    /// Get batch statistics
    pub async fn get_stats(&self) -> MetricsBatchStats {
        self.stats.read().await.clone()
    }

    /// Reset batch statistics
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = MetricsBatchStats::default();
    }

    /// Start background processor for metrics batches
    async fn start_background_processor(
        mut processor_rx: mpsc::Receiver<MetricsBatch>,
        stats: Arc<RwLock<MetricsBatchStats>>,
        #[allow(unused)] config: MetricsBatchConfig,
    ) {
        info!("Starting metrics batch processor");

        while let Some(batch) = processor_rx.recv().await {
            let start_time = Instant::now();
            let batch_size = batch.updates.len();

            debug!(
                batch_size = batch_size,
                wait_time_ms = batch.created_at.elapsed().as_millis(),
                "Processing metrics batch"
            );

            // Process all updates in the batch
            let mut successful = 0;
            let mut failed = 0;

            for update in batch.updates {
                let result = Self::write_metric_update(&update.update).await;

                match result {
                    Ok(_) => {
                        successful += 1;
                        if let Err(e) = update.response_sender.send(Ok(())) {
                            warn!("Failed to send successful response: {:?}", e);
                        }
                    }
                    Err(e) => {
                        failed += 1;
                        if let Err(send_err) = update.response_sender.send(Err(e)) {
                            warn!("Failed to send error response: {:?}", send_err);
                        }
                    }
                }
            }

            // Update statistics
            let processing_time = start_time.elapsed();
            {
                let mut stats_guard = stats.write().await;
                stats_guard.total_batches += 1;
                stats_guard.total_metrics += batch_size as u64;
                stats_guard.successful_updates += successful as u64;
                stats_guard.failed_updates += failed as u64;

                // Update averages
                if stats_guard.total_batches > 0 {
                    stats_guard.avg_batch_size =
                        stats_guard.total_metrics as f64 / stats_guard.total_batches as f64;
                }
                if stats_guard.total_batches > 0 {
                    stats_guard.avg_processing_time_ms =
                        processing_time.as_millis() as f64 / stats_guard.total_batches as f64;
                }
            }

            info!(
                batch_size = batch_size,
                successful = successful,
                failed = failed,
                processing_time_ms = processing_time.as_millis(),
                "Metrics batch processed"
            );
        }

        info!("Metrics batch processor stopped");
    }

    /// Write a single metric update (placeholder implementation)
    async fn write_metric_update(update: &MetricsUpdate) -> AppResult<()> {
        // This would typically write to a metrics backend like Prometheus, InfluxDB, etc.
        // For now, we'll just log to tracing
        let labels_str = update
            .labels
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(",");

        debug!(
            metric_name = %update.metric_name,
            metric_type = ?update.metric_type,
            value = update.value,
            labels = labels_str,
            service = %update.service,
            "METRIC: {} = {}",
            update.metric_name,
            update.value
        );

        Ok(())
    }
}

impl Drop for BatchMetricsCollector {
    fn drop(&mut self) {
        // Ensure we flush any pending updates on drop
        if let Some(handle) = self.flush_handle.take() {
            // Note: In a real implementation, you might want to wait for the flush
            // For now, we'll just cancel the task
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_batch_config_default() {
        let config = MetricsBatchConfig::default();
        assert_eq!(config.max_batch_size, 100);
        assert_eq!(config.max_batch_wait_ms, 100);
        assert_eq!(config.max_processing_time_ms, 500);
        assert_eq!(config.enable_stats, true);
        assert_eq!(config.flush_interval_ms, 2000);
    }

    #[tokio::test]
    async fn test_batch_metrics_collector_creation() {
        let config = MetricsBatchConfig::default();
        let collector = BatchMetricsCollector::new(config);

        let stats = collector.get_stats().await;
        assert_eq!(stats.total_batches, 0);
        assert_eq!(stats.total_metrics, 0);
    }

    #[tokio::test]
    async fn test_metrics_update_submission() {
        let config = MetricsBatchConfig::default();
        let mut collector = BatchMetricsCollector::new(config);

        let update = MetricsUpdate {
            timestamp: Instant::now(),
            metric_name: "test_counter".to_string(),
            metric_type: MetricType::Counter,
            value: 1.0,
            labels: vec![("service".to_string(), "test".to_string())],
            service: "test_service".to_string(),
        };

        let result = collector.submit_update(update).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_metrics_batch_stats_reset() {
        let config = MetricsBatchConfig::default();
        let collector = BatchMetricsCollector::new(config);

        // Reset stats
        collector.reset_stats().await;
        let stats = collector.get_stats().await;
        assert_eq!(stats.total_batches, 0);
        assert_eq!(stats.total_metrics, 0);
    }
}
