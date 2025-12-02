//! Smart Garbage Collection for Raw Disk Cache
//!
//! This module implements intelligent garbage collection with multiple
//! eviction strategies, adaptive triggering, and performance monitoring.

use super::{CacheDirectory, DiskLocation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{debug, info};

/// Eviction strategy for garbage collection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvictionStrategy {
    /// Least Recently Used - evict entries that haven't been accessed recently
    LRU,
    /// Least Frequently Used - evict entries with lowest access count
    LFU,
    /// First In First Out - evict oldest entries by insertion time
    FIFO,
}

/// GC trigger configuration
#[derive(Debug, Clone)]
pub struct GCTriggerConfig {
    /// Minimum free space ratio to maintain (0.0 - 1.0)
    pub min_free_ratio: f64,
    /// Target free space ratio after GC (0.0 - 1.0)
    pub target_free_ratio: f64,
    /// Enable adaptive triggering based on allocation patterns
    pub adaptive: bool,
    /// Minimum time between GC runs
    pub min_interval: Duration,
}

impl Default for GCTriggerConfig {
    fn default() -> Self {
        Self {
            min_free_ratio: 0.1,  // Trigger when < 10% free
            target_free_ratio: 0.3, // Target 30% free after GC
            adaptive: true,
            min_interval: Duration::from_secs(60),
        }
    }
}

/// GC configuration
#[derive(Debug, Clone)]
pub struct GCConfig {
    /// Eviction strategy to use
    pub strategy: EvictionStrategy,
    /// Trigger configuration
    pub trigger: GCTriggerConfig,
    /// Enable incremental GC (process in batches)
    pub incremental: bool,
    /// Batch size for incremental GC
    pub batch_size: usize,
    /// TTL in seconds (0 = no TTL-based eviction)
    pub ttl_secs: u64,
}

impl Default for GCConfig {
    fn default() -> Self {
        Self {
            strategy: EvictionStrategy::LRU,
            trigger: GCTriggerConfig::default(),
            incremental: true,
            batch_size: 100,
            ttl_secs: 0,
        }
    }
}

/// GC performance metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GCMetrics {
    /// Total number of GC runs
    pub total_runs: u64,
    /// Total entries evicted
    pub total_evicted: u64,
    /// Total bytes freed
    pub total_bytes_freed: u64,
    /// Total time spent in GC
    pub total_duration: Duration,
    /// Last GC run time (not serialized)
    #[serde(skip)]
    pub last_run: Option<Instant>,
    /// Last GC duration
    pub last_duration: Option<Duration>,
    /// Last GC entries evicted
    pub last_evicted: usize,
    /// Number of adaptive adjustments
    pub adaptive_adjustments: u64,
}

impl GCMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_run(&mut self, evicted: usize, bytes_freed: u64, duration: Duration) {
        self.total_runs += 1;
        self.total_evicted += evicted as u64;
        self.total_bytes_freed += bytes_freed;
        self.total_duration += duration;
        self.last_run = Some(Instant::now());
        self.last_duration = Some(duration);
        self.last_evicted = evicted;
    }

    pub fn record_adaptive_adjustment(&mut self) {
        self.adaptive_adjustments += 1;
    }

    pub fn average_duration(&self) -> Duration {
        if self.total_runs > 0 {
            self.total_duration / self.total_runs as u32
        } else {
            Duration::ZERO
        }
    }

    pub fn average_evicted(&self) -> f64 {
        if self.total_runs > 0 {
            self.total_evicted as f64 / self.total_runs as f64
        } else {
            0.0
        }
    }
}



/// Smart garbage collector
pub struct SmartGC {
    config: GCConfig,
    metrics: GCMetrics,
    /// Access frequency tracking for LFU
    access_freq: HashMap<String, u64>,
    /// Insertion order tracking for FIFO
    insertion_order: Vec<String>,
    /// Adaptive state
    adaptive_state: AdaptiveState,
}

