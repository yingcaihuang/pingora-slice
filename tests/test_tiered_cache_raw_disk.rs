//! Integration tests for TieredCache with raw disk backend

use bytes::Bytes;
use pingora_slice::models::ByteRange;
use pingora_slice::tiered_cache::TieredCache;
use std::time::Duration;
use tempfile::TempDir;

#[tokio::test]
async fn test_raw_disk_backend_basic() {
    let temp_dir = TempDir::new().unwrap();
    let device_path = temp_dir.path().join("raw_disk_cache");
    
    // Create cache with raw disk backend
    let cache = TieredCache::new_with_raw_disk(
        Duration::from_secs(60),
        1024 * 1024, // 1MB L1
        &device_path,
        10 * 1024 * 1024, // 10MB total
        4096, // 4KB blocks
        false, // Don't use O_DIRECT in tests
    )
    .await
    .unwrap();
    
    let range = ByteRange::new(0, 999).unwrap();
    let data = Bytes::from(vec![1u8; 1000]);
    
    // Store
    cache.store("http://example.com/file", &range, data.clone()).unwrap();
    
    // Give async write time to complete
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Clear L1 to force L2 lookup
    let stats = cache.get_stats();
    assert_eq!(stats.l1_entries, 1);
    
    // Lookup should hit L1
    let result = cache.lookup("http://example.com/file", &range).await.unwrap();
    assert_eq!(result, Some(data.clone()));
    
    let stats = cache.get_stats();
    assert_eq!(stats.l1_hits, 1);
}

#[tokio::test]
async fn test_raw_disk_backend_l2_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let device_path = temp_dir.path().join("raw_disk_cache");
    let range = ByteRange::new(0, 999).unwrap();
    let data = Bytes::from(vec![2u8; 1000]);
    
    // Create cache and store data
    {
        let cache = TieredCache::new_with_raw_disk(
            Duration::from_secs(60),
            1024 * 1024,
            &device_path,
            10 * 1024 * 1024,
            4096,
            false,
        )
        .await
        .unwrap();
        
        cache.store("http://example.com/file2", &range, data.clone()).unwrap();
        
        // Wait for async write
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    
    // Create new cache instance (simulates restart)
    let cache2 = TieredCache::new_with_raw_disk(
        Duration::from_secs(60),
        1024 * 1024,
        &device_path,
        10 * 1024 * 1024,
        4096,
        false,
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
async fn test_raw_disk_backend_purge() {
    let temp_dir = TempDir::new().unwrap();
    let device_path = temp_dir.path().join("raw_disk_cache");
    
    let cache = TieredCache::new_with_raw_disk(
        Duration::from_secs(60),
        1024 * 1024,
        &device_path,
        10 * 1024 * 1024,
        4096,
        false,
    )
    .await
    .unwrap();
    
    let range = ByteRange::new(0, 999).unwrap();
    let data = Bytes::from(vec![1u8; 1000]);
    
    // Store data
    cache.store("http://example.com/file", &range, data.clone()).unwrap();
    
    // Wait for async write
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Verify it's cached
    let result = cache.lookup("http://example.com/file", &range).await.unwrap();
    assert_eq!(result, Some(data));
    
    // Purge the entry
    let purged = cache.purge("http://example.com/file", &range).await.unwrap();
    assert!(purged);
    
    // Wait for async delete
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Verify it's gone from L1
    let result = cache.lookup("http://example.com/file", &range).await.unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn test_raw_disk_backend_stats() {
    let temp_dir = TempDir::new().unwrap();
    let device_path = temp_dir.path().join("raw_disk_cache");
    
    let cache = TieredCache::new_with_raw_disk(
        Duration::from_secs(60),
        1024 * 1024,
        &device_path,
        10 * 1024 * 1024,
        4096,
        false,
    )
    .await
    .unwrap();
    
    // Check that raw disk stats are available
    let raw_stats = cache.raw_disk_stats().await;
    assert!(raw_stats.is_some());
    
    let stats = raw_stats.unwrap();
    assert_eq!(stats.entries, 0);
    assert!(stats.free_blocks > 0);
    
    // Store some data
    let range = ByteRange::new(0, 999).unwrap();
    let data = Bytes::from(vec![1u8; 1000]);
    cache.store("http://example.com/file", &range, data).unwrap();
    
    // Wait for async write
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Check stats again
    let raw_stats = cache.raw_disk_stats().await.unwrap();
    assert_eq!(raw_stats.entries, 1);
    assert!(raw_stats.used_blocks > 0);
}

#[tokio::test]
async fn test_backend_type() {
    use pingora_slice::tiered_cache::L2Backend;
    
    let temp_dir = TempDir::new().unwrap();
    
    // File backend
    let file_cache = TieredCache::new(
        Duration::from_secs(60),
        1024 * 1024,
        temp_dir.path(),
    )
    .await
    .unwrap();
    
    assert_eq!(file_cache.l2_backend(), L2Backend::File);
    assert!(file_cache.raw_disk_stats().await.is_none());
    
    // Raw disk backend
    let device_path = temp_dir.path().join("raw_disk_cache");
    let raw_cache = TieredCache::new_with_raw_disk(
        Duration::from_secs(60),
        1024 * 1024,
        &device_path,
        10 * 1024 * 1024,
        4096,
        false,
    )
    .await
    .unwrap();
    
    assert_eq!(raw_cache.l2_backend(), L2Backend::RawDisk);
    assert!(raw_cache.raw_disk_stats().await.is_some());
}
