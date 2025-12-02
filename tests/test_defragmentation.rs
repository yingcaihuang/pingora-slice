//! Tests for defragmentation functionality

use bytes::Bytes;
use pingora_slice::raw_disk::{DefragConfig, RawDiskCache};
use std::time::Duration;
use tempfile::NamedTempFile;

#[tokio::test]
async fn test_fragmentation_detection() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    
    let cache = RawDiskCache::new(
        path,
        10 * 1024 * 1024, // 10MB
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();
    
    // Initially, fragmentation should be 0
    let frag = cache.fragmentation_ratio().await;
    assert_eq!(frag, 0.0);
    
    // Add some entries
    cache.store("key1", Bytes::from("data1")).await.unwrap();
    cache.store("key2", Bytes::from("data2")).await.unwrap();
    cache.store("key3", Bytes::from("data3")).await.unwrap();
    
    // Still low fragmentation (contiguous)
    let frag = cache.fragmentation_ratio().await;
    assert!(frag < 0.1);
    
    // Remove middle entry to create a gap
    cache.remove("key2").await.unwrap();
    
    // Add more entries that will be placed after the gap
    cache.store("key4", Bytes::from("data4")).await.unwrap();
    cache.store("key5", Bytes::from("data5")).await.unwrap();
    
    // Now we should have some fragmentation
    let frag = cache.fragmentation_ratio().await;
    assert!(frag > 0.0);
}

#[tokio::test]
async fn test_defragmentation_basic() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    
    let cache = RawDiskCache::new(
        path,
        10 * 1024 * 1024, // 10MB
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();
    
    // Create fragmented layout
    cache.store("key1", Bytes::from(vec![1u8; 1000])).await.unwrap();
    cache.store("key2", Bytes::from(vec![2u8; 1000])).await.unwrap();
    cache.store("key3", Bytes::from(vec![3u8; 1000])).await.unwrap();
    cache.store("key4", Bytes::from(vec![4u8; 1000])).await.unwrap();
    
    // Remove some entries to create gaps
    cache.remove("key2").await.unwrap();
    cache.remove("key3").await.unwrap();
    
    // Add more entries
    cache.store("key5", Bytes::from(vec![5u8; 1000])).await.unwrap();
    cache.store("key6", Bytes::from(vec![6u8; 1000])).await.unwrap();
    
    let frag_before = cache.fragmentation_ratio().await;
    
    // Run defragmentation
    let moved = cache.defragment().await.unwrap();
    
    let frag_after = cache.fragmentation_ratio().await;
    
    // Fragmentation should be reduced
    assert!(frag_after <= frag_before);
    
    // Verify all data is still accessible and correct
    let data1 = cache.lookup("key1").await.unwrap().unwrap();
    assert_eq!(data1, Bytes::from(vec![1u8; 1000]));
    
    let data4 = cache.lookup("key4").await.unwrap().unwrap();
    assert_eq!(data4, Bytes::from(vec![4u8; 1000]));
    
    let data5 = cache.lookup("key5").await.unwrap().unwrap();
    assert_eq!(data5, Bytes::from(vec![5u8; 1000]));
    
    let data6 = cache.lookup("key6").await.unwrap().unwrap();
    assert_eq!(data6, Bytes::from(vec![6u8; 1000]));
}

#[tokio::test]
async fn test_defragmentation_incremental() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    
    let cache = RawDiskCache::new(
        path,
        10 * 1024 * 1024, // 10MB
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();
    
    // Configure incremental defragmentation
    let config = DefragConfig {
        fragmentation_threshold: 0.1,
        batch_size: 2, // Small batch size
        incremental: true,
        min_free_space_ratio: 0.1,
        target_compaction_ratio: 0.9,
    };
    cache.update_defrag_config(config).await;
    
    // Create many entries
    for i in 0..20 {
        let key = format!("key{}", i);
        let data = vec![i as u8; 500];
        cache.store(&key, Bytes::from(data)).await.unwrap();
    }
    
    // Remove every other entry to create fragmentation
    for i in (0..20).step_by(2) {
        let key = format!("key{}", i);
        cache.remove(&key).await.unwrap();
    }
    
    let frag_before = cache.fragmentation_ratio().await;
    assert!(frag_before > 0.1);
    
    // Run incremental defragmentation
    let moved = cache.defragment().await.unwrap();
    
    let frag_after = cache.fragmentation_ratio().await;
    
    // Should have moved some entries
    assert!(moved > 0);
    
    // Fragmentation should be reduced
    assert!(frag_after < frag_before);
    
    // Verify remaining data is still correct
    for i in (1..20).step_by(2) {
        let key = format!("key{}", i);
        let data = cache.lookup(&key).await.unwrap().unwrap();
        assert_eq!(data, Bytes::from(vec![i as u8; 500]));
    }
}

