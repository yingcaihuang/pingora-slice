//! Two-tier cache implementation with L1 (memory) and L2 (disk) storage
//!
//! This module provides a high-performance caching system that combines:
//! - L1 Cache: In-memory HashMap for fast access to hot data
//! - L2 Cache: Disk-based storage for persistence and cold data
//!
//! Features:
//! - Automatic promotion of frequently accessed items to L1
//! - Asynchronous write-behind to L2 for minimal latency impact
//! - LRU eviction for L1 when memory limit is reached
//! - Persistent storage survives restarts
//! - Configurable cache sizes and TTL

use crate::error::{Result, SliceError};
use crate::models::ByteRange;
use bytes::Bytes;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Message for async disk write operations
#[derive(Debug)]
enum DiskWriteMessage {
    Write {
        key: String,
        data: Bytes,
        expires_at: SystemTime,
    },
    Delete {
        key: String,
    },
    Shutdown,
}

/// L1 cache entry with access tracking
#[derive(Clone)]
struct L1Entry {
    data: Bytes,
    expires_at: SystemTime,
    last_accessed: SystemTime,
    access_count: u64,
}

/// L2 disk cache metadata
#[derive(Clone)]
#[allow(dead_code)]
struct L2Metadata {
    expires_at: SystemTime,
    size_bytes: usize,
}

/// Cache statistics
#[derive(Debug, Clone, Default)]
pub struct TieredCacheStats {
    pub l1_entries: usize,
    pub l1_bytes: usize,
    pub l1_hits: u64,
    pub l2_hits: u64,
    pub misses: u64,
    pub disk_writes: u64,
    pub disk_errors: u64,
}

/// Two-tier cache with memory (L1) and disk (L2) storage
pub struct TieredCache {
    // L1: In-memory cache
    l1_storage: Arc<RwLock<HashMap<String, L1Entry>>>,
    l1_max_size_bytes: usize,
    l1_current_size: Arc<RwLock<usize>>,
    
    // L2: Disk cache
    l2_base_path: PathBuf,
    l2_enabled: bool,
    
    // Configuration
    ttl: Duration,
    
    // Statistics
    stats: Arc<RwLock<TieredCacheStats>>,
    
    // Async disk writer
    disk_writer_tx: Option<mpsc::UnboundedSender<DiskWriteMessage>>,
}

impl TieredCache {
    /// Create a new two-tier cache
    ///
    /// # Arguments
    /// * `ttl` - Time-to-live for cached items
    /// * `l1_max_size_bytes` - Maximum L1 (memory) cache size
    /// * `l2_base_path` - Base directory for L2 (disk) cache
    pub async fn new(
        ttl: Duration,
        l1_max_size_bytes: usize,
        l2_base_path: impl AsRef<Path>,
    ) -> Result<Self> {
        let l2_base_path = l2_base_path.as_ref().to_path_buf();
        
        // Create L2 directory if it doesn't exist
        if let Err(e) = fs::create_dir_all(&l2_base_path).await {
            warn!("Failed to create L2 cache directory: {}", e);
            return Ok(Self::memory_only(ttl, l1_max_size_bytes));
        }
        
        info!(
            "Initializing two-tier cache: L1={}MB, L2={:?}",
            l1_max_size_bytes / 1024 / 1024,
            l2_base_path
        );
        
        // Start async disk writer
        let (tx, rx) = mpsc::unbounded_channel();
        let l2_path_clone = l2_base_path.clone();
        let stats_clone = Arc::new(RwLock::new(TieredCacheStats::default()));
        let stats_for_writer = stats_clone.clone();
        
        tokio::spawn(async move {
            Self::disk_writer_task(rx, l2_path_clone, stats_for_writer).await;
        });
        
        Ok(TieredCache {
            l1_storage: Arc::new(RwLock::new(HashMap::new())),
            l1_max_size_bytes,
            l1_current_size: Arc::new(RwLock::new(0)),
            l2_base_path,
            l2_enabled: true,
            ttl,
            stats: stats_clone,
            disk_writer_tx: Some(tx),
        })
    }
    
    /// Create a memory-only cache (L2 disabled)
    pub fn memory_only(ttl: Duration, l1_max_size_bytes: usize) -> Self {
        warn!("Creating memory-only cache (L2 disabled)");
        
        TieredCache {
            l1_storage: Arc::new(RwLock::new(HashMap::new())),
            l1_max_size_bytes,
            l1_current_size: Arc::new(RwLock::new(0)),
            l2_base_path: PathBuf::new(),
            l2_enabled: false,
            ttl,
            stats: Arc::new(RwLock::new(TieredCacheStats::default())),
            disk_writer_tx: None,
        }
    }
    
