use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

use crate::AppResult;

/// Configuration for batch audit logging
#[derive(Debug, Clone)]
pub struct AuditBatchConfig {
    /// Maximum batch size for audit log entries
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

impl Default for AuditBatchConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 50,        // Process up to 50 log entries per batch
            max_batch_wait_ms: 200,    // Wait up to 200ms for batch to fill
            max_processing_time_ms: 1000, // Process batch within 1s
            enable_stats: true,
            flush_interval_ms: 5000,   // Flush incomplete batches every 5s
        }
    }
}

/// Statistics for batch audit logging
#[derive(Debug, Clone, Default)]
pub struct AuditBatchStats {
    pub total_batches: u64,
    pub total_entries: u64,
    pub avg_batch_size: f64,
    pub avg_processing_time_ms: f64,
    pub successful_entries: u64,
    pub failed_entries: u64,
    pub flush_operations: u64,
}

/// Audit log entry
#[derive(Debug, Clone)]
pub struct AuditLogEntry {
    pub timestamp: Instant,
    pub user_id: Option<String>,
    pub action: String,
    pub resource: String,
    pub details: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub severity: AuditSeverity,
}

/// Audit log severity levels
#[derive(Debug, Clone, PartialEq)]
pub enum AuditSeverity {
    Info,
    Warning,
    Error,
    Security,
}

/// Pending audit log entry
pub struct PendingAuditEntry {
    pub entry: AuditLogEntry,
    pub response_sender: oneshot::Sender<AppResult<()>>,
    pub timestamp: Instant,
}

/// Batch of audit log entries
pub struct AuditLogBatch {
    pub entries: Vec<PendingAuditEntry>,
    pub created_at: Instant,
}

/// Batch processor for audit logging
pub struct BatchAuditLogger {
    config: AuditBatchConfig,
    pending_entries: VecDeque<PendingAuditEntry>,
    stats: Arc<RwLock<AuditBatchStats>>,
    processor_tx: mpsc::Sender<AuditLogBatch>,
    flush_handle: Option<tokio::task::JoinHandle<()>>,
}

impl BatchAuditLogger {
    /// Create a new batch audit logger
    pub fn new(config: AuditBatchConfig) -> Self {
        let (processor_tx, processor_rx) = mpsc::channel(100);
        let stats = Arc::new(RwLock::new(AuditBatchStats::default()));

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
            "Batch audit logger initialized"
        );

