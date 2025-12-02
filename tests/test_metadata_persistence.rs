//! Tests for metadata persistence

use bytes::Bytes;
use pingora_slice::raw_disk::RawDiskCache;
use std::time::Duration;
use tokio::fs;

#[tokio::test]
async fn test_metadata_save_and_load() {
    let cache_file = "/tmp/test_metadata_persistence";
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
    
    // Create cache and add some data
    {
        let cache = RawDiskCache::new(
            cache_file,
            10 * 1024 * 1024, // 10MB
            4096,
            Duration::from_secs(3600),
        ).await.unwrap();
        
        // Store some entries
        for i in 0..10 {
            let key = format!("key_{}", i);
            let data = Bytes::from(format!("data for key {}", i));
            cache.store(&key, data).await.unwrap();
        }
        
        // Save metadata
        cache.save_metadata().await.unwrap();
        
        let stats = cache.stats().await;
        assert_eq!(stats.entries, 10);
    }
    
    // Reopen cache and load metadata
    {
        let cache = RawDiskCache::new(
            cache_file,
            10 * 1024 * 1024,
            4096,
            Duration::from_secs(3600),
        ).await.unwrap();
        
        // Load metadata
        cache.load_metadata().await.unwrap();
        
        let stats = cache.stats().await;
        assert_eq!(stats.entries, 10);
        
        // Verify we can read the data
        for i in 0..10 {
            let key = format!("key_{}", i);
            let expected = format!("data for key {}", i);
            let data = cache.lookup(&key).await.unwrap().unwrap();
            assert_eq!(data, Bytes::from(expected));
        }
    }
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}

#[tokio::test]
async fn test_metadata_persistence_with_updates() {
    let cache_file = "/tmp/test_metadata_updates";
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
    
    // Create cache and add data
    {
        let cache = RawDiskCache::new(
            cache_file,
            10 * 1024 * 1024,
            4096,
            Duration::from_secs(3600),
        ).await.unwrap();
        
        // Store entries
        for i in 0..5 {
            let key = format!("key_{}", i);
            let data = Bytes::from(format!("data_{}", i));
            cache.store(&key, data).await.unwrap();
        }
        
        cache.save_metadata().await.unwrap();
    }
    
    // Reopen, load, modify, and save again
    {
        let cache = RawDiskCache::new(
            cache_file,
            10 * 1024 * 1024,
            4096,
            Duration::from_secs(3600),
        ).await.unwrap();
        
        cache.load_metadata().await.unwrap();
        
        // Add more entries
        for i in 5..10 {
            let key = format!("key_{}", i);
            let data = Bytes::from(format!("data_{}", i));
            cache.store(&key, data).await.unwrap();
        }
        
        // Remove some entries
        cache.remove("key_0").await.unwrap();
        cache.remove("key_1").await.unwrap();
        
        cache.save_metadata().await.unwrap();
        
        let stats = cache.stats().await;
        assert_eq!(stats.entries, 8); // 10 - 2
    }
    
    // Reopen and verify
    {
        let cache = RawDiskCache::new(
            cache_file,
            10 * 1024 * 1024,
            4096,
            Duration::from_secs(3600),
        ).await.unwrap();
        
        cache.load_metadata().await.unwrap();
        
        let stats = cache.stats().await;
        assert_eq!(stats.entries, 8);
        
        // Verify removed entries are gone
        assert!(cache.lookup("key_0").await.unwrap().is_none());
        assert!(cache.lookup("key_1").await.unwrap().is_none());
        
        // Verify other entries exist
        for i in 2..10 {
            let key = format!("key_{}", i);
            assert!(cache.lookup(&key).await.unwrap().is_some());
        }
    }
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}

#[tokio::test]
async fn test_metadata_load_on_empty_cache() {
    let cache_file = "/tmp/test_metadata_empty";
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
    
    let cache = RawDiskCache::new(
        cache_file,
        10 * 1024 * 1024,
        4096,
        Duration::from_secs(3600),
    ).await.unwrap();
    
    // Try to load metadata from empty cache (should not fail)
    cache.load_metadata().await.unwrap();
    
    let stats = cache.stats().await;
    assert_eq!(stats.entries, 0);
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}
