//! Defragmentation module for raw disk cache
//!
//! This module provides functionality to detect and reduce fragmentation
//! in the raw disk cache, improving space utilization and performance.

use super::{CacheDirectory, DiskLocation};
use std::time::{Duration, Instant};

/// Defragmentation configuration
#[derive(Debug, Clone)]
pub struct DefragConfig {
    /// Fragmentation ratio threshold to trigger defragmentation (0.0-1.0)
    /// Higher values mean more fragmentation is tolerated
    pub fragmentation_threshold: f64,
    
    /// Maximum number of entries to move in one defragmentation cycle
    pub batch_size: usize,
    
    /// Whether to run defragmentation incrementally
    pub incremental: bool,
    
    /// Minimum free space ratio required to perform defragmentation
    /// We need some free space to move entries around
    pub min_free_space_ratio: f64,
    
    /// Target compaction ratio (how tightly to pack data)
    /// 0.95 means try to use 95% of space with minimal gaps
    pub target_compaction_ratio: f64,
}

impl Default for DefragConfig {
    fn default() -> Self {
        Self {
            fragmentation_threshold: 0.3, // Trigger at 30% fragmentation
            batch_size: 100,
            incremental: true,
            min_free_space_ratio: 0.15, // Need at least 15% free space
            target_compaction_ratio: 0.95,
        }
    }
}

/// Defragmentation statistics
#[derive(Debug, Clone, Default)]
pub struct DefragStats {
    /// Total number of defragmentation runs
    pub total_runs: u64,
    
    /// Total number of entries moved
    pub total_entries_moved: u64,
    
    /// Total bytes moved
    pub total_bytes_moved: u64,
    
    /// Total time spent defragmenting
    pub total_duration: Duration,
    
    /// Last defragmentation time
    pub last_run: Option<Instant>,
    
    /// Fragmentation ratio before last run
    pub last_fragmentation_before: f64,
    
    /// Fragmentation ratio after last run
    pub last_fragmentation_after: f64,
    
    /// Number of failed moves
    pub failed_moves: u64,
}

impl DefragStats {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn record_run(
        &mut self,
        entries_moved: usize,
        bytes_moved: u64,
        duration: Duration,
        frag_before: f64,
        frag_after: f64,
    ) {
        self.total_runs += 1;
        self.total_entries_moved += entries_moved as u64;
        self.total_bytes_moved += bytes_moved;
        self.total_duration += duration;
        self.last_run = Some(Instant::now());
        self.last_fragmentation_before = frag_before;
        self.last_fragmentation_after = frag_after;
    }
    
    pub fn record_failed_move(&mut self) {
        self.failed_moves += 1;
    }
}

/// Represents a gap in the disk space
#[derive(Debug, Clone)]
struct Gap {
    offset: u64,
    size: usize,
}

/// Defragmentation manager
pub struct DefragManager {
    config: DefragConfig,
    stats: DefragStats,
}

impl DefragManager {
    pub fn new(config: DefragConfig) -> Self {
        Self {
            config,
            stats: DefragStats::new(),
        }
    }
    
    pub fn config(&self) -> &DefragConfig {
        &self.config
    }
    
    pub fn stats(&self) -> &DefragStats {
        &self.stats
    }
    
    pub fn stats_mut(&mut self) -> &mut DefragStats {
        &mut self.stats
    }
    
    pub fn update_config(&mut self, config: DefragConfig) {
        self.config = config;
    }
    
