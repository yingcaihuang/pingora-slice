//! Tests for TTL (Time-To-Live) support in raw disk cache

use bytes::Bytes;
use pingora_slice::raw_disk::RawDiskCache;
use std::time::Duration;
use tempfile::NamedTempFile;

#[tokio::test]
async fn test_ttl_expiration_on_lookup() {
    // Create cache with 1 second TTL
    let temp_file = NamedTempFile::new().unwrap();
    let cache = RawDiskCache::new(
        temp_file.path(),
        10 * 1024 * 1024, // 10MB
        4096,
        Duration::from_secs(1),
    )
    .await
    .unwrap();

    // Store data
    let key = "test_key";
    let data = Bytes::from("test data");
    cache.store(key, data.clone()).await.unwrap();

    // Verify data is accessible immediately
    let result = cache.lookup(key).await.unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap(), data);

    // Wait for TTL to expire
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Lookup should return None and remove the entry
    let result = cache.lookup(key).await.unwrap();
    assert!(result.is_none());

    // Verify entry was removed from cache
    let stats = cache.stats().await;
    assert_eq!(stats.entries, 0);
}

#[tokio::test]
async fn test_ttl_zero_copy_lookup() {
    // Create cache with 1 second TTL
    let temp_file = NamedTempFile::new().unwrap();
    let cache = RawDiskCache::new(
        temp_file.path(),
        10 * 1024 * 1024,
        4096,
        Duration::from_secs(1),
    )
    .await
    .unwrap();

    // Store data
    let key = "test_key";
    let data = Bytes::from("test data");
    cache.store(key, data.clone()).await.unwrap();

    // Verify data is accessible immediately
    let result = cache.lookup_zero_copy(key).await.unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap(), data);

    // Wait for TTL to expire
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Lookup should return None and remove the entry
    let result = cache.lookup_zero_copy(key).await.unwrap();
    assert!(result.is_none());

    // Verify entry was removed
    let stats = cache.stats().await;
    assert_eq!(stats.entries, 0);
}

#[tokio::test]
async fn test_cleanup_expired() {
    // Create cache with 1 second TTL
    let temp_file = NamedTempFile::new().unwrap();
    let cache = RawDiskCache::new(
        temp_file.path(),
        10 * 1024 * 1024,
        4096,
        Duration::from_secs(1),
    )
    .await
    .unwrap();

    // Store multiple entries
    for i in 0..10 {
        let key = format!("key_{}", i);
        let data = Bytes::from(format!("data_{}", i));
        cache.store(&key, data).await.unwrap();
    }

    // Verify all entries are present
    let stats = cache.stats().await;
    assert_eq!(stats.entries, 10);

    // Wait for TTL to expire
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Run cleanup
    let removed = cache.cleanup_expired().await.unwrap();
    assert_eq!(removed, 10);

    // Verify all entries were removed
    let stats = cache.stats().await;
    assert_eq!(stats.entries, 0);
}

#[tokio::test]
async fn test_cleanup_expired_partial() {
    // Create cache with 1 second TTL for more predictable timing
    let temp_file = NamedTempFile::new().unwrap();
    let cache = RawDiskCache::new(
        temp_file.path(),
        10 * 1024 * 1024,
        4096,
        Duration::from_secs(1),
    )
    .await
    .unwrap();

    // Store first batch of entries
    for i in 0..5 {
        let key = format!("old_key_{}", i);
        let data = Bytes::from(format!("old_data_{}", i));
        cache.store(&key, data).await.unwrap();
    }

    // Wait for first batch to expire completely (add extra margin)
    tokio::time::sleep(Duration::from_millis(1500)).await;

    // Store second batch of entries (these should NOT expire)
    for i in 0..5 {
        let key = format!("new_key_{}", i);
        let data = Bytes::from(format!("new_data_{}", i));
        cache.store(&key, data).await.unwrap();
    }

    // Verify all entries are present
    let stats = cache.stats().await;
    assert_eq!(stats.entries, 10);

    // Run cleanup - should only remove first batch (expired)
    let removed = cache.cleanup_expired().await.unwrap();
    assert_eq!(removed, 5, "Should remove only the first batch of 5 entries");

    // Verify only new entries remain
    let stats = cache.stats().await;
    assert_eq!(stats.entries, 5, "Should have 5 entries remaining");

    // Verify new entries are still accessible
    for i in 0..5 {
        let key = format!("new_key_{}", i);
        let result = cache.lookup(&key).await.unwrap();
        assert!(result.is_some(), "New entry {} should still exist", i);
    }

    // Verify old entries are gone
    for i in 0..5 {
        let key = format!("old_key_{}", i);
        let result = cache.lookup(&key).await.unwrap();
        assert!(result.is_none(), "Old entry {} should be expired", i);
    }
}

#[tokio::test]
async fn test_ttl_disabled() {
    // Create cache with TTL disabled (0 seconds)
    let temp_file = NamedTempFile::new().unwrap();
    let cache = RawDiskCache::new(
        temp_file.path(),
        10 * 1024 * 1024,
        4096,
        Duration::from_secs(0),
    )
    .await
    .unwrap();

    // Store data
    let key = "test_key";
    let data = Bytes::from("test data");
    cache.store(key, data.clone()).await.unwrap();

    // Wait some time
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Data should still be accessible (TTL disabled)
    let result = cache.lookup(key).await.unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap(), data);

    // Cleanup should not remove anything
    let removed = cache.cleanup_expired().await.unwrap();
    assert_eq!(removed, 0);

    let stats = cache.stats().await;
    assert_eq!(stats.entries, 1);
}

