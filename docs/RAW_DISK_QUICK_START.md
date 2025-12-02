# Raw Disk Cache Quick Start Guide

## Overview

This guide will help you quickly set up and use the raw disk cache backend for TieredCache.

## Prerequisites

- Pingora Slice installed
- Sufficient disk space for cache (recommended: 10GB+)
- Write permissions to cache directory

## Quick Setup

### Step 1: Create Cache File

```bash
# Create a 10GB file for raw disk cache
sudo fallocate -l 10G /var/cache/pingora-slice-raw

# Set permissions
sudo chown $USER:$USER /var/cache/pingora-slice-raw
```

### Step 2: Configure

Create or update your configuration file (`pingora_slice.yaml`):

```yaml
# L1 (memory) cache size: 100MB
l1_cache_size_bytes: 104857600

# L2 backend type: raw_disk
l2_backend: "raw_disk"

# Raw disk cache configuration
raw_disk_cache:
  device_path: "/var/cache/pingora-slice-raw"
  total_size: 10737418240  # 10GB
  block_size: 4096          # 4KB
  use_direct_io: true
  enable_compression: true
  enable_prefetch: true
  enable_zero_copy: true

# Cache TTL: 1 hour
cache_ttl: 3600
```

### Step 3: Start Service

```bash
# Start the service
pingora-slice --config pingora_slice.yaml

# Or with systemd
sudo systemctl start pingora-slice
```

### Step 4: Verify

```bash
# Check logs
journalctl -u pingora-slice -f

# Look for:
# "Initializing raw disk cache backend"
# "Two-tier cache initialized"
# "L2 (raw_disk): /var/cache/pingora-slice-raw"
```

## Configuration Options

### Minimal Configuration

```yaml
l2_backend: "raw_disk"
raw_disk_cache:
  device_path: "/var/cache/pingora-slice-raw"
  total_size: 10737418240
  block_size: 4096
  use_direct_io: false  # Safe default
```

### Recommended Production Configuration

```yaml
l2_backend: "raw_disk"
raw_disk_cache:
  device_path: "/var/cache/pingora-slice-raw"
  total_size: 107374182400  # 100GB
  block_size: 4096
  use_direct_io: true       # Better performance
  enable_compression: true  # Save space
  enable_prefetch: true     # Better latency
  enable_zero_copy: true    # Reduce CPU
```

### High-Performance Configuration

```yaml
l2_backend: "raw_disk"
raw_disk_cache:
  device_path: "/dev/nvme0n1p1"  # Dedicated SSD
  total_size: 1099511627776      # 1TB
  block_size: 8192               # 8KB for large files
  use_direct_io: true
  enable_compression: false      # Already compressed content
  enable_prefetch: true
  enable_zero_copy: true
```

## Performance Tuning

### Block Size Selection

| Content Type | Recommended Block Size |
|-------------|----------------------|
| Small files (< 100KB) | 2KB - 4KB |
| Medium files (100KB - 10MB) | 4KB - 8KB |
| Large files (> 10MB) | 8KB - 16KB |
| Mixed workload | 4KB (default) |

### O_DIRECT Usage

**Enable O_DIRECT when:**
- Using dedicated cache storage
- Cache device is not shared
- Predictable performance is critical

**Disable O_DIRECT when:**
- Sharing storage with other applications
- Using network-attached storage
- Testing/development

### Compression

**Enable compression for:**
- Text content (HTML, CSS, JS, JSON, XML)
- Uncompressed images (BMP, TIFF)
- Log files

**Disable compression for:**
- Already compressed content (JPEG, PNG, MP4, ZIP)
- Very small files (< 1KB)
- CPU-constrained systems

## Monitoring

### Check Cache Statistics

```bash
# Via metrics endpoint
curl http://localhost:9090/stats

# Look for:
# - l1_entries: Number of entries in L1
# - l1_hits: L1 cache hits
# - l2_hits: L2 cache hits
# - misses: Cache misses
```

### Monitor Raw Disk Cache

```rust
// In your application
let stats = cache.raw_disk_stats().await;
if let Some(stats) = stats {
    println!("Entries: {}", stats.entries);
    println!("Used blocks: {}", stats.used_blocks);
    println!("Free blocks: {}", stats.free_blocks);
    println!("Fragmentation: {:.2}%", stats.fragmentation_ratio * 100.0);
}
```

### Key Metrics

| Metric | Good | Warning | Critical |
|--------|------|---------|----------|
| Hit Rate | > 80% | 50-80% | < 50% |
| Fragmentation | < 20% | 20-40% | > 40% |
| Free Space | > 20% | 10-20% | < 10% |

