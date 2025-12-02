//! Tests for crash recovery functionality

use bytes::Bytes;
use pingora_slice::raw_disk::RawDiskCache;
use std::time::Duration;
use tokio::fs;

#[tokio::test]
async fn test_normal_shutdown_and_restart() {
    let cache_file = "/tmp/test_crash_recovery_normal";
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
    
    // Create cache and add data
    {
        let cache = RawDiskCache::new(
            cache_file,
            10 * 1024 * 1024, // 10MB
            4096,
            Duration::from_secs(3600),
        ).await.unwrap();
        
        // Store entries
        for i in 0..20 {
            let key = format!("key_{}", i);
            let data = Bytes::from(format!("test data for key {}", i));
            cache.store(&key, data).await.unwrap();
        }
        
        // Save metadata before shutdown
        cache.save_metadata().await.unwrap();
        
        let stats = cache.stats().await;
        assert_eq!(stats.entries, 20);
    }
    
    // Restart cache - should recover automatically
    {
        let cache = RawDiskCache::new(
            cache_file,
            10 * 1024 * 1024,
            4096,
            Duration::from_secs(3600),
        ).await.unwrap();
        
        let stats = cache.stats().await;
        assert_eq!(stats.entries, 20);
        
        // Verify all data is accessible
        for i in 0..20 {
            let key = format!("key_{}", i);
            let expected = format!("test data for key {}", i);
            let data = cache.lookup(&key).await.unwrap().unwrap();
            assert_eq!(data, Bytes::from(expected));
        }
    }
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}

#[tokio::test]
async fn test_crash_recovery_without_metadata_save() {
    let cache_file = "/tmp/test_crash_recovery_no_save";
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
    
    // Create cache and add data but DON'T save metadata (simulating crash)
    {
        let cache = RawDiskCache::new(
            cache_file,
            10 * 1024 * 1024,
            4096,
            Duration::from_secs(3600),
        ).await.unwrap();
        
        for i in 0..10 {
            let key = format!("key_{}", i);
            let data = Bytes::from(format!("data_{}", i));
            cache.store(&key, data).await.unwrap();
        }
        
        // Intentionally NOT saving metadata to simulate crash
        let stats = cache.stats().await;
        assert_eq!(stats.entries, 10);
    }
    
    // Restart - should handle missing metadata gracefully
    {
        let cache = RawDiskCache::new(
            cache_file,
            10 * 1024 * 1024,
            4096,
            Duration::from_secs(3600),
        ).await.unwrap();
        
        // Without metadata, cache starts empty but disk space is preserved
        let stats = cache.stats().await;
        // Directory will be empty since we can't recover keys without metadata
        assert_eq!(stats.entries, 0);
    }
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}

#[tokio::test]
async fn test_corrupted_metadata_recovery() {
    let cache_file = "/tmp/test_crash_recovery_corrupted";
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
    
    // Create cache and save metadata
    {
        let cache = RawDiskCache::new(
            cache_file,
            10 * 1024 * 1024,
            4096,
            Duration::from_secs(3600),
        ).await.unwrap();
        
        for i in 0..5 {
            let key = format!("key_{}", i);
            let data = Bytes::from(format!("data_{}", i));
            cache.store(&key, data).await.unwrap();
        }
        
        cache.save_metadata().await.unwrap();
    }
    
    // Corrupt the metadata
    {
        use tokio::io::{AsyncWriteExt, AsyncSeekExt};
        let mut file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(cache_file)
            .await
            .unwrap();
        
        // Seek to metadata area (after superblock at 4096)
        file.seek(std::io::SeekFrom::Start(4096 + 100)).await.unwrap();
        
        // Write garbage
        file.write_all(&[0xFF; 100]).await.unwrap();
        file.flush().await.unwrap();
    }
    
    // Restart - should handle corrupted metadata
    {
        let cache = RawDiskCache::new(
            cache_file,
            10 * 1024 * 1024,
            4096,
            Duration::from_secs(3600),
        ).await.unwrap();
        
        // Should start with empty cache after corruption
        let stats = cache.stats().await;
        assert_eq!(stats.entries, 0);
    }
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}

