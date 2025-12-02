//! Integration tests for Raw Disk Cache

use bytes::Bytes;
use pingora_slice::raw_disk::RawDiskCache;
use std::time::Duration;
use tokio::fs;

#[tokio::test]
async fn test_raw_disk_cache_basic() {
    let cache_file = "/tmp/test_raw_disk_cache_basic";
    
    // Clean up any existing file
    fs::remove_file(cache_file).await.ok();
    
    // Create cache
    let cache = RawDiskCache::new(
        cache_file,
        10 * 1024 * 1024, // 10MB
        4096,              // 4KB blocks
        Duration::from_secs(3600), // 1 hour TTL
    ).await.unwrap();
    
    // Test store and lookup
    let key = "test_key";
    let data = Bytes::from("Hello, Raw Disk Cache!");
    
    // Store data
    cache.store(key, data.clone()).await.unwrap();
    
    // Lookup data
    let retrieved = cache.lookup(key).await.unwrap().unwrap();
    assert_eq!(retrieved, data);
    
    // Test miss
    let missing = cache.lookup("nonexistent").await.unwrap();
    assert!(missing.is_none());
    
    // Test remove
    let removed = cache.remove(key).await.unwrap();
    assert!(removed);
    
    // Verify removed
    let after_remove = cache.lookup(key).await.unwrap();
    assert!(after_remove.is_none());
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}

#[tokio::test]
async fn test_raw_disk_cache_multiple_entries() {
    let cache_file = "/tmp/test_raw_disk_cache_multiple";
    
    // Clean up any existing file
    fs::remove_file(cache_file).await.ok();
    
    // Create cache
    let cache = RawDiskCache::new(
        cache_file,
        10 * 1024 * 1024, // 10MB
        4096,              // 4KB blocks
        Duration::from_secs(3600), // 1 hour TTL
    ).await.unwrap();
    
    // Store multiple entries
    for i in 0..10 {
        let key = format!("key_{}", i);
        let data = Bytes::from(format!("data for key {}", i));
        cache.store(&key, data).await.unwrap();
    }
    
    // Verify all entries
    for i in 0..10 {
        let key = format!("key_{}", i);
        let expected = format!("data for key {}", i);
        let retrieved = cache.lookup(&key).await.unwrap().unwrap();
        assert_eq!(retrieved, Bytes::from(expected));
    }
    
    // Check stats
    let stats = cache.stats().await;
    assert_eq!(stats.entries, 10);
    assert!(stats.used_blocks > 0);
    assert!(stats.hits > 0);
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}

#[tokio::test]
async fn test_raw_disk_cache_large_data() {
    let cache_file = "/tmp/test_raw_disk_cache_large";
    
    // Clean up any existing file
    fs::remove_file(cache_file).await.ok();
    
    // Create cache
    let cache = RawDiskCache::new(
        cache_file,
        10 * 1024 * 1024, // 10MB
        4096,              // 4KB blocks
        Duration::from_secs(3600), // 1 hour TTL
    ).await.unwrap();
    
    // Store large data (multiple blocks)
    let key = "large_key";
    let large_data = Bytes::from(vec![0xAB; 10000]); // 10KB data
    
    cache.store(key, large_data.clone()).await.unwrap();
    
    // Retrieve and verify
    let retrieved = cache.lookup(key).await.unwrap().unwrap();
    assert_eq!(retrieved.len(), large_data.len());
    assert_eq!(retrieved, large_data);
    
    // Check that blocks were used
    // Note: With compression enabled, highly compressible data (like repeated bytes)
    // may compress to less than 1 block, so we just verify some blocks are used
    let stats = cache.stats().await;
    assert!(stats.used_blocks >= 1); // At least 1 block should be used
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}

