# io_uring Performance Tuning Guide

## Overview

This guide provides recommendations for tuning io_uring performance based on different workload characteristics and hardware configurations.

## Quick Start

### Default Configuration (Recommended for Most Workloads)

```rust
use pingora_slice::raw_disk::{RawDiskCache, IoUringConfig};
use std::time::Duration;

let config = IoUringConfig::default();
let cache = RawDiskCache::new_with_io_uring(
    "/path/to/cache",
    100 * 1024 * 1024,
    4096,
    Duration::from_secs(3600),
    config,
).await?;
```

## Configuration Parameters

### Queue Depth

The queue depth determines how many I/O operations can be in flight simultaneously.

#### Small Queue Depth (32-64)

**Best for:**
- Low-concurrency workloads
- Memory-constrained systems
- Development/testing

**Configuration:**
```rust
let config = IoUringConfig {
    queue_depth: 32,
    ..Default::default()
};
```

**Characteristics:**
- Lower memory usage (~128KB)
- Suitable for <10 concurrent operations
- Minimal overhead

#### Medium Queue Depth (128-256)

**Best for:**
- General-purpose workloads
- Moderate concurrency (10-50 operations)
- Production deployments

**Configuration:**
```rust
let config = IoUringConfig {
    queue_depth: 128,  // Default
    ..Default::default()
};
```

**Characteristics:**
- Balanced memory/performance (~512KB)
- Good for most use cases
- Recommended starting point

#### Large Queue Depth (512-1024)

**Best for:**
- High-concurrency workloads
- Batch processing
- Maximum throughput scenarios

**Configuration:**
```rust
let config = IoUringConfig {
    queue_depth: 1024,
    ..Default::default()
};
```

**Characteristics:**
- Higher memory usage (~4MB)
- Best for >50 concurrent operations
- Maximum throughput

### SQPOLL Mode

Submission Queue Polling mode allows the kernel to poll for new submissions, reducing latency.

**Advantages:**
- Lower submission latency
- Reduced system call overhead
- Better for latency-sensitive workloads

**Disadvantages:**
- Requires elevated privileges (CAP_SYS_NICE or root)
- Consumes a dedicated kernel thread
- May increase CPU usage

**Configuration:**
```rust
let config = IoUringConfig {
    queue_depth: 256,
    use_sqpoll: true,  // Requires privileges
    ..Default::default()
};
```

**When to use:**
- Latency-critical applications
- When you have elevated privileges
- Systems with spare CPU cores

### IOPOLL Mode

I/O Polling mode polls for I/O completions instead of using interrupts.

**Advantages:**
- Lower completion latency
- Better for high-performance storage (NVMe)
- Reduced interrupt overhead

**Disadvantages:**
- Requires polling-capable devices
- Increases CPU usage
- Not beneficial for slower storage

**Configuration:**
```rust
let config = IoUringConfig {
    queue_depth: 256,
    use_iopoll: true,  // Best with NVMe
    ..Default::default()
};
```

**When to use:**
- NVMe storage devices
- Ultra-low latency requirements
- High-throughput scenarios

### Block Size

The block size affects alignment and I/O granularity.

**Common Values:**
- 512 bytes: Legacy compatibility
- 4096 bytes: Standard page size (default)
- 8192 bytes: Some modern SSDs
- 16384 bytes: Large block optimization

**Configuration:**
```rust
let config = IoUringConfig {
    queue_depth: 256,
    block_size: 4096,  // Default
    ..Default::default()
};
```

## Workload-Specific Tuning

### High-Throughput Batch Processing

**Characteristics:**
- Many concurrent operations
- Large data transfers
- Throughput > latency

**Recommended Configuration:**
```rust
let config = IoUringConfig {
    queue_depth: 1024,
    use_sqpoll: false,
    use_iopoll: true,  // If using NVMe
    block_size: 4096,
};
```

**Additional Tips:**
- Use batch operations: `cache.lookup_batch()`
- Increase cache size
- Consider larger block sizes (8KB-16KB)

### Low-Latency Interactive

**Characteristics:**
- Low concurrency
- Small data transfers
- Latency > throughput

**Recommended Configuration:**
```rust
let config = IoUringConfig {
    queue_depth: 64,
    use_sqpoll: true,  // If privileges available
    use_iopoll: true,  // If using NVMe
    block_size: 4096,
};
```

**Additional Tips:**
- Use direct operations: `store_with_io_uring()`
- Minimize queue depth
- Enable SQPOLL if possible

### Mixed Workload

**Characteristics:**
- Variable concurrency
- Mixed operation sizes
- Balanced requirements

**Recommended Configuration:**
```rust
let config = IoUringConfig {
    queue_depth: 256,
    use_sqpoll: false,
    use_iopoll: false,
    block_size: 4096,
};
```

