//! Example demonstrating io_uring support for raw disk cache
//!
//! This example shows how to use io_uring for high-performance I/O operations
//! on Linux systems.

#[cfg(target_os = "linux")]
use bytes::Bytes;
#[cfg(target_os = "linux")]
use pingora_slice::raw_disk::{IoUringConfig, RawDiskCache};
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};
#[cfg(target_os = "linux")]
use tempfile::NamedTempFile;

#[cfg(target_os = "linux")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== io_uring Raw Disk Cache Example ===\n");

    // Create a temporary file for the cache
    let temp_file = NamedTempFile::new()?;
    let cache_path = temp_file.path();
    println!("Cache file: {}\n", cache_path.display());

    // Configure io_uring
    let io_uring_config = IoUringConfig {
        queue_depth: 256,      // Support up to 256 concurrent operations
        use_sqpoll: false,     // Don't use kernel polling (requires privileges)
        use_iopoll: false,     // Don't use I/O polling
        block_size: 4096,      // 4KB blocks
    };

    println!("io_uring Configuration:");
    println!("  Queue Depth: {}", io_uring_config.queue_depth);
    println!("  SQPOLL: {}", io_uring_config.use_sqpoll);
    println!("  IOPOLL: {}", io_uring_config.use_iopoll);
    println!("  Block Size: {} bytes\n", io_uring_config.block_size);

    // Create cache with io_uring
    let cache = RawDiskCache::new_with_io_uring(
        cache_path,
        100 * 1024 * 1024, // 100MB cache
        4096,              // 4KB blocks
        Duration::from_secs(3600), // 1 hour TTL
        io_uring_config,
    ).await?;

    println!("Cache initialized with io_uring backend\n");

    // Example 1: Basic store and lookup
    println!("Example 1: Basic Operations");
    println!("----------------------------");
    
    let key = "example_key";
    let data = Bytes::from("Hello from io_uring!");
    
    println!("Storing data with key '{}'...", key);
    cache.store_with_io_uring(key, data.clone()).await?;
    
    println!("Looking up data...");
    let result = cache.lookup_with_io_uring(key).await?;
    
    if let Some(retrieved) = result {
        println!("Retrieved: {}", String::from_utf8_lossy(&retrieved));
        assert_eq!(retrieved, data);
    }
    println!();

    // Example 2: Multiple operations
    println!("Example 2: Multiple Operations");
    println!("-------------------------------");
    
    let num_operations = 100;
    println!("Storing {} entries...", num_operations);
    
    let start = Instant::now();
    for i in 0..num_operations {
        let key = format!("key_{}", i);
        let data = Bytes::from(format!("Data for entry {}", i));
        cache.store_with_io_uring(&key, data).await?;
    }
    let store_duration = start.elapsed();
    
    println!("Stored {} entries in {:?}", num_operations, store_duration);
    println!("Average: {:?} per operation", store_duration / num_operations);
    println!();

    // Example 3: Batch lookup
    println!("Example 3: Batch Lookup");
    println!("-----------------------");
    
    let keys: Vec<String> = (0..10).map(|i| format!("key_{}", i)).collect();
    println!("Looking up {} keys in batch...", keys.len());
    
    let start = Instant::now();
    let results = cache.lookup_batch(&keys).await?;
    let lookup_duration = start.elapsed();
    
    let hits = results.iter().filter(|r| r.is_some()).count();
    println!("Retrieved {} entries in {:?}", hits, lookup_duration);
    println!("Average: {:?} per operation", lookup_duration / keys.len() as u32);
    println!();

    // Example 4: Large data
    println!("Example 4: Large Data");
    println!("---------------------");
    
    let large_data = Bytes::from(vec![0xAB; 1024 * 1024]); // 1MB
    println!("Storing 1MB of data...");
    
    let start = Instant::now();
    cache.store_with_io_uring("large_key", large_data.clone()).await?;
    let store_duration = start.elapsed();
    
    println!("Stored in {:?}", store_duration);
    
    println!("Retrieving 1MB of data...");
    let start = Instant::now();
    let result = cache.lookup_with_io_uring("large_key").await?;
    let lookup_duration = start.elapsed();
    
    println!("Retrieved in {:?}", lookup_duration);
    
    if let Some(retrieved) = result {
        assert_eq!(retrieved.len(), large_data.len());
        println!("Data verified successfully");
    }
    println!();

    // Example 5: Cache statistics
    println!("Example 5: Cache Statistics");
    println!("---------------------------");
    
    let stats = cache.stats().await;
    println!("Entries: {}", stats.entries);
    println!("Used Blocks: {}", stats.used_blocks);
    println!("Free Blocks: {}", stats.free_blocks);
    println!("Total Blocks: {}", stats.total_blocks);
    println!("Utilization: {:.2}%", 
             (stats.used_blocks as f64 / stats.total_blocks as f64) * 100.0);
    println!("Cache Hits: {}", stats.hits);
    println!("Cache Misses: {}", stats.misses);
    println!();

    // Example 6: Performance comparison
    println!("Example 6: Performance Comparison");
    println!("---------------------------------");
    
    // Create a standard cache for comparison
    let temp_file2 = NamedTempFile::new()?;
    let standard_cache = RawDiskCache::new(
        temp_file2.path(),
        100 * 1024 * 1024,
        4096,
        Duration::from_secs(3600),
    ).await?;

    let test_data = Bytes::from(vec![0xCD; 64 * 1024]); // 64KB
    let num_ops = 50;

    // Test standard I/O
    println!("Testing standard I/O ({} operations)...", num_ops);
    let start = Instant::now();
    for i in 0..num_ops {
        standard_cache.store(&format!("std_{}", i), test_data.clone()).await?;
    }
    let standard_duration = start.elapsed();
    println!("Standard I/O: {:?} ({:?} per op)", 
             standard_duration, standard_duration / num_ops);

    // Test io_uring
    println!("Testing io_uring ({} operations)...", num_ops);
    let start = Instant::now();
    for i in 0..num_ops {
        cache.store_with_io_uring(&format!("uring_{}", i), test_data.clone()).await?;
    }
    let io_uring_duration = start.elapsed();
    println!("io_uring: {:?} ({:?} per op)", 
             io_uring_duration, io_uring_duration / num_ops);

    let speedup = standard_duration.as_secs_f64() / io_uring_duration.as_secs_f64();
    println!("Speedup: {:.2}x", speedup);
    println!();

    println!("=== Example Complete ===");

    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("This example requires Linux with io_uring support.");
    println!("io_uring is not available on this platform.");
}
