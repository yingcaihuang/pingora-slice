//! Raw Disk Cache Implementation
//!
//! This module implements a high-performance cache that directly manages
//! disk blocks without relying on the filesystem, similar to Apache Traffic Server.

pub mod allocator;
pub mod directory;
pub mod disk_io;
pub mod superblock;
pub mod types;

pub use allocator::BlockAllocator;
pub use directory::CacheDirectory;
pub use disk_io::DiskIOManager;
pub use superblock::Superblock;
pub use types::*;

use bytes::Bytes;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Raw disk cache implementation
pub struct RawDiskCache {
    superblock: Arc<RwLock<Superblock>>,
    directory: Arc<RwLock<CacheDirectory>>,
    allocator: Arc<RwLock<BlockAllocator>>,
    disk_io: Arc<DiskIOManager>,
    ttl: Duration,
}

impl RawDiskCache {
    /// Create a new raw disk cache
    pub async fn new(
        device_path: impl AsRef<Path>,
        total_size: u64,
        block_size: usize,
        ttl: Duration,
    ) -> Result<Self, RawDiskError> {
        info!(
            "Initializing raw disk cache: path={}, size={}, block_size={}",
            device_path.as_ref().display(),
            total_size,
            block_size
        );

        // Initialize disk I/O manager
        let disk_io = Arc::new(DiskIOManager::new(device_path, block_size).await?);

        // Try to load existing superblock or create new one
        let superblock = match Superblock::load(&disk_io).await {
            Ok(sb) => {
                info!("Loaded existing superblock");
                sb
            }
            Err(_) => {
                info!("Creating new superblock");
                let sb = Superblock::new(total_size, block_size as u32);
                sb.save(&disk_io).await?;
                sb
            }
        };

        let total_blocks = superblock.total_blocks() as usize;

        // Initialize allocator
        let allocator = BlockAllocator::new(block_size, total_blocks);

        // Initialize directory
        let directory = CacheDirectory::new();

        Ok(Self {
            superblock: Arc::new(RwLock::new(superblock)),
            directory: Arc::new(RwLock::new(directory)),
            allocator: Arc::new(RwLock::new(allocator)),
            disk_io,
            ttl,
        })
    }

    /// Store data in cache
    pub async fn store(&self, key: &str, data: Bytes) -> Result<(), RawDiskError> {
        let data_len = data.len();
        
        // Calculate blocks needed
        let block_size = self.superblock.read().await.block_size as usize;
        let blocks_needed = (data_len + block_size - 1) / block_size;

        // Allocate space
        let mut allocator = self.allocator.write().await;
        let location = allocator.allocate(blocks_needed)?;
        drop(allocator);

        // Write data to disk
        self.disk_io.write_at(location.offset, &data).await?;

        // Update directory
        let mut directory = self.directory.write().await;
        directory.insert(key.to_string(), location);

        info!("Stored {} bytes at offset {} ({} blocks)", data_len, location.offset, blocks_needed);

        Ok(())
    }

    /// Lookup data in cache
    pub async fn lookup(&self, key: &str) -> Result<Option<Bytes>, RawDiskError> {
        // Check directory
        let directory = self.directory.read().await;
        let location = match directory.get(key) {
            Some(loc) => loc.clone(),
            None => return Ok(None),
        };
        drop(directory);

        // Read from disk
        let data = self.disk_io.read_at(location.offset, location.size as usize).await?;

        // Verify checksum
        if !location.verify_checksum(&data) {
            warn!("Checksum mismatch for key: {}", key);
            return Err(RawDiskError::ChecksumMismatch);
        }

        // Update LRU
        let mut directory = self.directory.write().await;
        directory.touch(key);

        Ok(Some(data))
    }

    /// Remove entry from cache
    pub async fn remove(&self, key: &str) -> Result<bool, RawDiskError> {
        let mut directory = self.directory.write().await;
        
        if let Some(location) = directory.remove(key) {
            // Free blocks
            let mut allocator = self.allocator.write().await;
            let block_size = self.superblock.read().await.block_size as usize;
            let blocks = (location.size as usize + block_size - 1) / block_size;
            allocator.free(location.offset, blocks)?;
            
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        let directory = self.directory.read().await;
        let allocator = self.allocator.read().await;
        
        CacheStats {
            entries: directory.len(),
            used_blocks: allocator.used_blocks(),
            free_blocks: allocator.free_blocks(),
            total_blocks: allocator.total_blocks(),
            hits: directory.hits(),
            misses: directory.misses(),
        }
    }

    /// Perform garbage collection
    pub async fn gc(&self, target_free_ratio: f64) -> Result<usize, RawDiskError> {
        let allocator = self.allocator.read().await;
        let current_free_ratio = allocator.free_blocks() as f64 / allocator.total_blocks() as f64;
        drop(allocator);

        if current_free_ratio >= target_free_ratio {
            return Ok(0);
        }

        info!("Starting GC: current_free={:.2}%, target={:.2}%", 
              current_free_ratio * 100.0, target_free_ratio * 100.0);

        let mut directory = self.directory.write().await;
        let victims = directory.select_lru_victims(0.1); // Remove 10% of entries
        
        let mut freed = 0;
        for key in victims {
            if let Some(location) = directory.remove(&key) {
                let mut allocator = self.allocator.write().await;
                let block_size = self.superblock.read().await.block_size as usize;
                let blocks = (location.size as usize + block_size - 1) / block_size;
                allocator.free(location.offset, blocks)?;
                freed += 1;
            }
        }

        info!("GC completed: freed {} entries", freed);
        Ok(freed)
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub entries: usize,
    pub used_blocks: usize,
    pub free_blocks: usize,
    pub total_blocks: usize,
    pub hits: u64,
    pub misses: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum RawDiskError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("No space available")]
    NoSpace,
    
    #[error("Invalid block size")]
    InvalidBlockSize,
    
    #[error("Checksum mismatch")]
    ChecksumMismatch,
    
    #[error("Invalid superblock")]
    InvalidSuperblock,
    
    #[error("Allocation error: {0}")]
    AllocationError(String),
}