#[tokio::test]
async fn test_raw_disk_cache_gc() {
    let cache_file = "/tmp/test_raw_disk_cache_gc";
    
    // Clean up any existing file
    fs::remove_file(cache_file).await.ok();
    
    // Create cache
    let cache = RawDiskCache::new(
        cache_file,
        500 * 1024, // 500KB (small for testing GC)
        4096,        // 4KB blocks
        Duration::from_secs(3600), // 1 hour TTL
    ).await.unwrap();
    
    // Fill cache with many entries to use most of the space
    // 500KB / 4KB = ~122 blocks, fill ~100 blocks (82%)
    for i in 0..100 {
        let key = format!("gc_key_{}", i);
        let data = Bytes::from(vec![i as u8; 3500]); // ~3.5KB each, uses 1 block
        cache.store(&key, data).await.unwrap();
    }
    
    let stats_before = cache.stats().await;
    println!("Before GC: {} entries, {} used blocks, {} free blocks, {:.1}% free", 
             stats_before.entries, stats_before.used_blocks, stats_before.free_blocks,
             (stats_before.free_blocks as f64 / stats_before.total_blocks as f64) * 100.0);
    
    // Configure GC to ensure it runs
    use pingora_slice::raw_disk::{GCConfig, GCTriggerConfig, EvictionStrategy};
    let gc_config = GCConfig {
        strategy: EvictionStrategy::LRU,
        trigger: GCTriggerConfig {
            min_free_ratio: 0.0,  // Always trigger
            target_free_ratio: 0.5,
            adaptive: false,
            min_interval: Duration::from_secs(0),
        },
        incremental: false,
        batch_size: 10,
        ttl_secs: 0,
    };
    cache.update_gc_config(gc_config).await;
    
    // Trigger garbage collection - target 50% free (currently should be < 50%)
    let freed = cache.gc(0.5).await.unwrap();
    
    let stats_after = cache.stats().await;
    println!("After GC: {} entries, {} used blocks, freed: {}", stats_after.entries, stats_after.used_blocks, freed);
    
    // Should have freed some entries
    assert!(freed > 0);
    assert!(stats_after.entries < stats_before.entries);
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}

#[tokio::test]
async fn test_o_direct_basic_functionality() {
    let cache_file = "/tmp/test_o_direct_basic";
    
    // Clean up any existing file
    fs::remove_file(cache_file).await.ok();
    
    // Create cache with O_DIRECT enabled
    let cache = RawDiskCache::new_with_options(
        cache_file,
        10 * 1024 * 1024, // 10MB
        4096,              // 4KB blocks
        Duration::from_secs(3600), // 1 hour TTL
        true,              // Enable O_DIRECT
    ).await.unwrap();
    
    // Test store and lookup with various data sizes
    let test_cases = vec![
        ("small", Bytes::from("Small data")),
        ("medium", Bytes::from(vec![0xAB; 1024])), // 1KB
        ("large", Bytes::from(vec![0xCD; 8192])),  // 8KB
        ("unaligned", Bytes::from(vec![0xEF; 5555])), // Unaligned size
    ];
    
    for (key, data) in test_cases {
        // Store data
        cache.store(key, data.clone()).await.unwrap();
        
        // Lookup data
        let retrieved = cache.lookup(key).await.unwrap().unwrap();
        assert_eq!(retrieved, data, "Data mismatch for key: {}", key);
    }
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}

#[tokio::test]
async fn test_o_direct_unaligned_offsets() {
    let cache_file = "/tmp/test_o_direct_unaligned";
    
    // Clean up any existing file
    fs::remove_file(cache_file).await.ok();
    
    // Create cache with O_DIRECT enabled
    let cache = RawDiskCache::new_with_options(
        cache_file,
        10 * 1024 * 1024, // 10MB
        4096,              // 4KB blocks
        Duration::from_secs(3600), // 1 hour TTL
        true,              // Enable O_DIRECT
    ).await.unwrap();
    
    // Store multiple entries with different sizes to create unaligned offsets
    for i in 0..20 {
        let key = format!("unaligned_{}", i);
        // Use varying sizes to ensure unaligned offsets
        let size = 1000 + (i * 137) % 3000; // Varying sizes
        let data = Bytes::from(vec![i as u8; size]);
        cache.store(&key, data.clone()).await.unwrap();
        
        // Immediately verify
        let retrieved = cache.lookup(&key).await.unwrap().unwrap();
        assert_eq!(retrieved, data, "Data mismatch for key: {}", key);
    }
    
    // Verify all entries again
    for i in 0..20 {
        let key = format!("unaligned_{}", i);
        let size = 1000 + (i * 137) % 3000;
        let expected = Bytes::from(vec![i as u8; size]);
        let retrieved = cache.lookup(&key).await.unwrap().unwrap();
        assert_eq!(retrieved, expected, "Data mismatch on re-read for key: {}", key);
    }
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}

