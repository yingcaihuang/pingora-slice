//! Tests for zero-copy operations in raw disk cache

use bytes::Bytes;
use pingora_slice::raw_disk::RawDiskCache;
use std::time::Duration;
use tokio::fs;

#[tokio::test]
async fn test_zero_copy_lookup_small_file() {
    let cache_file = "/tmp/test_zero_copy_small";
    
    // Clean up any existing file
    fs::remove_file(cache_file).await.ok();
    
    // Create cache
    let cache = RawDiskCache::new(
        cache_file,
        10 * 1024 * 1024, // 10MB
        4096,              // 4KB blocks
        Duration::from_secs(3600), // 1 hour TTL
    ).await.unwrap();
    
    // Store small data (below mmap threshold)
    let key = "small_key";
    let data = Bytes::from("Small data that won't use mmap");
    
    cache.store(key, data.clone()).await.unwrap();
    
    // Lookup using zero-copy (should fall back to regular read)
    let retrieved = cache.lookup_zero_copy(key).await.unwrap().unwrap();
    assert_eq!(retrieved, data);
    
    // Check stats - should have skipped mmap
    let stats = cache.zero_copy_stats().await;
    assert_eq!(stats.mmap_reads, 0);
    assert_eq!(stats.mmap_skipped, 1);
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}

#[tokio::test]
async fn test_zero_copy_lookup_large_file() {
    let cache_file = "/tmp/test_zero_copy_large";
    
    // Clean up any existing file
    fs::remove_file(cache_file).await.ok();
    
    // Create cache
    let cache = RawDiskCache::new(
        cache_file,
        10 * 1024 * 1024, // 10MB
        4096,              // 4KB blocks
        Duration::from_secs(3600), // 1 hour TTL
    ).await.unwrap();
    
    // Store large data (above mmap threshold of 64KB)
    let key = "large_key";
    let large_data = Bytes::from(vec![0xAB; 128 * 1024]); // 128KB
    
    cache.store(key, large_data.clone()).await.unwrap();
    
    // Lookup using zero-copy (should use mmap)
    let retrieved = cache.lookup_zero_copy(key).await.unwrap().unwrap();
    assert_eq!(retrieved.len(), large_data.len());
    assert_eq!(retrieved, large_data);
    
    // Check stats - should have used mmap
    let stats = cache.zero_copy_stats().await;
    assert_eq!(stats.mmap_reads, 1);
    assert_eq!(stats.mmap_bytes, 128 * 1024);
    assert_eq!(stats.mmap_skipped, 0);
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}

#[tokio::test]
async fn test_zero_copy_multiple_lookups() {
    let cache_file = "/tmp/test_zero_copy_multiple";
    
    // Clean up any existing file
    fs::remove_file(cache_file).await.ok();
    
    // Create cache
    let cache = RawDiskCache::new(
        cache_file,
        20 * 1024 * 1024, // 20MB
        4096,              // 4KB blocks
        Duration::from_secs(3600), // 1 hour TTL
    ).await.unwrap();
    
    // Store mix of small and large files
    let small_keys: Vec<String> = (0..5).map(|i| format!("small_{}", i)).collect();
    let large_keys: Vec<String> = (0..5).map(|i| format!("large_{}", i)).collect();
    
    // Store small files
    for key in &small_keys {
        let data = Bytes::from(vec![0x11; 10 * 1024]); // 10KB
        cache.store(key, data).await.unwrap();
    }
    
    // Store large files
    for key in &large_keys {
        let data = Bytes::from(vec![0x22; 100 * 1024]); // 100KB
        cache.store(key, data).await.unwrap();
    }
    
    // Lookup all files using zero-copy
    for key in &small_keys {
        let data = cache.lookup_zero_copy(key).await.unwrap().unwrap();
        assert_eq!(data.len(), 10 * 1024);
    }
    
    for key in &large_keys {
        let data = cache.lookup_zero_copy(key).await.unwrap().unwrap();
        assert_eq!(data.len(), 100 * 1024);
    }
    
    // Check stats
    let stats = cache.zero_copy_stats().await;
    // Should have used mmap for large files (may vary due to prefetch)
    assert!(stats.mmap_reads >= 4 && stats.mmap_reads <= 5, 
            "Expected 4-5 mmap reads, got {}", stats.mmap_reads);
    assert!(stats.mmap_bytes >= 4 * 100 * 1024 && stats.mmap_bytes <= 5 * 100 * 1024,
            "Expected 400-500KB mmap bytes, got {}", stats.mmap_bytes);
    assert_eq!(stats.mmap_skipped, 5); // 5 small files
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}

