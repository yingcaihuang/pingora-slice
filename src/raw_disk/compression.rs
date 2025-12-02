//! Compression support for raw disk cache
//!
//! This module provides transparent compression/decompression using zstd or lz4.

use bytes::Bytes;
use std::io::{Read, Write};
use tracing::debug;

/// Compression algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CompressionAlgorithm {
    /// No compression
    None,
    /// Zstandard compression (good balance of speed and ratio)
    Zstd,
    /// LZ4 compression (very fast, lower ratio)
    Lz4,
}

/// Compression configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CompressionConfig {
    /// Compression algorithm to use
    pub algorithm: CompressionAlgorithm,
    
    /// Compression level (1-22 for zstd, 1-12 for lz4)
    /// Higher = better compression but slower
    pub level: i32,
    
    /// Minimum size threshold for compression (bytes)
    /// Data smaller than this won't be compressed
    pub min_size: usize,
    
    /// Enable compression
    pub enabled: bool,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            algorithm: CompressionAlgorithm::Zstd,
            level: 3, // Default zstd level (good balance)
            min_size: 1024, // Don't compress data < 1KB
            enabled: true,
        }
    }
}

/// Compression statistics
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CompressionStats {
    /// Total bytes compressed
    pub total_compressed_bytes: u64,
    
    /// Total bytes after compression
    pub total_compressed_size: u64,
    
    /// Total bytes decompressed
    pub total_decompressed_bytes: u64,
    
    /// Number of compression operations
    pub compression_count: u64,
    
    /// Number of decompression operations
    pub decompression_count: u64,
    
    /// Number of times compression was skipped (data too small)
    pub skipped_count: u64,
    
    /// Number of times compressed data was larger (stored uncompressed)
    pub expansion_count: u64,
}

impl CompressionStats {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Calculate average compression ratio
    pub fn compression_ratio(&self) -> f64 {
        if self.total_compressed_bytes == 0 {
            return 1.0;
        }
        self.total_compressed_size as f64 / self.total_compressed_bytes as f64
    }
    
    /// Calculate space saved (bytes)
    pub fn space_saved(&self) -> u64 {
        self.total_compressed_bytes.saturating_sub(self.total_compressed_size)
    }
    
    /// Calculate space saved percentage
    pub fn space_saved_percent(&self) -> f64 {
        if self.total_compressed_bytes == 0 {
            return 0.0;
        }
        (self.space_saved() as f64 / self.total_compressed_bytes as f64) * 100.0
    }
    
    /// Record a compression operation
    pub fn record_compression(&mut self, original_size: usize, compressed_size: usize) {
        self.total_compressed_bytes += original_size as u64;
        self.total_compressed_size += compressed_size as u64;
        self.compression_count += 1;
    }
    
    /// Record a decompression operation
    pub fn record_decompression(&mut self, decompressed_size: usize) {
        self.total_decompressed_bytes += decompressed_size as u64;
        self.decompression_count += 1;
    }
    
    /// Record a skipped compression
    pub fn record_skipped(&mut self) {
        self.skipped_count += 1;
    }
    
    /// Record an expansion (compressed data was larger)
    pub fn record_expansion(&mut self) {
        self.expansion_count += 1;
    }
}

/// Compression manager
pub struct CompressionManager {
    config: CompressionConfig,
}

impl CompressionManager {
    /// Create a new compression manager
    pub fn new(config: CompressionConfig) -> Self {
        Self { config }
    }
    
    /// Get configuration
    pub fn config(&self) -> &CompressionConfig {
        &self.config
    }
    
    /// Update configuration
    pub fn update_config(&mut self, config: CompressionConfig) {
        self.config = config;
    }
    
    /// Compress data
    /// 
    /// Returns (compressed_data, was_compressed)
    /// If compression is disabled, too small, or results in larger data,
    /// returns the original data with was_compressed=false
    pub fn compress(&self, data: &[u8]) -> Result<(Bytes, bool), CompressionError> {
        // Check if compression is enabled
        if !self.config.enabled {
            return Ok((Bytes::copy_from_slice(data), false));
        }
        
        // Check minimum size threshold
        if data.len() < self.config.min_size {
            debug!("Skipping compression: data too small ({} bytes)", data.len());
            return Ok((Bytes::copy_from_slice(data), false));
        }
        
        // Compress based on algorithm
        let compressed = match self.config.algorithm {
            CompressionAlgorithm::None => {
                return Ok((Bytes::copy_from_slice(data), false));
            }
            CompressionAlgorithm::Zstd => {
                self.compress_zstd(data)?
            }
            CompressionAlgorithm::Lz4 => {
                self.compress_lz4(data)?
            }
        };
        
        // Check if compression actually reduced size
        if compressed.len() >= data.len() {
            debug!(
                "Compression expanded data: {} -> {} bytes, storing uncompressed",
                data.len(),
                compressed.len()
            );
            return Ok((Bytes::copy_from_slice(data), false));
        }
        
        debug!(
            "Compressed {} -> {} bytes ({:.1}% reduction)",
            data.len(),
            compressed.len(),
            (1.0 - compressed.len() as f64 / data.len() as f64) * 100.0
        );
        
        Ok((compressed, true))
    }
    