    /// Generate cache key from URL and byte range
    pub fn generate_cache_key(&self, url: &str, range: &ByteRange) -> String {
        format!("{}:{}:{}", url, range.start, range.end)
    }
    
    /// Lookup a slice in the cache (checks L1 then L2)
    pub async fn lookup(&self, url: &str, range: &ByteRange) -> Result<Option<Bytes>> {
        let key = self.generate_cache_key(url, range);
        let now = SystemTime::now();
        
        // Try L1 first
        {
            let mut storage = self.l1_storage.write().unwrap();
            if let Some(entry) = storage.get_mut(&key) {
                // Check expiration
                if entry.expires_at > now {
                    // Update access tracking
                    entry.last_accessed = now;
                    entry.access_count += 1;
                    
                    // Record L1 hit
                    self.stats.write().unwrap().l1_hits += 1;
                    
                    debug!("L1 cache hit: {}", key);
                    return Ok(Some(entry.data.clone()));
                } else {
                    // Expired, remove from L1
                    let removed = storage.remove(&key);
                    if let Some(removed_entry) = removed {
                        let mut size = self.l1_current_size.write().unwrap();
                        *size = size.saturating_sub(removed_entry.data.len());
                    }
                }
            }
        }
        
        // Try L2 if enabled
        if self.l2_enabled {
            if let Some(data) = self.lookup_l2(&key).await? {
                // Promote to L1
                self.store_l1(&key, data.clone(), now + self.ttl);
                
                // Record L2 hit
                self.stats.write().unwrap().l2_hits += 1;
                
                debug!("L2 cache hit (promoted to L1): {}", key);
                return Ok(Some(data));
            }
        }
        
        // Cache miss
        self.stats.write().unwrap().misses += 1;
        debug!("Cache miss: {}", key);
        Ok(None)
    }
    
    /// Store a slice in the cache (L1 + async L2)
    pub fn store(&self, url: &str, range: &ByteRange, data: Bytes) -> Result<()> {
        let key = self.generate_cache_key(url, range);
        let expires_at = SystemTime::now() + self.ttl;
        
        // Store in L1
        self.store_l1(&key, data.clone(), expires_at);
        
        // Async store in L2
        if self.l2_enabled {
            if let Some(tx) = &self.disk_writer_tx {
                let _ = tx.send(DiskWriteMessage::Write {
                    key,
                    data,
                    expires_at,
                });
            }
        }
        
        Ok(())
    }
    
    /// Store in L1 cache with LRU eviction
    fn store_l1(&self, key: &str, data: Bytes, expires_at: SystemTime) {
        let data_size = data.len();
        let now = SystemTime::now();
        
        let mut storage = self.l1_storage.write().unwrap();
        let mut current_size = self.l1_current_size.write().unwrap();
        
        // Remove old entry if exists
        if let Some(old_entry) = storage.remove(key) {
            *current_size = current_size.saturating_sub(old_entry.data.len());
        }
        
        // Evict LRU entries if needed
        while *current_size + data_size > self.l1_max_size_bytes && !storage.is_empty() {
            // Find LRU entry
            if let Some((lru_key, _)) = storage
                .iter()
                .min_by_key(|(_, entry)| entry.last_accessed)
                .map(|(k, v)| (k.clone(), v.clone()))
            {
                if let Some(removed) = storage.remove(&lru_key) {
                    *current_size = current_size.saturating_sub(removed.data.len());
                    debug!("Evicted LRU entry from L1: {}", lru_key);
                }
            } else {
                break;
            }
        }
        
        // Insert new entry
        storage.insert(
            key.to_string(),
            L1Entry {
                data,
                expires_at,
                last_accessed: now,
                access_count: 0,
            },
        );
        *current_size += data_size;
        
        debug!("Stored in L1: {} ({} bytes)", key, data_size);
    }
    
    /// Lookup in L2 disk cache
    async fn lookup_l2(&self, key: &str) -> Result<Option<Bytes>> {
        let file_path = self.get_l2_file_path(key);
        
        match fs::read(&file_path).await {
            Ok(data) => {
                // Check if file is expired (first 8 bytes = timestamp)
                if data.len() < 8 {
                    let _ = fs::remove_file(&file_path).await;
                    return Ok(None);
                }
                
                let timestamp_bytes: [u8; 8] = data[0..8].try_into().unwrap();
                let expires_at_secs = u64::from_le_bytes(timestamp_bytes);
                let expires_at = UNIX_EPOCH + Duration::from_secs(expires_at_secs);
                
                if expires_at <= SystemTime::now() {
                    // Expired, delete file
                    let _ = fs::remove_file(&file_path).await;
                    return Ok(None);
                }
                
                // Return data (skip first 8 bytes)
                Ok(Some(Bytes::from(data[8..].to_vec())))
            }
            Err(_) => Ok(None),
        }
    }
    