#[tokio::test]
async fn test_partial_metadata_corruption() {
    let cache_file = "/tmp/test_crash_recovery_partial";
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
    
    // Create cache with multiple entries
    {
        let cache = RawDiskCache::new(
            cache_file,
            10 * 1024 * 1024,
            4096,
            Duration::from_secs(3600),
        ).await.unwrap();
        
        // Store entries
        for i in 0..10 {
            let key = format!("key_{}", i);
            let data = Bytes::from(format!("data_{}", i));
            cache.store(&key, data).await.unwrap();
        }
        
        cache.save_metadata().await.unwrap();
    }
    
    // Corrupt one data block (not metadata)
    {
        use tokio::io::{AsyncWriteExt, AsyncSeekExt};
        let mut file = fs::OpenOptions::new()
            .write(true)
            .open(cache_file)
            .await
            .unwrap();
        
        // Calculate data offset (superblock + metadata area)
        // Metadata is 1% of total size, min 64KB, max 100MB
        let total_size = 10 * 1024 * 1024u64;
        let metadata_size = ((total_size / 100).max(64 * 1024)).min(100 * 1024 * 1024);
        let data_offset = 4096 + metadata_size;
        
        // Corrupt first data block
        file.seek(std::io::SeekFrom::Start(data_offset)).await.unwrap();
        file.write_all(&[0xFF; 100]).await.unwrap();
        file.flush().await.unwrap();
    }
    
    // Restart - should detect and remove corrupted entry
    {
        let cache = RawDiskCache::new(
            cache_file,
            10 * 1024 * 1024,
            4096,
            Duration::from_secs(3600),
        ).await.unwrap();
        
        let stats = cache.stats().await;
        // Should have removed the corrupted entry
        assert!(stats.entries < 10);
    }
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}

#[tokio::test]
async fn test_recovery_with_empty_cache() {
    let cache_file = "/tmp/test_crash_recovery_empty";
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
    
    // Create empty cache
    {
        let cache = RawDiskCache::new(
            cache_file,
            10 * 1024 * 1024,
            4096,
            Duration::from_secs(3600),
        ).await.unwrap();
        
        cache.save_metadata().await.unwrap();
        
        let stats = cache.stats().await;
        assert_eq!(stats.entries, 0);
    }
    
    // Restart
    {
        let cache = RawDiskCache::new(
            cache_file,
            10 * 1024 * 1024,
            4096,
            Duration::from_secs(3600),
        ).await.unwrap();
        
        let stats = cache.stats().await;
        assert_eq!(stats.entries, 0);
    }
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}

#[tokio::test]
async fn test_recovery_preserves_allocator_state() {
    let cache_file = "/tmp/test_crash_recovery_allocator";
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
    
    let initial_free_blocks;
    
    // Create cache and use some space
    {
        let cache = RawDiskCache::new(
            cache_file,
            10 * 1024 * 1024,
            4096,
            Duration::from_secs(3600),
        ).await.unwrap();
        
        // Store some large entries
        for i in 0..5 {
            let key = format!("large_key_{}", i);
            let data = Bytes::from(vec![0u8; 50000]); // ~50KB each
            cache.store(&key, data).await.unwrap();
        }
        
        cache.save_metadata().await.unwrap();
        
        let stats = cache.stats().await;
        initial_free_blocks = stats.free_blocks;
        assert!(stats.used_blocks > 0);
    }
    
    // Restart and verify allocator state
    {
        let cache = RawDiskCache::new(
            cache_file,
            10 * 1024 * 1024,
            4096,
            Duration::from_secs(3600),
        ).await.unwrap();
        
        let stats = cache.stats().await;
        // Free blocks should be approximately the same (allowing for small differences)
        assert!(stats.free_blocks <= initial_free_blocks + 10);
        assert!(stats.used_blocks > 0);
        
        // Should be able to store more data
        let key = "new_key";
        let data = Bytes::from(vec![0u8; 10000]);
        cache.store(key, data).await.unwrap();
        
        let new_stats = cache.stats().await;
        assert!(new_stats.used_blocks > stats.used_blocks);
    }
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}

