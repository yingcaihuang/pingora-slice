//! Prefetch optimization for raw disk cache
//!
//! This module implements access pattern detection and prefetch strategies
//! to reduce read latency by predicting and pre-loading data.

use super::DiskLocation;
use bytes::Bytes;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::debug;

/// Access pattern types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessPattern {
    /// Random access - no clear pattern
    Random,
    /// Sequential access - keys accessed in order
    Sequential,
    /// Temporal locality - same keys accessed repeatedly
    Temporal,
}

/// Access record for pattern detection
#[derive(Debug, Clone)]
struct AccessRecord {
    key: String,
    timestamp: Instant,
    offset: u64,
}

/// Prefetch strategy configuration
#[derive(Debug, Clone)]
pub struct PrefetchConfig {
    /// Enable prefetch optimization
    pub enabled: bool,
    /// Maximum number of entries to prefetch
    pub max_prefetch_entries: usize,
    /// Prefetch cache size (number of entries)
    pub cache_size: usize,
    /// Window size for pattern detection (number of accesses)
    pub pattern_window_size: usize,
    /// Sequential threshold (ratio of sequential accesses)
    pub sequential_threshold: f64,
    /// Temporal threshold (ratio of repeated accesses)
    pub temporal_threshold: f64,
}

impl Default for PrefetchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_prefetch_entries: 4,
            cache_size: 100,
            pattern_window_size: 20,
            sequential_threshold: 0.7,
            temporal_threshold: 0.5,
        }
    }
}

/// Prefetch cache entry
#[derive(Debug, Clone)]
struct PrefetchEntry {
    key: String,
    data: Bytes,
    location: DiskLocation,
    timestamp: Instant,
}

/// Access pattern detector
pub struct PatternDetector {
    /// Recent access history
    history: VecDeque<AccessRecord>,
    /// Maximum history size
    max_history: usize,
    /// Configuration
    config: PrefetchConfig,
}

impl PatternDetector {
    pub fn new(config: PrefetchConfig) -> Self {
        Self {
            history: VecDeque::with_capacity(config.pattern_window_size),
            max_history: config.pattern_window_size,
            config,
        }
    }

    /// Record an access
    pub fn record_access(&mut self, key: String, offset: u64) {
        let record = AccessRecord {
            key,
            timestamp: Instant::now(),
            offset,
        };

        self.history.push_back(record);

        // Keep history size bounded
        while self.history.len() > self.max_history {
            self.history.pop_front();
        }
    }

    /// Detect current access pattern
    pub fn detect_pattern(&self) -> AccessPattern {
        if self.history.len() < 3 {
            return AccessPattern::Random;
        }

        let sequential_score = self.calculate_sequential_score();
        let temporal_score = self.calculate_temporal_score();

        debug!(
            "Pattern scores - sequential: {:.2}, temporal: {:.2}",
            sequential_score, temporal_score
        );

        if sequential_score >= self.config.sequential_threshold {
            AccessPattern::Sequential
        } else if temporal_score >= self.config.temporal_threshold {
            AccessPattern::Temporal
        } else {
            AccessPattern::Random
        }
    }

    /// Calculate sequential access score (0.0 to 1.0)
    fn calculate_sequential_score(&self) -> f64 {
        if self.history.len() < 2 {
            return 0.0;
        }

        let mut sequential_count = 0;
        let mut total_pairs = 0;

        for window in self.history.iter().collect::<Vec<_>>().windows(2) {
            let prev = window[0];
            let curr = window[1];

            total_pairs += 1;

            // Check if offsets are increasing (sequential)
            if curr.offset > prev.offset {
                let diff = curr.offset - prev.offset;
                // Consider sequential if within reasonable range (e.g., < 10MB)
                if diff < 10 * 1024 * 1024 {
                    sequential_count += 1;
                }
            }
        }

        if total_pairs == 0 {
            0.0
        } else {
            sequential_count as f64 / total_pairs as f64
        }
    }

    /// Calculate temporal locality score (0.0 to 1.0)
    fn calculate_temporal_score(&self) -> f64 {
        if self.history.is_empty() {
            return 0.0;
        }

        // Count unique keys vs total accesses
        let mut key_counts: HashMap<&str, usize> = HashMap::new();
        for record in &self.history {
            *key_counts.entry(&record.key).or_insert(0) += 1;
        }

        // Calculate how many accesses are repeats
        let total_accesses = self.history.len();
        let unique_keys = key_counts.len();
        let repeat_accesses = total_accesses - unique_keys;

        repeat_accesses as f64 / total_accesses as f64
    }

    /// Predict next keys to prefetch based on pattern
    pub fn predict_next_keys(&self, current_key: &str, all_keys: &[String]) -> Vec<String> {
        let pattern = self.detect_pattern();

        match pattern {
            AccessPattern::Sequential => self.predict_sequential(current_key, all_keys),
            AccessPattern::Temporal => self.predict_temporal(),
            AccessPattern::Random => Vec::new(),
        }
    }