    /// Async disk writer task
    async fn disk_writer_task(
        mut rx: mpsc::UnboundedReceiver<DiskWriteMessage>,
        base_path: PathBuf,
        stats: Arc<RwLock<TieredCacheStats>>,
    ) {
        info!("Disk writer task started");
        
        while let Some(msg) = rx.recv().await {
            match msg {
                DiskWriteMessage::Write {
                    key,
                    data,
                    expires_at,
                } => {
                    if let Err(e) = Self::write_to_disk(&base_path, &key, &data, expires_at).await
                    {
                        error!("Failed to write to L2 cache: {}", e);
                        stats.write().unwrap().disk_errors += 1;
                    } else {
                        stats.write().unwrap().disk_writes += 1;
                    }
                }
                DiskWriteMessage::Delete { key } => {
                    let file_path = Self::get_l2_file_path_static(&base_path, &key);
                    if let Err(e) = fs::remove_file(&file_path).await {
                        if e.kind() != std::io::ErrorKind::NotFound {
                            warn!("Failed to delete L2 cache file {}: {}", file_path.display(), e);
                        }
                    } else {
                        debug!("Deleted from L2: {}", key);
                    }
                }
                DiskWriteMessage::Shutdown => {
                    info!("Disk writer task shutting down");
                    break;
                }
            }
        }
    }
    
