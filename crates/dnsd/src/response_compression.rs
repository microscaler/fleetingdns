use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use common::AppResult;

/// Configuration for DNS response compression
#[derive(Debug, Clone)]
pub struct CompressionConfig {
    /// Enable response compression
    pub enable_compression: bool,
    /// Minimum response size to compress (bytes)
    pub min_compress_size: usize,
    /// Compression level (1-9, higher = smaller but slower)
    pub compression_level: u8,
    /// Enable compression statistics
    pub enable_stats: bool,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enable_compression: false, // Disabled due to DNS client compatibility issues
            min_compress_size: 512,    // Only compress responses > 512 bytes
            compression_level: 6,      // Balanced compression level
            enable_stats: true,
        }
    }
}

/// Statistics for response compression
#[derive(Debug, Clone, Default)]
pub struct CompressionStats {
    pub total_responses: u64,
    pub compressed_responses: u64,
    pub total_original_size: u64,
    pub total_compressed_size: u64,
    pub compression_ratio: f64,
}

/// DNS response compressor for individual query optimization
pub struct ResponseCompressor {
    config: CompressionConfig,
    stats: Arc<RwLock<CompressionStats>>,
    // Cache for common compression patterns
    pattern_cache: Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>,
}

impl ResponseCompressor {
    /// Create a new response compressor
    pub fn new(config: CompressionConfig) -> Self {
        info!(
            "Creating ResponseCompressor with compression_level={}, min_size={}",
            config.compression_level, config.min_compress_size
        );

        Self {
            config,
            stats: Arc::new(RwLock::new(CompressionStats::default())),
            pattern_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Compress a DNS response if beneficial
    pub async fn compress_response(&self, response: Vec<u8>) -> AppResult<Vec<u8>> {
        if !self.config.enable_compression {
            return Ok(response);
        }

        // Don't compress small responses
        if response.len() < self.config.min_compress_size {
            self.update_stats(response.len(), response.len(), false)
                .await;
            return Ok(response);
        }

        // Check pattern cache first
        if let Some(cached) = self.get_cached_compression(&response).await {
            self.update_stats(response.len(), cached.len(), true).await;
            return Ok(cached);
        }

        // Compress the response
        match self.compress_data(&response).await {
            Ok(compressed) => {
                let compression_ratio = compressed.len() as f64 / response.len() as f64;

                // Only use compression if it's beneficial (ratio < 0.9)
                if compression_ratio < 0.9 {
                    self.cache_compression_pattern(&response, &compressed).await;
                    self.update_stats(response.len(), compressed.len(), true)
                        .await;
                    debug!(
                        "Compressed DNS response: {} -> {} bytes (ratio: {:.2})",
                        response.len(),
                        compressed.len(),
                        compression_ratio
                    );
                    Ok(compressed)
                } else {
                    // Compression not beneficial, return original
                    self.update_stats(response.len(), response.len(), false)
                        .await;
                    debug!(
                        "Compression not beneficial for {} byte response (ratio: {:.2})",
                        response.len(),
                        compression_ratio
                    );
                    Ok(response)
                }
            }
            Err(e) => {
                warn!(
                    "Compression failed for {} byte response: {}",
                    response.len(),
                    e
                );
                // Return original response on compression failure
                self.update_stats(response.len(), response.len(), false)
                    .await;
                Ok(response)
            }
        }
    }

    /// Decompress a DNS response
    pub async fn decompress_response(&self, compressed: Vec<u8>) -> AppResult<Vec<u8>> {
        if !self.config.enable_compression {
            return Ok(compressed);
        }

        // Check if this is a compressed response (we could add a header flag)
        // For now, we'll assume all responses need decompression if compression is enabled
        match self.decompress_data(&compressed).await {
            Ok(decompressed) => {
                debug!(
                    "Decompressed DNS response: {} -> {} bytes",
                    compressed.len(),
                    decompressed.len()
                );
                Ok(decompressed)
            }
            Err(e) => {
                warn!(
                    "Decompression failed for {} byte response: {}",
                    compressed.len(),
                    e
                );
                // Return original on decompression failure
                Ok(compressed)
            }
        }
    }

    /// Get compression statistics
    pub async fn get_stats(&self) -> CompressionStats {
        self.stats.read().await.clone()
    }

    /// Reset compression statistics
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = CompressionStats::default();
    }

    /// Compress data using zstd
    async fn compress_data(&self, data: &[u8]) -> AppResult<Vec<u8>> {
        // Use tokio::task::spawn_blocking for CPU-intensive compression
        let data = data.to_vec();
        let level = self.config.compression_level;

        tokio::task::spawn_blocking(move || {
            zstd::encode_all(&*data, level as i32)
                .map_err(|e| common::AppError::Message(format!("Compression failed: {e}")))
        })
        .await
        .map_err(|e| common::AppError::Message(format!("Compression task failed: {e}")))?
    }

    /// Decompress data using zstd
    async fn decompress_data(&self, data: &[u8]) -> AppResult<Vec<u8>> {
        let data = data.to_vec();

        tokio::task::spawn_blocking(move || {
            zstd::decode_all(&*data)
                .map_err(|e| common::AppError::Message(format!("Decompression failed: {e}")))
        })
        .await
        .map_err(|e| common::AppError::Message(format!("Decompression task failed: {e}")))?
    }

    /// Get cached compression pattern
    async fn get_cached_compression(&self, response: &[u8]) -> Option<Vec<u8>> {
        let cache = self.pattern_cache.read().await;
        cache.get(response).cloned()
    }

    /// Cache compression pattern
    async fn cache_compression_pattern(&self, original: &[u8], compressed: &[u8]) {
        let mut cache = self.pattern_cache.write().await;

        // Limit cache size to prevent memory issues
        if cache.len() >= 1000 {
            // Remove oldest entries (simple approach)
            let keys: Vec<_> = cache.keys().cloned().collect();
            for key in keys.iter().take(100) {
                cache.remove(key);
            }
        }

        cache.insert(original.to_vec(), compressed.to_vec());
    }

    /// Update compression statistics
    async fn update_stats(
        &self,
        original_size: usize,
        compressed_size: usize,
        was_compressed: bool,
    ) {
        if !self.config.enable_stats {
            return;
        }

        let mut stats = self.stats.write().await;
        stats.total_responses += 1;
        stats.total_original_size += original_size as u64;
        stats.total_compressed_size += compressed_size as u64;

        if was_compressed {
            stats.compressed_responses += 1;
        }

        // Update compression ratio
        if stats.total_original_size > 0 {
            stats.compression_ratio =
                stats.total_compressed_size as f64 / stats.total_original_size as f64;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_config_default() {
        let config = CompressionConfig::default();
        assert!(!config.enable_compression);
        assert_eq!(config.min_compress_size, 512);
        assert_eq!(config.compression_level, 6);
        assert!(config.enable_stats);
    }

    #[tokio::test]
    async fn test_compressor_creation() {
        let config = CompressionConfig::default();
        let compressor = ResponseCompressor::new(config);

        let stats = compressor.get_stats().await;
        assert_eq!(stats.total_responses, 0);
        assert_eq!(stats.compressed_responses, 0);
    }

    #[tokio::test]
    async fn test_compression_disabled() {
        let mut config = CompressionConfig::default();
        config.enable_compression = false;
        let compressor = ResponseCompressor::new(config);

        let test_data = vec![0u8; 1000];
        let compressed = compressor
            .compress_response(test_data.clone())
            .await
            .unwrap();

        // Should return original data when compression is disabled
        assert_eq!(compressed, test_data);
    }

    #[tokio::test]
    async fn test_small_response_no_compression() {
        let config = CompressionConfig::default();
        let compressor = ResponseCompressor::new(config);

        let small_data = vec![0u8; 100]; // Below min_compress_size
        let result = compressor
            .compress_response(small_data.clone())
            .await
            .unwrap();

        // Should return original data for small responses
        assert_eq!(result, small_data);
    }

    #[tokio::test]
    async fn test_compression_stats() {
        let mut config = CompressionConfig::default();
        config.enable_compression = true; // Enable compression for this test
        let compressor = ResponseCompressor::new(config);

        // Compress some data
        let test_data = vec![0u8; 2000]; // Large enough to compress
        let _compressed = compressor.compress_response(test_data).await.unwrap();

        let stats = compressor.get_stats().await;
        assert_eq!(stats.total_responses, 1);
        assert!(stats.compression_ratio > 0.0);
    }

    #[tokio::test]
    async fn test_stats_reset() {
        let config = CompressionConfig::default();
        let compressor = ResponseCompressor::new(config);

        // Add some stats
        let test_data = vec![0u8; 2000];
        let _compressed = compressor.compress_response(test_data).await.unwrap();

        // Reset stats
        compressor.reset_stats().await;
        let stats = compressor.get_stats().await;
        assert_eq!(stats.total_responses, 0);
        assert_eq!(stats.compressed_responses, 0);
    }
}
