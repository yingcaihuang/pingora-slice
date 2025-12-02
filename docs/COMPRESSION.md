# Data Compression Support

## Overview

The raw disk cache now supports transparent data compression to improve space utilization. Compression is applied automatically during storage and decompression happens transparently during retrieval.

## Features

- **Transparent Operation**: Compression/decompression is automatic - no changes needed to store/lookup API
- **Multiple Algorithms**: Support for Zstd (balanced) and LZ4 (fast)
- **Smart Compression**: Automatically skips small data and data that doesn't compress well
- **Detailed Statistics**: Track compression ratios, space saved, and operation counts
- **Configurable**: Adjust algorithm, compression level, and size thresholds

## Compression Algorithms

### Zstd (Default)
- **Best for**: General purpose, good balance of speed and compression ratio
- **Compression levels**: 1-22 (default: 3)
- **Typical ratio**: 2-5x for text/JSON data
- **Speed**: Moderate compression, fast decompression

### LZ4
- **Best for**: When speed is critical, real-time applications
- **Compression levels**: 1-12 (default: 4)
- **Typical ratio**: 1.5-3x for text/JSON data
- **Speed**: Very fast compression and decompression

### None
- **Best for**: Pre-compressed data, random data, or when CPU is constrained
- **Disables compression entirely**

## Configuration

```rust
use pingora_slice::raw_disk::{CompressionConfig, CompressionAlgorithm};

// Default configuration (Zstd, level 3, 1KB threshold)
let config = CompressionConfig::default();

// Custom Zstd configuration
let config = CompressionConfig {
    algorithm: CompressionAlgorithm::Zstd,
    level: 5,           // Higher = better compression, slower
    min_size: 2048,     // Only compress data >= 2KB
    enabled: true,
};

// LZ4 for maximum speed
let config = CompressionConfig {
    algorithm: CompressionAlgorithm::Lz4,
    level: 4,
    min_size: 1024,
    enabled: true,
};

// Disable compression
let config = CompressionConfig {
    algorithm: CompressionAlgorithm::None,
    enabled: false,
    ..Default::default()
};
```

## Usage

Compression is completely transparent:

```rust
use bytes::Bytes;
use pingora_slice::raw_disk::RawDiskCache;
use std::time::Duration;

// Create cache (compression enabled by default)
let cache = RawDiskCache::new(
    "cache.dat",
    100 * 1024 * 1024,  // 100MB
    4096,                // 4KB blocks
    Duration::from_secs(3600),
).await?;

// Store data - automatically compressed if beneficial
let data = Bytes::from("large compressible data".repeat(1000));
cache.store("key", data.clone()).await?;

// Retrieve data - automatically decompressed
let retrieved = cache.lookup("key").await?.unwrap();
assert_eq!(retrieved, data);  // Transparent!
```

## Compression Statistics

Monitor compression effectiveness:

```rust
let stats = cache.compression_stats().await;

println!("Compression ratio: {:.2}%", stats.compression_ratio() * 100.0);
println!("Space saved: {} bytes ({:.1}%)", 
         stats.space_saved(), 
         stats.space_saved_percent());
println!("Operations: {} compressed, {} decompressed",
         stats.compression_count,
         stats.decompression_count);
println!("Skipped: {} (too small)", stats.skipped_count);
println!("Expansions: {} (didn't help)", stats.expansion_count);
```

Statistics are also included in overall cache stats:

```rust
let cache_stats = cache.stats().await;
if let Some(comp_stats) = cache_stats.compression_stats {
    println!("Effective compression: {:.2}%", 
             comp_stats.compression_ratio() * 100.0);
}
```

## How It Works

### Storage Flow

1. **Compression Attempt**: Data is compressed using the configured algorithm
2. **Size Check**: If compressed size >= original size, store uncompressed
3. **Threshold Check**: Data below `min_size` is never compressed
4. **Metadata**: Compression flag and original size stored in `DiskLocation`
5. **Disk Write**: Compressed (or original) data written to disk

### Retrieval Flow

1. **Disk Read**: Read data from disk
2. **Checksum Verify**: Verify data integrity (on compressed data)
3. **Decompression**: If compressed flag set, decompress transparently
4. **Return**: Return original data to caller

## Performance Considerations

### When Compression Helps

- **Text data**: JSON, XML, logs, HTML (3-5x compression)
- **Repeated patterns**: Configuration files, templates (5-10x)
- **Structured data**: CSV, TSV (2-4x compression)
- **Large objects**: Better compression ratio on larger data

### When to Skip Compression

