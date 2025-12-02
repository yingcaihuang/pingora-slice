//! Block allocator for raw disk cache

use super::RawDiskError;
use bitvec::prelude::*;

/// Block allocator manages free space on disk
pub struct BlockAllocator {
    pub(crate) block_size: usize,
    total_blocks: usize,
    free_blocks: BitVec,
}

impl BlockAllocator {
    pub fn new(block_size: usize, total_blocks: usize) -> Self {
        let mut free_blocks = BitVec::with_capacity(total_blocks);
        free_blocks.resize(total_blocks, true); // All blocks initially free
        
        Self {
            block_size,
            total_blocks,
            free_blocks,
        }
    }
    
    pub fn allocate(&mut self, blocks_needed: usize) -> Result<super::DiskLocation, RawDiskError> {
        // Find contiguous free blocks
        let mut start = None;
        let mut count = 0;
        
        for (i, is_free) in self.free_blocks.iter().enumerate() {
            if *is_free {
                if start.is_none() {
                    start = Some(i);
                }
                count += 1;
                if count >= blocks_needed {
                    break;
                }
            } else {
                start = None;
                count = 0;
            }
        }
        
        if count < blocks_needed {
            return Err(RawDiskError::NoSpace);
        }
        
        let start_block = start.unwrap();
        
        // Mark blocks as used
        for i in start_block..(start_block + blocks_needed) {
            self.free_blocks.set(i, false);
        }
        
        let offset = (start_block * self.block_size) as u64;
        let size = (blocks_needed * self.block_size) as u32;
        
        Ok(super::DiskLocation {
            offset,
            size,
            checksum: 0,
            timestamp: 0,
            compressed: false,
            original_size: 0,
        })
    }
    
    pub fn free(&mut self, offset: u64, blocks: usize) -> Result<(), RawDiskError> {
        let start_block = (offset as usize) / self.block_size;
        
        for i in start_block..(start_block + blocks) {
            if i < self.total_blocks {
                self.free_blocks.set(i, true);
            }
        }
        
        Ok(())
    }
    
    pub fn used_blocks(&self) -> usize {
        self.free_blocks.count_zeros()
    }
    
    pub fn free_blocks(&self) -> usize {
        self.free_blocks.count_ones()
    }
    
    pub fn total_blocks(&self) -> usize {
        self.total_blocks
    }

    /// Mark blocks as used (for recovery)
    pub fn mark_used(&mut self, offset: u64, blocks: usize) -> Result<(), RawDiskError> {
        let start_block = (offset as usize) / self.block_size;
        
        if start_block + blocks > self.total_blocks {
            return Err(RawDiskError::AllocationError(format!(
                "Block range out of bounds: {} + {} > {}",
                start_block, blocks, self.total_blocks
            )));
        }
        
        for i in start_block..(start_block + blocks) {
            self.free_blocks.set(i, false);
        }
        
        Ok(())
    }
}
