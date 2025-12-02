//! Batch I/O operations for improved throughput
//!
//! This module implements write buffering and batch operations to reduce
//! the number of individual I/O operations and improve overall throughput.

use super::{DiskIOManager, RawDiskError};
use bytes::Bytes;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info};

/// A pending write operation
#[derive(Debug, Clone)]
pub(crate) struct PendingWrite {
    offset: u64,
    data: Bytes,
}

/// Write buffer for batching write operations
pub struct WriteBuffer {
    /// Pending writes
    pending: VecDeque<PendingWrite>,
    
    /// Maximum number of operations to buffer
    max_batch_size: usize,
    
    /// Maximum bytes to buffer before flushing
    max_buffer_bytes: usize,
    
    /// Current buffered bytes
    current_bytes: usize,
}

impl WriteBuffer {
    /// Create a new write buffer
    pub fn new(max_batch_size: usize, max_buffer_bytes: usize) -> Self {
        Self {
            pending: VecDeque::new(),
            max_batch_size,
            max_buffer_bytes,
            current_bytes: 0,
        }
    }
    
    /// Add a write operation to the buffer
    /// Returns true if the buffer should be flushed
    pub fn add(&mut self, offset: u64, data: Bytes) -> bool {
        let data_len = data.len();
        
        self.pending.push_back(PendingWrite { offset, data });
        self.current_bytes += data_len;
        
        // Check if we should flush
        self.should_flush()
    }
    
    /// Check if the buffer should be flushed
    pub fn should_flush(&self) -> bool {
        self.pending.len() >= self.max_batch_size || 
        self.current_bytes >= self.max_buffer_bytes
    }
    
    /// Get all pending writes and clear the buffer
    pub(crate) fn drain(&mut self) -> Vec<PendingWrite> {
        let writes: Vec<_> = self.pending.drain(..).collect();
        self.current_bytes = 0;
        writes
    }
    
    /// Get the number of pending operations
    pub fn len(&self) -> usize {
        self.pending.len()
    }
    
    /// Check if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
    
    /// Get current buffered bytes
    pub fn buffered_bytes(&self) -> usize {
        self.current_bytes
    }
}

/// Batch I/O manager for optimized disk operations
pub struct BatchIOManager {
    /// Underlying disk I/O manager
    disk_io: Arc<DiskIOManager>,
    
    /// Write buffer
    write_buffer: Arc<Mutex<WriteBuffer>>,
}

impl BatchIOManager {
    /// Create a new batch I/O manager
    pub fn new(
        disk_io: Arc<DiskIOManager>,
        max_batch_size: usize,
        max_buffer_bytes: usize,
    ) -> Self {
        Self {
            disk_io,
            write_buffer: Arc::new(Mutex::new(WriteBuffer::new(
                max_batch_size,
                max_buffer_bytes,
            ))),
        }
    }
    
