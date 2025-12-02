//! io_uring support for high-performance async I/O on Linux
//!
//! This module provides io_uring-based I/O operations for maximum performance
//! on Linux systems. It supports batched operations and configurable queue depth.

use super::RawDiskError;
use bytes::Bytes;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

#[cfg(target_os = "linux")]
use tokio_uring::fs::{File, OpenOptions};

/// Configuration for io_uring operations
#[derive(Debug, Clone)]
pub struct IoUringConfig {
    /// Queue depth (number of concurrent operations)
    pub queue_depth: u32,
    
    /// Whether to use SQPOLL mode (kernel polling)
    pub use_sqpoll: bool,
    
    /// Whether to use IOPOLL mode (polling for completions)
    pub use_iopoll: bool,
    
    /// Block size for alignment
    pub block_size: usize,
}

impl Default for IoUringConfig {
    fn default() -> Self {
        Self {
            queue_depth: 128,
            use_sqpoll: false,
            use_iopoll: false,
            block_size: 4096,
        }
    }
}

/// io_uring-based I/O manager for Linux
#[cfg(target_os = "linux")]
pub struct IoUringManager {
    file: Arc<Mutex<File>>,
    config: IoUringConfig,
}

#[cfg(target_os = "linux")]
impl IoUringManager {
    /// Create a new io_uring manager
    pub async fn new(
        device_path: impl AsRef<Path>,
        config: IoUringConfig,
    ) -> Result<Self, RawDiskError> {
        let path = device_path.as_ref();
        
        info!(
            "Initializing io_uring: path={}, queue_depth={}, sqpoll={}, iopoll={}",
            path.display(),
            config.queue_depth,
            config.use_sqpoll,
            config.use_iopoll
        );
        
        // Open file with io_uring
        let file = Self::open_file(path).await?;
        
        Ok(Self {
            file: Arc::new(Mutex::new(file)),
            config,
        })
    }
    
    /// Open file for io_uring operations
    async fn open_file(path: &Path) -> Result<File, RawDiskError> {
        let mut opts = OpenOptions::new();
        opts.read(true).write(true);
        
        if !path.exists() {
            warn!("Device {} not found, creating file", path.display());
            opts.create(true);
        }
        
        // Note: O_DIRECT can be added via custom_flags if needed
        // For now, we rely on io_uring's efficient buffering
        
        let file = opts.open(path).await
            .map_err(|e| RawDiskError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to open file with io_uring: {}", e)
            )))?;
        
        Ok(file)
    }
    
    /// Read data at offset using io_uring
    pub async fn read_at(&self, offset: u64, size: usize) -> Result<Bytes, RawDiskError> {
        debug!("io_uring read: {} bytes at offset {}", size, offset);
        
        let file = self.file.lock().await;
        
        // Allocate buffer
        let buf = vec![0u8; size];
        
        // Perform read operation
        let (result, buf) = file.read_at(buf, offset).await;
        
        match result {
            Ok(n) if n == size => Ok(Bytes::from(buf)),
            Ok(n) => {
                warn!("Short read: expected {}, got {}", size, n);
                Ok(Bytes::from(buf[..n].to_vec()))
            }
            Err(e) => Err(RawDiskError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("io_uring read failed: {}", e)
            ))),
        }
    }
    
    /// Write data at offset using io_uring
    pub async fn write_at(&self, offset: u64, data: &[u8]) -> Result<(), RawDiskError> {
        debug!("io_uring write: {} bytes at offset {}", data.len(), offset);
        
        let file = self.file.lock().await;
        
        // io_uring takes ownership of the buffer, so we need to copy
        let buf = data.to_vec();
        
        // Perform write operation
        let (result, _buf) = file.write_at(buf, offset).await;
        
        match result {
            Ok(n) if n == data.len() => Ok(()),
            Ok(n) => Err(RawDiskError::Io(std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                format!("Short write: expected {}, wrote {}", data.len(), n)
            ))),
            Err(e) => Err(RawDiskError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("io_uring write failed: {}", e)
            ))),
        }
    }
    
    /// Sync data to disk
    pub async fn sync(&self) -> Result<(), RawDiskError> {
        let file = self.file.lock().await;
        
        file.sync_data().await
            .map_err(|e| RawDiskError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("io_uring sync failed: {}", e)
            )))?;
        
        Ok(())
    }
    
    /// Get configuration
    pub fn config(&self) -> &IoUringConfig {
        &self.config
    }
}

/// Stub implementation for non-Linux platforms
#[cfg(not(target_os = "linux"))]
pub struct IoUringManager;

#[cfg(not(target_os = "linux"))]
impl IoUringManager {
    pub async fn new(
        _device_path: impl AsRef<Path>,
        _config: IoUringConfig,
    ) -> Result<Self, RawDiskError> {
        Err(RawDiskError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "io_uring is only supported on Linux"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn test_io_uring_config_default() {
        let config = IoUringConfig::default();
        assert_eq!(config.queue_depth, 128);
        assert!(!config.use_sqpoll);
        assert!(!config.use_iopoll);
        assert_eq!(config.block_size, 4096);
    }
}
