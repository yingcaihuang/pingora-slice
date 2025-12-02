//! Example demonstrating zero-copy operations in raw disk cache
//!
//! This example shows how to use memory-mapped I/O (mmap) for efficient
//! large file access, reducing memory copy overhead.

use bytes::Bytes;
use pingora_slice::raw_disk::RawDiskCache;
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== Raw Disk Cache Zero-Copy Example ===\n");

    // Create a temporary cache file
    let cache_file = "/tmp/zero_copy_example_cache";
    
    // Clean up any existing file
    tokio::fs::remove_file(cache_file).await.ok();

    // Create cache with 100MB capacity
    println!("Creating raw disk cache...");
    let cache = RawDiskCache::new(
        cache_file,
        100 * 1024 * 1024, // 100MB
        4096,               // 4KB blocks
        Duration::from_secs(3600), // 1 hour TTL
    )
    .await?;

    println!("Cache created successfully");
    println!("Zero-copy available: {}\n", cache.is_zero_copy_available());

    // Example 1: Small file (below mmap threshold)
    println!("--- Example 1: Small File (10KB) ---");
    let small_key = "small_file";
    let small_data = Bytes::from(vec![0x11; 10 * 1024]); // 10KB
    
    cache.store(small_key, small_data.clone()).await?;
    println!("Stored small file: {} bytes", small_data.len());
    
    let retrieved = cache.lookup_zero_copy(small_key).await?.unwrap();
    println!("Retrieved using zero-copy: {} bytes", retrieved.len());
    println!("Data matches: {}", retrieved == small_data);
    
    let stats = cache.zero_copy_stats().await;
    println!("Stats: mmap_reads={}, mmap_skipped={}\n", stats.mmap_reads, stats.mmap_skipped);

    // Example 2: Large file (above mmap threshold)
    println!("--- Example 2: Large File (1MB) ---");
    let large_key = "large_file";
    let large_data = Bytes::from(vec![0x22; 1024 * 1024]); // 1MB
    
    cache.store(large_key, large_data.clone()).await?;
    println!("Stored large file: {} bytes", large_data.len());
    
    let retrieved = cache.lookup_zero_copy(large_key).await?.unwrap();
    println!("Retrieved using zero-copy (mmap): {} bytes", retrieved.len());
    println!("Data matches: {}", retrieved == large_data);
    
    let stats = cache.zero_copy_stats().await;
    println!("Stats: mmap_reads={}, mmap_bytes={} MB\n", 
             stats.mmap_reads, stats.mmap_bytes / 1024 / 1024);

    // Example 3: Performance comparison
    println!("--- Example 3: Performance Comparison ---");
    
    // Store multiple large files
    let num_files = 20;
    let file_size = 2 * 1024 * 1024; // 2MB each
    
    println!("Storing {} files of {} MB each...", num_files, file_size / 1024 / 1024);
    for i in 0..num_files {
        let key = format!("perf_file_{}", i);
        let data = Bytes::from(vec![i as u8; file_size]);
        cache.store(&key, data).await?;
    }
    
    // Benchmark regular lookup
    println!("\nBenchmarking regular lookup...");
    let start = Instant::now();
    for i in 0..num_files {
        let key = format!("perf_file_{}", i);
        cache.lookup(&key).await?;
    }
    let regular_time = start.elapsed();
    let regular_throughput = (num_files * file_size) as f64 / regular_time.as_secs_f64() / 1024.0 / 1024.0;
    
    // Benchmark zero-copy lookup
    println!("Benchmarking zero-copy lookup...");
    let start = Instant::now();
    for i in 0..num_files {
        let key = format!("perf_file_{}", i);
        cache.lookup_zero_copy(&key).await?;
    }
    let zero_copy_time = start.elapsed();
    let zero_copy_throughput = (num_files * file_size) as f64 / zero_copy_time.as_secs_f64() / 1024.0 / 1024.0;
    
    println!("\nResults:");
    println!("  Regular lookup:   {:?} ({:.2} MB/s)", regular_time, regular_throughput);
    println!("  Zero-copy lookup: {:?} ({:.2} MB/s)", zero_copy_time, zero_copy_throughput);
    println!("  Speedup:          {:.2}x", regular_time.as_secs_f64() / zero_copy_time.as_secs_f64());
    
    // Example 4: Cache statistics
    println!("\n--- Example 4: Cache Statistics ---");
    let cache_stats = cache.stats().await;
    println!("Cache entries: {}", cache_stats.entries);
    println!("Used blocks: {} / {}", cache_stats.used_blocks, cache_stats.total_blocks);
    println!("Cache hits: {}, misses: {}", cache_stats.hits, cache_stats.misses);
    
    if let Some(zc_stats) = cache_stats.zero_copy_stats {
        println!("\nZero-copy statistics:");
        println!("  mmap reads:       {}", zc_stats.mmap_reads);
        println!("  mmap bytes:       {} MB", zc_stats.mmap_bytes / 1024 / 1024);
        println!("  mmap skipped:     {}", zc_stats.mmap_skipped);
        println!("  sendfile xfers:   {}", zc_stats.sendfile_transfers);
        println!("  sendfile bytes:   {} MB", zc_stats.sendfile_bytes / 1024 / 1024);
    }

    // Example 5: Mixed workload
    println!("\n--- Example 5: Mixed Workload ---");
    println!("Storing mix of small and large files...");
    
    for i in 0..10 {
        // Small files
        let key = format!("mixed_small_{}", i);
        let data = Bytes::from(vec![0x33; 5 * 1024]); // 5KB
        cache.store(&key, data).await?;
        
        // Large files
        let key = format!("mixed_large_{}", i);
        let data = Bytes::from(vec![0x44; 500 * 1024]); // 500KB
        cache.store(&key, data).await?;
    }
    
    println!("Reading all files with zero-copy...");
    for i in 0..10 {
        cache.lookup_zero_copy(&format!("mixed_small_{}", i)).await?;
        cache.lookup_zero_copy(&format!("mixed_large_{}", i)).await?;
    }
    
    let final_stats = cache.zero_copy_stats().await;
    println!("\nFinal zero-copy statistics:");
    println!("  Total mmap reads:     {}", final_stats.mmap_reads);
    println!("  Total mmap bytes:     {} MB", final_stats.mmap_bytes / 1024 / 1024);
    println!("  Total mmap skipped:   {}", final_stats.mmap_skipped);
    println!("  Efficiency:           {:.1}% used mmap", 
             (final_stats.mmap_reads as f64 / (final_stats.mmap_reads + final_stats.mmap_skipped) as f64) * 100.0);

    // Clean up
    println!("\nCleaning up...");
    tokio::fs::remove_file(cache_file).await.ok();
    
    println!("\n=== Example Complete ===");
    Ok(())
}
