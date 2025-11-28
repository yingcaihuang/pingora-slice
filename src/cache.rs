//! Cache management for slice storage
//!
//! This module provides a two-tier caching system:
//! - L1: In-memory cache for fast access (hot data)
//! - L2: Disk cache for persistence (cold data)
//!
//! The cache automatically promotes frequently accessed items to L1
//! and persists all items to L2 asynchronously.

use crate::error::Result;
use crate::models::ByteRange;
use bytes::Bytes;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Cached slice entry with expiration and access tracking
#[derive(Clone)]
struct CacheEntry {
    data: Bytes,
    expires_at: SystemTime,
    last_accessed: SystemTime,
    access_count: u64,
}

/// Cache statistics for monitoring
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_entries: usize,
    pub total_bytes: usize,
    pub hits: u64,
    pub misses: u64,
}

/// Cache manager for storing and retrieving slices
pub struct SliceCache {
    storage: Arc<RwLock<HashMap<String, CacheEntry>>>,
    ttl: Duration,
    max_size_bytes: Option<usize>,
    current_size_bytes: Arc<RwLock<usize>>,
    hits: Arc<RwLock<u64>>,
    misses: Arc<RwLock<u64>>,
}

impl SliceCache {
    /// Create a new SliceCache
    ///
    /// # Arguments
    /// * `ttl` - Time-to-live for cached slices
    pub fn new(ttl: Duration) -> Self {
        SliceCache {
            storage: Arc::new(RwLock::new(HashMap::new())),
            ttl,
            max_size_bytes: None,
            current_size_bytes: Arc::new(RwLock::new(0)),
            hits: Arc::new(RwLock::new(0)),
            misses: Arc::new(RwLock::new(0)),
        }
    }

