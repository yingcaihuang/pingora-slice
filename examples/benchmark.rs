//! Benchmark tool for measuring Pingora Slice performance
//!
//! This tool measures various performance metrics including:
//! - Request latency
//! - Throughput
//! - Memory usage
//! - Cache hit rates

use pingora_slice::{SliceCache, SliceConfig};
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() {
    println!("=== Pingora Slice Performance Benchmark ===\n");

    // Run benchmarks
    benchmark_cache_operations().await;
    benchmark_config_validation();
    benchmark_memory_usage().await;

    println!("\n=== Benchmark Complete ===");
}

/// Benchmark cache operations
async fn benchmark_cache_operations() {
    println!("--- Cache Operations Benchmark ---");

    let cache = SliceCache::new(Duration::from_secs(3600));
    let iterations = 10000;

    // Benchmark cache writes
    let start = Instant::now();
    for i in 0..iterations {
        let range = pingora_slice::ByteRange::new(i * 1024, (i + 1) * 1024 - 1).unwrap();
        let data = bytes::Bytes::from(vec![0u8; 1024]);
        let _ = cache.store_slice("http://example.com/file.bin", &range, data).await;
    }
    let write_duration = start.elapsed();
    let write_ops_per_sec = iterations as f64 / write_duration.as_secs_f64();

    println!("  Cache Writes:");
    println!("    Total: {} operations", iterations);
    println!("    Duration: {:?}", write_duration);
    println!("    Throughput: {:.2} ops/sec", write_ops_per_sec);

    // Benchmark cache reads (hits)
    let start = Instant::now();
    for i in 0..iterations {
        let range = pingora_slice::ByteRange::new(i * 1024, (i + 1) * 1024 - 1).unwrap();
        let _ = cache.lookup_slice("http://example.com/file.bin", &range).await;
    }
    let read_duration = start.elapsed();
    let read_ops_per_sec = iterations as f64 / read_duration.as_secs_f64();

    println!("  Cache Reads (hits):");
    println!("    Total: {} operations", iterations);
    println!("    Duration: {:?}", read_duration);
    println!("    Throughput: {:.2} ops/sec", read_ops_per_sec);

    // Benchmark cache reads (misses)
    let start = Instant::now();
    for i in 0..iterations {
        let range = pingora_slice::ByteRange::new((i + iterations) * 1024, (i + iterations + 1) * 1024 - 1).unwrap();
        let _ = cache.lookup_slice("http://example.com/file.bin", &range).await;
    }
    let miss_duration = start.elapsed();
    let miss_ops_per_sec = iterations as f64 / miss_duration.as_secs_f64();

    println!("  Cache Reads (misses):");
    println!("    Total: {} operations", iterations);
    println!("    Duration: {:?}", miss_duration);
    println!("    Throughput: {:.2} ops/sec", miss_ops_per_sec);

    // Get cache stats
    let stats = cache.get_stats();
    println!("  Cache Stats:");
    println!("    Total entries: {}", stats.total_entries);
    println!("    Total bytes: {} ({:.2} MB)", stats.total_bytes, stats.total_bytes as f64 / 1024.0 / 1024.0);
    println!("    Hits: {}", stats.hits);
    println!("    Misses: {}", stats.misses);
    println!("    Hit rate: {:.2}%", stats.hits as f64 / (stats.hits + stats.misses) as f64 * 100.0);

    println!();
}

/// Benchmark configuration validation
fn benchmark_config_validation() {
    println!("--- Configuration Validation Benchmark ---");

    let iterations = 100000;

    // Benchmark valid config validation
    let config = SliceConfig::default();
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = config.validate();
    }
    let duration = start.elapsed();
    let ops_per_sec = iterations as f64 / duration.as_secs_f64();

    println!("  Config Validation:");
    println!("    Total: {} operations", iterations);
    println!("    Duration: {:?}", duration);
    println!("    Throughput: {:.2} ops/sec", ops_per_sec);
    println!("    Average latency: {:.2} µs", duration.as_micros() as f64 / iterations as f64);

    println!();
}

/// Benchmark memory usage with different cache sizes
async fn benchmark_memory_usage() {
    println!("--- Memory Usage Benchmark ---");

    let test_cases = vec![
        ("No size limit", None),
        ("10MB limit", Some(10 * 1024 * 1024)),
        ("100MB limit", Some(100 * 1024 * 1024)),
    ];

    for (name, max_size) in test_cases {
        println!("  Test case: {}", name);

        let cache = if let Some(size) = max_size {
            SliceCache::with_max_size(Duration::from_secs(3600), size)
        } else {
            SliceCache::new(Duration::from_secs(3600))
        };

        // Store 1000 slices of 1MB each
        let slice_count = 1000;
        let slice_size = 1024 * 1024; // 1MB

        let start = Instant::now();
        for i in 0..slice_count {
            let range = pingora_slice::ByteRange::new(
                i * slice_size,
                (i + 1) * slice_size - 1
            ).unwrap();
            let data = bytes::Bytes::from(vec![0u8; slice_size as usize]);
            let _ = cache.store_slice("http://example.com/large.bin", &range, data).await;
        }
        let duration = start.elapsed();

        let stats = cache.get_stats();
        println!("    Duration: {:?}", duration);
        println!("    Entries stored: {}", stats.total_entries);
        println!("    Total bytes: {} ({:.2} MB)", stats.total_bytes, stats.total_bytes as f64 / 1024.0 / 1024.0);
        
        if let Some(limit) = max_size {
            let usage_percent = stats.total_bytes as f64 / limit as f64 * 100.0;
            println!("    Memory usage: {:.2}% of limit", usage_percent);
        }

        println!();
    }
}