#[tokio::test]
async fn test_zero_copy_vs_regular_lookup() {
    let cache_file = "/tmp/test_zero_copy_comparison";
    
    // Clean up any existing file
    fs::remove_file(cache_file).await.ok();
    
    // Create cache
    let cache = RawDiskCache::new(
        cache_file,
        10 * 1024 * 1024, // 10MB
        4096,              // 4KB blocks
        Duration::from_secs(3600), // 1 hour TTL
    ).await.unwrap();
    
    // Store test data
    let key = "test_key";
    let data = Bytes::from(vec![0xCD; 200 * 1024]); // 200KB
    
    cache.store(key, data.clone()).await.unwrap();
    
    // Lookup using regular method
    let regular_result = cache.lookup(key).await.unwrap().unwrap();
    assert_eq!(regular_result, data);
    
    // Lookup using zero-copy method
    let zero_copy_result = cache.lookup_zero_copy(key).await.unwrap().unwrap();
    assert_eq!(zero_copy_result, data);
    
    // Both should return the same data
    assert_eq!(regular_result, zero_copy_result);
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}

#[tokio::test]
async fn test_zero_copy_checksum_verification() {
    let cache_file = "/tmp/test_zero_copy_checksum";
    
    // Clean up any existing file
    fs::remove_file(cache_file).await.ok();
    
    // Create cache
    let cache = RawDiskCache::new(
        cache_file,
        10 * 1024 * 1024, // 10MB
        4096,              // 4KB blocks
        Duration::from_secs(3600), // 1 hour TTL
    ).await.unwrap();
    
    // Store data
    let key = "checksum_key";
    let data = Bytes::from(vec![0xEF; 150 * 1024]); // 150KB
    
    cache.store(key, data.clone()).await.unwrap();
    
    // Lookup using zero-copy - should verify checksum
    let retrieved = cache.lookup_zero_copy(key).await.unwrap().unwrap();
    assert_eq!(retrieved, data);
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}

#[tokio::test]
async fn test_zero_copy_availability() {
    let cache_file = "/tmp/test_zero_copy_availability";
    
    // Clean up any existing file
    fs::remove_file(cache_file).await.ok();
    
    // Create cache
    let cache = RawDiskCache::new(
        cache_file,
        10 * 1024 * 1024, // 10MB
        4096,              // 4KB blocks
        Duration::from_secs(3600), // 1 hour TTL
    ).await.unwrap();
    
    // Check if zero-copy is available
    assert!(cache.is_zero_copy_available());
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}

#[tokio::test]
#[cfg(target_os = "linux")]
async fn test_sendfile_availability() {
    let cache_file = "/tmp/test_sendfile_availability";
    
    // Clean up any existing file
    fs::remove_file(cache_file).await.ok();
    
    // Create cache
    let cache = RawDiskCache::new(
        cache_file,
        10 * 1024 * 1024, // 10MB
        4096,              // 4KB blocks
        Duration::from_secs(3600), // 1 hour TTL
    ).await.unwrap();
    
    // Store some data
    let key = "sendfile_key";
    let data = Bytes::from(vec![0x99; 50 * 1024]); // 50KB
    cache.store(key, data).await.unwrap();
    
    // Note: We can't easily test actual sendfile without a real socket
    // But we can verify the method exists and returns an error for invalid fd
    let result = cache.sendfile_to_socket(key, -1).await;
    assert!(result.is_err()); // Should fail with invalid fd
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}

