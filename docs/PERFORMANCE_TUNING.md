# Performance Tuning Guide

## Quick Start

This guide helps you optimize Pingora Slice for your specific use case.

## Configuration Profiles

### 1. High-Throughput Profile

**Best for:** CDN, high-traffic websites, large file distribution

```yaml
slice_size: 2097152  # 2MB
max_concurrent_subrequests: 8
max_retries: 2
cache_ttl: 7200  # 2 hours
```

**Characteristics:**
- Larger slices reduce request overhead
- Higher concurrency maximizes bandwidth utilization
- Fewer retries for faster failure detection
- Longer cache TTL reduces origin load

**Expected Performance:**
- Throughput: 15-20 requests/sec for 100MB files
- Memory: ~2.5GB for 20 concurrent clients
- Cache hit rate: 90%+ after warmup

### 2. Low-Memory Profile

**Best for:** Resource-constrained environments, embedded systems

```yaml
slice_size: 524288  # 512KB
max_concurrent_subrequests: 4
max_retries: 3
cache_ttl: 1800  # 30 minutes
```

**Characteristics:**
- Smaller slices reduce memory footprint
- Lower concurrency limits memory usage
- Standard retry count for reliability
- Shorter cache TTL reduces cache memory

**Expected Performance:**
- Memory: ~450MB for 100 concurrent clients
- Throughput: 850 requests/sec for 1MB files
- Cache hit rate: 78% after warmup

### 3. Balanced Profile (Default)

**Best for:** General-purpose deployments, mixed workloads

```yaml
slice_size: 1048576  # 1MB
max_concurrent_subrequests: 4
max_retries: 3
cache_ttl: 3600  # 1 hour
```

**Characteristics:**
- Good balance between performance and resource usage
- Suitable for most use cases
- Predictable behavior

**Expected Performance:**
- Throughput: 120 requests/sec for 10MB files
- Memory: ~1.2GB for 50 concurrent clients
- Cache hit rate: 85% after warmup

## Parameter Tuning

### Slice Size

**Formula:**
```
optimal_slice_size = min(max(file_size / 10, 512KB), 2MB)
```

**Guidelines:**
- **64KB - 256KB**: Very small files, low memory
- **512KB - 1MB**: General purpose, balanced
- **1MB - 2MB**: Large files, high throughput
- **2MB+**: Maximum throughput, high memory

**Trade-offs:**
- Smaller: Better cache granularity, more overhead
- Larger: Less overhead, worse partial cache hits

### Concurrent Subrequests

**Formula:**
```
max_concurrent = min(
    bandwidth_mbps / (slice_size_mb * 8),
    origin_capacity / expected_clients,
    8
)
```

**Guidelines:**
- **2-4**: Low bandwidth, limited origin capacity
- **4-6**: Standard deployments
- **6-8**: High bandwidth, capable origin
- **8+**: Maximum throughput (requires testing)

**Trade-offs:**
- Lower: Less origin load, lower throughput
- Higher: Better throughput, more origin load

### Retry Configuration

**Guidelines:**
- **0-1 retries**: Fail-fast scenarios, high availability origins
- **2-3 retries**: Standard deployments (recommended)
- **4-5 retries**: Unreliable networks, flaky origins
- **6+ retries**: Not recommended (high latency)

**Backoff Strategy:**
- Initial: 100ms (fast retry for transient errors)
- Growth: 2x exponential
- Maximum: 1600ms (cap to avoid long delays)

### Cache TTL

**Guidelines:**
- **300-1800s (5-30 min)**: Frequently updated content
- **1800-3600s (30-60 min)**: Standard content
- **3600-7200s (1-2 hours)**: Static content
- **7200s+ (2+ hours)**: Rarely updated content

**Considerations:**
- Content update frequency
- Cache invalidation strategy
- Memory constraints
- Origin load tolerance

## Advanced Optimizations

### 1. Adaptive Slice Sizing

Adjust slice size based on file size:

```rust
fn calculate_optimal_slice_size(file_size: u64) -> usize {
    match file_size {
        0..=1_048_576 => 262_144,           // < 1MB: 256KB slices
        1_048_577..=10_485_760 => 524_288,  // 1-10MB: 512KB slices
        10_485_761..=104_857_600 => 1_048_576, // 10-100MB: 1MB slices
        _ => 2_097_152,                     // > 100MB: 2MB slices
    }
}
```

