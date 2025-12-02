# TieredCache with Raw Disk Cache Integration

## Overview

The TieredCache module now supports two L2 (disk) cache backends:

1. **File-based cache** (default): Traditional filesystem-based caching
2. **Raw disk cache**: Direct block management for maximum performance

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      TieredCache                             │
├─────────────────────────────────────────────────────────────┤
│  L1 Cache (Memory)                                          │
│  - In-memory HashMap                                         │
│  - LRU eviction                                             │
│  - Fast access to hot data                                  │
├─────────────────────────────────────────────────────────────┤
│  L2 Cache (Disk)                                            │
│  ┌──────────────────┐  or  ┌──────────────────────────┐   │
│  │  File Backend    │      │  Raw Disk Backend        │   │
│  │  - Filesystem    │      │  - Direct block mgmt     │   │
│  │  - Simple        │      │  - O_DIRECT support      │   │
│  │  - Portable      │      │  - Compression           │   │
│  │                  │      │  - Prefetching           │   │
│  │                  │      │  - Zero-copy             │   │
│  └──────────────────┘      └──────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

## Configuration

### File-based Backend (Default)

```yaml
# L2 cache backend type
l2_backend: "file"

# L2 cache directory
l2_cache_dir: "/var/cache/pingora-slice"

# Enable L2 cache
enable_l2_cache: true
```

### Raw Disk Backend

```yaml
# L2 cache backend type
l2_backend: "raw_disk"

# Raw disk cache configuration
raw_disk_cache:
  # Path to device/file
  device_path: "/var/cache/pingora-slice-raw"
  
  # Total size (10GB)
  total_size: 10737418240
  
  # Block size (4KB)
  block_size: 4096
  
  # Use O_DIRECT
  use_direct_io: true
  
  # Enable compression
  enable_compression: true
  
  # Enable prefetching
  enable_prefetch: true
  
  # Enable zero-copy
  enable_zero_copy: true
```

## Features Comparison

| Feature | File Backend | Raw Disk Backend |
|---------|-------------|------------------|
| Setup Complexity | Low | Medium |
| Performance | Good | Excellent |
| Compression | No | Yes |
| Prefetching | No | Yes |
| Zero-copy | No | Yes |
| O_DIRECT | No | Yes |
| Fragmentation Control | No | Yes |
| Smart GC | No | Yes |
| Portability | High | Medium |

## Performance Benefits

### Raw Disk Cache Advantages

1. **Direct Block Management**
   - Bypasses filesystem overhead
   - Predictable performance
   - Better control over disk layout

2. **O_DIRECT Support**
   - Bypasses OS page cache
   - Reduces memory pressure
   - More predictable latency

3. **Compression**
   - Transparent compression/decompression
   - Increases effective cache size
   - Configurable algorithms (zstd, lz4)

4. **Prefetching**
   - Pattern detection
   - Automatic prefetch of predicted data
   - Reduces read latency

5. **Zero-copy Operations**
   - mmap for large files
   - sendfile for socket transfers
   - Reduces memory copies

6. **Smart Garbage Collection**
   - Multiple eviction strategies (LRU, LFU, FIFO)
   - Adaptive triggering
   - Incremental GC

7. **Defragmentation**
   - Online defragmentation
   - Maintains high space utilization
   - Background operation

## Usage

### Creating a TieredCache

#### File-based Backend

```rust
use pingora_slice::tiered_cache::TieredCache;
use std::time::Duration;

let cache = TieredCache::new(
    Duration::from_secs(3600),  // TTL
    100 * 1024 * 1024,          // L1 size (100MB)
    "/var/cache/pingora-slice", // L2 directory
)
.await?;
```

#### Raw Disk Backend

```rust
use pingora_slice::tiered_cache::TieredCache;
use std::time::Duration;

let cache = TieredCache::new_with_raw_disk(
    Duration::from_secs(3600),           // TTL
    100 * 1024 * 1024,                   // L1 size (100MB)
    "/var/cache/pingora-slice-raw",      // Device path
    10 * 1024 * 1024 * 1024,            // Total size (10GB)
    4096,                                // Block size (4KB)
    true,                                // Use O_DIRECT
)
.await?;
```

### Operations

All operations work the same regardless of backend:

```rust
// Store data
cache.store(url, &range, data)?;

// Lookup data
if let Some(data) = cache.lookup(url, &range).await? {
    // Use cached data
}

// Purge specific entry
cache.purge(url, &range).await?;

// Purge all entries for a URL
cache.purge_url(url).await?;

// Purge all entries
cache.purge_all().await?;

// Get statistics
let stats = cache.get_stats();
```

### Backend-specific Operations

#### Raw Disk Cache Statistics