    /// Write data to disk
    async fn write_to_disk(
        base_path: &Path,
        key: &str,
        data: &Bytes,
        expires_at: SystemTime,
    ) -> Result<()> {
        let file_path = Self::get_l2_file_path_static(base_path, key);
        
        // Create parent directory if needed
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                SliceError::CacheError(format!("Failed to create cache directory: {}", e))
            })?;
        }
        
        // Write timestamp + data
        let expires_at_secs = expires_at
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let mut file = fs::File::create(&file_path).await.map_err(|e| {
            SliceError::CacheError(format!("Failed to create cache file: {}", e))
        })?;
        
        file.write_all(&expires_at_secs.to_le_bytes())
            .await
            .map_err(|e| SliceError::CacheError(format!("Failed to write timestamp: {}", e)))?;
        
        file.write_all(data).await.map_err(|e| {
            SliceError::CacheError(format!("Failed to write data: {}", e))
        })?;
        
        file.sync_all().await.map_err(|e| {
            SliceError::CacheError(format!("Failed to sync file: {}", e))
        })?;
        
        debug!("Wrote to L2: {} ({} bytes)", key, data.len());
        Ok(())
    }
    
    /// Get L2 file path for a key
    fn get_l2_file_path(&self, key: &str) -> PathBuf {
        Self::get_l2_file_path_static(&self.l2_base_path, key)
    }
    
    /// Get L2 file path (static version)
    fn get_l2_file_path_static(base_path: &Path, key: &str) -> PathBuf {
        // Use hash to create subdirectories (avoid too many files in one dir)
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let hash = hasher.finish();
        
        // Create 2-level directory structure: XX/YY/filename
        let dir1 = format!("{:02x}", (hash >> 8) & 0xFF);
        let dir2 = format!("{:02x}", hash & 0xFF);
        
        // Sanitize key for filename
        let filename = key
            .replace('/', "_")
            .replace(':', "_")
            .replace('?', "_")
            .replace('&', "_");
        
        base_path.join(dir1).join(dir2).join(filename)
    }
    
    /// Get cache statistics
    pub fn get_stats(&self) -> TieredCacheStats {
        let mut stats = self.stats.read().unwrap().clone();
        
        let storage = self.l1_storage.read().unwrap();
        stats.l1_entries = storage.len();
        stats.l1_bytes = *self.l1_current_size.read().unwrap();
        
        stats
    }
    
    /// Batch lookup multiple slices
    pub async fn lookup_multiple(
        &self,
        url: &str,
        ranges: &[ByteRange],
    ) -> HashMap<usize, Bytes> {
        let mut result = HashMap::new();
        
        for (idx, range) in ranges.iter().enumerate() {
            if let Ok(Some(data)) = self.lookup(url, range).await {
                result.insert(idx, data);
            }
        }
        
        result
    }
    
    /// Purge a specific cached slice from both L1 and L2
    ///
    /// # Arguments
    /// * `url` - The URL of the cached resource
    /// * `range` - The byte range of the slice to purge
    ///
    /// # Returns
    /// `true` if the entry was found and removed, `false` otherwise
    pub async fn purge(&self, url: &str, range: &ByteRange) -> Result<bool> {
        let key = self.generate_cache_key(url, range);
        
        // Remove from L1
        let removed_from_l1 = {
            let mut storage = self.l1_storage.write().unwrap();
            if let Some(entry) = storage.remove(&key) {
                let mut size = self.l1_current_size.write().unwrap();
                *size = size.saturating_sub(entry.data.len());
                debug!("Purged from L1: {}", key);
                true
            } else {
                false
            }
        };
        
        // Remove from L2 (async)
        if self.l2_enabled {
            if let Some(tx) = &self.disk_writer_tx {
                let _ = tx.send(DiskWriteMessage::Delete { key: key.clone() });
            }
        }
        
        info!("Purged cache entry: {} (L1: {})", key, removed_from_l1);
        Ok(removed_from_l1)
    }
    
    /// Purge all cached slices for a specific URL
    ///
    /// This removes all cache entries whose keys start with the given URL.
    ///
    /// # Arguments
    /// * `url` - The URL of the resource to purge
    ///
    /// # Returns
    /// The number of entries purged from L1
    pub async fn purge_url(&self, url: &str) -> Result<usize> {
        let url_prefix = format!("{}:", url);
        let mut purged_count = 0;
        
        // Collect keys to remove (to avoid holding lock during iteration)
        let keys_to_remove: Vec<String> = {
            let storage = self.l1_storage.read().unwrap();
            storage
                .keys()
                .filter(|k| k.starts_with(&url_prefix))
                .cloned()
                .collect()
        };
        
        // Remove from L1
        {
            let mut storage = self.l1_storage.write().unwrap();
            let mut size = self.l1_current_size.write().unwrap();
            
            for key in &keys_to_remove {
                if let Some(entry) = storage.remove(key) {
                    *size = size.saturating_sub(entry.data.len());
                    purged_count += 1;
                    debug!("Purged from L1: {}", key);
                }
            }
        }
        
        // Remove from L2 (async)
        if self.l2_enabled {
            if let Some(tx) = &self.disk_writer_tx {
                for key in keys_to_remove {
                    let _ = tx.send(DiskWriteMessage::Delete { key });
                }
            }
        }
        
        info!("Purged {} cache entries for URL: {}", purged_count, url);
        Ok(purged_count)
    }
    
    /// Purge all cached entries from both L1 and L2
    ///
    /// # Returns
    /// The number of entries purged from L1
    pub async fn purge_all(&self) -> Result<usize> {
        // Collect all keys
        let all_keys: Vec<String> = {
            let storage = self.l1_storage.read().unwrap();
            storage.keys().cloned().collect()
        };
        
        let purged_count = all_keys.len();
        
        // Clear L1
        {
            let mut storage = self.l1_storage.write().unwrap();
            storage.clear();
            let mut size = self.l1_current_size.write().unwrap();
            *size = 0;
        }
        
        // Remove from L2 (async)
        if self.l2_enabled {
            if let Some(tx) = &self.disk_writer_tx {
                for key in all_keys {
                    let _ = tx.send(DiskWriteMessage::Delete { key });
                }
            }
        }
        
        info!("Purged all cache entries: {} total", purged_count);
        Ok(purged_count)
    }
}

impl Drop for TieredCache {
    fn drop(&mut self) {
        // Send shutdown signal to disk writer
        if let Some(tx) = &self.disk_writer_tx {
            let _ = tx.send(DiskWriteMessage::Shutdown);
        }
        
        info!("TieredCache dropped");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_l1_cache() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let cache = TieredCache::new(
            Duration::from_secs(60),
            1024 * 1024, // 1MB
            temp_dir.path(),
        )
        .await
        .unwrap();
        
        let range = ByteRange::new(0, 999).unwrap();
        let data = Bytes::from(vec![1u8; 1000]);
        
        // Store
        cache.store("http://example.com/file", &range, data.clone()).unwrap();
        
        // Lookup should hit L1
        let result = cache.lookup("http://example.com/file", &range).await.unwrap();
        assert_eq!(result, Some(data));
        
        let stats = cache.get_stats();
        assert_eq!(stats.l1_hits, 1);
    }
    
    #[tokio::test]
    async fn test_l2_persistence() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let range = ByteRange::new(0, 999).unwrap();
        let data = Bytes::from(vec![2u8; 1000]);
        