    /// Calculate fragmentation ratio
    /// 
    /// Fragmentation is measured as the ratio of wasted space in gaps
    /// to total used space. A higher ratio means more fragmentation.
    /// 
    /// Formula: fragmentation = (total_gap_space - largest_gap) / total_used_space
    /// 
    /// This accounts for the fact that one large gap is not fragmentation,
    /// but many small gaps are.
    pub fn calculate_fragmentation(
        &self,
        directory: &CacheDirectory,
        total_blocks: usize,
        block_size: usize,
        data_offset: u64,
    ) -> f64 {
        if directory.is_empty() {
            return 0.0;
        }
        
        // Collect all allocated regions sorted by offset
        let mut regions: Vec<(u64, u64)> = directory
            .iter()
            .map(|(_, loc)| {
                let relative_offset = loc.offset - data_offset;
                let size = loc.size as u64;
                (relative_offset, size)
            })
            .collect();
        
        regions.sort_by_key(|(offset, _)| *offset);
        
        // Find gaps between allocated regions
        let mut gaps = Vec::new();
        let mut prev_end = 0u64;
        
        for (offset, size) in &regions {
            if *offset > prev_end {
                let gap_size = (*offset - prev_end) as usize;
                gaps.push(Gap {
                    offset: prev_end,
                    size: gap_size,
                });
            }
            prev_end = offset + size;
        }
        
        // Add final gap if any
        let total_size = (total_blocks * block_size) as u64;
        if prev_end < total_size {
            gaps.push(Gap {
                offset: prev_end,
                size: (total_size - prev_end) as usize,
            });
        }
        
        if gaps.is_empty() {
            return 0.0;
        }
        
        // Calculate total gap space and find largest gap
        let total_gap_space: usize = gaps.iter().map(|g| g.size).sum();
        let largest_gap = gaps.iter().map(|g| g.size).max().unwrap_or(0);
        
        // Calculate total used space
        let total_used_space: u64 = regions.iter().map(|(_, size)| size).sum();
        
        if total_used_space == 0 {
            return 0.0;
        }
        
        // Fragmentation is the ratio of "wasted" gap space to used space
        // We subtract the largest gap because one large gap is not fragmentation
        let wasted_space = total_gap_space.saturating_sub(largest_gap);
        let fragmentation = wasted_space as f64 / total_used_space as f64;
        
        fragmentation.min(1.0) // Cap at 1.0
    }
    
    /// Check if defragmentation should be triggered
    pub fn should_defragment(
        &self,
        fragmentation_ratio: f64,
        free_space_ratio: f64,
    ) -> bool {
        fragmentation_ratio >= self.config.fragmentation_threshold
            && free_space_ratio >= self.config.min_free_space_ratio
    }
    