#[tokio::test]
#[ignore] // Performance test, run with --ignored
async fn test_zero_copy_performance() {
    use std::time::Instant;
    
    let cache_file = "/tmp/test_zero_copy_perf";
    
    // Clean up any existing file
    fs::remove_file(cache_file).await.ok();
    
    // Create cache
    let cache = RawDiskCache::new(
        cache_file,
        100 * 1024 * 1024, // 100MB
        4096,               // 4KB blocks
        Duration::from_secs(3600), // 1 hour TTL
    ).await.unwrap();
    
    // Store test data
    let num_files = 50;
    let file_size = 1024 * 1024; // 1MB each
    
    for i in 0..num_files {
        let key = format!("perf_key_{}", i);
        let data = Bytes::from(vec![i as u8; file_size]);
        cache.store(&key, data).await.unwrap();
    }
    
    // Benchmark regular lookup
    let start = Instant::now();
    for i in 0..num_files {
        let key = format!("perf_key_{}", i);
        cache.lookup(&key).await.unwrap();
    }
    let regular_time = start.elapsed();
    
    // Benchmark zero-copy lookup
    let start = Instant::now();
    for i in 0..num_files {
        let key = format!("perf_key_{}", i);
        cache.lookup_zero_copy(&key).await.unwrap();
    }
    let zero_copy_time = start.elapsed();
    
    println!("\n=== Zero-Copy Performance Comparison ===");
    println!("Test: {} files of {} MB each", num_files, file_size / 1024 / 1024);
    println!("\nRegular lookup:   {:?} ({:.2} MB/s)", 
             regular_time,
             (num_files * file_size) as f64 / regular_time.as_secs_f64() / 1024.0 / 1024.0);
    println!("Zero-copy lookup: {:?} ({:.2} MB/s)", 
             zero_copy_time,
             (num_files * file_size) as f64 / zero_copy_time.as_secs_f64() / 1024.0 / 1024.0);
    println!("Speedup:          {:.2}x", 
             regular_time.as_secs_f64() / zero_copy_time.as_secs_f64());
    
    // Print stats
    let stats = cache.zero_copy_stats().await;
    println!("\nZero-copy stats:");
    println!("  mmap reads:     {}", stats.mmap_reads);
    println!("  mmap bytes:     {} MB", stats.mmap_bytes / 1024 / 1024);
    println!("  mmap skipped:   {}", stats.mmap_skipped);
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}

#[tokio::test]
async fn test_zero_copy_with_prefetch() {
    let cache_file = "/tmp/test_zero_copy_prefetch";
    
    // Clean up any existing file
    fs::remove_file(cache_file).await.ok();
    
    // Create cache
    let cache = RawDiskCache::new(
        cache_file,
        20 * 1024 * 1024, // 20MB
        4096,              // 4KB blocks
        Duration::from_secs(3600), // 1 hour TTL
    ).await.unwrap();
    
    // Store sequential data
    for i in 0..10 {
        let key = format!("seq_{}", i);
        let data = Bytes::from(vec![i as u8; 100 * 1024]); // 100KB each
        cache.store(&key, data).await.unwrap();
    }
    
    // Access sequentially to trigger prefetch
    for i in 0..5 {
        let key = format!("seq_{}", i);
        cache.lookup_zero_copy(&key).await.unwrap();
    }
    
    // Give prefetch time to work
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Access later keys - some might be prefetched
    for i in 5..10 {
        let key = format!("seq_{}", i);
        let data = cache.lookup_zero_copy(&key).await.unwrap().unwrap();
        assert_eq!(data.len(), 100 * 1024);
    }
    
    // Check that zero-copy was used
    let stats = cache.zero_copy_stats().await;
    assert!(stats.mmap_reads > 0);
    
    // Clean up
    fs::remove_file(cache_file).await.ok();
}