```rust
if let Some(raw_stats) = cache.raw_disk_stats().await {
    println!("Entries: {}", raw_stats.entries);
    println!("Used blocks: {}", raw_stats.used_blocks);
    println!("Free blocks: {}", raw_stats.free_blocks);
    println!("Fragmentation: {:.2}%", raw_stats.fragmentation_ratio * 100.0);
    
    if let Some(compression) = raw_stats.compression_stats {
        println!("Compression ratio: {:.2}", compression.compression_ratio());
    }
}
```

## Smooth Backend Switching

### Migration from File to Raw Disk

1. **Prepare raw disk cache device/file**:
   ```bash
   # Create a file for raw disk cache
   fallocate -l 10G /var/cache/pingora-slice-raw
   ```

2. **Update configuration**:
   ```yaml
   l2_backend: "raw_disk"
   raw_disk_cache:
     device_path: "/var/cache/pingora-slice-raw"
     total_size: 10737418240
     block_size: 4096
     use_direct_io: true
   ```

3. **Restart service**:
   ```bash
   systemctl restart pingora-slice
   ```

4. **Monitor performance**:
   ```bash
   # Check cache statistics
   curl http://localhost:9090/stats
   ```

### Migration from Raw Disk to File

1. **Update configuration**:
   ```yaml
   l2_backend: "file"
   l2_cache_dir: "/var/cache/pingora-slice"
   ```

2. **Restart service**:
   ```bash
   systemctl restart pingora-slice
   ```

Note: Cache data is not migrated between backends. The cache will be empty after switching.

## Best Practices

### When to Use File Backend

- Development and testing
- Small cache sizes (< 1GB)
- Shared storage with other applications
- Portability is important
- Simple setup required

### When to Use Raw Disk Backend

- Production deployments
- Large cache sizes (> 10GB)
- Dedicated cache storage
- Maximum performance required
- Advanced features needed (compression, prefetch, etc.)

### Raw Disk Cache Tuning

1. **Block Size**:
   - Match filesystem block size (usually 4KB)
   - Larger blocks (8KB, 16KB) for large files
   - Smaller blocks (2KB) for many small files

2. **O_DIRECT**:
   - Enable for dedicated cache devices
   - Disable if sharing storage with other apps
   - Test performance with your workload

3. **Compression**:
   - Enable for text/JSON/XML content
   - Disable for already compressed content (images, videos)
   - Test compression ratio vs CPU overhead

4. **Total Size**:
   - Leave 10-20% free space for GC
   - Monitor fragmentation ratio
   - Adjust based on cache hit rate

## Monitoring

### Key Metrics

```rust
let stats = cache.get_stats();

// L1 metrics
println!("L1 entries: {}", stats.l1_entries);
println!("L1 bytes: {}", stats.l1_bytes);
println!("L1 hits: {}", stats.l1_hits);

// L2 metrics
println!("L2 hits: {}", stats.l2_hits);
println!("Misses: {}", stats.misses);

// Raw disk specific
if let Some(raw_stats) = cache.raw_disk_stats().await {
    println!("Fragmentation: {:.2}%", raw_stats.fragmentation_ratio * 100.0);
    println!("GC runs: {}", raw_stats.gc_metrics.unwrap().total_runs);
}
```

### Health Checks

```rust
// Check backend type
match cache.l2_backend() {
    L2Backend::File => println!("Using file backend"),
    L2Backend::RawDisk => println!("Using raw disk backend"),
}

// Check cache hit rate
let hit_rate = stats.l1_hits as f64 / (stats.l1_hits + stats.l2_hits + stats.misses) as f64;
println!("Hit rate: {:.2}%", hit_rate * 100.0);
```

## Troubleshooting

### Raw Disk Cache Issues

1. **"Failed to create raw disk cache"**
   - Check device_path exists and is writable
   - Verify total_size doesn't exceed available space
   - Check permissions

2. **"Metadata too large"**
   - Reduce number of cached entries
   - Increase total_size
   - Run GC to free space

3. **High fragmentation**
   - Run defragmentation: `cache.defragment().await?`
   - Adjust GC configuration
   - Consider larger block_size

4. **Poor performance**
   - Enable O_DIRECT if on dedicated device
   - Tune block_size for your workload
   - Check disk I/O statistics
   - Consider enabling compression

## See Also

- [Raw Disk Cache Design](RAW_DISK_CACHE_DESIGN.md)
- [Compression](COMPRESSION.md)
- [Prefetch Optimization](PREFETCH_OPTIMIZATION.md)
- [Zero-copy Implementation](ZERO_COPY_IMPLEMENTATION.md)
- [Smart GC](SMART_GC.md)
- [Defragmentation](DEFRAGMENTATION.md)
