# Streaming Proxy Configuration and Monitoring

This document describes how to configure and monitor the Pingora Slice streaming proxy with integrated configuration management, TieredCache support (including raw disk), Prometheus metrics, and health check endpoints.

## Table of Contents

- [Overview](#overview)
- [Configuration](#configuration)
- [Cache Backends](#cache-backends)
- [Health Check Endpoint](#health-check-endpoint)
- [Metrics](#metrics)
- [Examples](#examples)

## Overview

The streaming proxy provides a production-ready HTTP proxy with:

- **Configuration Management**: Load settings from YAML files
- **Flexible Caching**: Support for file-based and raw disk cache backends
- **Health Checks**: HTTP endpoints for monitoring service health
- **Metrics**: Prometheus-compatible metrics for observability

## Configuration

### Loading Configuration

The streaming proxy can be created from a configuration file:

```rust
use pingora_slice::StreamingProxy;

let proxy = StreamingProxy::from_config("config.yaml").await?;
```

### Configuration File Format

Configuration files use YAML format:

```yaml
# Cache configuration
enable_cache: true
cache_ttl: 3600  # seconds

# L1 (memory) cache
l1_cache_size_bytes: 104857600  # 100MB

# L2 (disk) cache
enable_l2_cache: true
l2_backend: "file"  # or "raw_disk"
l2_cache_dir: "/var/cache/pingora-slice"

# Upstream server
upstream_address: "origin.example.com:80"

# Metrics endpoint (optional)
metrics_endpoint:
  enabled: true
  address: "127.0.0.1:9090"
```

### Configuration Options

#### Cache Settings

- `enable_cache` (bool): Enable/disable caching
- `cache_ttl` (u64): Cache TTL in seconds
- `l1_cache_size_bytes` (usize): L1 (memory) cache size in bytes
- `enable_l2_cache` (bool): Enable/disable L2 (disk) cache
- `l2_backend` (string): Backend type - "file" or "raw_disk"
- `l2_cache_dir` (string): Directory for file-based cache or device path for raw disk

#### Raw Disk Cache Settings

When using `l2_backend: "raw_disk"`, configure the raw disk cache:

```yaml
raw_disk_cache:
  device_path: "/dev/sdb1"  # or "/tmp/cache-file"
  total_size: 10737418240  # 10GB
  block_size: 4096  # 4KB
  use_direct_io: true
  enable_compression: true
  enable_prefetch: true
  enable_zero_copy: true
```

Options:
- `device_path` (string): Path to block device or file
- `total_size` (u64): Total cache size in bytes
- `block_size` (usize): Block size (must be power of 2, 512B-1MB)
- `use_direct_io` (bool): Use O_DIRECT for I/O
- `enable_compression` (bool): Enable data compression
- `enable_prefetch` (bool): Enable prefetching
- `enable_zero_copy` (bool): Enable zero-copy operations

#### Upstream Settings

- `upstream_address` (string): Upstream server address (host:port)

#### Metrics Settings

```yaml
metrics_endpoint:
  enabled: true
  address: "127.0.0.1:9090"
```

Options:
- `enabled` (bool): Enable/disable metrics endpoint
- `address` (string): Address to bind metrics server

## Cache Backends

### File-Based Cache

The file-based cache stores data in the filesystem:

```yaml
l2_backend: "file"
l2_cache_dir: "/var/cache/pingora-slice"
```

**Pros:**
- Simple to configure
- Works on any filesystem
- Easy to inspect and debug

**Cons:**
- Slower than raw disk
- Subject to filesystem overhead
- Limited by filesystem performance

### Raw Disk Cache

The raw disk cache provides direct block-level access:

```yaml
l2_backend: "raw_disk"
raw_disk_cache:
  device_path: "/dev/sdb1"
  total_size: 10737418240
  block_size: 4096
  use_direct_io: true
```

**Pros:**
- Much faster than file-based cache
- Lower CPU usage
- Predictable performance
- Supports O_DIRECT for bypassing page cache

**Cons:**
- Requires dedicated device or file
- More complex configuration
- Requires careful capacity planning

**Performance Features:**
- **O_DIRECT**: Bypass kernel page cache for lower latency
- **Compression**: Reduce storage usage (zstd/lz4)
- **Prefetching**: Anticipate access patterns
- **Zero-copy**: Minimize memory copies

## Health Check Endpoint

The streaming proxy includes a built-in health check HTTP server.

### Starting the Health Check Service

```rust
use pingora_slice::HealthCheckService;
use std::sync::Arc;

let health_service = Arc::new(HealthCheckService::new());

// Start on port 8081
tokio::spawn(async move {
    health_service.start("127.0.0.1:8081").await.unwrap();
});
```

### Endpoints

#### GET /health

Returns the overall health status of the service.

**Response (Healthy):**
```json
{
  "status": "healthy"
}
```
HTTP Status: 200 OK

**Response (Unhealthy):**
```json
{
  "status": "unhealthy"
}
```
HTTP Status: 503 Service Unavailable

#### GET /ready

Returns the readiness status (whether the service is ready to accept traffic).

**Response:**
```json
{
  "status": "healthy"
}
```

#### GET /live

Returns the liveness status (whether the service is running).

**Response:**
```json
{
  "status": "healthy"
}
```

### Setting Health Status

```rust
use pingora_slice::HealthStatus;

// Set to degraded
health_service.set_status(HealthStatus::Degraded).await;

// Set to unhealthy
health_service.set_status(HealthStatus::Unhealthy).await;

// Set back to healthy
health_service.set_status(HealthStatus::Healthy).await;
```

## Metrics

### Cache Metrics

Get cache statistics programmatically:

```rust
// Get cache statistics
let stats = proxy.cache_stats();
println!("L1 hits: {}", stats.l1_hits);
println!("L2 hits: {}", stats.l2_hits);
println!("Misses: {}", stats.misses);
println!("Hit rate: {:.2}%", stats.cache_hit_rate());
```

### Raw Disk Metrics

Get raw disk cache statistics (if using raw disk backend):

```rust
if let Some(raw_stats) = proxy.raw_disk_stats().await {
    println!("Total blocks: {}", raw_stats.total_blocks);
    println!("Used blocks: {}", raw_stats.used_blocks);
    println!("Fragmentation: {:.2}%", raw_stats.fragmentation_rate * 100.0);
    println!("Hit rate: {:.2}%", raw_stats.hit_rate * 100.0);
}
```

### Prometheus Metrics

When metrics endpoint is enabled, Prometheus-compatible metrics are exposed:

```bash
curl http://localhost:9090/metrics
```

**Available Metrics:**

Cache metrics:
- `cache_l1_entries` - Number of entries in L1 cache
- `cache_l1_bytes` - Bytes used in L1 cache
- `cache_l1_hits_total` - Total L1 cache hits
- `cache_l2_hits_total` - Total L2 cache hits
- `cache_misses_total` - Total cache misses
- `cache_hit_rate` - Cache hit rate percentage

Raw disk metrics (when using raw disk backend):
- `raw_disk_total_blocks` - Total blocks in raw disk cache
- `raw_disk_used_blocks` - Used blocks in raw disk cache
- `raw_disk_free_blocks` - Free blocks in raw disk cache
- `raw_disk_fragmentation_rate` - Fragmentation rate (0.0-1.0)
- `raw_disk_cache_entries` - Number of cache entries
- `raw_disk_hit_rate` - Hit rate (0.0-1.0)

## Examples

### Example 1: File-Based Cache

```yaml
# config.yaml
enable_cache: true
cache_ttl: 3600
l1_cache_size_bytes: 104857600  # 100MB
enable_l2_cache: true
l2_backend: "file"
l2_cache_dir: "/var/cache/pingora-slice"
upstream_address: "origin.example.com:80"

metrics_endpoint:
  enabled: true
  address: "127.0.0.1:9090"
```

```rust
use pingora_slice::{StreamingProxy, HealthCheckService};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let proxy = StreamingProxy::from_config("config.yaml").await?;
    
    // Start health check
    let health = Arc::new(HealthCheckService::new());
    tokio::spawn(async move {
        health.start("127.0.0.1:8081").await.unwrap();
    });
    
    // Use proxy...
    Ok(())
}
```

### Example 2: Raw Disk Cache

```yaml
# config.yaml
enable_cache: true
cache_ttl: 3600
l1_cache_size_bytes: 104857600  # 100MB
enable_l2_cache: true
l2_backend: "raw_disk"

raw_disk_cache:
  device_path: "/dev/sdb1"
  total_size: 10737418240  # 10GB
  block_size: 4096
  use_direct_io: true
  enable_compression: true
  enable_prefetch: true
  enable_zero_copy: true

upstream_address: "origin.example.com:80"

metrics_endpoint:
  enabled: true
  address: "127.0.0.1:9090"
```

### Example 3: Memory-Only Cache

```yaml
# config.yaml
enable_cache: true
cache_ttl: 3600
l1_cache_size_bytes: 1073741824  # 1GB
enable_l2_cache: false  # Disable L2 cache
upstream_address: "origin.example.com:80"
```

## Monitoring Best Practices

### Health Checks

1. **Kubernetes Liveness Probe:**
   ```yaml
   livenessProbe:
     httpGet:
       path: /live
       port: 8081
     initialDelaySeconds: 10
     periodSeconds: 10
   ```

2. **Kubernetes Readiness Probe:**
   ```yaml
   readinessProbe:
     httpGet:
       path: /ready
       port: 8081
     initialDelaySeconds: 5
     periodSeconds: 5
   ```

### Metrics Collection

1. **Prometheus Scrape Config:**
   ```yaml
   scrape_configs:
     - job_name: 'pingora-slice'
       static_configs:
         - targets: ['localhost:9090']
   ```

2. **Grafana Dashboard:**
   - Import the provided dashboard JSON
   - Monitor cache hit rates
   - Track raw disk fragmentation
   - Alert on unhealthy status

### Alerting

Example Prometheus alerts:

```yaml
groups:
  - name: pingora_slice
    rules:
      - alert: LowCacheHitRate
        expr: cache_hit_rate < 50
        for: 5m
        annotations:
          summary: "Cache hit rate below 50%"
      
      - alert: HighFragmentation
        expr: raw_disk_fragmentation_rate > 0.5
        for: 10m
        annotations:
          summary: "Raw disk fragmentation above 50%"
      
      - alert: ServiceUnhealthy
        expr: up{job="pingora-slice"} == 0
        for: 1m
        annotations:
          summary: "Pingora Slice service is down"
```

## Troubleshooting

### Cache Not Working

1. Check configuration:
   ```bash
   # Verify cache is enabled
   grep enable_cache config.yaml
   ```

2. Check cache statistics:
   ```rust
   let stats = proxy.cache_stats();
   println!("Hits: {}, Misses: {}", stats.l1_hits + stats.l2_hits, stats.misses);
   ```

3. Check logs for cache errors

### Raw Disk Cache Issues

1. **Permission denied:**
   - Ensure process has read/write access to device
   - Check SELinux/AppArmor policies

2. **Performance issues:**
   - Enable O_DIRECT: `use_direct_io: true`
   - Increase block size for large files
   - Enable compression for better space utilization

3. **High fragmentation:**
   - Run defragmentation (see raw disk docs)
   - Increase cache size
   - Adjust block size

### Health Check Not Responding

1. Check if health check server is running:
   ```bash
   curl http://localhost:8081/health
   ```

2. Check firewall rules:
   ```bash
   sudo iptables -L | grep 8081
   ```

3. Check logs for health check errors

## See Also

- [Streaming Proxy Documentation](STREAMING_PROXY.md)
- [Raw Disk Cache Documentation](RAW_DISK_CACHE_DESIGN.md)
- [Configuration Reference](CONFIGURATION.md)
- [Deployment Guide](DEPLOYMENT.md)
