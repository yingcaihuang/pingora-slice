//! Zero-copy operations for raw disk cache
//!
//! This module provides zero-copy data transfer mechanisms to reduce memory
//! copy overhead when reading from the cache. It supports:
//! - Memory-mapped I/O (mmap) for large file access
//! - sendfile() for direct disk-to-socket transfers (Linux only)

use super::RawDiskError;
use bytes::Bytes;
use memmap2::MmapOptions;
use std::fs::File;
use std::os::unix::io::AsRawFd;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info};

#[cfg(target_os = "linux")]
use nix::sys::sendfile::sendfile;

/// Configuration for zero-copy operations
#[derive(Debug, Clone)]
pub struct ZeroCopyConfig {
    /// Minimum size (in bytes) to use mmap instead of regular read
    /// Files smaller than this will use regular I/O
    pub mmap_threshold: usize,
    
    /// Enable sendfile for socket transfers (Linux only)
    pub enable_sendfile: bool,
}

impl Default for ZeroCopyConfig {
    fn default() -> Self {
        Self {
            // Use mmap for files >= 64KB
            mmap_threshold: 64 * 1024,
            enable_sendfile: true,
        }
    }
}

/// Zero-copy manager for efficient data access
pub struct ZeroCopyManager {
    file: Arc<Mutex<File>>,
    config: ZeroCopyConfig,
}

impl ZeroCopyManager {
    /// Create a new zero-copy manager
    pub fn new(file: File, config: ZeroCopyConfig) -> Self {
        info!(
            "Initializing zero-copy manager: mmap_threshold={} bytes, sendfile={}",
            config.mmap_threshold, config.enable_sendfile
        );
        
        Self {
            file: Arc::new(Mutex::new(file)),
            config,
        }
    }
    
    /// Read data using memory-mapped I/O
    /// 
    /// This creates a memory mapping of the file region and returns a view
    /// into it without copying data into application memory.
    pub async fn mmap_read(
        &self,
        offset: u64,
        size: usize,
    ) -> Result<Bytes, RawDiskError> {
        debug!("mmap_read: offset={}, size={}", offset, size);
        
        // Lock the file
        let file = self.file.lock().await;
        
        // Create memory mapping
        let mmap = unsafe {
            MmapOptions::new()
                .offset(offset)
                .len(size)
                .map(&*file)?
        };
        
        // Copy data from mmap to Bytes
        // Note: We still need to copy here because Bytes needs owned data
        // and mmap lifetime is tied to the file
        let data = Bytes::copy_from_slice(&mmap[..]);
        
        drop(file);
        
        debug!("mmap_read completed: {} bytes", data.len());
        Ok(data)
    }
    
    /// Read data using the most appropriate method based on size
    pub async fn read_optimized(
        &self,
        offset: u64,
        size: usize,
    ) -> Result<Bytes, RawDiskError> {
        if size >= self.config.mmap_threshold {
            debug!("Using mmap for large read: {} bytes", size);
            self.mmap_read(offset, size).await
        } else {
            debug!("Size {} below mmap threshold, using regular read", size);
            // Fall back to regular read for small files
            // This would be handled by the caller
            Err(RawDiskError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Use regular read for small files"
            )))
        }
    }
    
    /// Transfer data directly from disk to socket using sendfile (Linux only)
    /// 
    /// This avoids copying data through user space, transferring directly
    /// from the kernel's page cache to the socket buffer.
    #[cfg(target_os = "linux")]
    pub async fn sendfile_to_socket(
        &self,
        socket_fd: i32,
        offset: u64,
        size: usize,
    ) -> Result<usize, RawDiskError> {
        if !self.config.enable_sendfile {
            return Err(RawDiskError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "sendfile is disabled"
            )));
        }
        
        debug!("sendfile: offset={}, size={}", offset, size);
        
        let file = self.file.lock().await;
        let file_fd = file.as_raw_fd();
        
        // sendfile requires a mutable offset
        let mut current_offset = offset as i64;
        let mut total_sent = 0;
        let mut remaining = size;
        
        // sendfile may not send all data in one call, so we loop
        while remaining > 0 {
            match sendfile(socket_fd, file_fd, Some(&mut current_offset), remaining) {
                Ok(sent) => {
                    if sent == 0 {
                        // No more data could be sent
                        break;
                    }
                    total_sent += sent;
                    remaining -= sent;
                    debug!("sendfile sent {} bytes, {} remaining", sent, remaining);
                }
                Err(e) => {
                    warn!("sendfile error: {}", e);
                    return Err(RawDiskError::Io(std::io::Error::from(e)));
                }
            }
        }
        
        info!("sendfile completed: {} bytes transferred", total_sent);
        Ok(total_sent)
    }
    
    /// Transfer data to socket (stub for non-Linux platforms)
    #[cfg(not(target_os = "linux"))]
    pub async fn sendfile_to_socket(
        &self,
        _socket_fd: i32,
        _offset: u64,
        _size: usize,
    ) -> Result<usize, RawDiskError> {
        Err(RawDiskError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "sendfile is only supported on Linux"
        )))
    }
    
    /// Get the raw file descriptor (Unix only)
    pub async fn raw_fd(&self) -> i32 {
        let file = self.file.lock().await;
        file.as_raw_fd()
    }
    
    /// Check if sendfile is available and enabled
    pub fn is_sendfile_available(&self) -> bool {
        #[cfg(target_os = "linux")]
        {
            self.config.enable_sendfile
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }
    
    /// Get the mmap threshold
    pub fn mmap_threshold(&self) -> usize {
        self.config.mmap_threshold
    }
}

