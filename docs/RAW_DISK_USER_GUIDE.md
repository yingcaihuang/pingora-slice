# Raw Disk Cache User Guide

## Table of Contents

1. [Introduction](#introduction)
2. [Getting Started](#getting-started)
3. [Configuration Guide](#configuration-guide)
4. [Performance Tuning](#performance-tuning)
5. [Troubleshooting](#troubleshooting)
6. [Best Practices](#best-practices)
7. [Advanced Topics](#advanced-topics)
8. [FAQ](#faq)

## Introduction

### What is Raw Disk Cache?

The Raw Disk Cache is a high-performance caching backend that directly manages disk blocks without relying on the filesystem. This approach provides:

- **Better Performance**: 4-5x improvement for small files, 1.5-2x for large files
- **Predictable Behavior**: Direct control over disk layout and I/O patterns
- **Advanced Features**: Compression, prefetching, zero-copy operations, io_uring support
- **Efficient Space Management**: Smart garbage collection and defragmentation
- **Higher Space Utilization**: 95%+ vs 70-80% with filesystem-based caching

### When to Use Raw Disk Cache

**Ideal for:**
- High-concurrency scenarios (>100 concurrent requests)
- Large numbers of small files (<1MB)
- Performance-critical applications
- Predictable latency requirements
- Dedicated cache storage devices

**Consider alternatives when:**
- Using shared storage with other applications
- Need easy cache inspection with standard tools
- Running in development/testing environments
- Limited operational expertise

### Architecture Overview

```
┌─────────────────────────────────────────┐
│     L1 Cache (Memory - LRU)             │
│     Fast access, limited size           │
└──────────────┬──────────────────────────┘
               │ Cache miss
               ↓
┌─────────────────────────────────────────┐
│     L2 Cache (Raw Disk)                 │
│  ┌────────────────────────────────────┐ │
│  │  Metadata (In-Memory Index)        │ │
│  │  - Hash table: Key → Location      │ │
│  │  - Free space bitmap                 │ │
│  │  - LRU eviction list               │ │
│  └────────────────────────────────────┘ │
│  ┌────────────────────────────────────┐ │
│  │  Data Blocks (On-Disk Storage)     │ │
│  │  - Direct block management         │ │
│  │  - Optional compression            │ │
│  │  - Checksum verification           │ │
│  └────────────────────────────────────┘ │
└──────────────┬──────────────────────────┘
               │
               ↓
        ┌──────────────┐
        │  Raw Device  │
        │  or File     │
        └──────────────┘
```

## Getting Started

### Prerequisites

- Pingora Slice installed
- Sufficient disk space (recommended: 10GB+ for production)
- Write permissions to cache location
- Linux kernel 5.1+ (for io_uring support, optional)

### Quick Setup (5 Minutes)

#### Step 1: Create Cache Storage

```bash
# Create a 10GB cache file
sudo fallocate -l 10G /var/cache/pingora-slice-raw

# Set ownership and permissions
sudo chown $USER:$USER /var/cache/pingora-slice-raw
sudo chmod 600 /var/cache/pingora-slice-raw

# Verify creation
ls -lh /var/cache/pingora-slice-raw
```

**Alternative: Use a dedicated block device**
```bash
# For dedicated SSD/NVMe partition
sudo chown $USER:$USER /dev/nvme0n1p1
# Note: Ensure partition is not mounted
```

#### Step 2: Configure Pingora Slice

Create or update `pingora_slice.yaml`:

```yaml
# L1 (memory) cache: 100MB
l1_cache_size_bytes: 104857600

# Enable L2 cache with raw disk backend
enable_l2_cache: true
l2_backend: "raw_disk"

# Raw disk cache configuration
raw_disk_cache:
  device_path: "/var/cache/pingora-slice-raw"
  total_size: 10737418240  # 10GB
  block_size: 4096          # 4KB blocks
  use_direct_io: true       # Bypass OS cache
  enable_compression: true  # Save space
  enable_prefetch: true     # Reduce latency
  enable_zero_copy: true    # Reduce CPU usage

# Cache TTL: 1 hour
cache_ttl: 3600
```

#### Step 3: Start the Service

```bash
# Start Pingora Slice
pingora-slice --config pingora_slice.yaml

# Or with systemd
sudo systemctl start pingora-slice
```

#### Step 4: Verify Operation

```bash
# Check logs for successful initialization
journalctl -u pingora-slice -f

# Look for these messages:
# ✓ "Initializing raw disk cache backend"
# ✓ "Two-tier cache initialized"
# ✓ "L2 (raw_disk): /var/cache/pingora-slice-raw"

# Check cache statistics
curl http://localhost:9090/stats | jq '.raw_disk_stats'
```

### First Test

```bash
# Make a request to cache content
curl http://localhost:8080/test-file.bin

# Check cache hit (second request should be faster)
time curl http://localhost:8080/test-file.bin

# Verify cache statistics
curl http://localhost:9090/stats | jq '{
  l1_hits, l2_hits, misses,
  hit_rate: ((.l1_hits + .l2_hits) / (.l1_hits + .l2_hits + .misses))
}'
```

## Configuration Guide

### Essential Parameters

#### `device_path` (Required)

Path to the raw disk cache device or file.

**Valid values:**
- Regular file: `/var/cache/pingora-slice-raw`
- Block device: `/dev/sdb`, `/dev/nvme0n1p1`

**Validation:**
- Must not be empty
- Must have write permissions
- Parent directory must exist (for files)

**Examples:**
```yaml
# Development: Use temp file
device_path: "/tmp/pingora-cache"

# Production: Use dedicated file
device_path: "/var/cache/pingora-slice-raw"

# High-performance: Use dedicated NVMe
device_path: "/dev/nvme0n1"
```

#### `total_size` (Required)

Total size of the cache in bytes.

**Validation:**
- Minimum: 1MB (1,048,576 bytes)
- Must be at least 10x `block_size`

**Sizing guidelines:**
```
Total Size = (Average File Size × Expected Entries × 1.2)
```

**Examples:**
```yaml
# Small deployment (1GB)
total_size: 1073741824

# Medium deployment (10GB)
total_size: 10737418240

# Large deployment (100GB)
total_size: 107374182400

# Enterprise (1TB)
total_size: 1099511627776
```

#### `block_size` (Required)

Block size for disk allocation in bytes.

**Validation:**
- Must be a power of 2
- Range: 512 bytes to 1MB
- `total_size` must be ≥ 10x `block_size`

**Selection guide:**

| Workload | Average File Size | Recommended Block Size |
|----------|------------------|----------------------|
| Small files | < 100KB | 4096 (4KB) |
| Medium files | 100KB - 10MB | 8192 (8KB) |
| Large files | > 10MB | 16384 (16KB) |
| Mixed | Varies | 4096 (4KB) |

**Formula:**
```
Block Size = Smallest power of 2 ≥ (Average File Size / 4)
```

**Examples:**
```yaml
# Small files (images, API responses)
block_size: 4096

# Medium files (documents, videos)
block_size: 8192

# Large files (downloads, archives)
block_size: 16384
```

### Performance Parameters

#### `use_direct_io` (Optional, default: true)

Enable O_DIRECT to bypass OS page cache.

**Advantages:**
- More predictable performance
- Reduces memory pressure
- Better for dedicated cache storage

**Disadvantages:**
- Requires proper alignment (handled automatically)
- May be slower on some systems
- Not beneficial for shared storage

**When to enable:**
- ✅ Dedicated cache storage
- ✅ Production environments
- ✅ Predictable performance needed

**When to disable:**
- ❌ Shared storage with other apps
- ❌ Network-attached storage
- ❌ Development/testing

```yaml
# Production (recommended)
use_direct_io: true

# Development/testing
use_direct_io: false
```

#### `enable_compression` (Optional, default: true)

Enable transparent data compression using zstd.

**Benefits:**
- Saves disk space (30-70% for text)
- Reduces I/O for compressible data
- Automatic compression/decompression

**Trade-offs:**
- Adds CPU overhead
- Less effective for already compressed data

**When to enable:**
- ✅ Text content (HTML, CSS, JS, JSON, XML)
- ✅ Uncompressed images (BMP, TIFF)
- ✅ Log files
- ✅ Disk space is limited

**When to disable:**
- ❌ Already compressed content (JPEG, PNG, MP4, ZIP)
- ❌ Very small files (< 1KB)
- ❌ CPU-constrained systems
- ❌ Maximum speed required

```yaml
# Text-heavy workload
enable_compression: true

# Media-heavy workload
enable_compression: false
```

#### `enable_prefetch` (Optional, default: true)

Enable automatic prefetching based on access patterns.

**Benefits:**
- Reduces latency for sequential access
- Ideal for video streaming
- Improves large file downloads

**Trade-offs:**
- May waste resources for random access
- Increases memory usage

**When to enable:**
- ✅ Video streaming
- ✅ Large file downloads
- ✅ Sequential access patterns

**When to disable:**
- ❌ Random access patterns
- ❌ Memory-constrained systems
- ❌ Small files only

```yaml
# Video streaming
enable_prefetch: true

# API responses (random access)
enable_prefetch: false
```

#### `enable_zero_copy` (Optional, default: true)

Enable zero-copy operations using mmap and sendfile.

**Benefits:**
- Reduces memory copies
- Improves performance for large files
- Lower CPU usage

**Requirements:**
- Linux kernel with sendfile support
- Works best with large files (>1MB)

```yaml
# Production (recommended)
enable_zero_copy: true

# Compatibility mode
enable_zero_copy: false
```

### Configuration Examples

#### Development Environment

```yaml
l2_backend: "raw_disk"
raw_disk_cache:
  device_path: "/tmp/pingora-cache"
  total_size: 1073741824      # 1GB
  block_size: 4096
  use_direct_io: false        # Better compatibility
  enable_compression: true
  enable_prefetch: false      # Save memory
  enable_zero_copy: false     # Save memory
```

#### Small Production Deployment

```yaml
l2_backend: "raw_disk"
raw_disk_cache:
  device_path: "/var/cache/pingora-slice-raw"
  total_size: 10737418240     # 10GB
  block_size: 4096
  use_direct_io: true
  enable_compression: true
  enable_prefetch: true
  enable_zero_copy: true
```

#### Large Production Deployment

```yaml
l2_backend: "raw_disk"
raw_disk_cache:
  device_path: "/mnt/cache/pingora-slice-raw"
  total_size: 107374182400    # 100GB
  block_size: 8192            # Larger blocks for efficiency
  use_direct_io: true
  enable_compression: true
  enable_prefetch: true
  enable_zero_copy: true
```

#### High-Performance (Dedicated NVMe)

```yaml
l2_backend: "raw_disk"
raw_disk_cache:
  device_path: "/dev/nvme0n1"
  total_size: 1099511627776   # 1TB
  block_size: 16384           # Large blocks
  use_direct_io: true
  enable_compression: false   # Max speed
  enable_prefetch: true
  enable_zero_copy: true
```

#### Space-Optimized

```yaml
l2_backend: "raw_disk"
raw_disk_cache:
  device_path: "/var/cache/pingora-slice-raw"
  total_size: 10737418240     # 10GB
  block_size: 4096            # Small blocks
  use_direct_io: true
  enable_compression: true    # Save space
  enable_prefetch: false      # Save memory
  enable_zero_copy: false     # Save memory
```

## Performance Tuning

### Understanding Performance Metrics

Monitor these key metrics:

```bash
# Get cache statistics
curl http://localhost:9090/stats | jq '{
  # Hit rate (target: >80%)
  hit_rate: ((.l1_hits + .l2_hits) / (.l1_hits + .l2_hits + .misses)),
  
  # Cache utilization
  l1_entries: .l1_entries,
  l2_entries: .raw_disk_stats.entries,
  
  # Space usage
  used_blocks: .raw_disk_stats.used_blocks,
  free_blocks: .raw_disk_stats.free_blocks,
  
  # Fragmentation (target: <20%)
  fragmentation: .raw_disk_stats.fragmentation_ratio
}'
```

### Performance Targets

| Metric | Good | Warning | Critical |
|--------|------|---------|----------|
| Hit Rate | >80% | 50-80% | <50% |
| Fragmentation | <20% | 20-40% | >40% |
| Free Space | >20% | 10-20% | <10% |
| P95 Latency | <10ms | 10-50ms | >50ms |

### Tuning for Different Workloads

#### Small Files (<100KB)

**Characteristics:**
- Many small objects
- High request rate
- Random access

**Recommended settings:**
```yaml
raw_disk_cache:
  block_size: 4096            # Small blocks minimize waste
  use_direct_io: true
  enable_compression: true    # Good compression ratio
  enable_prefetch: false      # Random access
  enable_zero_copy: false     # Not beneficial for small files
```

**Expected performance:**
- 20K+ ops/sec (reads)
- 5K+ ops/sec (writes)
- <2ms P95 latency

#### Large Files (>10MB)

**Characteristics:**
- Fewer large objects
- Sequential access
- High bandwidth

**Recommended settings:**
```yaml
raw_disk_cache:
  block_size: 16384           # Larger blocks for efficiency
  use_direct_io: true
  enable_compression: false   # Already compressed
  enable_prefetch: true       # Sequential access
  enable_zero_copy: true      # Significant benefit
```

**Expected performance:**
- 1.2GB/s+ throughput
- 100+ concurrent streams
- <10ms P95 latency

#### Mixed Workload

**Characteristics:**
- Variable file sizes
- Mixed access patterns
- Balanced requirements

**Recommended settings:**
```yaml
raw_disk_cache:
  block_size: 4096            # Balanced
  use_direct_io: true
  enable_compression: true    # Helps with text
  enable_prefetch: true       # Helps with sequential
  enable_zero_copy: true      # Helps with large files
```

### Hardware-Specific Tuning

#### NVMe SSDs

**Characteristics:**
- Very high IOPS (>100K)
- Low latency (<100µs)
- High bandwidth (>2GB/s)

**Recommended settings:**
```yaml
raw_disk_cache:
  device_path: "/dev/nvme0n1"
  block_size: 16384           # Leverage high bandwidth
  use_direct_io: true         # Bypass cache
  enable_compression: false   # CPU becomes bottleneck
  enable_prefetch: true
  enable_zero_copy: true
```

**Additional optimizations:**
- Use io_uring for maximum performance
- Consider larger block sizes (32KB-64KB)
- Monitor CPU usage (may become bottleneck)

#### SATA SSDs

**Characteristics:**
- Moderate IOPS (10K-50K)
- Moderate latency (100-500µs)
- Moderate bandwidth (500MB/s)

**Recommended settings:**
```yaml
raw_disk_cache:
  block_size: 8192            # Balanced
  use_direct_io: true
  enable_compression: true    # Save I/O
  enable_prefetch: true
  enable_zero_copy: true
```

#### HDDs (Not Recommended)

**Note:** Raw disk cache provides minimal benefit for HDDs. Consider using file-based cache instead.

If you must use HDDs:
```yaml
raw_disk_cache:
  block_size: 4096
  use_direct_io: false        # Page cache helps
  enable_compression: true    # Reduce I/O
  enable_prefetch: true       # Critical for sequential
  enable_zero_copy: false
```

### Monitoring and Optimization

#### Real-Time Monitoring

```bash
# Watch cache statistics
watch -n 5 'curl -s http://localhost:9090/stats | jq "{
  hit_rate: ((.l1_hits + .l2_hits) / (.l1_hits + .l2_hits + .misses)),
  entries: .raw_disk_stats.entries,
  fragmentation: .raw_disk_stats.fragmentation_ratio,
  free_pct: (.raw_disk_stats.free_blocks / (.raw_disk_stats.used_blocks + .raw_disk_stats.free_blocks))
}"'
```

#### Disk I/O Monitoring

```bash
# Monitor disk I/O
iostat -x 1 /dev/nvme0n1

# Key metrics:
# - %util: Should be <80% for good performance
# - await: Average wait time (target: <10ms)
# - r/s, w/s: Read/write operations per second
```

#### System Resource Monitoring

```bash
# CPU usage
top -p $(pgrep pingora-slice)

# Memory usage
ps aux | grep pingora-slice

# Network bandwidth
iftop -i eth0
```

### Performance Optimization Checklist

- [ ] Measured baseline performance
- [ ] Selected appropriate block size for workload
- [ ] Enabled O_DIRECT for dedicated storage
- [ ] Configured compression based on content type
- [ ] Enabled prefetch for sequential access
- [ ] Enabled zero-copy for large files
- [ ] Monitored fragmentation (trigger defrag if >40%)
- [ ] Verified hit rate (target >80%)
- [ ] Checked disk I/O utilization (<80%)
- [ ] Monitored memory usage (stable over time)

## Troubleshooting

### Common Issues and Solutions

#### Issue: Service Won't Start

**Symptoms:**
```
Failed to start pingora-slice.service
Error: Failed to create raw disk cache
```

**Diagnostic steps:**
```bash
# 1. Check logs
sudo journalctl -u pingora-slice -n 50 --no-pager

# 2. Verify file exists
ls -lh /var/cache/pingora-slice-raw

# 3. Check permissions
stat /var/cache/pingora-slice-raw

# 4. Verify disk space
df -h /var/cache

# 5. Test configuration
pingora-slice --config /etc/pingora-slice/config.yaml --validate
```

**Solutions:**

**Permission denied:**
```bash
sudo chown pingora:pingora /var/cache/pingora-slice-raw
sudo chmod 600 /var/cache/pingora-slice-raw
```

**File doesn't exist:**
```bash
sudo fallocate -l 10G /var/cache/pingora-slice-raw
sudo chown pingora:pingora /var/cache/pingora-slice-raw
```

**Insufficient disk space:**
```bash
# Check available space
df -h /var/cache

# Reduce cache size in config or free up space
```

**Invalid configuration:**
```bash
# Check YAML syntax
yamllint /etc/pingora-slice/config.yaml

# Verify block_size is power of 2
# Verify total_size >= 10 * block_size
```

#### Issue: Poor Performance

**Symptoms:**
- High latency (>50ms P95)
- Low throughput
- High CPU usage

**Diagnostic steps:**
```bash
# 1. Check cache hit rate
curl http://localhost:9090/stats | jq '{
  hit_rate: ((.l1_hits + .l2_hits) / (.l1_hits + .l2_hits + .misses))
}'

# 2. Check fragmentation
curl http://localhost:9090/stats | jq '.raw_disk_stats.fragmentation_ratio'

# 3. Monitor disk I/O
iostat -x 1

# 4. Check CPU usage
top -p $(pgrep pingora-slice)

# 5. Check memory usage
free -h
```

**Solutions:**

**Low hit rate (<50%):**
```yaml
# Increase cache size
raw_disk_cache:
  total_size: 21474836480  # Double size

# Increase cache TTL
cache_ttl: 7200  # 2 hours
```

**High fragmentation (>40%):**
```bash
# Restart service to trigger defragmentation
sudo systemctl restart pingora-slice

# Or manually trigger (if API available)
curl -X POST http://localhost:9090/admin/defragment
```

**High disk I/O wait:**
```yaml
# Enable O_DIRECT
raw_disk_cache:
  use_direct_io: true

# Increase block size
raw_disk_cache:
  block_size: 8192

# Consider faster storage (NVMe)
```

**High CPU usage:**
```yaml
# Disable compression
raw_disk_cache:
  enable_compression: false

# Reduce concurrent operations
max_concurrent_subrequests: 4
```

#### Issue: High Memory Usage

**Symptoms:**
- Memory grows unbounded
- OOM errors
- System swapping

**Diagnostic steps:**
```bash
# Check memory usage
ps aux | grep pingora-slice

# Check L1 cache size
curl http://localhost:9090/stats | jq '.l1_bytes'

# Monitor over time
watch -n 5 'ps aux | grep pingora-slice | awk "{print \$6}"'
```

**Solutions:**

**Reduce L1 cache size:**
```yaml
# Reduce from 100MB to 50MB
l1_cache_size_bytes: 52428800
```

**Disable memory-intensive features:**
```yaml
raw_disk_cache:
  enable_prefetch: false      # Saves memory
  enable_zero_copy: false     # Saves memory
```

**Reduce cache TTL:**
```yaml
# Shorter TTL = fewer cached items
cache_ttl: 1800  # 30 minutes
```

#### Issue: Cache Not Persisting

**Symptoms:**
- Empty cache after restart
- L2 hits always 0
- Metadata not saved

**Diagnostic steps:**
```bash
# Check logs for metadata save
sudo journalctl -u pingora-slice | grep -i metadata

# Verify file permissions
ls -lh /var/cache/pingora-slice-raw

# Check disk space
df -h /var/cache
```

**Solutions:**

**Metadata region too small:**
```yaml
# Increase total size
raw_disk_cache:
  total_size: 21474836480  # Double size
```

**Permission issues:**
```bash
sudo chown pingora:pingora /var/cache/pingora-slice-raw
sudo chmod 600 /var/cache/pingora-slice-raw
```

**Disk full:**
```bash
# Free up space or increase disk
df -h /var/cache
```

#### Issue: Data Corruption

**Symptoms:**
- Checksum verification failures
- Corrupted responses
- Cache lookup errors

**Diagnostic steps:**
```bash
# Check logs for checksum errors
sudo journalctl -u pingora-slice | grep -i checksum

# Verify disk health
sudo smartctl -a /dev/nvme0n1

# Check for disk errors
dmesg | grep -i error
```

**Solutions:**

**Immediate action:**
```bash
# Stop service
sudo systemctl stop pingora-slice

# Backup cache file
sudo cp /var/cache/pingora-slice-raw /backup/

# Clear cache and restart
sudo rm /var/cache/pingora-slice-raw
sudo fallocate -l 10G /var/cache/pingora-slice-raw
sudo chown pingora:pingora /var/cache/pingora-slice-raw
sudo systemctl start pingora-slice
```

**Prevention:**
```yaml
# Enable enhanced checksums
raw_disk_cache:
  enable_enhanced_checksum: true  # If available

# Use ECC memory
# Use enterprise-grade storage
# Regular disk health monitoring
```

### Diagnostic Commands Reference

```bash
# Service status
sudo systemctl status pingora-slice

# View logs (last 100 lines)
sudo journalctl -u pingora-slice -n 100 --no-pager

# Follow logs in real-time
sudo journalctl -u pingora-slice -f

# Cache statistics
curl http://localhost:9090/stats | jq .

# Raw disk specific stats
curl http://localhost:9090/stats | jq '.raw_disk_stats'

# Disk I/O statistics
iostat -x 1 5

# Disk usage
df -h /var/cache

# File details
ls -lh /var/cache/pingora-slice-raw
stat /var/cache/pingora-slice-raw

# Process information
ps aux | grep pingora-slice
top -p $(pgrep pingora-slice)

# Network statistics
netstat -an | grep :8080
ss -s

# Disk health (NVMe)
sudo nvme smart-log /dev/nvme0n1

# Disk health (SATA)
sudo smartctl -a /dev/sda
```

## Best Practices

### Capacity Planning

#### Sizing Formula

```
Total Size = (Average File Size × Expected Entries × 1.2) + Metadata Overhead

Where:
- Average File Size: Typical size of cached objects
- Expected Entries: Number of objects to cache
- 1.2: 20% buffer for fragmentation and growth
- Metadata Overhead: ~1% of total size
```

**Example calculation:**
```
Average File Size: 100KB
Expected Entries: 100,000
Buffer: 20%

Total Size = (100KB × 100,000 × 1.2) + 1%
          = 12GB + 120MB
          = ~12.2GB
```

#### Block Size Selection

```
Block Size = Smallest power of 2 ≥ (Average File Size / 4)

But constrained to: 512 bytes ≤ Block Size ≤ 1MB
```

**Example calculations:**

| Average File Size | Calculated | Recommended |
|------------------|------------|-------------|
| 10KB | 2.5KB → 4KB | 4KB |
| 50KB | 12.5KB → 16KB | 8KB or 16KB |
| 500KB | 125KB → 128KB | 16KB or 32KB |
| 5MB | 1.25MB → capped | 64KB |

#### Free Space Reserve

Always maintain 10-20% free space:

```yaml
# For 10GB cache, plan for 8-9GB actual usage
raw_disk_cache:
  total_size: 10737418240  # 10GB
  # Expect ~8-9GB usable after accounting for:
  # - Metadata overhead (~1%)
  # - Fragmentation (~5-10%)
  # - GC headroom (~10%)
```

### Operational Best Practices

#### 1. Monitoring

**Essential metrics to monitor:**
- Hit rate (target: >80%)
- Fragmentation ratio (target: <20%)
- Free space percentage (target: >20%)
- Disk I/O utilization (target: <80%)
- Request latency P95 (target: <10ms)

**Set up alerts:**
```yaml
# Example Prometheus alerts
- alert: LowCacheHitRate
  expr: cache_hit_rate < 0.5
  for: 10m

- alert: HighFragmentation
  expr: cache_fragmentation_ratio > 0.4
  for: 30m

- alert: LowFreeSpace
  expr: cache_free_space_ratio < 0.1
  for: 5m
```

#### 2. Regular Maintenance

**Daily:**
- Monitor hit rate and latency
- Check for errors in logs
- Verify disk health

**Weekly:**
- Review fragmentation ratio
- Check capacity trends
- Analyze access patterns

**Monthly:**
- Review and adjust configuration
- Capacity planning review
- Performance benchmarking

#### 3. Backup and Recovery

**Backup strategy:**
```bash
# Cache is ephemeral - no backup needed for data
# But backup configuration:
sudo cp /etc/pingora-slice/config.yaml \
  /backup/config.yaml.$(date +%Y%m%d)

# Document cache size and settings
# for disaster recovery
```

**Recovery procedure:**
```bash
# 1. Recreate cache file
sudo fallocate -l 10G /var/cache/pingora-slice-raw
sudo chown pingora:pingora /var/cache/pingora-slice-raw

# 2. Restore configuration
sudo cp /backup/config.yaml /etc/pingora-slice/config.yaml

# 3. Restart service
sudo systemctl start pingora-slice

# 4. Monitor warmup (1-4 hours typical)
watch -n 60 'curl -s http://localhost:9090/stats | jq .raw_disk_stats.entries'
```

#### 4. Security

**File permissions:**
```bash
# Restrict access to cache file
sudo chmod 600 /var/cache/pingora-slice-raw
sudo chown pingora:pingora /var/cache/pingora-slice-raw

# Restrict configuration
sudo chmod 640 /etc/pingora-slice/config.yaml
sudo chown root:pingora /etc/pingora-slice/config.yaml
```

**Network security:**
```yaml
# Bind metrics endpoint to localhost only
metrics_endpoint:
  address: "127.0.0.1:9090"

# Use firewall to restrict access
# sudo ufw allow from 10.0.0.0/8 to any port 9090
```

#### 5. Deployment

**Staging environment:**
- Test configuration changes
- Benchmark performance
- Validate under load
- Monitor for 24-48 hours

**Production rollout:**
- Deploy during low-traffic period
- Monitor closely for first hour
- Have rollback plan ready
- Document any issues

**Gradual rollout (large deployments):**
1. Deploy to 1 instance (canary)
2. Monitor for 24 hours
3. Deploy to 10% of instances
4. Monitor for 48 hours
5. Complete rollout

### Performance Best Practices

#### 1. Start with Defaults

```yaml
# Begin with recommended defaults
raw_disk_cache:
  block_size: 4096
  use_direct_io: true
  enable_compression: true
  enable_prefetch: true
  enable_zero_copy: true
```

#### 2. Measure Before Optimizing

```bash
# Establish baseline
ab -n 10000 -c 100 http://localhost:8080/test-file

# Record metrics
curl http://localhost:9090/stats > baseline-stats.json
```

#### 3. Tune Incrementally

- Change one parameter at a time
- Measure impact
- Document changes
- Revert if performance degrades

#### 4. Use Appropriate Hardware

**Minimum:**
- 2 CPU cores
- 4GB RAM
- SATA SSD

**Recommended:**
- 4+ CPU cores
- 8GB+ RAM
- NVMe SSD

**High-performance:**
- 8+ CPU cores
- 16GB+ RAM
- Enterprise NVMe SSD
- Dedicated cache storage

#### 5. Optimize for Your Workload

**Small files:** Optimize for IOPS
```yaml
block_size: 4096
use_direct_io: true
enable_compression: true
```

**Large files:** Optimize for bandwidth
```yaml
block_size: 16384
enable_zero_copy: true
enable_prefetch: true
```

**Mixed:** Use balanced settings
```yaml
block_size: 4096
# Enable all features
```

## Advanced Topics

### Using io_uring for Maximum Performance

io_uring provides the highest performance I/O on Linux (kernel 5.1+).

**Benefits:**
- 2-3x higher IOPS
- Lower CPU usage
- Better scalability

**Configuration:**
```yaml
raw_disk_cache:
  device_path: "/dev/nvme0n1"
  total_size: 107374182400
  block_size: 8192
  use_direct_io: true
  # io_uring is automatically used if available
```

**Tuning io_uring:**

See [IO_URING_TUNING.md](IO_URING_TUNING.md) for detailed tuning guide.

**Quick recommendations:**

| Workload | Queue Depth | SQPOLL | IOPOLL |
|----------|-------------|--------|--------|
| Low latency | 64 | Yes* | Yes* |
| High throughput | 512-1024 | No | Yes* |
| Balanced | 128-256 | No | No |

*Requires elevated privileges or NVMe storage

### Compression Tuning

**Compression levels:**

The cache uses zstd compression with adaptive levels:
- Level 1-3: Fast compression, lower ratio
- Level 4-9: Balanced (default: level 3)
- Level 10+: High compression, slower

**When to adjust:**
```yaml
# Fast compression (CPU-constrained)
# Note: Compression level is automatic, but you can:
enable_compression: true  # Uses adaptive level

# Disable for pre-compressed content
enable_compression: false
```

**Monitoring compression effectiveness:**
```bash
curl http://localhost:9090/stats | jq '{
  compression_ratio: .raw_disk_stats.compression_ratio,
  compressed_bytes: .raw_disk_stats.compressed_bytes,
  uncompressed_bytes: .raw_disk_stats.uncompressed_bytes
}'

# Good compression: ratio > 0.3 (30% savings)
# Poor compression: ratio < 0.1 (10% savings)
```

### Defragmentation

**Automatic defragmentation:**

The cache automatically defragments when:
- Fragmentation ratio > 40%
- Free space < 10%
- On startup (if needed)

**Manual defragmentation:**
```bash
# Restart service (triggers defrag on startup)
sudo systemctl restart pingora-slice

# Or use API (if available)
curl -X POST http://localhost:9090/admin/defragment
```

**Monitoring fragmentation:**
```bash
watch -n 60 'curl -s http://localhost:9090/stats | \
  jq ".raw_disk_stats.fragmentation_ratio"'

# Target: < 0.2 (20%)
# Warning: > 0.4 (40%)
# Critical: > 0.6 (60%)
```

### Garbage Collection Tuning

**GC triggers:**
- Free space < 10%
- Periodic (every hour)
- Manual trigger

**GC strategies:**
- LRU (default): Evict least recently used
- LFU: Evict least frequently used
- FIFO: Evict oldest entries

**Monitoring GC:**
```bash
# Check GC statistics
curl http://localhost:9090/stats | jq '{
  gc_runs: .raw_disk_stats.gc_runs,
  gc_reclaimed_bytes: .raw_disk_stats.gc_reclaimed_bytes,
  gc_reclaimed_entries: .raw_disk_stats.gc_reclaimed_entries
}'
```

### Using Block Devices

**Advantages of block devices:**
- Better performance
- No filesystem overhead
- Direct hardware access

**Setup:**
```bash
# 1. Identify device
lsblk

# 2. Ensure device is not mounted
sudo umount /dev/nvme0n1p1

# 3. Set permissions
sudo chown pingora:pingora /dev/nvme0n1p1

# 4. Configure
```

```yaml
raw_disk_cache:
  device_path: "/dev/nvme0n1p1"
  total_size: 107374182400  # Must match partition size
  block_size: 8192
  use_direct_io: true
```

**Important notes:**
- Device will be overwritten
- No filesystem needed
- Cannot share with other applications
- Backup any existing data first

### Multi-Tier Caching

**Architecture:**
```
L1 (Memory) → L2 (Raw Disk) → Origin
```

**Configuration:**
```yaml
# L1: Fast, small, in-memory
l1_cache_size_bytes: 104857600  # 100MB

# L2: Large, persistent, raw disk
enable_l2_cache: true
l2_backend: "raw_disk"
raw_disk_cache:
  device_path: "/var/cache/pingora-slice-raw"
  total_size: 107374182400  # 100GB
  block_size: 4096
  use_direct_io: true
```

**Optimization tips:**
- L1 size: 1-5% of L2 size
- L1 TTL: Same as L2 TTL
- Monitor L1 hit rate (target: 30-50%)
- Monitor L2 hit rate (target: 80-90%)

### Hot Reload Configuration

**Supported changes without restart:**
- `slice_size`
- `max_concurrent_subrequests`
- `max_retries`
- `upstream_address`

**Requires restart:**
- `l2_backend`
- `raw_disk_cache.*`
- `l1_cache_size_bytes`
- `cache_ttl`

**Hot reload procedure:**
```bash
# 1. Update configuration file
sudo vi /etc/pingora-slice/config.yaml

# 2. Reload (if supported)
sudo systemctl reload pingora-slice

# 3. Verify changes
curl http://localhost:9090/stats | jq .config
```

### Crash Recovery

**Automatic recovery:**

The cache automatically recovers from crashes:
1. Loads metadata from disk
2. Validates checksums
3. Rebuilds corrupted metadata
4. Resumes operation

**Recovery process:**
```bash
# On startup, check logs:
sudo journalctl -u pingora-slice -n 100 | grep -i recovery

# Look for:
# "Loading metadata from disk"
# "Metadata loaded successfully"
# "Recovered X entries"
```

**Manual recovery (if needed):**
```bash
# 1. Stop service
sudo systemctl stop pingora-slice

# 2. Backup cache file
sudo cp /var/cache/pingora-slice-raw /backup/

# 3. Clear and recreate
sudo rm /var/cache/pingora-slice-raw
sudo fallocate -l 10G /var/cache/pingora-slice-raw
sudo chown pingora:pingora /var/cache/pingora-slice-raw

# 4. Restart
sudo systemctl start pingora-slice
```

## FAQ

### General Questions

**Q: What's the difference between raw disk cache and file-based cache?**

A: Raw disk cache directly manages disk blocks without a filesystem, providing:
- 4-5x better performance for small files
- 95%+ space utilization vs 70-80%
- More predictable behavior
- Advanced features (compression, prefetch, zero-copy)

Trade-off: More complex to operate and debug.

**Q: Can I use raw disk cache in production?**

A: Yes! Raw disk cache is production-ready and provides significant performance benefits. Ensure you:
- Test thoroughly in staging
- Monitor key metrics
- Have rollback plan ready
- Follow best practices in this guide

**Q: How much performance improvement can I expect?**

A: Typical improvements:
- Small files (<100KB): 4-5x faster
- Large files (>10MB): 1.5-2x faster
- Space utilization: 95%+ vs 70-80%
- Latency: 2-5x lower P95

Actual results depend on workload and hardware.

**Q: What happens if the cache file is deleted?**

A: The cache will be empty and will start fresh. This is safe - the cache is ephemeral. Performance will be degraded until the cache warms up (typically 1-4 hours).

### Configuration Questions

**Q: How do I choose the right block size?**

A: Use this formula:
```
Block Size = Smallest power of 2 ≥ (Average File Size / 4)
```

Or use these guidelines:
- Small files (<100KB): 4KB
- Medium files (100KB-10MB): 8KB
- Large files (>10MB): 16KB

**Q: Should I enable compression?**

A: Enable compression if:
- ✅ Caching text content (HTML, JSON, XML)
- ✅ Disk space is limited
- ✅ Content is compressible

Disable compression if:
- ❌ Content is already compressed (JPEG, MP4, ZIP)
- ❌ CPU is constrained
- ❌ Maximum speed is critical

**Q: What's the minimum cache size?**

A: Technical minimum: 1MB
Practical minimum: 1GB for production

Recommended:
- Small deployment: 10GB
- Medium deployment: 100GB
- Large deployment: 1TB+

**Q: Can I use a regular file instead of a block device?**

A: Yes! Regular files work well and are easier to manage:
```yaml
device_path: "/var/cache/pingora-slice-raw"
```

Block devices provide slightly better performance but are harder to manage.

### Performance Questions

**Q: Why is my hit rate low?**

A: Common causes:
1. Cache is still warming up (wait 1-4 hours)
2. Cache is too small (increase `total_size`)
3. TTL is too short (increase `cache_ttl`)
4. Traffic pattern is random (expected)

**Q: Why is performance worse than file-based cache?**

A: Check these:
1. Block size too small or too large
2. O_DIRECT disabled on shared storage
3. Compression enabled for compressed content
4. Disk is slow (HDD instead of SSD)
5. High fragmentation (>40%)

**Q: How do I reduce memory usage?**

A: Try these:
1. Reduce L1 cache size
2. Disable prefetch
3. Disable zero-copy
4. Reduce cache TTL

### Operational Questions

**Q: How do I migrate from file-based to raw disk cache?**

A: See [RAW_DISK_MIGRATION_GUIDE.md](RAW_DISK_MIGRATION_GUIDE.md) for detailed steps.

Quick summary:
1. Create cache file
2. Update configuration
3. Restart service
4. Monitor warmup (1-4 hours)

**Q: How do I back up the cache?**

A: The cache is ephemeral - no backup needed. Just backup your configuration:
```bash
sudo cp /etc/pingora-slice/config.yaml /backup/
```

To recreate cache after disaster:
```bash
sudo fallocate -l 10G /var/cache/pingora-slice-raw
sudo systemctl restart pingora-slice
```

**Q: What happens during a crash?**

A: The cache automatically recovers:
1. Loads metadata from disk
2. Validates data integrity
3. Rebuilds if needed
4. Resumes operation

Some cached data may be lost, but the cache will work correctly.

**Q: How do I monitor the cache?**

A: Use the metrics endpoint:
```bash
# Overall statistics
curl http://localhost:9090/stats

# Raw disk specific
curl http://localhost:9090/stats | jq '.raw_disk_stats'
```

Key metrics:
- Hit rate (target: >80%)
- Fragmentation (target: <20%)
- Free space (target: >20%)

**Q: When should I defragment?**

A: Defragmentation is automatic when fragmentation > 40%. You can also manually trigger:
```bash
sudo systemctl restart pingora-slice
```

**Q: Can I resize the cache?**

A: Yes, but requires restart:
1. Stop service
2. Resize file: `fallocate -l 20G /var/cache/pingora-slice-raw`
3. Update config: `total_size: 21474836480`
4. Restart service

Note: Existing cache data will be lost.

### Troubleshooting Questions

**Q: Service won't start - "Permission denied"**

A: Fix permissions:
```bash
sudo chown pingora:pingora /var/cache/pingora-slice-raw
sudo chmod 600 /var/cache/pingora-slice-raw
```

**Q: Service won't start - "Invalid configuration"**

A: Check:
1. Block size is power of 2
2. Total size ≥ 10 × block size
3. YAML syntax is valid
4. File exists

**Q: Getting checksum errors**

A: Possible causes:
1. Disk corruption (check with `smartctl`)
2. Memory errors (check with `memtest`)
3. Software bug (check logs)

Solution: Clear cache and restart:
```bash
sudo systemctl stop pingora-slice
sudo rm /var/cache/pingora-slice-raw
sudo fallocate -l 10G /var/cache/pingora-slice-raw
sudo chown pingora:pingora /var/cache/pingora-slice-raw
sudo systemctl start pingora-slice
```

**Q: High CPU usage**

A: Try:
1. Disable compression
2. Reduce concurrent operations
3. Check for high request rate
4. Verify disk is not bottleneck

**Q: Cache fills up quickly**

A: Solutions:
1. Increase cache size
2. Reduce TTL
3. Implement cache key filtering
4. Check for cache stampede

## Additional Resources

### Documentation

- [Quick Start Guide](RAW_DISK_QUICK_START.md) - Get started in 5 minutes
- [Configuration Reference](RAW_DISK_CONFIGURATION.md) - Detailed configuration options
- [Migration Guide](RAW_DISK_MIGRATION_GUIDE.md) - Migrate between backends
- [Design Document](RAW_DISK_CACHE_DESIGN.md) - Architecture and design
- [Metrics Reference](RAW_DISK_METRICS.md) - Available metrics
- [Performance Tuning](PERFORMANCE_TUNING.md) - General performance guide
- [io_uring Tuning](IO_URING_TUNING.md) - Advanced I/O optimization

### Examples

```bash
# View example configurations
ls examples/pingora_slice_raw_disk*.yaml

# Run examples
cargo run --example raw_disk_metrics_example
```

### Support

For issues or questions:
1. Check this guide and related documentation
2. Review logs: `journalctl -u pingora-slice -f`
3. Check metrics: `curl http://localhost:9090/stats`
4. Open an issue on GitHub with:
   - Configuration file
   - Relevant logs
   - System information
   - Steps to reproduce

## Conclusion

The raw disk cache provides significant performance improvements for high-concurrency, performance-critical applications. By following this guide, you can:

- ✅ Configure the cache correctly for your workload
- ✅ Tune performance for optimal results
- ✅ Troubleshoot common issues
- ✅ Operate the cache reliably in production

**Key takeaways:**
1. Start with recommended defaults
2. Measure before optimizing
3. Monitor key metrics continuously
4. Follow best practices
5. Test thoroughly before production

For additional help, refer to the related documentation or open an issue on GitHub.

---

**Document Version:** 1.0  
**Last Updated:** 2024-12-01  
**Applies to:** Pingora Slice with Raw Disk Cache support
