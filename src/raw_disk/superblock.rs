//! Superblock management for raw disk cache

use super::{disk_io::DiskIOManager, types::*, RawDiskError};
use std::sync::Arc;

/// Superblock stores metadata about the cache
#[derive(Debug, Clone)]
pub struct Superblock {
    magic: u32,
    version: u32,
    block_size: u32,
    total_size: u64,
    metadata_offset: u64,
    metadata_size: u64,
    data_offset: u64,
}

impl Superblock {
    pub const SIZE: usize = 4096; // 4KB
    
    /// Create a new superblock
    pub fn new(total_size: u64, block_size: u32) -> Self {
        let metadata_offset = Self::SIZE as u64;
        // Allocate 1% of total size for metadata, min 64KB, max 100MB
        // For small caches, use a smaller minimum to leave room for data
        let metadata_size = ((total_size / 100).max(64 * 1024)).min(100 * 1024 * 1024);
        let data_offset = metadata_offset + metadata_size;
        
        Self {
            magic: MAGIC_NUMBER,
            version: VERSION,
            block_size,
            total_size,
            metadata_offset,
            metadata_size,
            data_offset,
        }
    }
    
    /// Load superblock from disk
    pub async fn load(disk_io: &Arc<DiskIOManager>) -> Result<Self, RawDiskError> {
        let data = disk_io.read_at(0, Self::SIZE).await?;
        
        if data.len() < 32 {
            return Err(RawDiskError::InvalidSuperblock);
        }
        
        let magic = u32::from_le_bytes(data[0..4].try_into().unwrap());
        if magic != MAGIC_NUMBER {
            return Err(RawDiskError::InvalidSuperblock);
        }
        
        let version = u32::from_le_bytes(data[4..8].try_into().unwrap());
        let block_size = u32::from_le_bytes(data[8..12].try_into().unwrap());
        let total_size = u64::from_le_bytes(data[12..20].try_into().unwrap());
        let metadata_offset = u64::from_le_bytes(data[20..28].try_into().unwrap());
        let metadata_size = u64::from_le_bytes(data[28..36].try_into().unwrap());
        let data_offset = u64::from_le_bytes(data[36..44].try_into().unwrap());
        
        Ok(Self {
            magic,
            version,
            block_size,
            total_size,
            metadata_offset,
            metadata_size,
            data_offset,
        })
    }
    
    /// Save superblock to disk
    pub async fn save(&self, disk_io: &Arc<DiskIOManager>) -> Result<(), RawDiskError> {
        let mut data = vec![0u8; Self::SIZE];
        
        data[0..4].copy_from_slice(&self.magic.to_le_bytes());
        data[4..8].copy_from_slice(&self.version.to_le_bytes());
        data[8..12].copy_from_slice(&self.block_size.to_le_bytes());
        data[12..20].copy_from_slice(&self.total_size.to_le_bytes());
        data[20..28].copy_from_slice(&self.metadata_offset.to_le_bytes());
        data[28..36].copy_from_slice(&self.metadata_size.to_le_bytes());
        data[36..44].copy_from_slice(&self.data_offset.to_le_bytes());
        
        disk_io.write_at(0, &data).await?;
        Ok(())
    }
    
    /// Get total number of blocks
    pub fn total_blocks(&self) -> u64 {
        if self.total_size <= self.data_offset {
            return 0;
        }
        (self.total_size - self.data_offset) / self.block_size as u64
    }
    
    /// Get block size
    pub fn block_size(&self) -> u32 {
        self.block_size
    }
    
    /// Get metadata offset
    pub fn metadata_offset(&self) -> u64 {
        self.metadata_offset
    }
    
    /// Get metadata size
    pub fn metadata_size(&self) -> u64 {
        self.metadata_size
    }
    
    /// Get data offset
    pub fn data_offset(&self) -> u64 {
        self.data_offset
    }
}