- **Already compressed**: Images (JPEG, PNG), videos, archives
- **Random data**: Encrypted data, hashes, random bytes
- **Small objects**: Overhead exceeds benefit (< 1KB)
- **CPU constrained**: When CPU is the bottleneck

### Tuning Guidelines

**For maximum space savings:**
```rust
CompressionConfig {
    algorithm: CompressionAlgorithm::Zstd,
    level: 10,          // Higher compression
    min_size: 512,      // Compress more data
    enabled: true,
}
```

**For maximum speed:**
```rust
CompressionConfig {
    algorithm: CompressionAlgorithm::Lz4,
    level: 1,           // Fastest
    min_size: 4096,     // Only large data
    enabled: true,
}
```

**For balanced performance:**
```rust
CompressionConfig::default()  // Zstd level 3, 1KB threshold
```

## Compression Ratios

Typical compression ratios for different data types:

| Data Type | Zstd (level 3) | LZ4 (level 4) |
|-----------|----------------|---------------|
| JSON API responses | 3-5x | 2-3x |
| HTML pages | 3-4x | 2-3x |
| Log files | 4-6x | 2-4x |
| CSV data | 3-5x | 2-3x |
| Repeated text | 10-20x | 5-10x |
| Random data | 1x (skipped) | 1x (skipped) |
| Images (JPEG) | 1x (skipped) | 1x (skipped) |

## Integration with Other Features

### Zero-Copy Operations

Compression works seamlessly with zero-copy:

```rust
// Data is decompressed even with zero-copy lookup
let data = cache.lookup_zero_copy("key").await?.unwrap();
```

### Prefetching

Prefetched data is stored decompressed in the prefetch cache for fast access.

### Defragmentation

Defragmentation preserves compression state - compressed entries remain compressed after being moved.

### TTL and GC

Expired entry cleanup and garbage collection work normally with compressed data.

## Monitoring

Key metrics to monitor:

1. **Compression Ratio**: Should be < 0.5 for text data
2. **Space Saved**: Total bytes saved by compression
3. **Expansion Count**: High count indicates incompressible data
4. **Skipped Count**: High count may indicate threshold is too high

## Best Practices

1. **Profile Your Data**: Test compression on representative data samples
2. **Monitor Stats**: Track compression effectiveness in production
3. **Adjust Thresholds**: Tune `min_size` based on your data distribution
4. **Choose Algorithm**: Zstd for general use, LZ4 for speed-critical paths
5. **Consider CPU**: Disable compression if CPU is constrained
6. **Test Thoroughly**: Verify compression doesn't impact your latency SLAs

## Example: Compression Analysis

```rust
use pingora_slice::raw_disk::RawDiskCache;
use bytes::Bytes;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cache = RawDiskCache::new(
        "cache.dat",
        100 * 1024 * 1024,
        4096,
        Duration::from_secs(3600),
    ).await?;

    // Store various data types
    let json_data = Bytes::from(r#"{"key":"value"}"#.repeat(1000));
    cache.store("json", json_data).await?;

    let html_data = Bytes::from("<html><body>content</body></html>".repeat(500));
    cache.store("html", html_data).await?;

    // Analyze compression effectiveness
    let stats = cache.compression_stats().await;
    
    println!("Compression Analysis:");
    println!("  Total data: {} bytes", stats.total_compressed_bytes);
    println!("  After compression: {} bytes", stats.total_compressed_size);
    println!("  Ratio: {:.2}%", stats.compression_ratio() * 100.0);
    println!("  Space saved: {} bytes ({:.1}%)",
             stats.space_saved(),
             stats.space_saved_percent());
    
    if stats.compression_ratio() < 0.5 {
        println!("  ✓ Excellent compression!");
    } else if stats.compression_ratio() < 0.7 {
        println!("  ✓ Good compression");
    } else {
        println!("  ⚠ Poor compression - consider disabling");
    }

    Ok(())
}
```

## Troubleshooting

### Poor Compression Ratios

- Check if data is already compressed (images, videos, archives)
- Verify data has patterns/repetition
- Try increasing compression level
- Consider switching to Zstd if using LZ4

### High CPU Usage

- Lower compression level (e.g., Zstd level 1-3)
- Switch to LZ4 for faster compression
- Increase `min_size` threshold
- Disable compression for hot paths

### Expansion Count High

- Normal for random/encrypted data
- System correctly stores uncompressed
- Consider increasing `min_size` to skip more data

## Future Enhancements

Potential future improvements:

- Per-key compression hints
- Adaptive compression level based on CPU load
- Additional algorithms (Brotli, Snappy)
- Compression dictionary support for similar data
- Parallel compression for large objects