#[tokio::test]
#[ignore] // This is a performance test, run with --ignored
async fn test_o_direct_performance_comparison() {
    use std::time::Instant;
    
    let buffered_file = "/tmp/test_perf_buffered";
    let direct_file = "/tmp/test_perf_direct";
    
    // Clean up any existing files
    fs::remove_file(buffered_file).await.ok();
    fs::remove_file(direct_file).await.ok();
    
    let cache_size = 100 * 1024 * 1024; // 100MB
    let block_size = 4096;
    let ttl = Duration::from_secs(3600);
    
    // Create caches
    let cache_buffered = RawDiskCache::new_with_options(
        buffered_file,
        cache_size,
        block_size,
        ttl,
        false, // Buffered I/O
    ).await.unwrap();
    
    let cache_direct = RawDiskCache::new_with_options(
        direct_file,
        cache_size,
        block_size,
        ttl,
        true, // O_DIRECT
    ).await.unwrap();
    
    // Test data
    let num_entries = 100;
    let data_size = 64 * 1024; // 64KB per entry
    let test_data: Vec<(String, Bytes)> = (0..num_entries)
        .map(|i| {
            let key = format!("perf_key_{}", i);
            let data = Bytes::from(vec![i as u8; data_size]);
            (key, data)
        })
        .collect();
    
    // Benchmark buffered writes
    let start = Instant::now();
    for (key, data) in &test_data {
        cache_buffered.store(key, data.clone()).await.unwrap();
    }
    let buffered_write_time = start.elapsed();
    
    // Benchmark O_DIRECT writes
    let start = Instant::now();
    for (key, data) in &test_data {
        cache_direct.store(key, data.clone()).await.unwrap();
    }
    let direct_write_time = start.elapsed();
    
    // Benchmark buffered reads
    let start = Instant::now();
    for (key, _) in &test_data {
        cache_buffered.lookup(key).await.unwrap();
    }
    let buffered_read_time = start.elapsed();
    
    // Benchmark O_DIRECT reads
    let start = Instant::now();
    for (key, _) in &test_data {
        cache_direct.lookup(key).await.unwrap();
    }
    let direct_read_time = start.elapsed();
    
    // Print results
    println!("\n=== O_DIRECT Performance Comparison ===");
    println!("Test: {} entries of {} KB each", num_entries, data_size / 1024);
    println!("\nWrite Performance:");
    println!("  Buffered I/O: {:?} ({:.2} MB/s)", 
             buffered_write_time,
             (num_entries * data_size) as f64 / buffered_write_time.as_secs_f64() / 1024.0 / 1024.0);
    println!("  O_DIRECT:     {:?} ({:.2} MB/s)", 
             direct_write_time,
             (num_entries * data_size) as f64 / direct_write_time.as_secs_f64() / 1024.0 / 1024.0);
    println!("  Speedup:      {:.2}x", 
             buffered_write_time.as_secs_f64() / direct_write_time.as_secs_f64());
    
    println!("\nRead Performance:");
    println!("  Buffered I/O: {:?} ({:.2} MB/s)", 
             buffered_read_time,
             (num_entries * data_size) as f64 / buffered_read_time.as_secs_f64() / 1024.0 / 1024.0);
    println!("  O_DIRECT:     {:?} ({:.2} MB/s)", 
             direct_read_time,
             (num_entries * data_size) as f64 / direct_read_time.as_secs_f64() / 1024.0 / 1024.0);
    println!("  Speedup:      {:.2}x", 
             buffered_read_time.as_secs_f64() / direct_read_time.as_secs_f64());
    
    // Clean up
    fs::remove_file(buffered_file).await.ok();
    fs::remove_file(direct_file).await.ok();
}

#[tokio::test]
async fn test_o_direct_disabled_on_unsupported_systems() {
    let cache_file = "/tmp/test_o_direct_fallback";
    
    // Clean up any existing file
    fs::remove_file(cache_file).await.ok();
    
    // Create cache requesting O_DIRECT
    let cache = RawDiskCache::new_with_options(
        cache_file,
        10 * 1024 * 1024,
        4096,
        Duration::from_secs(3600),
        true, // Request O_DIRECT
    ).await.unwrap();
    
    // Cache should work regardless of O_DIRECT support
    let key = "test_key";
    let data = Bytes::from("Test data");
    
    cache.store(key, data.clone()).await.unwrap();
    let retrieved = cache.lookup(key).await.unwrap().unwrap();
    assert_eq!(retrieved, data);
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}