/// Adaptive GC state
#[derive(Debug, Clone)]
struct AdaptiveState {
    /// Recent allocation failure count
    allocation_failures: u64,
    /// Recent allocation success count
    allocation_successes: u64,
    /// Adjusted min_free_ratio
    adjusted_min_free_ratio: f64,
    /// Last adjustment time
    last_adjustment: Option<Instant>,
}

impl SmartGC {
    pub fn new(config: GCConfig) -> Self {
        Self {
            adaptive_state: AdaptiveState {
                allocation_failures: 0,
                allocation_successes: 0,
                adjusted_min_free_ratio: config.trigger.min_free_ratio,
                last_adjustment: None,
            },
            config,
            metrics: GCMetrics::new(),
            access_freq: HashMap::new(),
            insertion_order: Vec::new(),
        }
    }

    /// Check if GC should be triggered
    pub fn should_trigger(&mut self, free_ratio: f64) -> bool {
        // Check minimum interval
        if let Some(last_run) = self.metrics.last_run {
            if last_run.elapsed() < self.config.trigger.min_interval {
                return false;
            }
        }

        // Use adaptive threshold if enabled
        let threshold = if self.config.trigger.adaptive {
            self.adaptive_state.adjusted_min_free_ratio
        } else {
            self.config.trigger.min_free_ratio
        };

        free_ratio < threshold
    }

    /// Record an insertion for FIFO tracking
    pub fn record_insertion(&mut self, key: String) {
        if self.config.strategy == EvictionStrategy::FIFO {
            self.insertion_order.push(key);
        }
    }

    /// Record an access for LFU tracking
    pub fn record_access(&mut self, key: &str) {
        if self.config.strategy == EvictionStrategy::LFU {
            *self.access_freq.entry(key.to_string()).or_insert(0) += 1;
        }
    }

    /// Record allocation result for adaptive GC
    pub fn record_allocation(&mut self, success: bool) {
        if !self.config.trigger.adaptive {
            return;
        }

        if success {
            self.adaptive_state.allocation_successes += 1;
        } else {
            self.adaptive_state.allocation_failures += 1;
        }

        // Adjust threshold if we see patterns
        let total = self.adaptive_state.allocation_failures 
            + self.adaptive_state.allocation_successes;
        
        if total >= 100 {
            let failure_rate = self.adaptive_state.allocation_failures as f64 / total as f64;
            
            // If failure rate is high, increase min_free_ratio
            if failure_rate > 0.1 {
                let old_ratio = self.adaptive_state.adjusted_min_free_ratio;
                self.adaptive_state.adjusted_min_free_ratio = 
                    (old_ratio * 1.2).min(0.5); // Cap at 50%
                
                info!(
                    "Adaptive GC: increased min_free_ratio from {:.2}% to {:.2}% (failure rate: {:.2}%)",
                    old_ratio * 100.0,
                    self.adaptive_state.adjusted_min_free_ratio * 100.0,
                    failure_rate * 100.0
                );
                
                self.metrics.record_adaptive_adjustment();
            } else if failure_rate < 0.01 {
                // If failure rate is very low, decrease min_free_ratio
                let old_ratio = self.adaptive_state.adjusted_min_free_ratio;
                self.adaptive_state.adjusted_min_free_ratio = 
                    (old_ratio * 0.9).max(self.config.trigger.min_free_ratio);
                
                debug!(
                    "Adaptive GC: decreased min_free_ratio from {:.2}% to {:.2}%",
                    old_ratio * 100.0,
                    self.adaptive_state.adjusted_min_free_ratio * 100.0
                );
                
                self.metrics.record_adaptive_adjustment();
            }
            
            // Reset counters
            self.adaptive_state.allocation_failures = 0;
            self.adaptive_state.allocation_successes = 0;
            self.adaptive_state.last_adjustment = Some(Instant::now());
        }
    }

