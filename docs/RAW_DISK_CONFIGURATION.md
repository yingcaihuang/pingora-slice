# Raw Disk Cache Configuration Guide

This guide explains how to configure the raw disk cache backend for Pingora Slice.

## Table of Contents

- [Overview](#overview)
- [Configuration Options](#configuration-options)
- [Validation Rules](#validation-rules)
- [Hot Reload](#hot-reload)
- [Examples](#examples)
- [Best Practices](#best-practices)

## Overview

The raw disk cache provides high-performance caching by directly managing disk blocks without relying on the filesystem. This approach offers several advantages:

- **Better Performance**: Bypasses filesystem overhead
- **Predictable Behavior**: Direct control over disk layout
- **Advanced Features**: Compression, prefetching, zero-copy operations
- **Efficient Space Management**: Smart garbage collection and defragmentation

## Configuration Options

### Basic Configuration

To enable raw disk cache, set `l2_backend` to `"raw_disk"` and provide a `raw_disk_cache` configuration:

```yaml
enable_l2_cache: true
l2_backend: "raw_disk"

raw_disk_cache:
  device_path: "/var/cache/pingora-slice-raw"
  total_size: 10737418240  # 10GB
  block_size: 4096
  use_direct_io: true
  enable_compression: true
  enable_prefetch: true
  enable_zero_copy: true
```

### Configuration Parameters

#### `device_path` (required)

Path to the raw disk cache device or file.

- **Type**: String
- **Validation**: Must not be empty
- **Examples**:
  - Block device: `/dev/sdb`
  - Regular file: `/var/cache/pingora-slice-raw`

**Note**: If using a regular file, it will be created and pre-allocated to the specified size.

#### `total_size` (required)

Total size of the raw disk cache in bytes.

- **Type**: u64
- **Validation**: Must be at least 1MB (1,048,576 bytes)
- **Default**: 10GB (10,737,418,240 bytes)
- **Recommended**: 
  - Small deployments: 1-10GB
  - Medium deployments: 10-100GB
  - Large deployments: 100GB+

#### `block_size` (required)

Block size in bytes for disk allocation.

- **Type**: usize
- **Validation**: 
  - Must be a power of 2
  - Must be between 512 bytes and 1MB
  - `total_size` must be at least 10x `block_size`
- **Default**: 4096 (4KB)
- **Recommended**:
  - Small files (< 100KB): 4096 (4KB)
  - Medium files (100KB - 10MB): 8192 (8KB) or 16384 (16KB)
  - Large files (> 10MB): 32768 (32KB) or 65536 (64KB)

#### `use_direct_io` (optional)

Whether to use O_DIRECT for disk I/O.

- **Type**: bool
- **Default**: true
- **Description**: O_DIRECT bypasses the OS page cache, providing more predictable performance
- **Recommended**: true for production environments

#### `enable_compression` (optional)

Whether to enable data compression.

- **Type**: bool
- **Default**: true
- **Description**: Compresses data using zstd before writing to disk
- **Benefits**: Saves disk space, reduces I/O for compressible data
- **Trade-off**: Adds CPU overhead for compression/decompression

#### `enable_prefetch` (optional)

Whether to enable automatic prefetching.

- **Type**: bool
- **Default**: true
- **Description**: Automatically prefetches related data based on access patterns
- **Benefits**: Reduces latency for sequential access patterns
- **Best for**: Video streaming, large file downloads

#### `enable_zero_copy` (optional)

Whether to enable zero-copy operations.

- **Type**: bool
- **Default**: true
- **Description**: Uses mmap and sendfile for efficient data transfer
- **Benefits**: Reduces memory copies, improves performance for large files
- **Requirements**: Linux kernel with sendfile support

## Validation Rules

The configuration system validates all settings before applying them:

### Device Path Validation

```rust
// ✅ Valid
device_path: "/var/cache/pingora-slice-raw"
device_path: "/dev/sdb"

// ❌ Invalid
device_path: ""  // Empty path not allowed
```

### Total Size Validation

```rust
// ✅ Valid
total_size: 1048576        // 1MB (minimum)
total_size: 10737418240    // 10GB
total_size: 107374182400   // 100GB

// ❌ Invalid
total_size: 512000  // Less than 1MB
```

### Block Size Validation

```rust
// ✅ Valid (powers of 2)
block_size: 512     // 512 bytes
block_size: 4096    // 4KB
block_size: 8192    // 8KB
block_size: 1048576 // 1MB

// ❌ Invalid
block_size: 256     // Too small (< 512)
block_size: 3000    // Not a power of 2
block_size: 2097152 // Too large (> 1MB)
```

### Size Relationship Validation

```rust
// ✅ Valid (total_size >= 10 * block_size)
total_size: 10485760  // 10MB
block_size: 1048576   // 1MB (10x)

// ❌ Invalid
total_size: 5242880   // 5MB
block_size: 1048576   // 1MB (only 5x)
```

## Hot Reload

The configuration system supports hot reloading, allowing you to update settings without restarting the server.

### Using Hot Reload

```rust
use pingora_slice::config::SliceConfig;

// Load initial configuration
let mut config = SliceConfig::from_file("config.yaml")?;

// Later, reload from file
let changes = config.reload_from_file("config.yaml")?;

// Check what changed
if changes.has_changes() {
    println!("Configuration updated:");
    for change in changes.summary() {
        println!("  - {}", change);
    }
    
    // Check if cache restart is needed
    if changes.requires_cache_restart() {
        println!("Warning: Cache restart required for changes to take effect");
    }
}
```

### Changes Requiring Cache Restart

Some configuration changes require restarting the cache to take effect:

- `cache_ttl` - Cache time-to-live
- `l1_cache_size_bytes` - L1 cache size
- `l2_cache_dir` - L2 cache directory
- `enable_l2_cache` - L2 cache enabled/disabled
- `l2_backend` - L2 backend type
- `raw_disk_cache` - Any raw disk cache settings

### Changes Applied Immediately

These changes take effect immediately without restart:

- `slice_size` - Slice size for new requests
- `max_concurrent_subrequests` - Concurrency limit
- `max_retries` - Retry limit
- `slice_patterns` - URL patterns
- `upstream_address` - Upstream server address

## Examples

### Minimal Configuration

```yaml
enable_l2_cache: true
l2_backend: "raw_disk"

raw_disk_cache:
  device_path: "/var/cache/pingora-slice-raw"
  total_size: 1073741824  # 1GB
  block_size: 4096
```

### High-Performance Configuration

```yaml
enable_l2_cache: true
l2_backend: "raw_disk"

raw_disk_cache:
  device_path: "/dev/nvme0n1p1"  # Use dedicated NVMe device
  total_size: 107374182400       # 100GB
  block_size: 8192               # 8KB blocks
  use_direct_io: true            # Bypass page cache
  enable_compression: false      # Disable for maximum speed
  enable_prefetch: true          # Enable for sequential access
  enable_zero_copy: true         # Enable for large files
```

### Space-Optimized Configuration

```yaml
enable_l2_cache: true
l2_backend: "raw_disk"

raw_disk_cache:
  device_path: "/var/cache/pingora-slice-raw"
  total_size: 10737418240        # 10GB
  block_size: 4096               # Small blocks for efficiency
  use_direct_io: true
  enable_compression: true       # Enable to save space
  enable_prefetch: false         # Disable to save memory
  enable_zero_copy: false        # Disable to save memory
```

### Development Configuration

```yaml
enable_l2_cache: true
l2_backend: "raw_disk"

raw_disk_cache:
  device_path: "/tmp/pingora-cache"  # Use temp directory
  total_size: 104857600              # 100MB (small for testing)
  block_size: 4096
  use_direct_io: false               # Disable for compatibility
  enable_compression: true
  enable_prefetch: true
  enable_zero_copy: true
```

## Best Practices

### 1. Choose the Right Block Size

- **Small files (< 100KB)**: Use 4KB blocks to minimize waste
- **Medium files (100KB - 10MB)**: Use 8KB or 16KB blocks
- **Large files (> 10MB)**: Use 32KB or 64KB blocks

### 2. Size Your Cache Appropriately

- Calculate based on your working set size
- Rule of thumb: 2-3x your hot data size
- Monitor cache hit rates and adjust

### 3. Use O_DIRECT in Production

- Provides more predictable performance
- Reduces memory pressure on the system
- Requires proper alignment (handled automatically)

### 4. Enable Compression for Compressible Data

- Great for text, JSON, XML, HTML
- Less effective for already compressed data (images, videos)
- Monitor compression ratio to verify effectiveness

### 5. Enable Prefetching for Sequential Access

- Ideal for video streaming
- Beneficial for large file downloads
- May waste resources for random access patterns

### 6. Monitor and Tune

- Use metrics endpoint to monitor cache performance
- Adjust configuration based on observed patterns
- Test changes in staging before production

### 7. Plan for Growth

- Start with conservative sizes
- Monitor disk usage and hit rates
- Scale up as needed based on metrics

### 8. Use Dedicated Storage

- For best performance, use a dedicated disk or partition
- NVMe SSDs provide excellent performance
- Avoid sharing with OS or application storage

## Troubleshooting

### Configuration Validation Errors

```
Error: raw_disk block_size must be a power of 2, got 3000
```

**Solution**: Use a power of 2 (512, 1024, 2048, 4096, 8192, etc.)

```
Error: raw_disk total_size must be at least 10x block_size
```

**Solution**: Increase `total_size` or decrease `block_size`

### Runtime Errors

```
Error: Failed to create raw disk cache: No space available
```

**Solution**: 
- Check disk space on the device
- Reduce `total_size` in configuration
- Clean up old cache files

```
Error: Failed to create raw disk cache: Permission denied
```

**Solution**:
- Ensure the process has write permissions to `device_path`
- If using a block device, may need root privileges
- Check SELinux/AppArmor policies

## See Also

- [Raw Disk Cache Design](RAW_DISK_CACHE_DESIGN.md)
- [Raw Disk Quick Start](RAW_DISK_QUICK_START.md)
- [Raw Disk Migration Guide](RAW_DISK_MIGRATION_GUIDE.md)
- [Configuration Reference](CONFIGURATION.md)
