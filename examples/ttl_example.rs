//! Example demonstrating TTL (Time-To-Live) support in raw disk cache
//!
//! This example shows:
//! - Setting TTL when creating a cache
//! - Automatic expiration on lookup
//! - Manual cleanup of expired entries
//! - TTL integration with GC

use bytes::Bytes;
use pingora_slice::raw_disk::RawDiskCache;
use std::time::Duration;
use tempfile::NamedTempFile;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== Raw Disk Cache TTL Example ===\n");

    // Create a temporary file for the cache
    let temp_file = NamedTempFile::new()?;
    println!("Cache file: {:?}", temp_file.path());

    // Create cache with 5 second TTL
    let ttl = Duration::from_secs(5);
    let cache = RawDiskCache::new(
        temp_file.path(),
        10 * 1024 * 1024, // 10MB
        4096,             // 4KB blocks
        ttl,
    )
    .await?;

    println!("Created cache with TTL: {:?}\n", ttl);

    // Store some data
    println!("Storing entries...");
    for i in 0..10 {
        let key = format!("key_{}", i);
        let data = Bytes::from(format!("data_{}", i));
        cache.store(&key, data).await?;
    }

    let stats = cache.stats().await;
    println!("Stored {} entries\n", stats.entries);

    // Lookup immediately - should succeed
    println!("Looking up key_0 immediately...");
    let result = cache.lookup("key_0").await?;
    assert!(result.is_some());
    println!("✓ Found: {:?}\n", String::from_utf8_lossy(&result.unwrap()));

    // Wait 2 seconds
    println!("Waiting 2 seconds...");
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Lookup should still succeed
    println!("Looking up key_1 after 2 seconds...");
    let result = cache.lookup("key_1").await?;
    assert!(result.is_some());
    println!("✓ Found: {:?}\n", String::from_utf8_lossy(&result.unwrap()));

    // Wait for TTL to expire
    println!("Waiting for TTL to expire (4 more seconds)...");
    tokio::time::sleep(Duration::from_secs(4)).await;

    // Lookup should now fail and remove the entry
    println!("Looking up key_2 after TTL expired...");
    let result = cache.lookup("key_2").await?;
    assert!(result.is_none());
    println!("✗ Entry expired and removed\n");

    // Check stats
    let stats = cache.stats().await;
    println!("Entries after expiration: {}\n", stats.entries);

    // Manual cleanup of all expired entries
    println!("Running manual cleanup of expired entries...");
    let removed = cache.cleanup_expired().await?;
    println!("✓ Removed {} expired entries\n", removed);

    let stats = cache.stats().await;
    println!("Entries after cleanup: {}\n", stats.entries);

    // Store new entries
    println!("Storing new entries...");
    for i in 10..15 {
        let key = format!("key_{}", i);
        let data = Bytes::from(format!("data_{}", i));
        cache.store(&key, data).await?;
    }

    let stats = cache.stats().await;
    println!("Total entries: {}\n", stats.entries);

    // Demonstrate TTL with GC
    println!("=== TTL Integration with GC ===\n");

    // Store many entries to fill cache
    println!("Filling cache...");
    for i in 0..50 {
        let key = format!("bulk_key_{}", i);
        let data = Bytes::from(vec![0u8; 10_000]); // 10KB each
        cache.store(&key, data).await?;
    }

    let stats = cache.stats().await;
    println!("Entries: {}", stats.entries);
    println!("Used blocks: {}", stats.used_blocks);
    println!("Free blocks: {}\n", stats.free_blocks);

    // Wait for some entries to expire
    println!("Waiting 6 seconds for entries to expire...");
    tokio::time::sleep(Duration::from_secs(6)).await;

    // Run GC - should prioritize expired entries
    println!("Running GC...");
    let removed = cache.run_smart_gc().await?;
    println!("✓ GC removed {} entries\n", removed);

    let stats = cache.stats().await;
    println!("After GC:");
    println!("  Entries: {}", stats.entries);
    println!("  Used blocks: {}", stats.used_blocks);
    println!("  Free blocks: {}", stats.free_blocks);

    if let Some(gc_metrics) = &stats.gc_metrics {
        println!("\nGC Metrics:");
        println!("  Total runs: {}", gc_metrics.total_runs);
        println!("  Total evicted: {}", gc_metrics.total_evicted);
        println!("  Total bytes freed: {}", gc_metrics.total_bytes_freed);
    }

    println!("\n=== TTL Example Complete ===");

    Ok(())
}