    /// Select victims for eviction
    pub fn select_victims(
        &mut self,
        directory: &CacheDirectory,
        target_count: usize,
        _block_size: usize,
    ) -> Vec<(String, DiskLocation)> {
        let start = Instant::now();
        
        // First, try to select expired entries if TTL is configured
        let mut victims = if self.config.ttl_secs > 0 {
            self.select_expired_entries(directory, target_count)
        } else {
            Vec::new()
        };

        // If we need more victims, use the configured strategy
        if victims.len() < target_count {
            let remaining = target_count - victims.len();
            let strategy_victims = match self.config.strategy {
                EvictionStrategy::LRU => self.select_lru_victims(directory, remaining),
                EvictionStrategy::LFU => self.select_lfu_victims(directory, remaining),
                EvictionStrategy::FIFO => self.select_fifo_victims(directory, remaining),
            };
            victims.extend(strategy_victims);
        }

        debug!(
            "Selected {} victims using {:?} strategy (TTL: {} secs) in {:?}",
            victims.len(),
            self.config.strategy,
            self.config.ttl_secs,
            start.elapsed()
        );

        victims
    }

    /// Select LRU victims
    fn select_lru_victims(
        &self,
        directory: &CacheDirectory,
        target_count: usize,
    ) -> Vec<(String, DiskLocation)> {
        // Get LRU order from directory
        let lru_keys = directory.select_lru_victims(1.0); // Get all keys in LRU order
        
        let mut victims = Vec::new();
        for key in lru_keys.iter().take(target_count) {
            if let Some(location) = directory.get(key) {
                victims.push((key.clone(), location.clone()));
            }
        }
        
        victims
    }

    /// Select LFU victims
    fn select_lfu_victims(
        &self,
        directory: &CacheDirectory,
        target_count: usize,
    ) -> Vec<(String, DiskLocation)> {
        // Collect all entries with their access frequencies
        let mut entries: Vec<_> = directory
            .iter()
            .map(|(key, location)| {
                let freq = self.access_freq.get(key).copied().unwrap_or(0);
                (key.clone(), location.clone(), freq)
            })
            .collect();

        // Sort by frequency (ascending)
        entries.sort_by_key(|(_, _, freq)| *freq);

        // Take the least frequently used
        entries
            .into_iter()
            .take(target_count)
            .map(|(key, location, _)| (key, location))
            .collect()
    }

    /// Select FIFO victims
    fn select_fifo_victims(
        &self,
        directory: &CacheDirectory,
        target_count: usize,
    ) -> Vec<(String, DiskLocation)> {
        // Use insertion order
        let mut victims = Vec::new();
        
        for key in self.insertion_order.iter().take(target_count) {
            if let Some(location) = directory.get(key) {
                victims.push((key.clone(), location.clone()));
            }
        }
        
        victims
    }

    /// Select expired entries as victims
    fn select_expired_entries(
        &self,
        directory: &CacheDirectory,
        target_count: usize,
    ) -> Vec<(String, DiskLocation)> {
        directory
            .iter()
            .filter(|(_, location)| location.is_expired(self.config.ttl_secs))
            .take(target_count)
            .map(|(key, location)| (key.clone(), location.clone()))
            .collect()
    }

    /// Clean up tracking data for removed keys
    pub fn cleanup_removed_keys(&mut self, keys: &[String]) {
        for key in keys {
            self.access_freq.remove(key);
            self.insertion_order.retain(|k| k != key);
        }
    }

    /// Get GC metrics
    pub fn metrics(&self) -> &GCMetrics {
        &self.metrics
    }

    /// Get mutable GC metrics
    pub fn metrics_mut(&mut self) -> &mut GCMetrics {
        &mut self.metrics
    }

    /// Get current configuration
    pub fn config(&self) -> &GCConfig {
        &self.config
    }

