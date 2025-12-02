//! Example demonstrating compression support in raw disk cache
//!
//! This example shows:
//! - Storing data with automatic compression
//! - Retrieving compressed data (transparent decompression)
//! - Compression statistics and ratios
//! - Different compression algorithms (zstd vs lz4)

use bytes::Bytes;
use pingora_slice::raw_disk::{
    CompressionAlgorithm, CompressionConfig, RawDiskCache,
};
use std::time::Duration;
use tempfile::TempDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== Raw Disk Cache Compression Example ===\n");

    // Create temporary directory for cache
    let temp_dir = TempDir::new()?;
    let cache_path = temp_dir.path().join("cache.dat");

    // Create cache with default compression (zstd, level 3)
    println!("1. Creating cache with default compression (zstd, level 3)");
    let cache = RawDiskCache::new(
        &cache_path,
        100 * 1024 * 1024, // 100MB
        4096,              // 4KB blocks
        Duration::from_secs(3600), // 1 hour TTL
    )
    .await?;

    // Create some compressible test data
    let compressible_data = "This is a test string that should compress very well. "
        .repeat(1000);
    let compressible_bytes = Bytes::from(compressible_data);

    println!("\n2. Storing compressible data ({} bytes)", compressible_bytes.len());
    cache.store("compressible", compressible_bytes.clone()).await?;

    // Create some incompressible test data (random-like)
    let incompressible_data: Vec<u8> = (0..5000)
        .map(|i| ((i * 7 + 13) % 256) as u8)
        .collect();
    let incompressible_bytes = Bytes::from(incompressible_data);

    println!("3. Storing incompressible data ({} bytes)", incompressible_bytes.len());
    cache.store("incompressible", incompressible_bytes.clone()).await?;

    // Create small data (below compression threshold)
    let small_data = Bytes::from("small");
    println!("\n4. Storing small data ({} bytes, below threshold)", small_data.len());
    cache.store("small", small_data.clone()).await?;

    // Retrieve and verify data
    println!("\n5. Retrieving data (transparent decompression)");
    
    let retrieved_compressible = cache.lookup("compressible").await?
        .expect("compressible data should exist");
    assert_eq!(retrieved_compressible, compressible_bytes);
    println!("   ✓ Compressible data retrieved and verified");

    let retrieved_incompressible = cache.lookup("incompressible").await?
        .expect("incompressible data should exist");
    assert_eq!(retrieved_incompressible, incompressible_bytes);
    println!("   ✓ Incompressible data retrieved and verified");

    let retrieved_small = cache.lookup("small").await?
        .expect("small data should exist");
    assert_eq!(retrieved_small, small_data);
    println!("   ✓ Small data retrieved and verified");

    // Display compression statistics
    println!("\n6. Compression Statistics:");
    let stats = cache.compression_stats().await;
    println!("   Total compressed: {} bytes", stats.total_compressed_bytes);
    println!("   Total after compression: {} bytes", stats.total_compressed_size);
    println!("   Compression ratio: {:.2}%", stats.compression_ratio() * 100.0);
    println!("   Space saved: {} bytes ({:.1}%)", 
             stats.space_saved(), 
             stats.space_saved_percent());
    println!("   Compression operations: {}", stats.compression_count);
    println!("   Decompression operations: {}", stats.decompression_count);
    println!("   Skipped (too small): {}", stats.skipped_count);
    println!("   Expansions (didn't help): {}", stats.expansion_count);

    // Display overall cache statistics
    println!("\n7. Overall Cache Statistics:");
    let cache_stats = cache.stats().await;
    println!("   Entries: {}", cache_stats.entries);
    println!("   Used blocks: {}", cache_stats.used_blocks);
    println!("   Free blocks: {}", cache_stats.free_blocks);
    
    if let Some(comp_stats) = cache_stats.compression_stats {
        println!("   Effective compression ratio: {:.2}%", 
                 comp_stats.compression_ratio() * 100.0);
    }

    // Demonstrate different compression algorithms
    println!("\n8. Testing LZ4 compression (faster, lower ratio)");
    
    // Note: In a real application, you would create a new cache with different config
    // For this example, we'll just show the config
    let lz4_config = CompressionConfig {
        algorithm: CompressionAlgorithm::Lz4,
        level: 4,
        min_size: 1024,
        enabled: true,
    };
    println!("   LZ4 config: {:?}", lz4_config);

    println!("\n9. Testing Zstd with higher compression level");
    let zstd_high_config = CompressionConfig {
        algorithm: CompressionAlgorithm::Zstd,
        level: 10, // Higher compression
        min_size: 1024,
        enabled: true,
    };
    println!("   Zstd high config: {:?}", zstd_high_config);

    // Demonstrate disabling compression
    println!("\n10. Compression can be disabled via config");
    let no_compression_config = CompressionConfig {
        algorithm: CompressionAlgorithm::None,
        level: 0,
        min_size: 0,
        enabled: false,
    };
    println!("    No compression config: {:?}", no_compression_config);

    println!("\n=== Example Complete ===");
    println!("\nKey Takeaways:");
    println!("• Compression is transparent - store/lookup work the same");
    println!("• Zstd provides good balance of speed and compression ratio");
    println!("• LZ4 is faster but compresses less");
    println!("• Small data is automatically skipped");
    println!("• Data that doesn't compress well is stored uncompressed");
    println!("• Compression stats help monitor effectiveness");

    Ok(())
}