**Additional Tips:**
- Monitor performance metrics
- Adjust queue depth based on load
- Use batch operations when possible

## Hardware-Specific Tuning

### NVMe SSDs

**Characteristics:**
- Very high IOPS (>100K)
- Low latency (<100µs)
- Polling-capable

**Recommended Configuration:**
```rust
let config = IoUringConfig {
    queue_depth: 512,
    use_sqpoll: true,
    use_iopoll: true,
    block_size: 4096,
};
```

### SATA SSDs

**Characteristics:**
- Moderate IOPS (~10K-50K)
- Moderate latency (~100-500µs)
- Interrupt-based

**Recommended Configuration:**
```rust
let config = IoUringConfig {
    queue_depth: 128,
    use_sqpoll: false,
    use_iopoll: false,
    block_size: 4096,
};
```

### HDDs

**Characteristics:**
- Low IOPS (<200)
- High latency (>5ms)
- Sequential preferred

**Recommended Configuration:**
```rust
let config = IoUringConfig {
    queue_depth: 32,
    use_sqpoll: false,
    use_iopoll: false,
    block_size: 4096,
};
```

**Note:** io_uring provides minimal benefit for HDDs. Consider using standard I/O.

## Monitoring and Optimization

### Performance Metrics

Monitor these metrics to optimize configuration:

```rust
let stats = cache.stats().await;
println!("Entries: {}", stats.entries);
println!("Hits: {}", stats.hits);
println!("Misses: {}", stats.misses);
println!("Hit Rate: {:.2}%", 
         (stats.hits as f64 / (stats.hits + stats.misses) as f64) * 100.0);
```

### System Metrics

Monitor system-level metrics:

```bash
# I/O statistics
iostat -x 1

# CPU usage
top -p $(pgrep pingora-slice)

# io_uring statistics (if available)
cat /proc/$(pgrep pingora-slice)/io
```

### Benchmarking

Use the provided example for benchmarking:

```bash
cargo run --example io_uring_example --release
```

## Common Issues and Solutions

### Issue: Poor Performance

**Symptoms:**
- Slower than standard I/O
- High CPU usage
- Low throughput

**Solutions:**
1. Check queue depth - may be too small or too large
2. Verify hardware supports io_uring features
3. Disable SQPOLL/IOPOLL if not beneficial
4. Use batch operations

### Issue: High Memory Usage

**Symptoms:**
- Excessive memory consumption
- OOM errors

**Solutions:**
1. Reduce queue depth
2. Reduce cache size
3. Monitor with `stats.buffered_bytes`

### Issue: Permission Errors

**Symptoms:**
- "Permission denied" with SQPOLL
- Cannot enable IOPOLL

**Solutions:**
1. Run with elevated privileges
2. Add CAP_SYS_NICE capability
3. Disable SQPOLL: `use_sqpoll: false`

## Best Practices

### 1. Start with Defaults

Always start with default configuration and measure:

```rust
let config = IoUringConfig::default();
```

### 2. Measure Before Optimizing

Use benchmarks to establish baseline:

```bash
cargo run --example io_uring_example --release
```

### 3. Tune Incrementally

Change one parameter at a time and measure impact.

### 4. Use Batch Operations

Batch operations provide better performance:

```rust
// Good
let results = cache.lookup_batch(&keys).await?;

// Less efficient
for key in keys {
    cache.lookup_with_io_uring(&key).await?;
}
```

### 5. Monitor in Production

Continuously monitor metrics and adjust as needed.

## Advanced Tuning

### Kernel Parameters

Optimize kernel parameters for io_uring:

```bash
# Increase max locked memory (for SQPOLL)
ulimit -l unlimited

# Increase max open files
ulimit -n 65536

# Tune I/O scheduler (for NVMe)
echo none > /sys/block/nvme0n1/queue/scheduler
```

### CPU Affinity

Pin io_uring threads to specific CPUs:

```bash
# Pin to CPUs 0-3
taskset -c 0-3 ./pingora-slice
```

### NUMA Considerations

For NUMA systems, ensure cache and storage are on the same node:

```bash
# Check NUMA topology
numactl --hardware

# Run on specific NUMA node
numactl --cpunodebind=0 --membind=0 ./pingora-slice
```

## References

- [io_uring Documentation](https://kernel.dk/io_uring.pdf)
- [Linux Performance Tuning](https://www.kernel.org/doc/html/latest/admin-guide/sysctl/vm.html)
- [NVMe Optimization](https://www.kernel.org/doc/html/latest/block/index.html)

## See Also

- [IO_URING_IMPLEMENTATION.md](IO_URING_IMPLEMENTATION.md)
- [PERFORMANCE_TUNING.md](PERFORMANCE_TUNING.md)
- [RAW_DISK_CACHE_DESIGN.md](RAW_DISK_CACHE_DESIGN.md)