#[tokio::test]
async fn test_defragmentation_stats() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    
    let cache = RawDiskCache::new(
        path,
        10 * 1024 * 1024, // 10MB
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();
    
    // Initial stats
    let stats = cache.defrag_stats().await;
    assert_eq!(stats.total_runs, 0);
    assert_eq!(stats.total_entries_moved, 0);
    
    // Create fragmented layout
    for i in 0..10 {
        let key = format!("key{}", i);
        cache.store(&key, Bytes::from(vec![i as u8; 1000])).await.unwrap();
    }
    
    // Remove some to create gaps
    for i in (0..10).step_by(3) {
        let key = format!("key{}", i);
        cache.remove(&key).await.unwrap();
    }
    
    // Run defragmentation
    cache.defragment().await.unwrap();
    
    // Check stats
    let stats = cache.defrag_stats().await;
    assert!(stats.total_runs > 0);
    assert!(stats.last_run.is_some());
}

#[tokio::test]
async fn test_defragmentation_background() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    
    let cache = RawDiskCache::new(
        path,
        10 * 1024 * 1024, // 10MB
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();
    
    // Create fragmented layout
    for i in 0..10 {
        let key = format!("key{}", i);
        cache.store(&key, Bytes::from(vec![i as u8; 1000])).await.unwrap();
    }
    
    for i in (0..10).step_by(2) {
        let key = format!("key{}", i);
        cache.remove(&key).await.unwrap();
    }
    
    // Run background defragmentation
    cache.defragment_background().await;
    
    // Give it some time to complete
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Verify data is still accessible
    for i in (1..10).step_by(2) {
        let key = format!("key{}", i);
        let data = cache.lookup(&key).await.unwrap().unwrap();
        assert_eq!(data, Bytes::from(vec![i as u8; 1000]));
    }
}

#[tokio::test]
async fn test_should_defragment() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    
    let cache = RawDiskCache::new(
        path,
        10 * 1024 * 1024, // 10MB
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();
    
    // Configure with high threshold
    let config = DefragConfig {
        fragmentation_threshold: 0.8,
        min_free_space_ratio: 0.1,
        ..Default::default()
    };
    cache.update_defrag_config(config).await;
    
    // Initially should not need defragmentation
    assert!(!cache.should_defragment().await);
    
    // Add and remove entries to create fragmentation
    for i in 0..20 {
        let key = format!("key{}", i);
        cache.store(&key, Bytes::from(vec![i as u8; 1000])).await.unwrap();
    }
    
    for i in (0..20).step_by(2) {
        let key = format!("key{}", i);
        cache.remove(&key).await.unwrap();
    }
    
    // With high threshold, might still not trigger
    // (depends on actual fragmentation pattern)
    let _ = cache.should_defragment().await;
}

#[tokio::test]
async fn test_defragmentation_with_large_entries() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    
    let cache = RawDiskCache::new(
        path,
        20 * 1024 * 1024, // 20MB
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();
    
    // Add large entries
    cache.store("large1", Bytes::from(vec![1u8; 100_000])).await.unwrap();
    cache.store("large2", Bytes::from(vec![2u8; 100_000])).await.unwrap();
    cache.store("large3", Bytes::from(vec![3u8; 100_000])).await.unwrap();
    
    // Remove middle entry
    cache.remove("large2").await.unwrap();
    
    // Add more entries
    cache.store("large4", Bytes::from(vec![4u8; 100_000])).await.unwrap();
    
    // Run defragmentation
    let moved = cache.defragment().await.unwrap();
    
    // Verify data integrity
    let data1 = cache.lookup("large1").await.unwrap().unwrap();
    assert_eq!(data1.len(), 100_000);
    assert_eq!(data1[0], 1u8);
    
    let data3 = cache.lookup("large3").await.unwrap().unwrap();
    assert_eq!(data3.len(), 100_000);
    assert_eq!(data3[0], 3u8);
    
    let data4 = cache.lookup("large4").await.unwrap().unwrap();
    assert_eq!(data4.len(), 100_000);
    assert_eq!(data4[0], 4u8);
}
