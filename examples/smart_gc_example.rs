//! Example demonstrating smart garbage collection features
//!
//! This example shows how to:
//! - Configure different eviction strategies (LRU, LFU, FIFO)
//! - Enable adaptive GC triggering
//! - Use incremental GC
//! - Monitor GC performance metrics

use bytes::Bytes;
use pingora_slice::raw_disk::{EvictionStrategy, GCConfig, GCTriggerConfig, RawDiskCache};
use std::time::Duration;
use tempfile::NamedTempFile;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== Smart GC Example ===\n");

    // Create a temporary file for the cache
    let temp_file = NamedTempFile::new()?;
    let path = temp_file.path();

    // Create cache with 5MB capacity
    let cache = RawDiskCache::new(
        path,
        5 * 1024 * 1024, // 5MB
        4096,
        Duration::from_secs(3600),
    )
    .await?;

    println!("Created cache with 5MB capacity\n");

    // Example 1: LRU Strategy
    println!("--- Example 1: LRU (Least Recently Used) Strategy ---");
    demonstrate_lru_strategy(&cache).await?;

    // Example 2: LFU Strategy
    println!("\n--- Example 2: LFU (Least Frequently Used) Strategy ---");
    demonstrate_lfu_strategy(&cache).await?;

    // Example 3: FIFO Strategy
    println!("\n--- Example 3: FIFO (First In First Out) Strategy ---");
    demonstrate_fifo_strategy(&cache).await?;

    // Example 4: Adaptive GC
    println!("\n--- Example 4: Adaptive GC Triggering ---");
    demonstrate_adaptive_gc(&cache).await?;

    // Example 5: Incremental GC
    println!("\n--- Example 5: Incremental GC ---");
    demonstrate_incremental_gc(&cache).await?;

    // Example 6: GC Metrics
    println!("\n--- Example 6: GC Performance Metrics ---");
    display_gc_metrics(&cache).await?;

    println!("\n=== Example Complete ===");
    Ok(())
}

async fn demonstrate_lru_strategy(cache: &RawDiskCache) -> Result<(), Box<dyn std::error::Error>> {
    // Configure LRU strategy
    let gc_config = GCConfig {
        strategy: EvictionStrategy::LRU,
        trigger: GCTriggerConfig {
            min_free_ratio: 0.3,
            target_free_ratio: 0.5,
            adaptive: false,
            min_interval: Duration::from_secs(0),
        },
        incremental: false,
        batch_size: 10,
        ttl_secs: 0,
    };
    cache.update_gc_config(gc_config).await;

    println!("Configured LRU eviction strategy");
    println!("  - Trigger when free space < 30%");
    println!("  - Target 50% free space after GC");

    // Fill cache
    for i in 0..20 {
        let key = format!("lru_key{}", i);
        let data = Bytes::from(vec![i as u8; 100 * 1024]); // 100KB each
        cache.store(&key, data).await?;
    }
    println!("Stored 20 entries (100KB each)");

    // Access some keys to make them recently used
    for i in 10..20 {
        let key = format!("lru_key{}", i);
        cache.lookup(&key).await?;
    }
    println!("Accessed keys 10-19 (making them recently used)");

    // Run GC
    let freed = cache.run_smart_gc().await?;
    println!("GC freed {} entries", freed);
    println!("LRU strategy evicted least recently used entries (keys 0-9)");

    Ok(())
}

async fn demonstrate_lfu_strategy(cache: &RawDiskCache) -> Result<(), Box<dyn std::error::Error>> {
    // Configure LFU strategy
    let gc_config = GCConfig {
        strategy: EvictionStrategy::LFU,
        trigger: GCTriggerConfig {
            min_free_ratio: 0.3,
            target_free_ratio: 0.5,
            adaptive: false,
            min_interval: Duration::from_secs(0),
        },
        incremental: false,
        batch_size: 10,
        ttl_secs: 0,
    };
    cache.update_gc_config(gc_config).await;

    println!("Configured LFU eviction strategy");

    // Fill cache
    for i in 0..20 {
        let key = format!("lfu_key{}", i);
        let data = Bytes::from(vec![i as u8; 100 * 1024]);
        cache.store(&key, data).await?;
    }
    println!("Stored 20 entries");

    // Access some keys multiple times
    for _ in 0..10 {
        for i in 10..20 {
            let key = format!("lfu_key{}", i);
            cache.lookup(&key).await?;
        }
    }
    println!("Accessed keys 10-19 ten times each (high frequency)");

    // Run GC
    let freed = cache.run_smart_gc().await?;
    println!("GC freed {} entries", freed);
    println!("LFU strategy evicted least frequently used entries");

    Ok(())
}