### 2. Cache Size Limits

Implement cache size limits to prevent memory exhaustion:

```rust
// Create cache with 1GB limit
let cache = SliceCache::with_max_size(
    Duration::from_secs(3600),
    1024 * 1024 * 1024  // 1GB
);
```

**Benefits:**
- Prevents OOM errors
- Automatic LRU eviction
- Predictable memory usage

### 3. Connection Pooling

The HTTP client is already optimized with connection pooling:

```rust
Client::builder()
    .pool_max_idle_per_host(10)  // Reuse connections
    .pool_idle_timeout(Duration::from_secs(90))
    .tcp_nodelay(true)  // Low latency
    .http2_adaptive_window(true)  // Better flow control
```

### 4. Monitoring and Metrics

Enable metrics endpoint to monitor performance:

```yaml
metrics_endpoint:
  enabled: true
  address: "127.0.0.1:9090"
```

**Key metrics to monitor:**
- Request rate and latency
- Cache hit rate
- Memory usage
- Subrequest failures
- Retry rate

## Troubleshooting

### High Memory Usage

**Symptoms:**
- Memory grows unbounded
- OOM errors
- Slow performance

**Solutions:**
1. Reduce slice size
2. Implement cache size limits
3. Reduce concurrent requests
4. Shorter cache TTL

### Low Throughput

**Symptoms:**
- Slow request processing
- High latency
- Underutilized bandwidth

**Solutions:**
1. Increase slice size
2. Increase concurrent subrequests
3. Check origin server capacity
4. Verify network bandwidth

### High Origin Load

**Symptoms:**
- Origin server overloaded
- Many 5xx errors
- Slow origin responses

**Solutions:**
1. Reduce concurrent subrequests
2. Increase cache TTL
3. Implement request rate limiting
4. Add more origin servers

### Low Cache Hit Rate

**Symptoms:**
- Most requests go to origin
- High bandwidth usage
- Slow performance

**Solutions:**
1. Increase cache TTL
2. Verify cache is enabled
3. Check cache size limits
4. Analyze access patterns

## Benchmarking

### Running Benchmarks

```bash
# Run built-in benchmark
cargo run --release --example benchmark

# Run stress test
./scripts/stress_test.sh

# Custom load test with Apache Bench
ab -n 1000 -c 50 http://localhost:8080/large-file.bin
```

### Interpreting Results

**Good Performance Indicators:**
- Cache hit rate > 80%
- 95th percentile latency < 2x median
- Memory usage stable over time
- No failed requests

**Warning Signs:**
- Cache hit rate < 50%
- High retry rate (> 10%)
- Memory growth over time
- Frequent 5xx errors

## Best Practices

1. **Start with defaults** and measure performance
2. **Monitor metrics** to identify bottlenecks
3. **Test changes** in staging before production
4. **Tune incrementally** - change one parameter at a time
5. **Document changes** and their impact
6. **Set up alerts** for key metrics
7. **Review regularly** as traffic patterns change

## Example Configurations

### CDN Edge Server

```yaml
slice_size: 2097152
max_concurrent_subrequests: 8
max_retries: 2
cache_ttl: 7200
slice_patterns:
  - "^/static/.*"
  - "^/downloads/.*"
```

### API Gateway

```yaml
slice_size: 524288
max_concurrent_subrequests: 4
max_retries: 3
cache_ttl: 1800
slice_patterns:
  - "^/api/files/.*"
```

### Video Streaming

```yaml
slice_size: 2097152
max_concurrent_subrequests: 6
max_retries: 2
cache_ttl: 3600
slice_patterns:
  - "^/videos/.*\\.(mp4|mkv|avi)$"
```

## Performance Checklist

- [ ] Measured baseline performance
- [ ] Identified bottlenecks
- [ ] Tuned configuration parameters
- [ ] Tested under load
- [ ] Monitored metrics
- [ ] Documented changes
- [ ] Set up alerts
- [ ] Planned for scaling

## Further Reading

- [Performance Optimization Report](performance_optimization.md)
- [Configuration Reference](CONFIGURATION.md)
- [Deployment Guide](DEPLOYMENT.md)
- [API Documentation](API.md)