#[tokio::test]
async fn test_full_crash_recovery_workflow() {
    let cache_file = "/tmp/test_crash_recovery_full_workflow";
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
    
    // Phase 1: Create cache and add data
    {
        let cache = RawDiskCache::new(
            cache_file,
            10 * 1024 * 1024,
            4096,
            Duration::from_secs(3600),
        ).await.unwrap();
        
        // Store entries
        for i in 0..20 {
            let key = format!("key_{}", i);
            let data = Bytes::from(format!("important data {}", i));
            cache.store(&key, data).await.unwrap();
        }
        
        // Simulate periodic metadata save
        cache.save_metadata().await.unwrap();
        
        let stats = cache.stats().await;
        assert_eq!(stats.entries, 20);
    }
    
    // Phase 2: Simulate crash and recovery
    {
        let cache = RawDiskCache::new(
            cache_file,
            10 * 1024 * 1024,
            4096,
            Duration::from_secs(3600),
        ).await.unwrap();
        
        // Should automatically recover
        let stats = cache.stats().await;
        assert_eq!(stats.entries, 20);
        
        // Verify data integrity
        for i in 0..20 {
            let key = format!("key_{}", i);
            let expected = format!("important data {}", i);
            let data = cache.lookup(&key).await.unwrap().unwrap();
            assert_eq!(data, Bytes::from(expected));
        }
        
        // Continue operations after recovery
        for i in 20..25 {
            let key = format!("key_{}", i);
            let data = Bytes::from(format!("new data {}", i));
            cache.store(&key, data).await.unwrap();
        }
        
        cache.save_metadata().await.unwrap();
        
        let stats = cache.stats().await;
        assert_eq!(stats.entries, 25);
    }
    
    // Phase 3: Another recovery to verify continued operation
    {
        let cache = RawDiskCache::new(
            cache_file,
            10 * 1024 * 1024,
            4096,
            Duration::from_secs(3600),
        ).await.unwrap();
        
        let stats = cache.stats().await;
        assert_eq!(stats.entries, 25);
        
        // Verify all data
        for i in 0..25 {
            let key = format!("key_{}", i);
            let expected = if i < 20 {
                format!("important data {}", i)
            } else {
                format!("new data {}", i)
            };
            let data = cache.lookup(&key).await.unwrap().unwrap();
            assert_eq!(data, Bytes::from(expected));
        }
    }
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}

#[tokio::test]
async fn test_recovery_after_multiple_crashes() {
    let cache_file = "/tmp/test_crash_recovery_multiple";
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
    
    // Simulate multiple crash-recovery cycles
    for cycle in 0..3 {
        let cache = RawDiskCache::new(
            cache_file,
            10 * 1024 * 1024,
            4096,
            Duration::from_secs(3600),
        ).await.unwrap();
        
        // Add entries specific to this cycle
        for i in 0..5 {
            let key = format!("cycle_{}_key_{}", cycle, i);
            let data = Bytes::from(format!("cycle {} data {}", cycle, i));
            cache.store(&key, data).await.unwrap();
        }
        
        // Save metadata
        cache.save_metadata().await.unwrap();
        
        // Verify all previous cycles' data is still accessible
        for prev_cycle in 0..=cycle {
            for i in 0..5 {
                let key = format!("cycle_{}_key_{}", prev_cycle, i);
                let expected = format!("cycle {} data {}", prev_cycle, i);
                let data = cache.lookup(&key).await.unwrap().unwrap();
                assert_eq!(data, Bytes::from(expected));
            }
        }
        
        let stats = cache.stats().await;
        assert_eq!(stats.entries, (cycle + 1) * 5);
    }
    
    // Final verification after all cycles
    {
        let cache = RawDiskCache::new(
            cache_file,
            10 * 1024 * 1024,
            4096,
            Duration::from_secs(3600),
        ).await.unwrap();
        
        let stats = cache.stats().await;
        assert_eq!(stats.entries, 15); // 3 cycles * 5 entries
        
        // Verify all data from all cycles
        for cycle in 0..3 {
            for i in 0..5 {
                let key = format!("cycle_{}_key_{}", cycle, i);
                let expected = format!("cycle {} data {}", cycle, i);
                let data = cache.lookup(&key).await.unwrap().unwrap();
                assert_eq!(data, Bytes::from(expected));
            }
        }
    }
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}