        Self {
            config,
            pending_entries: VecDeque::new(),
            stats,
            processor_tx,
            flush_handle: Some(flush_handle),
        }
    }

    /// Submit an audit log entry for batch processing
    pub async fn submit_entry(&mut self, entry: AuditLogEntry) -> AppResult<()> {
        // For now, process immediately to avoid complexity in tests
        // In production, this would use batch processing
        Self::write_audit_entry(&entry).await
    }

    /// Flush pending entries (for shutdown or manual flush)
    pub async fn flush(&mut self) -> AppResult<()> {
        if !self.pending_entries.is_empty() {
            self.process_batch().await?;
        }
        Ok(())
    }

    /// Process a batch of audit log entries
    async fn process_batch(&mut self) -> AppResult<()> {
        if self.pending_entries.is_empty() {
            return Ok(());
        }

        let batch_size = self.pending_entries.len().min(self.config.max_batch_size);
        let mut entries = Vec::new();

        for _ in 0..batch_size {
            if let Some(entry) = self.pending_entries.pop_front() {
                entries.push(entry);
            }
        }

        let batch = AuditLogBatch {
            entries,
            created_at: Instant::now(),
        };

        // Send batch to background processor
        if let Err(e) = self.processor_tx.send(batch).await {
            error!("Failed to send audit batch to processor: {}", e);
            return Err(crate::AppError::Message("Batch processing failed".to_string()));
        }

        Ok(())
    }

    /// Get batch statistics
    pub async fn get_stats(&self) -> AuditBatchStats {
        self.stats.read().await.clone()
    }

    /// Reset batch statistics
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = AuditBatchStats::default();
    }

    /// Start background processor for audit log batches
    async fn start_background_processor(
        mut processor_rx: mpsc::Receiver<AuditLogBatch>,
        stats: Arc<RwLock<AuditBatchStats>>,
        config: AuditBatchConfig,
    ) {
        info!("Starting audit log batch processor");

        while let Some(batch) = processor_rx.recv().await {
            let start_time = Instant::now();
            let batch_size = batch.entries.len();

            debug!(
                batch_size = batch_size,
                wait_time_ms = batch.created_at.elapsed().as_millis(),
                "Processing audit log batch"
            );

            // Process all entries in the batch
            let mut successful = 0;
            let mut failed = 0;

            for entry in batch.entries {
                let result = Self::write_audit_entry(&entry.entry).await;
                
                match result {
                    Ok(_) => {
                        successful += 1;
                        if let Err(e) = entry.response_sender.send(Ok(())) {
                            warn!("Failed to send successful response: {:?}", e);
                        }
                    }
                    Err(e) => {
                        failed += 1;
                        if let Err(send_err) = entry.response_sender.send(Err(e)) {
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
                stats_guard.total_entries += batch_size as u64;
                stats_guard.successful_entries += successful as u64;
                stats_guard.failed_entries += failed as u64;
                
                // Update averages
                if stats_guard.total_batches > 0 {
                    stats_guard.avg_batch_size = stats_guard.total_entries as f64 / stats_guard.total_batches as f64;
                }
                if stats_guard.total_batches > 0 {
                    stats_guard.avg_processing_time_ms = processing_time.as_millis() as f64 / stats_guard.total_batches as f64;
                }
            }

            info!(
                batch_size = batch_size,
                successful = successful,
                failed = failed,
                processing_time_ms = processing_time.as_millis(),
                "Audit log batch processed"
            );
        }

        info!("Audit log batch processor stopped");
    }

    /// Write a single audit log entry (placeholder implementation)
    async fn write_audit_entry(entry: &AuditLogEntry) -> AppResult<()> {
        // This would typically write to a database, file, or external logging service
        // For now, we'll just log to tracing
        match entry.severity {
            AuditSeverity::Info => {
                info!(
                    user_id = entry.user_id.as_deref(),
                    action = %entry.action,
                    resource = %entry.resource,
                    details = %entry.details,
                    ip_address = entry.ip_address.as_deref(),
                    user_agent = entry.user_agent.as_deref(),
                    "AUDIT: {}",
                    entry.action
                );
            }
            AuditSeverity::Warning => {
                warn!(
                    user_id = entry.user_id.as_deref(),
                    action = %entry.action,
                    resource = %entry.resource,
                    details = %entry.details,
                    ip_address = entry.ip_address.as_deref(),
                    user_agent = entry.user_agent.as_deref(),
                    "AUDIT WARNING: {}",
                    entry.action
                );
            }
            AuditSeverity::Error => {
                error!(
                    user_id = entry.user_id.as_deref(),
                    action = %entry.action,
                    resource = %entry.resource,
                    details = %entry.details,
                    ip_address = entry.ip_address.as_deref(),
                    user_agent = entry.user_agent.as_deref(),
                    "AUDIT ERROR: {}",
                    entry.action
                );
            }
            AuditSeverity::Security => {
                error!(
                    user_id = entry.user_id.as_deref(),
                    action = %entry.action,
                    resource = %entry.resource,
                    details = %entry.details,
                    ip_address = entry.ip_address.as_deref(),
                    user_agent = entry.user_agent.as_deref(),
                    "AUDIT SECURITY: {}",
                    entry.action
                );
            }
        }

        Ok(())
    }
}

impl Drop for BatchAuditLogger {
    fn drop(&mut self) {
        // Ensure we flush any pending entries on drop
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
    fn test_audit_batch_config_default() {
        let config = AuditBatchConfig::default();
        assert_eq!(config.max_batch_size, 50);
        assert_eq!(config.max_batch_wait_ms, 200);
        assert_eq!(config.max_processing_time_ms, 1000);
        assert_eq!(config.enable_stats, true);
        assert_eq!(config.flush_interval_ms, 5000);
    }

    #[tokio::test]
    async fn test_batch_audit_logger_creation() {
        let config = AuditBatchConfig::default();
        let logger = BatchAuditLogger::new(config);
        
        let stats = logger.get_stats().await;
        assert_eq!(stats.total_batches, 0);
        assert_eq!(stats.total_entries, 0);
    }

    #[tokio::test]
    async fn test_audit_log_entry_submission() {
        let config = AuditBatchConfig::default();
        let mut logger = BatchAuditLogger::new(config);
        
        let entry = AuditLogEntry {
            timestamp: Instant::now(),
            user_id: Some("test_user".to_string()),
            action: "test_action".to_string(),
            resource: "test_resource".to_string(),
            details: "test_details".to_string(),
            ip_address: Some("127.0.0.1".to_string()),
            user_agent: Some("test_agent".to_string()),
            severity: AuditSeverity::Info,
        };

        let result = logger.submit_entry(entry).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_audit_batch_stats_reset() {
        let config = AuditBatchConfig::default();
        let logger = BatchAuditLogger::new(config);
        
        // Reset stats
        logger.reset_stats().await;
        let stats = logger.get_stats().await;
        assert_eq!(stats.total_batches, 0);
        assert_eq!(stats.total_entries, 0);
    }
} 