    /// Write data with buffering
    /// Returns true if a flush was performed
    pub async fn write_buffered(
        &self,
        offset: u64,
        data: Bytes,
    ) -> Result<bool, RawDiskError> {
        let mut buffer = self.write_buffer.lock().await;
        let should_flush = buffer.add(offset, data);
        
        if should_flush {
            let writes = buffer.drain();
            drop(buffer);
            
            self.flush_writes(writes).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    
    /// Manually flush all pending writes
    pub async fn flush(&self) -> Result<usize, RawDiskError> {
        let mut buffer = self.write_buffer.lock().await;
        let writes = buffer.drain();
        let count = writes.len();
        drop(buffer);
        
        if !writes.is_empty() {
            self.flush_writes(writes).await?;
        }
        
        Ok(count)
    }
    
    /// Flush a batch of writes to disk
    async fn flush_writes(&self, writes: Vec<PendingWrite>) -> Result<(), RawDiskError> {
        if writes.is_empty() {
            return Ok(());
        }
        
        debug!("Flushing {} writes to disk", writes.len());
        
        // Sort writes by offset for sequential I/O
        let mut sorted_writes = writes;
        sorted_writes.sort_by_key(|w| w.offset);
        
        // Try to merge adjacent writes
        let merged = self.merge_adjacent_writes(sorted_writes);
        
        info!("Batch flush: {} operations", merged.len());
        
        // Execute all writes
        for write in merged {
            self.disk_io.write_at(write.offset, &write.data).await?;
        }
        
        // Sync once for all writes
        self.disk_io.sync().await?;
        
        Ok(())
    }
    
    /// Merge adjacent writes to reduce I/O operations
    fn merge_adjacent_writes(&self, writes: Vec<PendingWrite>) -> Vec<PendingWrite> {
        if writes.is_empty() {
            return writes;
        }
        
        let mut merged = Vec::new();
        let mut current = writes[0].clone();
        
        for write in writes.into_iter().skip(1) {
            let current_end = current.offset + current.data.len() as u64;
            
            // Check if writes are adjacent or overlapping
            if write.offset <= current_end + 4096 {
                // Merge writes
                let gap = if write.offset > current_end {
                    (write.offset - current_end) as usize
                } else {
                    0
                };
                
                let mut new_data = current.data.to_vec();
                
                // Add gap if needed
                if gap > 0 {
                    new_data.extend(vec![0u8; gap]);
                }
                
                // Add new data
                new_data.extend_from_slice(&write.data);
                
                current = PendingWrite {
                    offset: current.offset,
                    data: Bytes::from(new_data),
                };
            } else {
                // Not adjacent, save current and start new
                merged.push(current);
                current = write;
            }
        }
        
        merged.push(current);
        merged
    }
    
    /// Batch read multiple locations
    pub async fn read_batch(
        &self,
        locations: Vec<(u64, usize)>,
    ) -> Result<Vec<Bytes>, RawDiskError> {
        if locations.is_empty() {
            return Ok(Vec::new());
        }
        
        debug!("Batch reading {} locations", locations.len());
        
        // Sort by offset for sequential reads
        let mut sorted_locations = locations;
        sorted_locations.sort_by_key(|(offset, _)| *offset);
        
        // Try to merge adjacent reads
        let merged = self.merge_adjacent_reads(sorted_locations.clone());
        
        info!("Batch read: {} operations (merged from {})", merged.len(), sorted_locations.len());
        
        // Execute merged reads
        let mut merged_results = Vec::new();
        for (offset, size) in merged {
            let data = self.disk_io.read_at(offset, size).await?;
            merged_results.push((offset, data));
        }
        
        // Extract individual results from merged reads
        let mut results = Vec::new();
        for (req_offset, req_size) in sorted_locations {
            // Find which merged read contains this request
            for (merged_offset, merged_data) in &merged_results {
                if req_offset >= *merged_offset && 
                   req_offset + req_size as u64 <= *merged_offset + merged_data.len() as u64 {
                    let start = (req_offset - merged_offset) as usize;
                    let end = start + req_size;
                    results.push(merged_data.slice(start..end));
                    break;
                }
            }
        }
        
        Ok(results)
    }
    
    /// Merge adjacent read requests
    fn merge_adjacent_reads(&self, reads: Vec<(u64, usize)>) -> Vec<(u64, usize)> {
        if reads.is_empty() {
            return reads;
        }
        
        let mut merged = Vec::new();
        let mut current_offset = reads[0].0;
        let mut current_end = reads[0].0 + reads[0].1 as u64;
        
        for (offset, size) in reads.into_iter().skip(1) {
            let read_end = offset + size as u64;
            
            // Merge if reads are close (within 64KB)
            if offset <= current_end + 65536 {
                current_end = current_end.max(read_end);
            } else {
                // Save current merged read
                merged.push((current_offset, (current_end - current_offset) as usize));
                current_offset = offset;
                current_end = read_end;
            }
        }
        
        merged.push((current_offset, (current_end - current_offset) as usize));
        merged
    }
    
    /// Get statistics about buffered writes
    pub async fn buffer_stats(&self) -> BufferStats {
        let buffer = self.write_buffer.lock().await;
        BufferStats {
            pending_operations: buffer.len(),
            buffered_bytes: buffer.buffered_bytes(),
        }
    }
}

/// Statistics about the write buffer
#[derive(Debug, Clone)]
pub struct BufferStats {
    pub pending_operations: usize,
    pub buffered_bytes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_write_buffer() {
        let mut buffer = WriteBuffer::new(10, 1024);
        
        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);
        
        // Add some writes
        let data1 = Bytes::from("test1");
        let should_flush = buffer.add(0, data1);
        assert!(!should_flush);
        assert_eq!(buffer.len(), 1);
        
        // Add more writes until flush is needed
        for i in 1..10 {
            let data = Bytes::from(format!("test{}", i));
            buffer.add(i * 100, data);
        }
        
        assert!(buffer.should_flush());
    }
    
    #[test]
    fn test_write_buffer_size_limit() {
        let mut buffer = WriteBuffer::new(100, 100);
        
        // Add a large write
        let data = Bytes::from(vec![0u8; 150]);
        let should_flush = buffer.add(0, data);
        
        assert!(should_flush);
    }
    

}