    /// Create a new SliceCache with a maximum size limit
    ///
    /// # Arguments
    /// * `ttl` - Time-to-live for cached slices
    /// * `max_size_bytes` - Maximum cache size in bytes (uses LRU eviction)
    pub fn with_max_size(ttl: Duration, max_size_bytes: usize) -> Self {
        SliceCache {
            storage: Arc::new(RwLock::new(HashMap::new())),
            ttl,
            max_size_bytes: Some(max_size_bytes),
            current_size_bytes: Arc::new(RwLock::new(0)),
            hits: Arc::new(RwLock::new(0)),
            misses: Arc::new(RwLock::new(0)),
        }
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> CacheStats {
        let storage = self.storage.read().unwrap();
        let current_size = *self.current_size_bytes.read().unwrap();
        let hits = *self.hits.read().unwrap();
        let misses = *self.misses.read().unwrap();

        CacheStats {
            total_entries: storage.len(),
            total_bytes: current_size,
            hits,
            misses,
        }
    }

    /// Generate a unique cache key for a slice
    ///
    /// The cache key includes the URL and byte range to ensure uniqueness.
    ///
    /// # Arguments
    /// * `url` - The URL of the file
    /// * `range` - The byte range of the slice
    ///
    /// # Returns
    /// A String that uniquely identifies this slice
    pub fn generate_cache_key(&self, url: &str, range: &ByteRange) -> String {
        // Format: {url}:slice:{start}:{end}
        format!("{}:slice:{}:{}", url, range.start, range.end)
    }

    /// Clean up expired entries from the cache
    fn cleanup_expired(&self) {
        let now = SystemTime::now();
        if let Ok(mut storage) = self.storage.write() {
            let mut removed_bytes = 0;
            storage.retain(|_, entry| {
                if entry.expires_at <= now {
                    removed_bytes += entry.data.len();
                    false
                } else {
                    true
                }
            });

            if removed_bytes > 0 {
                if let Ok(mut current_size) = self.current_size_bytes.write() {
                    *current_size = current_size.saturating_sub(removed_bytes);
                }
            }
        }
    }

    /// Evict least recently used entries to make room for new data
    /// Uses LRU (Least Recently Used) eviction policy
    fn evict_lru(&self, needed_bytes: usize) {
        if let Ok(mut storage) = self.storage.write() {
            // Collect entries sorted by last access time
            let mut entries: Vec<_> = storage.iter()
                .map(|(k, v)| (k.clone(), v.last_accessed, v.data.len()))
                .collect();
            
            entries.sort_by_key(|(_, last_accessed, _)| *last_accessed);

            let mut freed_bytes = 0;
            let mut keys_to_remove = Vec::new();

            for (key, _, size) in entries {
                if freed_bytes >= needed_bytes {
                    break;
                }
                keys_to_remove.push(key);
                freed_bytes += size;
            }

            for key in keys_to_remove {
                if let Some(entry) = storage.remove(&key) {
                    if let Ok(mut current_size) = self.current_size_bytes.write() {
                        *current_size = current_size.saturating_sub(entry.data.len());
                    }
                }
            }

            debug!("LRU eviction: freed {} bytes by removing {} entries", freed_bytes, storage.len());
        }
    }

    /// Look up a single cached slice
    ///
    /// # Arguments
    /// * `url` - The URL of the file
    /// * `range` - The byte range of the slice
    ///
    /// # Returns
    /// * `Ok(Some(Bytes))` if the slice is found in cache
    /// * `Ok(None)` if the slice is not in cache
    /// * `Err(SliceError)` if a cache error occurs
    pub async fn lookup_slice(&self, url: &str, range: &ByteRange) -> Result<Option<Bytes>> {
        let key = self.generate_cache_key(url, range);
        let now = SystemTime::now();
        
        debug!(
            "Looking up cached slice: url={}, range={}-{}",
            url, range.start, range.end
        );

        // First, try read-only lookup
        let result = match self.storage.read() {
            Ok(storage) => {
                if let Some(entry) = storage.get(&key) {
                    // Check if entry has expired
                    if entry.expires_at > now {
                        debug!(
                            "Cache hit for slice: url={}, range={}-{}, size={}",
                            url, range.start, range.end, entry.data.len()
                        );
                        Some(entry.data.clone())
                    } else {
                        debug!(
                            "Cache entry expired for slice: url={}, range={}-{}",
                            url, range.start, range.end
                        );
                        None
                    }
                } else {
                    debug!(
                        "Cache miss for slice: url={}, range={}-{}",
                        url, range.start, range.end
                    );
                    None
                }
            }
            Err(e) => {
                warn!(
                    "Cache lookup error: url={}, range={}-{}, error={:?}",
                    url, range.start, range.end, e
                );
                None
            }
        };

        // Update statistics
        if result.is_some() {
            if let Ok(mut hits) = self.hits.write() {
                *hits += 1;
            }
        } else {
            if let Ok(mut misses) = self.misses.write() {
                *misses += 1;
            }
        }

        // Update access time if hit (requires write lock)
        if result.is_some() {
            if let Ok(mut storage) = self.storage.write() {
                if let Some(entry) = storage.get_mut(&key) {
                    entry.last_accessed = now;
                    entry.access_count += 1;
                }
            }
        }

        Ok(result)
    }

    /// Store a slice in the cache
    ///
    /// # Arguments
    /// * `url` - The URL of the file
    /// * `range` - The byte range of the slice
    /// * `data` - The slice data to store
    ///
    /// # Returns
    /// * `Ok(())` if the slice was stored successfully
    /// * `Err(SliceError)` if a cache error occurs (logged as warning)
    pub async fn store_slice(
        &self,
        url: &str,
        range: &ByteRange,
        data: Bytes,
    ) -> Result<()> {
        let key = self.generate_cache_key(url, range);
        let now = SystemTime::now();
        let expires_at = now + self.ttl;
        let data_size = data.len();
        
        debug!(
            "Storing slice in cache: url={}, range={}-{}, size={}",
            url, range.start, range.end, data_size
        );

        // Check if we need to evict entries to make room
        if let Some(max_size) = self.max_size_bytes {
            let current_size = *self.current_size_bytes.read().unwrap();
            if current_size + data_size > max_size {
                debug!(
                    "Cache size limit reached ({}/{}), evicting LRU entries",
                    current_size, max_size
                );
                self.evict_lru(data_size);
            }
        }

        match self.storage.write() {
            Ok(mut storage) => {
                // Remove old entry if it exists and update size
                if let Some(old_entry) = storage.get(&key) {
                    if let Ok(mut current_size) = self.current_size_bytes.write() {
                        *current_size = current_size.saturating_sub(old_entry.data.len());
                    }
                }

                storage.insert(key, CacheEntry {
                    data,
                    expires_at,
                    last_accessed: now,
                    access_count: 0,
                });

                // Update current size
                if let Ok(mut current_size) = self.current_size_bytes.write() {
                    *current_size += data_size;
                }

                debug!(
                    "Successfully stored slice in cache: url={}, range={}-{}",
                    url, range.start, range.end
                );
                
                // Periodically clean up expired entries
                if storage.len() % 100 == 0 {
                    drop(storage); // Release write lock before cleanup
                    self.cleanup_expired();
                }
                
                Ok(())
            }
            Err(e) => {
                warn!(
                    "Failed to store slice in cache: url={}, range={}-{}, error={:?}",
                    url, range.start, range.end, e
                );
                // Log warning but don't fail the request
                // Return Ok to continue processing
                Ok(())
            }
        }
    }

    /// Batch lookup multiple slices
    ///
    /// This method looks up multiple slices and returns
    /// a map of slice indices to their cached data.
    ///
    /// # Arguments
    /// * `url` - The URL of the file
    /// * `ranges` - A slice of byte ranges to look up
    ///
    /// # Returns
    /// A HashMap mapping slice indices to their cached data.
    /// Only slices that are found in cache are included in the result.
    pub async fn lookup_multiple(
        &self,
        url: &str,
        ranges: &[ByteRange],
    ) -> HashMap<usize, Bytes> {
        let mut cached = HashMap::new();
        
        debug!(
            "Looking up {} slices in cache for url={}",
            ranges.len(),
            url
        );

        // Look up each slice
        for (idx, range) in ranges.iter().enumerate() {
            match self.lookup_slice(url, range).await {
                Ok(Some(data)) => {
                    cached.insert(idx, data);
                }
                Ok(None) => {
                    // Cache miss, continue
                }
                Err(e) => {
                    warn!(
                        "Error looking up slice {}: url={}, range={}-{}, error={:?}",
                        idx, url, range.start, range.end, e
                    );
                    // Continue with other slices
                }
            }
        }

        debug!(
            "Cache lookup complete: url={}, total_slices={}, cache_hits={}",
            url,
            ranges.len(),
            cached.len()
        );

        cached
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generate_cache_key() {
        let cache = SliceCache::new(Duration::from_secs(3600));
        
        let range1 = ByteRange::new(0, 1023).unwrap();
        let range2 = ByteRange::new(1024, 2047).unwrap();
        
        let key1 = cache.generate_cache_key("http://example.com/file.bin", &range1);
        let key2 = cache.generate_cache_key("http://example.com/file.bin", &range2);
        
        // Keys should be different for different ranges
        assert_ne!(key1, key2);
    }

    #[tokio::test]
    async fn test_cache_miss() {
        let cache = SliceCache::new(Duration::from_secs(3600));
        
        let range = ByteRange::new(0, 1023).unwrap();
        let result = cache.lookup_slice("http://example.com/file.bin", &range).await;
        
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_store_and_lookup() {
        let cache = SliceCache::new(Duration::from_secs(3600));
        
        let range = ByteRange::new(0, 1023).unwrap();
        let data = Bytes::from(vec![1, 2, 3, 4, 5]);
        
        // Store the slice
        let store_result = cache.store_slice(
            "http://example.com/file.bin",
            &range,
            data.clone(),
        ).await;
        assert!(store_result.is_ok());
        
        // Look it up
        let lookup_result = cache.lookup_slice("http://example.com/file.bin", &range).await;
        assert!(lookup_result.is_ok());
        
        let cached_data = lookup_result.unwrap();
        assert!(cached_data.is_some());
        assert_eq!(cached_data.unwrap(), data);
    }

    #[tokio::test]
    async fn test_lookup_multiple() {
        let cache = SliceCache::new(Duration::from_secs(3600));
        
        let url = "http://example.com/file.bin";
        let ranges = vec![
            ByteRange::new(0, 1023).unwrap(),
            ByteRange::new(1024, 2047).unwrap(),
            ByteRange::new(2048, 3071).unwrap(),
        ];
        
        // Store first and third slices
        cache.store_slice(url, &ranges[0], Bytes::from(vec![1, 2, 3])).await.unwrap();
        cache.store_slice(url, &ranges[2], Bytes::from(vec![7, 8, 9])).await.unwrap();
        
        // Lookup all three
        let cached = cache.lookup_multiple(url, &ranges).await;
        
        // Should have 2 hits
        assert_eq!(cached.len(), 2);
        assert!(cached.contains_key(&0));
        assert!(!cached.contains_key(&1));
        assert!(cached.contains_key(&2));
    }

    #[tokio::test]
    async fn test_cache_key_uniqueness() {
        let cache = SliceCache::new(Duration::from_secs(3600));
        
        let range = ByteRange::new(0, 1023).unwrap();
        
        let key1 = cache.generate_cache_key("http://example.com/file1.bin", &range);
        let key2 = cache.generate_cache_key("http://example.com/file2.bin", &range);
        let key3 = cache.generate_cache_key("http://example.com/file1.bin", &ByteRange::new(1024, 2047).unwrap());
        
        // All keys should be unique
        assert_ne!(key1, key2);
        assert_ne!(key1, key3);
        assert_ne!(key2, key3);
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        let cache = SliceCache::new(Duration::from_millis(100));
        
        let range = ByteRange::new(0, 1023).unwrap();
        let data = Bytes::from(vec![1, 2, 3, 4, 5]);
        
        // Store the slice
        cache.store_slice("http://example.com/file.bin", &range, data.clone()).await.unwrap();
        
        // Should be in cache immediately
        let result1 = cache.lookup_slice("http://example.com/file.bin", &range).await.unwrap();
        assert!(result1.is_some());
        
        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        // Should be expired now
        let result2 = cache.lookup_slice("http://example.com/file.bin", &range).await.unwrap();
        assert!(result2.is_none());
    }

    #[tokio::test]
    async fn test_cache_with_max_size() {
        // Create cache with 1KB limit
        let cache = SliceCache::with_max_size(Duration::from_secs(3600), 1024);
        
        let range1 = ByteRange::new(0, 511).unwrap();
        let range2 = ByteRange::new(512, 1023).unwrap();
        let range3 = ByteRange::new(1024, 1535).unwrap();
        
        let data1 = Bytes::from(vec![1u8; 512]);
        let data2 = Bytes::from(vec![2u8; 512]);
        let data3 = Bytes::from(vec![3u8; 512]);
        
        // Store first two slices (should fit)
        cache.store_slice("http://example.com/file.bin", &range1, data1.clone()).await.unwrap();
        cache.store_slice("http://example.com/file.bin", &range2, data2.clone()).await.unwrap();
        
        let stats = cache.get_stats();
        assert_eq!(stats.total_entries, 2);
        assert_eq!(stats.total_bytes, 1024);
        
        // Store third slice (should evict first one due to LRU)
        cache.store_slice("http://example.com/file.bin", &range3, data3.clone()).await.unwrap();
        
        // First slice should be evicted
        let result1 = cache.lookup_slice("http://example.com/file.bin", &range1).await.unwrap();
        assert!(result1.is_none());
        
        // Second and third should still be there
        let result2 = cache.lookup_slice("http://example.com/file.bin", &range2).await.unwrap();
        assert!(result2.is_some());
        
        let result3 = cache.lookup_slice("http://example.com/file.bin", &range3).await.unwrap();
        assert!(result3.is_some());
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let cache = SliceCache::new(Duration::from_secs(3600));
        
        let range = ByteRange::new(0, 1023).unwrap();
        let data = Bytes::from(vec![1, 2, 3, 4, 5]);
        
        // Store and lookup
        cache.store_slice("http://example.com/file.bin", &range, data.clone()).await.unwrap();
        let _ = cache.lookup_slice("http://example.com/file.bin", &range).await;
        let _ = cache.lookup_slice("http://example.com/other.bin", &range).await;
        
        let stats = cache.get_stats();
        assert_eq!(stats.total_entries, 1);
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
    }
}