    /// Decompress data
    pub fn decompress(&self, data: &[u8], was_compressed: bool) -> Result<Bytes, CompressionError> {
        if !was_compressed {
            return Ok(Bytes::copy_from_slice(data));
        }
        
        match self.config.algorithm {
            CompressionAlgorithm::None => {
                Ok(Bytes::copy_from_slice(data))
            }
            CompressionAlgorithm::Zstd => {
                self.decompress_zstd(data)
            }
            CompressionAlgorithm::Lz4 => {
                self.decompress_lz4(data)
            }
        }
    }
    
    /// Compress using zstd
    fn compress_zstd(&self, data: &[u8]) -> Result<Bytes, CompressionError> {
        let mut encoder = zstd::Encoder::new(Vec::new(), self.config.level)?;
        encoder.write_all(data)?;
        let compressed = encoder.finish()?;
        Ok(Bytes::from(compressed))
    }
    
    /// Decompress using zstd
    fn decompress_zstd(&self, data: &[u8]) -> Result<Bytes, CompressionError> {
        let mut decoder = zstd::Decoder::new(data)?;
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;
        Ok(Bytes::from(decompressed))
    }
    
    /// Compress using lz4
    fn compress_lz4(&self, data: &[u8]) -> Result<Bytes, CompressionError> {
        let mut encoder = lz4::EncoderBuilder::new()
            .level(self.config.level as u32)
            .build(Vec::new())?;
        encoder.write_all(data)?;
        let (compressed, result) = encoder.finish();
        result?;
        Ok(Bytes::from(compressed))
    }
    
    /// Decompress using lz4
    fn decompress_lz4(&self, data: &[u8]) -> Result<Bytes, CompressionError> {
        let mut decoder = lz4::Decoder::new(data)?;
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;
        Ok(Bytes::from(decompressed))
    }
}

/// Compression errors
#[derive(Debug, thiserror::Error)]
pub enum CompressionError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Compression failed: {0}")]
    CompressionFailed(String),
    
    #[error("Decompression failed: {0}")]
    DecompressionFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_compression_disabled() {
        let config = CompressionConfig {
            enabled: false,
            ..Default::default()
        };
        let manager = CompressionManager::new(config);
        
        let data = b"test data that should not be compressed";
        let (compressed, was_compressed) = manager.compress(data).unwrap();
        
        assert!(!was_compressed);
        assert_eq!(compressed.as_ref(), data);
    }
    
    #[test]
    fn test_compression_too_small() {
        let config = CompressionConfig {
            enabled: true,
            min_size: 100,
            ..Default::default()
        };
        let manager = CompressionManager::new(config);
        
        let data = b"small";
        let (compressed, was_compressed) = manager.compress(data).unwrap();
        
        assert!(!was_compressed);
        assert_eq!(compressed.as_ref(), data);
    }
    
    #[test]
    fn test_zstd_compression_roundtrip() {
        let config = CompressionConfig {
            enabled: true,
            algorithm: CompressionAlgorithm::Zstd,
            level: 3,
            min_size: 10,
        };
        let manager = CompressionManager::new(config);
        
        // Create compressible data
        let data = b"This is a test string that should compress well. ".repeat(100);
        
        let (compressed, was_compressed) = manager.compress(&data).unwrap();
        assert!(was_compressed);
        assert!(compressed.len() < data.len());
        
        let decompressed = manager.decompress(&compressed, true).unwrap();
        assert_eq!(decompressed.as_ref(), data.as_slice());
    }
    
    #[test]
    fn test_lz4_compression_roundtrip() {
        let config = CompressionConfig {
            enabled: true,
            algorithm: CompressionAlgorithm::Lz4,
            level: 4,
            min_size: 10,
        };
        let manager = CompressionManager::new(config);
        
        // Create compressible data
        let data = b"This is a test string that should compress well. ".repeat(100);
        
        let (compressed, was_compressed) = manager.compress(&data).unwrap();
        assert!(was_compressed);
        assert!(compressed.len() < data.len());
        
        let decompressed = manager.decompress(&compressed, true).unwrap();
        assert_eq!(decompressed.as_ref(), data.as_slice());
    }
    
    #[test]
    fn test_compression_stats() {
        let mut stats = CompressionStats::new();
        
        stats.record_compression(1000, 500);
        stats.record_compression(2000, 1000);
        
        assert_eq!(stats.total_compressed_bytes, 3000);
        assert_eq!(stats.total_compressed_size, 1500);
        assert_eq!(stats.compression_count, 2);
        assert_eq!(stats.compression_ratio(), 0.5);
        assert_eq!(stats.space_saved(), 1500);
        assert_eq!(stats.space_saved_percent(), 50.0);
    }
    
    #[test]
    fn test_incompressible_data() {
        let config = CompressionConfig {
            enabled: true,
            algorithm: CompressionAlgorithm::Zstd,
            level: 3,
            min_size: 10,
        };
        let manager = CompressionManager::new(config);
        
        // Random data doesn't compress well
        let data: Vec<u8> = (0..100).map(|i| (i * 7 + 13) as u8).collect();
        
        let (compressed, was_compressed) = manager.compress(&data).unwrap();
        
        // Should detect that compression didn't help and return original
        if was_compressed {
            // If it was compressed, verify roundtrip still works
            let decompressed = manager.decompress(&compressed, true).unwrap();
            assert_eq!(decompressed.as_ref(), data.as_slice());
        } else {
            assert_eq!(compressed.as_ref(), data.as_slice());
        }
    }
}