## Troubleshooting

### Issue: "Failed to create raw disk cache"

**Solution:**
```bash
# Check file exists
ls -lh /var/cache/pingora-slice-raw

# Check permissions
sudo chown $USER:$USER /var/cache/pingora-slice-raw

# Check disk space
df -h /var/cache
```

### Issue: "Metadata too large"

**Solution:**
```yaml
# Increase cache size
raw_disk_cache:
  total_size: 21474836480  # Double to 20GB
```

### Issue: Poor performance

**Solution:**
```yaml
# Enable O_DIRECT
raw_disk_cache:
  use_direct_io: true

# Increase block size for large files
raw_disk_cache:
  block_size: 8192

# Check disk I/O
iostat -x 1
```

### Issue: High fragmentation

**Solution:**
```bash
# Restart service to trigger defragmentation
sudo systemctl restart pingora-slice

# Or manually trigger in code
cache.defragment().await?;
```

## Switching Backends

### From File to Raw Disk

1. **Backup current cache** (optional):
   ```bash
   tar -czf cache-backup.tar.gz /var/cache/pingora-slice
   ```

2. **Create raw disk cache**:
   ```bash
   sudo fallocate -l 10G /var/cache/pingora-slice-raw
   ```

3. **Update configuration**:
   ```yaml
   l2_backend: "raw_disk"
   raw_disk_cache:
     device_path: "/var/cache/pingora-slice-raw"
     total_size: 10737418240
     block_size: 4096
     use_direct_io: true
   ```

4. **Restart service**:
   ```bash
   sudo systemctl restart pingora-slice
   ```

### From Raw Disk to File

1. **Update configuration**:
   ```yaml
   l2_backend: "file"
   l2_cache_dir: "/var/cache/pingora-slice"
   ```

2. **Restart service**:
   ```bash
   sudo systemctl restart pingora-slice
   ```

3. **Remove raw disk cache** (optional):
   ```bash
   rm /var/cache/pingora-slice-raw
   ```

## Best Practices

### 1. Size Planning

```
Total Size = (Average File Size × Expected Entries) × 1.2
```

Example:
- Average file size: 100KB
- Expected entries: 100,000
- Total size: 100KB × 100,000 × 1.2 = 12GB

### 2. Block Size Selection

```
Block Size = Smallest power of 2 ≥ (Average File Size / 4)
```

Example:
- Average file size: 50KB
- Block size: 16KB (next power of 2 after 12.5KB)

### 3. Free Space Reserve

Always reserve 10-20% free space for:
- Garbage collection
- Defragmentation
- Temporary allocations

### 4. Monitoring Schedule

- **Real-time**: Hit rate, latency
- **Hourly**: Fragmentation, free space
- **Daily**: GC runs, defrag runs
- **Weekly**: Capacity planning

## Example Configurations

### Development

```yaml
l2_backend: "raw_disk"
raw_disk_cache:
  device_path: "/tmp/pingora-cache"
  total_size: 1073741824  # 1GB
  block_size: 4096
  use_direct_io: false
  enable_compression: true
  enable_prefetch: false
  enable_zero_copy: false
```

### Production (Small)

```yaml
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

### Production (Large)

```yaml
l2_backend: "raw_disk"
raw_disk_cache:
  device_path: "/mnt/cache/pingora-slice-raw"
  total_size: 107374182400  # 100GB
  block_size: 8192
  use_direct_io: true
  enable_compression: true
  enable_prefetch: true
  enable_zero_copy: true
```

### High-Performance (Dedicated SSD)

```yaml
l2_backend: "raw_disk"
raw_disk_cache:
  device_path: "/dev/nvme0n1"
  total_size: 1099511627776  # 1TB
  block_size: 16384
  use_direct_io: true
  enable_compression: false
  enable_prefetch: true
  enable_zero_copy: true
```

## Next Steps

- Read [TieredCache Documentation](TIERED_CACHE.md) for detailed information
- Review [Raw Disk Cache Design](RAW_DISK_CACHE_DESIGN.md) for architecture details
- Check [Performance Tuning](PERFORMANCE_TUNING.md) for optimization tips
- See [Integration Summary](RAW_DISK_INTEGRATION_SUMMARY.md) for implementation details

## Support

For issues or questions:
1. Check logs: `journalctl -u pingora-slice -f`
2. Review documentation in `docs/` directory
3. Open an issue on GitHub
