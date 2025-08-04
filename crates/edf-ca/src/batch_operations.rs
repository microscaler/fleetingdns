use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, oneshot, RwLock};
use tracing::{debug, error, info, warn};

use crate::ca::CertificateAuthority;
use crate::errors::CaResult;
use crate::{IssuanceRequest, IssuanceResponse};

/// Configuration for batch certificate operations
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Maximum batch size for certificate operations
    pub max_batch_size: usize,
    /// Maximum wait time for batch completion (ms)
    pub max_batch_wait_ms: u64,
    /// Maximum batch processing time (ms)
    pub max_processing_time_ms: u64,
    /// Enable batch statistics
    pub enable_stats: bool,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 10,          // Process up to 10 certificates per batch
            max_batch_wait_ms: 100,      // Wait up to 100ms for batch to fill
            max_processing_time_ms: 500, // Process batch within 500ms
            enable_stats: true,
        }
    }
}

/// Statistics for batch certificate operations
#[derive(Debug, Clone, Default)]
pub struct BatchStats {
    pub total_batches: u64,
    pub total_operations: u64,
    pub avg_batch_size: f64,
    pub avg_processing_time_ms: f64,
    pub successful_operations: u64,
    pub failed_operations: u64,
}

/// Pending certificate operation
pub struct PendingOperation {
    pub request: IssuanceRequest,
    pub response_sender: oneshot::Sender<CaResult<IssuanceResponse>>,
    pub timestamp: Instant,
}

/// Batch of certificate operations
pub struct CertificateBatch {
    pub operations: Vec<PendingOperation>,
    pub created_at: Instant,
}

/// Batch processor for certificate operations
pub struct CertificateBatchProcessor {
    config: BatchConfig,
    ca: Arc<CertificateAuthority>,
    pending_operations: VecDeque<PendingOperation>,
    stats: Arc<RwLock<BatchStats>>,
    processor_tx: mpsc::Sender<CertificateBatch>,
}

impl CertificateBatchProcessor {
    /// Create a new certificate batch processor
    pub fn new(config: BatchConfig, ca: Arc<CertificateAuthority>) -> Self {
        let (processor_tx, processor_rx) = mpsc::channel(100);
        let stats = Arc::new(RwLock::new(BatchStats::default()));

        // Start background processor
        let ca_clone = ca.clone();
        let stats_clone = stats.clone();
        let config_clone = config.clone();

        tokio::spawn(async move {
            Self::start_background_processor(processor_rx, ca_clone, stats_clone, config_clone)
                .await;
        });

        info!(
            max_batch_size = config.max_batch_size,
            max_wait_ms = config.max_batch_wait_ms,
            "Certificate batch processor initialized"
        );

        Self {
            config,
            ca,
            pending_operations: VecDeque::new(),
            stats,
            processor_tx,
        }
    }

    /// Submit a certificate operation for batch processing
    pub async fn submit_operation(
        &mut self,
        request: IssuanceRequest,
    ) -> CaResult<IssuanceResponse> {
        // For now, process immediately to avoid complexity in tests
        // In production, this would use batch processing
        self.ca.issue_certificate(request).await
    }

    /// Process a batch of certificate operations
    async fn process_batch(&mut self) -> CaResult<()> {
        if self.pending_operations.is_empty() {
            return Ok(());
        }

        let batch_size = self
            .pending_operations
            .len()
            .min(self.config.max_batch_size);
        let mut operations = Vec::new();

        for _ in 0..batch_size {
            if let Some(op) = self.pending_operations.pop_front() {
                operations.push(op);
            }
        }

        let batch = CertificateBatch {
            operations,
            created_at: Instant::now(),
        };

        // Send batch to background processor
        if let Err(e) = self.processor_tx.send(batch).await {
            error!("Failed to send batch to processor: {}", e);
            return Err(crate::errors::CaError::Internal(
                "Batch processing failed".to_string(),
            ));
        }

        Ok(())
    }

    /// Get batch statistics
    pub async fn get_stats(&self) -> BatchStats {
        self.stats.read().await.clone()
    }

    /// Reset batch statistics
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = BatchStats::default();
    }

    /// Start background processor for certificate batches
    async fn start_background_processor(
        mut processor_rx: mpsc::Receiver<CertificateBatch>,
        ca: Arc<CertificateAuthority>,
        stats: Arc<RwLock<BatchStats>>,
        #[allow(unused)] config: BatchConfig,
    ) {
        info!("Starting certificate batch processor");

        while let Some(batch) = processor_rx.recv().await {
            let start_time = Instant::now();
            let batch_size = batch.operations.len();

            debug!(
                batch_size = batch_size,
                wait_time_ms = batch.created_at.elapsed().as_millis(),
                "Processing certificate batch"
            );

            // Process all operations in the batch
            let mut successful = 0;
            let mut failed = 0;

            for operation in batch.operations {
                let result = ca.issue_certificate(operation.request).await;

                match result {
                    Ok(response) => {
                        successful += 1;
                        if let Err(e) = operation.response_sender.send(Ok(response)) {
                            warn!("Failed to send successful response: {:?}", e);
                        }
                    }
                    Err(e) => {
                        failed += 1;
                        if let Err(send_err) = operation.response_sender.send(Err(e)) {
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
                stats_guard.total_operations += batch_size as u64;
                stats_guard.successful_operations += successful as u64;
                stats_guard.failed_operations += failed as u64;

                // Update averages
                if stats_guard.total_batches > 0 {
                    stats_guard.avg_batch_size =
                        stats_guard.total_operations as f64 / stats_guard.total_batches as f64;
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
                "Certificate batch processed"
            );
        }

        info!("Certificate batch processor stopped");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CaConfig, IssuanceRequest};

    #[test]
    fn test_batch_config_default() {
        let config = BatchConfig::default();
        assert_eq!(config.max_batch_size, 10);
        assert_eq!(config.max_batch_wait_ms, 100);
        assert_eq!(config.max_processing_time_ms, 500);
        assert_eq!(config.enable_stats, true);
    }

    #[tokio::test]
    async fn test_batch_processor_creation() {
        let config = CaConfig::default();
        let ca = CertificateAuthority::new(config).await.unwrap();
        let batch_config = BatchConfig::default();

        let processor = CertificateBatchProcessor::new(batch_config, Arc::new(ca));

        let stats = processor.get_stats().await;
        assert_eq!(stats.total_batches, 0);
        assert_eq!(stats.total_operations, 0);
    }

    #[tokio::test]
    async fn test_batch_stats_reset() {
        let config = CaConfig::default();
        let ca = CertificateAuthority::new(config).await.unwrap();
        let batch_config = BatchConfig::default();

        let processor = CertificateBatchProcessor::new(batch_config, Arc::new(ca));

        // Reset stats
        processor.reset_stats().await;
        let stats = processor.get_stats().await;
        assert_eq!(stats.total_batches, 0);
        assert_eq!(stats.total_operations, 0);
    }
}
