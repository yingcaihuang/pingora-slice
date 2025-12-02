//! Example demonstrating batch I/O operations for improved throughput

use bytes::Bytes;
use pingora_slice::raw_disk::RawDiskCache;
use std::time::{Duration, Instant};
use tempfile::NamedTempFile;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== Batch I/O Example ===\n");

    // Create a temporary file for the cache
    let temp_file = NamedTempFile::new()?;
    let path = temp_file.path();

    // Create cache with 100MB capacity
    let cache = RawDiskCache::new_with_options(
        path,
        100 * 1024 * 1024, // 100MB
        4096,              // 4KB blocks
        Duration::from_secs(3600),
        false, // Disable O_DIRECT for this example
    )
    .await?;

    println!("Cache initialized with 100MB capacity\n");

    // Demonstrate buffered writes
    println!("--- Buffered Writes ---");
    let start = Instant::now();
    
    for i in 0..100 {
        let key = format!("buffered_key_{}", i);
        let data = Bytes::from(vec![i as u8; 4096]);
        cache.store_buffered(&key, data).await?;
    }
    
    // Flush remaining writes
    let flushed = cache.flush_writes().await?;
    let buffered_duration = start.elapsed();
    
    println!("Stored 100 entries using buffered writes");
    println!("Flushed {} operations", flushed);
    println!("Time: {:?}\n", buffered_duration);

    // Demonstrate direct writes for comparison
    println!("--- Direct Writes ---");
    let start = Instant::now();
    
    for i in 0..100 {
        let key = format!("direct_key_{}", i);
        let data = Bytes::from(vec![i as u8; 4096]);
        cache.store(&key, data).await?;
    }
    
    let direct_duration = start.elapsed();
    
    println!("Stored 100 entries using direct writes");
    println!("Time: {:?}\n", direct_duration);

    // Show performance improvement
    let improvement = direct_duration.as_secs_f64() / buffered_duration.as_secs_f64();
    println!("Buffered writes are {:.2}x faster\n", improvement);

    // Demonstrate batch reads
    println!("--- Batch Reads ---");
    
    let keys: Vec<String> = (0..50)
        .map(|i| format!("buffered_key_{}", i))
        .collect();
    
    let start = Instant::now();
    let results = cache.lookup_batch(&keys).await?;
    let batch_read_duration = start.elapsed();
    
    let hits = results.iter().filter(|r| r.is_some()).count();
    println!("Batch read {} keys: {} hits", keys.len(), hits);
    println!("Time: {:?}\n", batch_read_duration);

    // Compare with individual reads
    println!("--- Individual Reads ---");
    
    let start = Instant::now();
    let mut individual_hits = 0;
    
    for key in &keys {
        if cache.lookup(key).await?.is_some() {
            individual_hits += 1;
        }
    }
    
    let individual_read_duration = start.elapsed();
    
    println!("Individual read {} keys: {} hits", keys.len(), individual_hits);
    println!("Time: {:?}\n", individual_read_duration);

    // Show performance improvement
    let read_improvement = individual_read_duration.as_secs_f64() / batch_read_duration.as_secs_f64();
    println!("Batch reads are {:.2}x faster\n", read_improvement);

    // Show cache statistics
    println!("--- Cache Statistics ---");
    let stats = cache.stats().await;
    println!("Total entries: {}", stats.entries);
    println!("Used blocks: {}", stats.used_blocks);
    println!("Free blocks: {}", stats.free_blocks);
    println!("Hit rate: {:.2}%", 
        stats.hits as f64 / (stats.hits + stats.misses) as f64 * 100.0);
    println!("Pending writes: {}", stats.pending_writes);
    println!("Buffered bytes: {}", stats.buffered_bytes);

    Ok(())
}