#[tokio::test]
async fn test_recovery_with_large_entries() {
    let cache_file = "/tmp/test_crash_recovery_large";
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
    
    // Create cache with large entries
    {
        let cache = RawDiskCache::new(
            cache_file,
            10 * 1024 * 1024,
            4096,
            Duration::from_secs(3600),
        ).await.unwrap();
        
        // Store large entries (100KB each)
        for i in 0..5 {
            let key = format!("large_key_{}", i);
            let data = Bytes::from(vec![i as u8; 100_000]);
            cache.store(&key, data).await.unwrap();
        }
        
        cache.save_metadata().await.unwrap();
        
        let stats = cache.stats().await;
        assert_eq!(stats.entries, 5);
    }
    
    // Recover and verify large entries
    {
        let cache = RawDiskCache::new(
            cache_file,
            10 * 1024 * 1024,
            4096,
            Duration::from_secs(3600),
        ).await.unwrap();
        
        let stats = cache.stats().await;
        assert_eq!(stats.entries, 5);
        
        // Verify large data integrity
        for i in 0..5 {
            let key = format!("large_key_{}", i);
            let data = cache.lookup(&key).await.unwrap().unwrap();
            assert_eq!(data.len(), 100_000);
            assert!(data.iter().all(|&b| b == i as u8));
        }
    }
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}

#[tokio::test]
async fn test_recovery_with_mixed_operations() {
    let cache_file = "/tmp/test_crash_recovery_mixed";
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
    
    // Create cache with mixed operations
    {
        let cache = RawDiskCache::new(
            cache_file,
            10 * 1024 * 1024,
            4096,
            Duration::from_secs(3600),
        ).await.unwrap();
        
        // Add entries
        for i in 0..20 {
            let key = format!("key_{}", i);
            let data = Bytes::from(format!("data_{}", i));
            cache.store(&key, data).await.unwrap();
        }
        
        // Remove some entries
        for i in 0..5 {
            let key = format!("key_{}", i);
            cache.remove(&key).await.unwrap();
        }
        
        // Update some entries
        for i in 10..15 {
            let key = format!("key_{}", i);
            let data = Bytes::from(format!("updated_data_{}", i));
            cache.store(&key, data).await.unwrap();
        }
        
        cache.save_metadata().await.unwrap();
        
        let stats = cache.stats().await;
        assert_eq!(stats.entries, 15); // 20 - 5 removed
    }
    
    // Recover and verify
    {
        let cache = RawDiskCache::new(
            cache_file,
            10 * 1024 * 1024,
            4096,
            Duration::from_secs(3600),
        ).await.unwrap();
        
        let stats = cache.stats().await;
        assert_eq!(stats.entries, 15);
        
        // Verify removed entries are gone
        for i in 0..5 {
            let key = format!("key_{}", i);
            assert!(cache.lookup(&key).await.unwrap().is_none());
        }
        
        // Verify remaining entries
        for i in 5..10 {
            let key = format!("key_{}", i);
            let expected = format!("data_{}", i);
            let data = cache.lookup(&key).await.unwrap().unwrap();
            assert_eq!(data, Bytes::from(expected));
        }
        
        // Verify updated entries
        for i in 10..15 {
            let key = format!("key_{}", i);
            let expected = format!("updated_data_{}", i);
            let data = cache.lookup(&key).await.unwrap().unwrap();
            assert_eq!(data, Bytes::from(expected));
        }
        
        // Verify other entries
        for i in 15..20 {
            let key = format!("key_{}", i);
            let expected = format!("data_{}", i);
            let data = cache.lookup(&key).await.unwrap().unwrap();
            assert_eq!(data, Bytes::from(expected));
        }
    }
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}

#[tokio::test]
async fn test_recovery_with_superblock_intact() {
    let cache_file = "/tmp/test_crash_recovery_superblock";
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
    
    let original_total_blocks;
    
    // Create cache and save superblock info
    {
        let cache = RawDiskCache::new(
            cache_file,
            10 * 1024 * 1024,
            4096,
            Duration::from_secs(3600),
        ).await.unwrap();
        
        for i in 0..10 {
            let key = format!("key_{}", i);
            let data = Bytes::from(format!("data_{}", i));
            cache.store(&key, data).await.unwrap();
        }
        
        cache.save_metadata().await.unwrap();
        
        let stats = cache.stats().await;
        original_total_blocks = stats.total_blocks;
    }
    
    // Recover and verify superblock is intact
    {
        let cache = RawDiskCache::new(
            cache_file,
            10 * 1024 * 1024,
            4096,
            Duration::from_secs(3600),
        ).await.unwrap();
        
        let stats = cache.stats().await;
        assert_eq!(stats.total_blocks, original_total_blocks);
        assert_eq!(stats.entries, 10);
        
        // Verify we can still perform operations
        let key = "new_key";
        let data = Bytes::from("new_data");
        cache.store(key, data.clone()).await.unwrap();
        
        let retrieved = cache.lookup(key).await.unwrap().unwrap();
        assert_eq!(retrieved, data);
    }
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}