    /// Select entries to move during defragmentation
    /// 
    /// Strategy: Move entries from the end of the disk to fill gaps at the beginning
    /// This compacts data towards the beginning of the disk
    pub fn select_entries_to_move(
        &self,
        directory: &CacheDirectory,
        _total_blocks: usize,
        block_size: usize,
        data_offset: u64,
        max_entries: usize,
    ) -> Vec<(String, DiskLocation)> {
        // Collect all entries with their relative offsets
        let mut entries: Vec<(String, DiskLocation, u64)> = directory
            .iter()
            .map(|(key, loc)| {
                let relative_offset = loc.offset - data_offset;
                (key.clone(), loc.clone(), relative_offset)
            })
            .collect();
        
        // Sort by offset (descending) - we want to move entries from the end
        entries.sort_by(|a, b| b.2.cmp(&a.2));
        
        // Find gaps at the beginning
        let mut regions: Vec<(u64, u64)> = entries
            .iter()
            .map(|(_, loc, rel_offset)| (*rel_offset, loc.size as u64))
            .collect();
        regions.sort_by_key(|(offset, _)| *offset);
        
        let mut gaps = Vec::new();
        let mut prev_end = 0u64;
        
        for (offset, size) in &regions {
            if *offset > prev_end {
                gaps.push(Gap {
                    offset: prev_end,
                    size: (*offset - prev_end) as usize,
                });
            }
            prev_end = offset + size;
        }
        
        // Select entries from the end that can fit in gaps at the beginning
        let mut selected = Vec::new();
        let mut remaining_gaps = gaps;
        
        for (key, loc, _) in entries.iter().take(max_entries) {
            let entry_size = loc.size as usize;
            
            // Find a gap that can fit this entry
            if let Some(gap_idx) = remaining_gaps
                .iter()
                .position(|g| g.size >= entry_size)
            {
                selected.push((key.clone(), loc.clone()));
                
                // Update the gap
                let gap = &mut remaining_gaps[gap_idx];
                gap.offset += entry_size as u64;
                gap.size -= entry_size;
                
                // Remove gap if it's too small to be useful
                if gap.size < block_size {
                    remaining_gaps.remove(gap_idx);
                }
                
                if selected.len() >= max_entries {
                    break;
                }
            }
        }
        
        selected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_fragmentation_calculation_empty() {
        let manager = DefragManager::new(DefragConfig::default());
        let directory = CacheDirectory::new();
        
        let frag = manager.calculate_fragmentation(&directory, 1000, 4096, 0);
        assert_eq!(frag, 0.0);
    }
    
    #[test]
    fn test_fragmentation_calculation_no_gaps() {
        let manager = DefragManager::new(DefragConfig::default());
        let mut directory = CacheDirectory::new();
        
        // Add contiguous entries
        directory.insert("key1".to_string(), DiskLocation {
            offset: 0,
            size: 4096,
            checksum: 0,
            timestamp: 0,
            compressed: false,
            original_size: 0,
        });
        directory.insert("key2".to_string(), DiskLocation {
            offset: 4096,
            size: 4096,
            checksum: 0,
            timestamp: 0,
            compressed: false,
            original_size: 0,
        });
        
        let frag = manager.calculate_fragmentation(&directory, 1000, 4096, 0);
        assert_eq!(frag, 0.0);
    }
    
    #[test]
    fn test_fragmentation_calculation_with_gaps() {
        let manager = DefragManager::new(DefragConfig::default());
        let mut directory = CacheDirectory::new();
        
        // Add entries with gaps
        directory.insert("key1".to_string(), DiskLocation {
            offset: 0,
            size: 4096,
            checksum: 0,
            timestamp: 0,
            compressed: false,
            original_size: 0,
        });
        directory.insert("key2".to_string(), DiskLocation {
            offset: 8192, // Gap of 4096 bytes
            size: 4096,
            checksum: 0,
            timestamp: 0,
            compressed: false,
            original_size: 0,
        });
        directory.insert("key3".to_string(), DiskLocation {
            offset: 16384, // Another gap of 4096 bytes
            size: 4096,
            checksum: 0,
            timestamp: 0,
            compressed: false,
            original_size: 0,
        });
        
        let frag = manager.calculate_fragmentation(&directory, 1000, 4096, 0);
        
        // Total used: 12288 bytes
        // Total gaps: 8192 bytes (two 4096-byte gaps) + large gap at end
        // Largest gap: the one at the end
        // Wasted space: 8192 bytes (the two small gaps)
        // Fragmentation: 8192 / 12288 = 0.666...
        assert!(frag > 0.6 && frag < 0.7);
    }
    
    #[test]
    fn test_should_defragment() {
        let manager = DefragManager::new(DefragConfig {
            fragmentation_threshold: 0.3,
            min_free_space_ratio: 0.15,
            ..Default::default()
        });
        
        // Should trigger: high fragmentation, enough free space
        assert!(manager.should_defragment(0.4, 0.2));
        
        // Should not trigger: low fragmentation
        assert!(!manager.should_defragment(0.2, 0.2));
        
        // Should not trigger: not enough free space
        assert!(!manager.should_defragment(0.4, 0.1));
    }
    
    #[test]
    fn test_select_entries_to_move() {
        let manager = DefragManager::new(DefragConfig::default());
        let mut directory = CacheDirectory::new();
        
        // Create a fragmented layout
        directory.insert("key1".to_string(), DiskLocation {
            offset: 0,
            size: 4096,
            checksum: 0,
            timestamp: 0,
            compressed: false,
            original_size: 0,
        });
        directory.insert("key2".to_string(), DiskLocation {
            offset: 16384, // Gap before this
            size: 4096,
            checksum: 0,
            timestamp: 0,
            compressed: false,
            original_size: 0,
        });
        directory.insert("key3".to_string(), DiskLocation {
            offset: 32768, // Gap before this
            size: 4096,
            checksum: 0,
            timestamp: 0,
            compressed: false,
            original_size: 0,
        });
        
        let selected = manager.select_entries_to_move(&directory, 1000, 4096, 0, 10);
        
        // Should select entries from the end to fill gaps
        assert!(!selected.is_empty());
    }
}
