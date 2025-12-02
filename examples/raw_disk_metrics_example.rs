//! Raw Disk Cache Metrics Example
//!
//! This example demonstrates how to use the monitoring and metrics
//! functionality of the raw disk cache.

use bytes::Bytes;
use pingora_slice::raw_disk::{RawDiskCache, metrics::format_prometheus_metrics};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("=== Raw Disk Cache Metrics Example ===\n");

    // Create a temporary file for the cache
    let temp_dir = tempfile::tempdir()?;
    let cache_file = temp_dir.path().join("metrics_cache.dat");

    // Create cache with 10MB size and 4KB blocks
    let cache = RawDiskCache::new(
        &cache_file,
        10 * 1024 * 1024, // 10MB
        4096,             // 4KB blocks
        Duration::from_secs(3600), // 1 hour TTL
    )
    .await?;

    println!("Cache created successfully\n");

    // Perform some operations to generate metrics
    println!("Performing cache operations...\n");

    // Store some data
    for i in 0..10 {
        let key = format!("key_{}", i);
        let data = Bytes::from(format!("value_{}", i).repeat(100));
        cache.store(&key, data).await?;
    }

    // Lookup some data (hits)
    for i in 0..5 {
        let key = format!("key_{}", i);
        let _ = cache.lookup(&key).await?;
    }

    // Lookup non-existent data (misses)
    for i in 10..15 {
        let key = format!("key_{}", i);
        let _ = cache.lookup(&key).await?;
    }

    // Remove some data
    for i in 0..3 {
        let key = format!("key_{}", i);
        cache.remove(&key).await?;
    }

    // Wait a bit for metrics to settle
    sleep(Duration::from_millis(100)).await;

    // Get metrics snapshot
    println!("=== Metrics Snapshot ===\n");
    let snapshot = cache.metrics_snapshot_async().await;

    println!("Operation Counters:");
    println!("  Store operations: {}", snapshot.store_operations);
    println!("  Lookup operations: {}", snapshot.lookup_operations);
    println!("  Remove operations: {}", snapshot.remove_operations);
    println!();

    println!("Success/Failure Metrics:");
    println!("  Store successes: {}", snapshot.store_successes);
    println!("  Store failures: {}", snapshot.store_failures);
    println!("  Lookup hits: {}", snapshot.lookup_hits);
    println!("  Lookup misses: {}", snapshot.lookup_misses);
    println!("  Cache hit rate: {:.2}%", snapshot.cache_hit_rate());
    println!("  Store success rate: {:.2}%", snapshot.store_success_rate());
    println!();

    println!("I/O Metrics:");
    println!("  Bytes written: {}", snapshot.bytes_written);
    println!("  Bytes read: {}", snapshot.bytes_read);
    println!("  Disk writes: {}", snapshot.disk_writes);
    println!("  Disk reads: {}", snapshot.disk_reads);
    println!();

    println!("Latency Metrics:");
    println!("  Avg store duration: {:.2}ms", snapshot.avg_store_duration_ms());
    println!("  Avg lookup duration: {:.2}ms", snapshot.avg_lookup_duration_ms());
    println!("  Avg remove duration: {:.2}ms", snapshot.avg_remove_duration_ms());
    println!();

    println!("Cache State:");
    println!("  Current entries: {}", snapshot.current_entries);
    println!("  Used blocks: {}", snapshot.used_blocks);
    println!("  Free blocks: {}", snapshot.free_blocks);
    println!("  Total blocks: {}", snapshot.total_blocks());
    println!("  Space utilization: {:.2}%", snapshot.space_utilization());
    println!();

    // Perform health check
    println!("=== Health Check ===\n");
    let is_healthy = cache.health_check().await;
    println!("Cache health status: {}", if is_healthy { "HEALTHY" } else { "UNHEALTHY" });
    println!();

    // Export Prometheus metrics
    println!("=== Prometheus Metrics ===\n");
    let prometheus_output = format_prometheus_metrics(&snapshot);
    println!("{}", prometheus_output);

    // Get detailed cache stats
    println!("=== Detailed Cache Stats ===\n");
    let stats = cache.stats().await;
    println!("Entries: {}", stats.entries);
    println!("Used blocks: {}", stats.used_blocks);
    println!("Free blocks: {}", stats.free_blocks);
    println!("Total blocks: {}", stats.total_blocks);
    println!("Fragmentation ratio: {:.2}%", stats.fragmentation_ratio * 100.0);
    println!();

    if let Some(gc_metrics) = stats.gc_metrics {
        println!("GC Metrics:");
        println!("  Total runs: {}", gc_metrics.total_runs);
        println!("  Total evictions: {}", gc_metrics.total_evicted);
        println!("  Total bytes freed: {}", gc_metrics.total_bytes_freed);
        println!();
    }

    if let Some(compression_stats) = stats.compression_stats {
        println!("Compression Stats:");
        println!("  Compressions: {}", compression_stats.compression_count);
        println!("  Decompressions: {}", compression_stats.decompression_count);
        println!("  Original bytes: {}", compression_stats.total_compressed_bytes);
        println!("  Compressed bytes: {}", compression_stats.total_compressed_size);
        if compression_stats.compression_count > 0 {
            println!("  Compression ratio: {:.2}%", compression_stats.compression_ratio() * 100.0);
        }
        println!();
    }

    println!("Example completed successfully!");

    Ok(())
}
