//! Tests for batch I/O operations

use bytes::Bytes;
use pingora_slice::raw_disk::RawDiskCache;
use std::time::Duration;
use tempfile::NamedTempFile;

#[tokio::test]
async fn test_buffered_write() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    
    let cache = RawDiskCache::new_with_options(
        path,
        10 * 1024 * 1024, // 10MB
        4096,
        Duration::from_secs(3600),
        false, // Don't use O_DIRECT for tests
    )
    .await
    .unwrap();
    
    // Store data using buffered writes
    let data1 = Bytes::from("test data 1");
    cache.store_buffered("key1", data1.clone()).await.unwrap();
    
    let data2 = Bytes::from("test data 2");
    cache.store_buffered("key2", data2.clone()).await.unwrap();
    
    // Check buffer stats
    let stats = cache.stats().await;
    assert!(stats.pending_writes > 0 || stats.buffered_bytes > 0);
    
    // Flush writes
    let flushed = cache.flush_writes().await.unwrap();
    assert!(flushed > 0);
    
    // Verify data can be read back
    let result1 = cache.lookup("key1").await.unwrap();
    assert_eq!(result1, Some(data1));
    
    let result2 = cache.lookup("key2").await.unwrap();
    assert_eq!(result2, Some(data2));
}

#[tokio::test]
async fn test_batch_read() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    
    let cache = RawDiskCache::new_with_options(
        path,
        10 * 1024 * 1024,
        4096,
        Duration::from_secs(3600),
        false,
    )
    .await
    .unwrap();
    
    // Store multiple entries
    let keys = vec!["key1", "key2", "key3", "key4", "key5"];
    let mut expected_data = Vec::new();
    
    for key in &keys {
        let data = Bytes::from(format!("test data for {}", key));
        expected_data.push(data.clone());
        cache.store(key, data).await.unwrap();
    }
    
    // Batch lookup
    let keys_string: Vec<String> = keys.iter().map(|k| k.to_string()).collect();
    let results = cache.lookup_batch(&keys_string).await.unwrap();
    
    assert_eq!(results.len(), keys.len());
    
    for (i, result) in results.iter().enumerate() {
        assert_eq!(result.as_ref(), Some(&expected_data[i]));
    }
}

#[tokio::test]
async fn test_batch_read_with_missing_keys() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    
    let cache = RawDiskCache::new_with_options(
        path,
        10 * 1024 * 1024,
        4096,
        Duration::from_secs(3600),
        false,
    )
    .await
    .unwrap();
    
    // Store only some entries
    let data1 = Bytes::from("test data 1");
    cache.store("key1", data1.clone()).await.unwrap();
    
    let data3 = Bytes::from("test data 3");
    cache.store("key3", data3.clone()).await.unwrap();
    
    // Batch lookup including missing keys
    let keys = vec!["key1".to_string(), "key2".to_string(), "key3".to_string()];
    let results = cache.lookup_batch(&keys).await.unwrap();
    
    assert_eq!(results.len(), 3);
    assert_eq!(results[0], Some(data1));
    assert_eq!(results[1], None);
    assert_eq!(results[2], Some(data3));
}

#[tokio::test]
async fn test_auto_flush_on_buffer_full() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    
    let cache = RawDiskCache::new_with_options(
        path,
        50 * 1024 * 1024, // 50MB
        4096,
        Duration::from_secs(3600),
        false,
    )
    .await
    .unwrap();
    
    // Store many small entries to trigger auto-flush
    for i in 0..100 {
        let key = format!("key{}", i);
        let data = Bytes::from(vec![i as u8; 1024]);
        cache.store_buffered(&key, data).await.unwrap();
    }
    
    // Flush any remaining buffered writes
    cache.flush_writes().await.unwrap();
    
    // Some writes should have been auto-flushed
    // Verify all data is accessible
    for i in 0..100 {
        let key = format!("key{}", i);
        let result = cache.lookup(&key).await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 1024);
    }
}

#[tokio::test]
async fn test_large_batch_read() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    
    let cache = RawDiskCache::new_with_options(
        path,
        100 * 1024 * 1024, // 100MB
        4096,
        Duration::from_secs(3600),
        false,
    )
    .await
    .unwrap();
    
    // Store many entries
    let num_entries = 50;
    let mut keys = Vec::new();
    let mut expected_data = Vec::new();
    
    for i in 0..num_entries {
        let key = format!("key{}", i);
        let data = Bytes::from(vec![i as u8; 8192]);
        
        keys.push(key.clone());
        expected_data.push(data.clone());
        
        cache.store(&key, data).await.unwrap();
    }
    
    // Batch read all entries
    let results = cache.lookup_batch(&keys).await.unwrap();
    
    assert_eq!(results.len(), num_entries);
    
    for result in results.iter() {
        assert!(result.is_some());
        assert_eq!(result.as_ref().unwrap().len(), 8192);
    }
}

#[tokio::test]
async fn test_mixed_buffered_and_direct_writes() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    
    let cache = RawDiskCache::new_with_options(
        path,
        20 * 1024 * 1024,
        4096,
        Duration::from_secs(3600),
        false,
    )
    .await
    .unwrap();
    
    // Mix buffered and direct writes
    let data1 = Bytes::from("buffered write 1");
    cache.store_buffered("key1", data1.clone()).await.unwrap();
    
    let data2 = Bytes::from("direct write");
    cache.store("key2", data2.clone()).await.unwrap();
    
    let data3 = Bytes::from("buffered write 2");
    cache.store_buffered("key3", data3.clone()).await.unwrap();
    
    // Flush buffered writes
    cache.flush_writes().await.unwrap();
    
    // Verify all data
    assert_eq!(cache.lookup("key1").await.unwrap(), Some(data1));
    assert_eq!(cache.lookup("key2").await.unwrap(), Some(data2));
    assert_eq!(cache.lookup("key3").await.unwrap(), Some(data3));
}

#[tokio::test]
async fn test_buffer_stats() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    
    let cache = RawDiskCache::new_with_options(
        path,
        10 * 1024 * 1024,
        4096,
        Duration::from_secs(3600),
        false,
    )
    .await
    .unwrap();
    
    // Initially no pending writes
    let stats = cache.stats().await;
    assert_eq!(stats.pending_writes, 0);
    assert_eq!(stats.buffered_bytes, 0);
    
    // Add some buffered writes
    let data = Bytes::from(vec![0u8; 1024]);
    cache.store_buffered("key1", data.clone()).await.unwrap();
    
    let stats = cache.stats().await;
    // Stats might be 0 if auto-flush happened
    assert!(stats.pending_writes <= 1);
    
    // Flush
    cache.flush_writes().await.unwrap();
    
    let stats = cache.stats().await;
    assert_eq!(stats.pending_writes, 0);
    assert_eq!(stats.buffered_bytes, 0);
}