    /// Predict next keys for sequential pattern
    fn predict_sequential(&self, current_key: &str, all_keys: &[String]) -> Vec<String> {
        // Find current key position
        if let Some(pos) = all_keys.iter().position(|k| k == current_key) {
            // Prefetch next N keys
            let end = (pos + 1 + self.config.max_prefetch_entries).min(all_keys.len());
            all_keys[pos + 1..end].to_vec()
        } else {
            Vec::new()
        }
    }

    /// Predict next keys for temporal pattern
    fn predict_temporal(&self) -> Vec<String> {
        // Find most frequently accessed keys in recent history
        let mut key_counts: HashMap<String, usize> = HashMap::new();
        for record in &self.history {
            *key_counts.entry(record.key.clone()).or_insert(0) += 1;
        }

        // Sort by frequency and return top N
        let mut keys: Vec<_> = key_counts.into_iter().collect();
        keys.sort_by(|a, b| b.1.cmp(&a.1));

        keys.into_iter()
            .take(self.config.max_prefetch_entries)
            .map(|(k, _)| k)
            .collect()
    }
}

/// Prefetch cache manager
pub struct PrefetchCache {
    /// Cached prefetched data
    cache: HashMap<String, PrefetchEntry>,
    /// LRU order
    lru: VecDeque<String>,
    /// Maximum cache size
    max_size: usize,
    /// Statistics
    hits: u64,
    misses: u64,
}

impl PrefetchCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: HashMap::with_capacity(max_size),
            lru: VecDeque::with_capacity(max_size),
            max_size,
            hits: 0,
            misses: 0,
        }
    }

    /// Insert prefetched data
    pub fn insert(&mut self, key: String, data: Bytes, location: DiskLocation) {
        // Evict if at capacity
        while self.cache.len() >= self.max_size {
            if let Some(old_key) = self.lru.pop_front() {
                self.cache.remove(&old_key);
            }
        }

        let entry = PrefetchEntry {
            key: key.clone(),
            data,
            location,
            timestamp: Instant::now(),
        };

        self.cache.insert(key.clone(), entry);
        self.lru.push_back(key);
    }

    /// Get prefetched data
    pub fn get(&mut self, key: &str) -> Option<Bytes> {
        if let Some(entry) = self.cache.get(key) {
            self.hits += 1;

            // Move to back of LRU
            self.lru.retain(|k| k != key);
            self.lru.push_back(key.to_string());

            Some(entry.data.clone())
        } else {
            self.misses += 1;
            None
        }
    }

    /// Remove entry from cache
    pub fn remove(&mut self, key: &str) {
        if self.cache.remove(key).is_some() {
            self.lru.retain(|k| k != key);
        }
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.cache.clear();
        self.lru.clear();
    }

    /// Get cache statistics
    pub fn stats(&self) -> PrefetchStats {
        PrefetchStats {
            cache_size: self.cache.len(),
            max_size: self.max_size,
            hits: self.hits,
            misses: self.misses,
            hit_rate: if self.hits + self.misses > 0 {
                self.hits as f64 / (self.hits + self.misses) as f64
            } else {
                0.0
            },
        }
    }
}

/// Prefetch statistics
#[derive(Debug, Clone)]
pub struct PrefetchStats {
    pub cache_size: usize,
    pub max_size: usize,
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
}

/// Prefetch manager coordinates pattern detection and caching
pub struct PrefetchManager {
    detector: Arc<RwLock<PatternDetector>>,
    cache: Arc<RwLock<PrefetchCache>>,
    config: PrefetchConfig,
}

impl PrefetchManager {
    pub fn new(config: PrefetchConfig) -> Self {
        let cache_size = config.cache_size;
        Self {
            detector: Arc::new(RwLock::new(PatternDetector::new(config.clone()))),
            cache: Arc::new(RwLock::new(PrefetchCache::new(cache_size))),
            config,
        }
    }

    /// Record an access and update pattern detection
    pub async fn record_access(&self, key: String, offset: u64) {
        if !self.config.enabled {
            return;
        }

        let mut detector = self.detector.write().await;
        detector.record_access(key, offset);
    }

    /// Check if data is in prefetch cache
    pub async fn get_prefetched(&self, key: &str) -> Option<Bytes> {
        if !self.config.enabled {
            return None;
        }

        let mut cache = self.cache.write().await;
        cache.get(key)
    }

    /// Store prefetched data
    pub async fn store_prefetched(&self, key: String, data: Bytes, location: DiskLocation) {
        if !self.config.enabled {
            return;
        }

        let mut cache = self.cache.write().await;
        cache.insert(key, data, location);
    }

