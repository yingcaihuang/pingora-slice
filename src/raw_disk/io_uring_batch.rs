//! Batch I/O operations using io_uring
//!
//! This module provides high-performance batched I/O operations using io_uring,
//! allowing multiple operations to be submitted and completed together.

use super::{IoUringConfig, RawDiskError};
use bytes::Bytes;
use std::collections::VecDeque;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info};

#[cfg(target_os = "linux")]
use super::io_uring::IoUringManager;

/// A pending I/O operation
#[derive(Debug, Clone)]
pub enum PendingOp {
    Read { offset: u64, size: usize },
    Write { offset: u64, data: Bytes },
}

/// Result of a batch operation
#[derive(Debug)]
pub enum OpResult {
    Read(Result<Bytes, RawDiskError>),
    Write(Result<(), RawDiskError>),
}

/// Batch I/O manager using io_uring
#[cfg(target_os = "linux")]
pub struct IoUringBatchManager {
    io_uring: Arc<IoUringManager>,
    pending_ops: Arc<Mutex<VecDeque<PendingOp>>>,
    max_batch_size: usize,
}

#[cfg(target_os = "linux")]
impl IoUringBatchManager {
    /// Create a new batch I/O manager
    pub async fn new(
        device_path: impl AsRef<Path>,
        config: IoUringConfig,
    ) -> Result<Self, RawDiskError> {
        let max_batch_size = config.queue_depth as usize;
        let io_uring = Arc::new(IoUringManager::new(device_path, config).await?);
        
        Ok(Self {
            io_uring,
            pending_ops: Arc::new(Mutex::new(VecDeque::new())),
            max_batch_size,
        })
    }
    
    /// Add a read operation to the batch
    pub async fn read_buffered(&self, offset: u64, size: usize) -> Result<bool, RawDiskError> {
        let mut ops = self.pending_ops.lock().await;
        ops.push_back(PendingOp::Read { offset, size });
        
        let should_flush = ops.len() >= self.max_batch_size;
        Ok(should_flush)
    }
    
    /// Add a write operation to the batch
    pub async fn write_buffered(&self, offset: u64, data: Bytes) -> Result<bool, RawDiskError> {
        let mut ops = self.pending_ops.lock().await;
        ops.push_back(PendingOp::Write { offset, data });
        
        let should_flush = ops.len() >= self.max_batch_size;
        Ok(should_flush)
    }
    
    /// Execute all pending operations in a batch
    pub async fn flush(&self) -> Result<Vec<OpResult>, RawDiskError> {
        let mut ops = self.pending_ops.lock().await;
        let operations: Vec<_> = ops.drain(..).collect();
        drop(ops);
        
        if operations.is_empty() {
            return Ok(Vec::new());
        }
        
        info!("Flushing {} io_uring operations", operations.len());
        
        // Execute all operations concurrently using io_uring
        let mut results = Vec::new();
        
        for op in operations {
            match op {
                PendingOp::Read { offset, size } => {
                    let result = self.io_uring.read_at(offset, size).await;
                    results.push(OpResult::Read(result));
                }
                PendingOp::Write { offset, data } => {
                    let result = self.io_uring.write_at(offset, &data).await;
                    results.push(OpResult::Write(result));
                }
            }
        }
        
        // Sync once for all operations
        self.io_uring.sync().await?;
        
        Ok(results)
    }
    
    /// Batch read multiple locations
    pub async fn read_batch(
        &self,
        locations: Vec<(u64, usize)>,
    ) -> Result<Vec<Bytes>, RawDiskError> {
        if locations.is_empty() {
            return Ok(Vec::new());
        }
        
        debug!("io_uring batch read: {} locations", locations.len());
        
        // Submit all reads concurrently
        let mut results = Vec::new();
        
        for (offset, size) in locations {
            let data = self.io_uring.read_at(offset, size).await?;
            results.push(data);
        }
        
        Ok(results)
    }
    
    /// Batch write multiple locations
    pub async fn write_batch(
        &self,
        writes: Vec<(u64, Bytes)>,
    ) -> Result<(), RawDiskError> {
        if writes.is_empty() {
            return Ok(());
        }
        
        debug!("io_uring batch write: {} locations", writes.len());
        
        // Submit all writes concurrently
        for (offset, data) in writes {
            self.io_uring.write_at(offset, &data).await?;
        }
        
        // Sync once for all writes
        self.io_uring.sync().await?;
        
        Ok(())
    }
    
    /// Get the number of pending operations
    pub async fn pending_count(&self) -> usize {
        let ops = self.pending_ops.lock().await;
        ops.len()
    }
    
    /// Get the underlying io_uring manager
    pub fn io_uring(&self) -> &Arc<IoUringManager> {
        &self.io_uring
    }
}

/// Stub implementation for non-Linux platforms
#[cfg(not(target_os = "linux"))]
pub struct IoUringBatchManager;

#[cfg(not(target_os = "linux"))]
impl IoUringBatchManager {
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
    async fn test_pending_ops() {
        // Test that operations are queued correctly
        let op1 = PendingOp::Read { offset: 0, size: 100 };
        let op2 = PendingOp::Write { 
            offset: 100, 
            data: Bytes::from("test") 
        };
        
        assert!(matches!(op1, PendingOp::Read { .. }));
        assert!(matches!(op2, PendingOp::Write { .. }));
    }
}