async fn demonstrate_fifo_strategy(cache: &RawDiskCache) -> Result<(), Box<dyn std::error::Error>> {
    // Configure FIFO strategy
    let gc_config = GCConfig {
        strategy: EvictionStrategy::FIFO,
        trigger: GCTriggerConfig {
            min_free_ratio: 0.3,
            target_free_ratio: 0.5,
            adaptive: false,
            min_interval: Duration::from_secs(0),
        },
        incremental: false,
        batch_size: 10,
        ttl_secs: 0,
    };
    cache.update_gc_config(gc_config).await;

    println!("Configured FIFO eviction strategy");

    // Fill cache
    for i in 0..20 {
        let key = format!("fifo_key{}", i);
        let data = Bytes::from(vec![i as u8; 100 * 1024]);
        cache.store(&key, data).await?;
    }
    println!("Stored 20 entries in order");

    // Run GC
    let freed = cache.run_smart_gc().await?;
    println!("GC freed {} entries", freed);
    println!("FIFO strategy evicted oldest entries (first inserted)");

    Ok(())
}

async fn demonstrate_adaptive_gc(cache: &RawDiskCache) -> Result<(), Box<dyn std::error::Error>> {
    // Configure adaptive GC
    let gc_config = GCConfig {
        strategy: EvictionStrategy::LRU,
        trigger: GCTriggerConfig {
            min_free_ratio: 0.2,
            target_free_ratio: 0.4,
            adaptive: true, // Enable adaptive triggering
            min_interval: Duration::from_secs(0),
        },
        incremental: false,
        batch_size: 10,
        ttl_secs: 0,
    };
    cache.update_gc_config(gc_config).await;

    println!("Configured adaptive GC triggering");
    println!("  - Automatically adjusts thresholds based on allocation patterns");
    println!("  - Increases threshold if allocation failures are frequent");
    println!("  - Decreases threshold if allocations succeed consistently");

    // Simulate workload
    for i in 0..30 {
        let key = format!("adaptive_key{}", i);
        let data = Bytes::from(vec![i as u8; 100 * 1024]);
        let _ = cache.store(&key, data).await; // Some may fail
    }

    let metrics = cache.gc_metrics().await;
    println!("Adaptive adjustments made: {}", metrics.adaptive_adjustments);

    Ok(())
}

async fn demonstrate_incremental_gc(cache: &RawDiskCache) -> Result<(), Box<dyn std::error::Error>> {
    // Configure incremental GC
    let gc_config = GCConfig {
        strategy: EvictionStrategy::LRU,
        trigger: GCTriggerConfig {
            min_free_ratio: 0.3,
            target_free_ratio: 0.5,
            adaptive: false,
            min_interval: Duration::from_secs(0),
        },
        incremental: true, // Enable incremental GC
        batch_size: 5,     // Process 5 entries at a time
        ttl_secs: 0,
    };
    cache.update_gc_config(gc_config).await;

    println!("Configured incremental GC");
    println!("  - Processes entries in small batches (5 at a time)");
    println!("  - Reduces GC pause times");
    println!("  - Allows other operations to proceed between batches");

    // Fill cache
    for i in 0..20 {
        let key = format!("inc_key{}", i);
        let data = Bytes::from(vec![i as u8; 100 * 1024]);
        cache.store(&key, data).await?;
    }

    // Run GC
    let start = std::time::Instant::now();
    let freed = cache.run_smart_gc().await?;
    let duration = start.elapsed();

    println!("Incremental GC freed {} entries in {:?}", freed, duration);
    println!("GC yielded between batches to allow other operations");

    Ok(())
}

async fn display_gc_metrics(cache: &RawDiskCache) -> Result<(), Box<dyn std::error::Error>> {
    let metrics = cache.gc_metrics().await;

    println!("GC Performance Metrics:");
    println!("  Total GC runs: {}", metrics.total_runs);
    println!("  Total entries evicted: {}", metrics.total_evicted);
    println!("  Total bytes freed: {} KB", metrics.total_bytes_freed / 1024);
    println!("  Total GC time: {:?}", metrics.total_duration);
    println!("  Average GC duration: {:?}", metrics.average_duration());
    println!("  Average entries per run: {:.1}", metrics.average_evicted());
    println!("  Adaptive adjustments: {}", metrics.adaptive_adjustments);

    if let Some(last_duration) = metrics.last_duration {
        println!("  Last GC duration: {:?}", last_duration);
        println!("  Last GC evicted: {} entries", metrics.last_evicted);
    }

    // Also show cache stats
    let stats = cache.stats().await;
    println!("\nCache Statistics:");
    println!("  Entries: {}", stats.entries);
    println!("  Used blocks: {}", stats.used_blocks);
    println!("  Free blocks: {}", stats.free_blocks);
    println!("  Total blocks: {}", stats.total_blocks);
    println!("  Free ratio: {:.2}%", 
             (stats.free_blocks as f64 / stats.total_blocks as f64) * 100.0);
    println!("  Cache hits: {}", stats.hits);
    println!("  Cache misses: {}", stats.misses);

    Ok(())
}