    /// Get predicted keys to prefetch
    pub async fn predict_prefetch_keys(
        &self,
        current_key: &str,
        all_keys: &[String],
    ) -> Vec<String> {
        if !self.config.enabled {
            return Vec::new();
        }

        let detector = self.detector.read().await;
        detector.predict_next_keys(current_key, all_keys)
    }

    /// Get current access pattern
    pub async fn current_pattern(&self) -> AccessPattern {
        let detector = self.detector.read().await;
        detector.detect_pattern()
    }

    /// Get prefetch statistics
    pub async fn stats(&self) -> PrefetchStats {
        let cache = self.cache.read().await;
        cache.stats()
    }

    /// Clear prefetch cache
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_detector_sequential() {
        let config = PrefetchConfig::default();
        let mut detector = PatternDetector::new(config);

        // Simulate sequential access
        for i in 0..10 {
            detector.record_access(format!("key_{}", i), i * 1000);
        }

        let pattern = detector.detect_pattern();
        assert_eq!(pattern, AccessPattern::Sequential);
    }

    #[test]
    fn test_pattern_detector_temporal() {
        let config = PrefetchConfig::default();
        let mut detector = PatternDetector::new(config);

        // Simulate repeated access to same keys with non-sequential offsets
        for _ in 0..5 {
            detector.record_access("key_1".to_string(), 1000);
            detector.record_access("key_2".to_string(), 5000);
            detector.record_access("key_1".to_string(), 1000);
            detector.record_access("key_3".to_string(), 9000);
        }

        let pattern = detector.detect_pattern();
        assert_eq!(pattern, AccessPattern::Temporal);
    }

    #[test]
    fn test_pattern_detector_random() {
        let config = PrefetchConfig::default();
        let mut detector = PatternDetector::new(config);

        // Simulate random access
        detector.record_access("key_5".to_string(), 5000);
        detector.record_access("key_1".to_string(), 1000);
        detector.record_access("key_9".to_string(), 9000);
        detector.record_access("key_3".to_string(), 3000);

        let pattern = detector.detect_pattern();
        assert_eq!(pattern, AccessPattern::Random);
    }

    #[test]
    fn test_prefetch_cache() {
        let mut cache = PrefetchCache::new(3);

        let loc1 = DiskLocation::new(1000, b"data1");
        let loc2 = DiskLocation::new(2000, b"data2");
        let loc3 = DiskLocation::new(3000, b"data3");
        let loc4 = DiskLocation::new(4000, b"data4");

        // Insert entries
        cache.insert("key1".to_string(), Bytes::from("data1"), loc1);
        cache.insert("key2".to_string(), Bytes::from("data2"), loc2);
        cache.insert("key3".to_string(), Bytes::from("data3"), loc3);

        // Cache should be full
        assert_eq!(cache.cache.len(), 3);

        // Get should work
        assert!(cache.get("key1").is_some());
        assert!(cache.get("key2").is_some());

        // Insert one more should evict oldest (key3 since key1 was accessed)
        cache.insert("key4".to_string(), Bytes::from("data4"), loc4);
        assert_eq!(cache.cache.len(), 3);
        assert!(cache.get("key3").is_none());
        assert!(cache.get("key4").is_some());
    }

    #[test]
    fn test_sequential_prediction() {
        let config = PrefetchConfig {
            max_prefetch_entries: 3,
            ..Default::default()
        };
        let mut detector = PatternDetector::new(config);

        // Create sequential pattern
        for i in 0..10 {
            detector.record_access(format!("key_{}", i), i * 1000);
        }

        let all_keys: Vec<String> = (0..20).map(|i| format!("key_{}", i)).collect();
        let predictions = detector.predict_next_keys("key_5", &all_keys);

        assert_eq!(predictions.len(), 3);
        assert_eq!(predictions[0], "key_6");
        assert_eq!(predictions[1], "key_7");
        assert_eq!(predictions[2], "key_8");
    }

    #[tokio::test]
    async fn test_prefetch_manager() {
        let config = PrefetchConfig::default();
        let manager = PrefetchManager::new(config);

        // Record sequential accesses
        for i in 0..10 {
            manager
                .record_access(format!("key_{}", i), i * 1000)
                .await;
        }

        // Check pattern
        let pattern = manager.current_pattern().await;
        assert_eq!(pattern, AccessPattern::Sequential);

        // Store prefetched data
        let loc = DiskLocation::new(1000, b"test");
        manager
            .store_prefetched("test_key".to_string(), Bytes::from("test"), loc)
            .await;

        // Retrieve prefetched data
        let data = manager.get_prefetched("test_key").await;
        assert!(data.is_some());
        assert_eq!(data.unwrap(), Bytes::from("test"));

        // Check stats
        let stats = manager.stats().await;
        assert_eq!(stats.hits, 1);
    }
}