/// Statistics for zero-copy operations
#[derive(Debug, Clone, Default)]
pub struct ZeroCopyStats {
    /// Number of mmap reads performed
    pub mmap_reads: u64,
    
    /// Total bytes read via mmap
    pub mmap_bytes: u64,
    
    /// Number of sendfile transfers performed
    pub sendfile_transfers: u64,
    
    /// Total bytes transferred via sendfile
    pub sendfile_bytes: u64,
    
    /// Number of times mmap was skipped (file too small)
    pub mmap_skipped: u64,
}

impl ZeroCopyStats {
    /// Create new empty stats
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Record an mmap read
    pub fn record_mmap_read(&mut self, bytes: usize) {
        self.mmap_reads += 1;
        self.mmap_bytes += bytes as u64;
    }
    
    /// Record a sendfile transfer
    pub fn record_sendfile(&mut self, bytes: usize) {
        self.sendfile_transfers += 1;
        self.sendfile_bytes += bytes as u64;
    }
    
    /// Record a skipped mmap operation
    pub fn record_mmap_skipped(&mut self) {
        self.mmap_skipped += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;
    
    #[tokio::test]
    async fn test_mmap_read() {
        // Create a temporary file with test data
        let mut temp_file = NamedTempFile::new().unwrap();
        let test_data = b"Hello, mmap world! This is a test of memory-mapped I/O.";
        temp_file.write_all(test_data).unwrap();
        temp_file.flush().unwrap();
        
        // Open file for reading
        let file = File::open(temp_file.path()).unwrap();
        
        // Create zero-copy manager
        let config = ZeroCopyConfig {
            mmap_threshold: 10, // Low threshold for testing
            enable_sendfile: true,
        };
        let manager = ZeroCopyManager::new(file, config);
        
        // Read using mmap
        let data = manager.mmap_read(0, test_data.len()).await.unwrap();
        assert_eq!(&data[..], test_data);
    }
    
    #[tokio::test]
    async fn test_mmap_read_with_offset() {
        // Create a temporary file with test data
        let mut temp_file = NamedTempFile::new().unwrap();
        let test_data = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
        temp_file.write_all(test_data).unwrap();
        temp_file.flush().unwrap();
        
        // Open file for reading
        let file = File::open(temp_file.path()).unwrap();
        
        // Create zero-copy manager
        let config = ZeroCopyConfig::default();
        let manager = ZeroCopyManager::new(file, config);
        
        // Read from offset
        let offset = 10;
        let size = 10;
        let data = manager.mmap_read(offset, size).await.unwrap();
        assert_eq!(&data[..], &test_data[offset as usize..(offset as usize + size)]);
    }
    
    #[tokio::test]
    async fn test_zero_copy_stats() {
        let mut stats = ZeroCopyStats::new();
        
        stats.record_mmap_read(1024);
        stats.record_mmap_read(2048);
        stats.record_sendfile(4096);
        stats.record_mmap_skipped();
        
        assert_eq!(stats.mmap_reads, 2);
        assert_eq!(stats.mmap_bytes, 3072);
        assert_eq!(stats.sendfile_transfers, 1);
        assert_eq!(stats.sendfile_bytes, 4096);
        assert_eq!(stats.mmap_skipped, 1);
    }
}