#[tokio::test]
async fn test_gc_prioritizes_expired_entries() {
    // Create small cache with 1 second TTL to force GC
    let temp_file = NamedTempFile::new().unwrap();
    let cache = RawDiskCache::new(
        temp_file.path(),
        1 * 1024 * 1024, // 1MB - small to trigger GC
        4096,
        Duration::from_secs(1),
    )
    .await
    .unwrap();

    // Store entries that will fill most of the cache
    for i in 0..50 {
        let key = format!("key_{}", i);
        let data = Bytes::from(vec![0u8; 10_000]); // 10KB each
        cache.store(&key, data).await.unwrap();
    }

    // Wait for TTL to expire
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Store new entries (not expired) - this should trigger GC
    for i in 50..60 {
        let key = format!("key_{}", i);
        let data = Bytes::from(vec![0u8; 10_000]);
        cache.store(&key, data).await.unwrap();
    }

    // Manually run GC to ensure it happens
    let removed = cache.run_smart_gc().await.unwrap();
    
    // Should have removed at least some entries
    assert!(removed > 0, "GC should have removed expired entries");

    // New entries should still be accessible
    for i in 50..60 {
        let key = format!("key_{}", i);
        let result = cache.lookup(&key).await.unwrap();
        assert!(result.is_some(), "New entry {} should still exist", i);
    }
}

#[tokio::test]
async fn test_ttl_with_multiple_lookups() {
    // Create cache with 1 second TTL for more predictable timing
    let temp_file = NamedTempFile::new().unwrap();
    let cache = RawDiskCache::new(
        temp_file.path(),
        10 * 1024 * 1024,
        4096,
        Duration::from_secs(1),
    )
    .await
    .unwrap();

    // Store data
    let key = "test_key";
    let data = Bytes::from("test data");
    cache.store(key, data.clone()).await.unwrap();

    // Immediate lookup should succeed
    let result = cache.lookup(key).await.unwrap();
    assert!(result.is_some(), "Immediate lookup should succeed");

    // Lookup after 500ms should still succeed
    tokio::time::sleep(Duration::from_millis(500)).await;
    let result = cache.lookup(key).await.unwrap();
    assert!(result.is_some(), "Lookup at 500ms should succeed");

    // Wait for TTL to definitely expire (total 2 seconds from store)
    tokio::time::sleep(Duration::from_millis(1600)).await;

    // Lookup should now fail
    let result = cache.lookup(key).await.unwrap();
    assert!(result.is_none(), "Entry should be expired after TTL");
    
    // Verify entry was removed
    let stats = cache.stats().await;
    assert_eq!(stats.entries, 0, "Expired entry should be removed from cache");
}

#[tokio::test]
async fn test_ttl_frees_disk_space() {
    // Create cache with 1 second TTL
    let temp_file = NamedTempFile::new().unwrap();
    let cache = RawDiskCache::new(
        temp_file.path(),
        10 * 1024 * 1024,
        4096,
        Duration::from_secs(1),
    )
    .await
    .unwrap();

    // Store large data
    let key = "large_key";
    let data = Bytes::from(vec![0u8; 100_000]); // 100KB
    cache.store(key, data).await.unwrap();

    // Check used blocks
    let stats_before = cache.stats().await;
    let used_before = stats_before.used_blocks;
    assert!(used_before > 0);

    // Wait for TTL to expire
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Cleanup expired entries
    let removed = cache.cleanup_expired().await.unwrap();
    assert_eq!(removed, 1);

    // Check that blocks were freed
    let stats_after = cache.stats().await;
    let used_after = stats_after.used_blocks;
    assert_eq!(used_after, 0);
    assert!(stats_after.free_blocks > stats_before.free_blocks);
}

#[tokio::test]
async fn test_ttl_with_batch_lookup() {
    // Create cache with 1 second TTL
    let temp_file = NamedTempFile::new().unwrap();
    let cache = RawDiskCache::new(
        temp_file.path(),
        10 * 1024 * 1024,
        4096,
        Duration::from_secs(1),
    )
    .await
    .unwrap();

    // Store multiple entries
    let keys: Vec<String> = (0..5).map(|i| format!("key_{}", i)).collect();
    for key in &keys {
        let data = Bytes::from(format!("data_{}", key));
        cache.store(key, data).await.unwrap();
    }

    // Batch lookup should work immediately
    let results = cache.lookup_batch(&keys).await.unwrap();
    assert_eq!(results.len(), 5);
    assert!(results.iter().all(|r| r.is_some()));

    // Wait for TTL to expire
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Individual lookups should remove expired entries
    for key in &keys {
        let result = cache.lookup(key).await.unwrap();
        assert!(result.is_none());
    }

    // Verify all entries were removed
    let stats = cache.stats().await;
    assert_eq!(stats.entries, 0);
}