    /// Update configuration
    pub fn update_config(&mut self, config: GCConfig) {
        self.config = config;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raw_disk::CacheDirectory;

    #[test]
    fn test_gc_trigger() {
        let config = GCConfig {
            trigger: GCTriggerConfig {
                min_free_ratio: 0.2,
                target_free_ratio: 0.3,
                adaptive: false,
                min_interval: Duration::from_secs(1),
            },
            ..Default::default()
        };

        let mut gc = SmartGC::new(config);

        // Should trigger when below threshold
        assert!(gc.should_trigger(0.15));
        assert!(gc.should_trigger(0.1));

        // Should not trigger when above threshold
        assert!(!gc.should_trigger(0.25));
        assert!(!gc.should_trigger(0.5));
    }

    #[test]
    fn test_adaptive_gc() {
        let config = GCConfig {
            trigger: GCTriggerConfig {
                min_free_ratio: 0.2,
                target_free_ratio: 0.3,
                adaptive: true,
                min_interval: Duration::from_secs(0),
            },
            ..Default::default()
        };

        let mut gc = SmartGC::new(config);

        // Record many allocation failures
        for _ in 0..90 {
            gc.record_allocation(false);
        }
        for _ in 0..10 {
            gc.record_allocation(true);
        }

        // Threshold should increase
        assert!(gc.adaptive_state.adjusted_min_free_ratio > 0.2);
    }

    #[test]
    fn test_lru_victim_selection() {
        let config = GCConfig {
            strategy: EvictionStrategy::LRU,
            ..Default::default()
        };

        let mut gc = SmartGC::new(config);
        let mut directory = CacheDirectory::new();

        // Add some entries
        for i in 0..10 {
            let key = format!("key{}", i);
            let location = DiskLocation::new(i * 1000, b"data");
            directory.insert(key, location);
        }

        // Touch some keys to update LRU
        directory.touch("key5");
        directory.touch("key7");
        directory.touch("key2");

        // Select victims
        let victims = gc.select_victims(&directory, 3, 4096);
        assert_eq!(victims.len(), 3);

        // The victims should be the least recently used
        // (not key5, key7, or key2)
        for (key, _) in &victims {
            assert!(key != "key5" && key != "key7" && key != "key2");
        }
    }

    #[test]
    fn test_lfu_victim_selection() {
        let config = GCConfig {
            strategy: EvictionStrategy::LFU,
            ..Default::default()
        };

        let mut gc = SmartGC::new(config);
        let mut directory = CacheDirectory::new();

        // Add entries
        for i in 0..5 {
            let key = format!("key{}", i);
            let location = DiskLocation::new(i * 1000, b"data");
            directory.insert(key.clone(), location);
            gc.record_insertion(key);
        }

        // Record different access frequencies
        for _ in 0..10 {
            gc.record_access("key0");
        }
        for _ in 0..5 {
            gc.record_access("key1");
        }
        for _ in 0..2 {
            gc.record_access("key2");
        }
        // key3 and key4 have 0 accesses

        // Select victims
        let victims = gc.select_victims(&directory, 2, 4096);
        assert_eq!(victims.len(), 2);

        // Should select key3 and key4 (least frequently used)
        let victim_keys: Vec<_> = victims.iter().map(|(k, _)| k.as_str()).collect();
        assert!(victim_keys.contains(&"key3") || victim_keys.contains(&"key4"));
    }

    #[test]
    fn test_fifo_victim_selection() {
        let config = GCConfig {
            strategy: EvictionStrategy::FIFO,
            ..Default::default()
        };

        let mut gc = SmartGC::new(config);
        let mut directory = CacheDirectory::new();

        // Add entries in order
        for i in 0..5 {
            let key = format!("key{}", i);
            let location = DiskLocation::new(i * 1000, b"data");
            directory.insert(key.clone(), location);
            gc.record_insertion(key);
        }

        // Select victims
        let victims = gc.select_victims(&directory, 2, 4096);
        assert_eq!(victims.len(), 2);

        // Should select key0 and key1 (first in)
        assert_eq!(victims[0].0, "key0");
        assert_eq!(victims[1].0, "key1");
    }

    #[test]
    fn test_gc_metrics() {
        let mut metrics = GCMetrics::new();

        metrics.record_run(10, 1024, Duration::from_millis(50));
        metrics.record_run(15, 2048, Duration::from_millis(75));

        assert_eq!(metrics.total_runs, 2);
        assert_eq!(metrics.total_evicted, 25);
        assert_eq!(metrics.total_bytes_freed, 3072);
        assert_eq!(metrics.last_evicted, 15);
    }
}