        // Create cache and store data
        {
            let cache = TieredCache::new(
                Duration::from_secs(60),
                1024 * 1024,
                temp_dir.path(),
            )
            .await
            .unwrap();
            
            cache.store("http://example.com/file2", &range, data.clone()).unwrap();
            
            // Wait for async write
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        // Create new cache instance (simulates restart)
        let cache2 = TieredCache::new(
            Duration::from_secs(60),
            1024 * 1024,
            temp_dir.path(),
        )
        .await
        .unwrap();
        
        // Should hit L2 and promote to L1
        let result = cache2.lookup("http://example.com/file2", &range).await.unwrap();
        assert_eq!(result, Some(data));
        
        let stats = cache2.get_stats();
        assert_eq!(stats.l2_hits, 1);
    }
    
    #[tokio::test]
    async fn test_purge_single_entry() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let cache = TieredCache::new(
            Duration::from_secs(60),
            1024 * 1024,
            temp_dir.path(),
        )
        .await
        .unwrap();
        
        let range = ByteRange::new(0, 999).unwrap();
        let data = Bytes::from(vec![1u8; 1000]);
        
        // Store data
        cache.store("http://example.com/file", &range, data.clone()).unwrap();
        
        // Verify it's cached
        let result = cache.lookup("http://example.com/file", &range).await.unwrap();
        assert_eq!(result, Some(data));
        
        // Purge the entry
        let purged = cache.purge("http://example.com/file", &range).await.unwrap();
        assert!(purged);
        
        // Verify it's gone
        let result = cache.lookup("http://example.com/file", &range).await.unwrap();
        assert_eq!(result, None);
    }
    
    #[tokio::test]
    async fn test_purge_url() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let cache = TieredCache::new(
            Duration::from_secs(60),
            1024 * 1024,
            temp_dir.path(),
        )
        .await
        .unwrap();
        
        // Store multiple slices for the same URL
        let url = "http://example.com/largefile";
        let range1 = ByteRange::new(0, 999).unwrap();
        let range2 = ByteRange::new(1000, 1999).unwrap();
        let range3 = ByteRange::new(2000, 2999).unwrap();
        let data = Bytes::from(vec![1u8; 1000]);
        
        cache.store(url, &range1, data.clone()).unwrap();
        cache.store(url, &range2, data.clone()).unwrap();
        cache.store(url, &range3, data.clone()).unwrap();
        
        // Store data for a different URL
        cache.store("http://example.com/other", &range1, data.clone()).unwrap();
        
        // Verify all are cached
        assert!(cache.lookup(url, &range1).await.unwrap().is_some());
        assert!(cache.lookup(url, &range2).await.unwrap().is_some());
        assert!(cache.lookup(url, &range3).await.unwrap().is_some());
        assert!(cache.lookup("http://example.com/other", &range1).await.unwrap().is_some());
        
        // Purge all slices for the URL
        let purged = cache.purge_url(url).await.unwrap();
        assert_eq!(purged, 3);
        
        // Verify they're gone
        assert!(cache.lookup(url, &range1).await.unwrap().is_none());
        assert!(cache.lookup(url, &range2).await.unwrap().is_none());
        assert!(cache.lookup(url, &range3).await.unwrap().is_none());
        
        // Verify other URL is still cached
        assert!(cache.lookup("http://example.com/other", &range1).await.unwrap().is_some());
    }
    
    #[tokio::test]
    async fn test_purge_all() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let cache = TieredCache::new(
            Duration::from_secs(60),
            1024 * 1024,
            temp_dir.path(),
        )
        .await
        .unwrap();
        
        // Store multiple entries
        let range = ByteRange::new(0, 999).unwrap();
        let data = Bytes::from(vec![1u8; 1000]);
        
        cache.store("http://example.com/file1", &range, data.clone()).unwrap();
        cache.store("http://example.com/file2", &range, data.clone()).unwrap();
        cache.store("http://example.com/file3", &range, data.clone()).unwrap();
        
        // Verify all are cached
        assert!(cache.lookup("http://example.com/file1", &range).await.unwrap().is_some());
        assert!(cache.lookup("http://example.com/file2", &range).await.unwrap().is_some());
        assert!(cache.lookup("http://example.com/file3", &range).await.unwrap().is_some());
        
        // Purge all
        let purged = cache.purge_all().await.unwrap();
        assert_eq!(purged, 3);
        
        // Verify all are gone
        assert!(cache.lookup("http://example.com/file1", &range).await.unwrap().is_none());
        assert!(cache.lookup("http://example.com/file2", &range).await.unwrap().is_none());
        assert!(cache.lookup("http://example.com/file3", &range).await.unwrap().is_none());
        
        let stats = cache.get_stats();
        assert_eq!(stats.l1_entries, 0);
        assert_eq!(stats.l1_bytes, 0);
    }
}
