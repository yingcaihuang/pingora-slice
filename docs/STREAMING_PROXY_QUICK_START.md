# Streaming Proxy Quick Start Guide

This guide will help you get started with the Pingora Slice streaming proxy with configuration and monitoring support.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
pingora-slice = "0.2.3"
```

## Quick Start

### 1. Create Configuration File

Create `config.yaml`:

```yaml
# Cache configuration
enable_cache: true
cache_ttl: 3600  # 1 hour

# L1 (memory) cache
l1_cache_size_bytes: 104857600  # 100MB

# L2 (file-based) cache
enable_l2_cache: true
l2_backend: "file"
l2_cache_dir: "/tmp/streaming-cache"

# Upstream server
upstream_address: "origin.example.com:80"

# Metrics endpoint
metrics_endpoint:
  enabled: true
  address: "127.0.0.1:9090"
```

### 2. Create Streaming Proxy

```rust
use pingora::prelude::*;
use pingora::proxy::http_proxy_service;
use pingora_slice::{StreamingProxy, HealthCheckService};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Load configuration and create proxy
    let proxy = StreamingProxy::from_config("config.yaml").await?;

    // Start health check endpoint
    let health = Arc::new(HealthCheckService::new());
    tokio::spawn(async move {
        health.start("127.0.0.1:8081").await.unwrap();
    });

    // Create Pingora server
    let mut server = Server::new(None)?;
    server.bootstrap();

    // Create proxy service
    let mut proxy_service = http_proxy_service(&server.configuration, proxy);
    proxy_service.add_tcp("0.0.0.0:8080");

    // Add service and run
    server.add_service(proxy_service);
    server.run_forever();
}
```

### 3. Run the Server

```bash
cargo run
```

### 4. Test the Proxy

```bash
# Fetch a file (will stream from origin on first request)
curl http://localhost:8080/test.dat -o /dev/null

# Fetch again (should be faster from cache)
curl http://localhost:8080/test.dat -o /dev/null

# Check health status
curl http://localhost:8081/health

# Check metrics (if enabled)
curl http://localhost:9090/metrics
```

## Configuration Options

### File-Based Cache (Simple)

```yaml
enable_cache: true
cache_ttl: 3600
l1_cache_size_bytes: 104857600  # 100MB
enable_l2_cache: true
l2_backend: "file"
l2_cache_dir: "/var/cache/pingora-slice"
upstream_address: "origin.example.com:80"
```

### Raw Disk Cache (High Performance)

```yaml
enable_cache: true
cache_ttl: 3600
l1_cache_size_bytes: 104857600  # 100MB
enable_l2_cache: true
l2_backend: "raw_disk"

raw_disk_cache:
  device_path: "/dev/sdb1"  # or "/tmp/cache-file"
  total_size: 10737418240  # 10GB
  block_size: 4096  # 4KB
  use_direct_io: true
  enable_compression: true
  enable_prefetch: true
  enable_zero_copy: true

upstream_address: "origin.example.com:80"
```

### Memory-Only Cache (No Persistence)

```yaml
enable_cache: true
cache_ttl: 3600
l1_cache_size_bytes: 1073741824  # 1GB
enable_l2_cache: false  # Disable L2 cache
upstream_address: "origin.example.com:80"
```

## Monitoring

### Health Check Endpoints

```bash
# Health status
curl http://localhost:8081/health
# Response: {"status":"healthy"}

# Readiness check
curl http://localhost:8081/ready
# Response: {"status":"healthy"}

# Liveness check
curl http://localhost:8081/live
# Response: {"status":"healthy"}
```

### Cache Statistics

```rust
// Get cache statistics
let stats = proxy.cache_stats();
println!("L1 hits: {}", stats.l1_hits);
println!("L2 hits: {}", stats.l2_hits);
println!("Misses: {}", stats.misses);
println!("Hit rate: {:.2}%", stats.cache_hit_rate());

// Get raw disk statistics (if using raw disk backend)
if let Some(raw_stats) = proxy.raw_disk_stats().await {
    println!("Total blocks: {}", raw_stats.total_blocks);
    println!("Used blocks: {}", raw_stats.used_blocks);
    println!("Fragmentation: {:.2}%", raw_stats.fragmentation_rate * 100.0);
}
```

## Kubernetes Deployment

### Deployment YAML

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: pingora-slice
spec:
  replicas: 3
  selector:
    matchLabels:
      app: pingora-slice
  template:
    metadata:
      labels:
        app: pingora-slice
    spec:
      containers:
      - name: pingora-slice
        image: your-registry/pingora-slice:latest
        ports:
        - containerPort: 8080
          name: http
        - containerPort: 8081
          name: health
        - containerPort: 9090
          name: metrics
        livenessProbe:
          httpGet:
            path: /live
            port: 8081
          initialDelaySeconds: 10
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 8081
          initialDelaySeconds: 5
          periodSeconds: 5
        volumeMounts:
        - name: config
          mountPath: /etc/pingora-slice
        - name: cache
          mountPath: /var/cache/pingora-slice
      volumes:
      - name: config
        configMap:
          name: pingora-slice-config
      - name: cache
        emptyDir: {}
```

### Service YAML

```yaml
apiVersion: v1
kind: Service
metadata:
  name: pingora-slice
spec:
  selector:
    app: pingora-slice
  ports:
  - name: http
    port: 80
    targetPort: 8080
  - name: metrics
    port: 9090
    targetPort: 9090
```

### ConfigMap YAML

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: pingora-slice-config
data:
  config.yaml: |
    enable_cache: true
    cache_ttl: 3600
    l1_cache_size_bytes: 104857600
    enable_l2_cache: true
    l2_backend: "file"
    l2_cache_dir: "/var/cache/pingora-slice"
    upstream_address: "origin.example.com:80"
    metrics_endpoint:
      enabled: true
      address: "0.0.0.0:9090"
```

## Prometheus Monitoring

### Scrape Configuration

```yaml
scrape_configs:
  - job_name: 'pingora-slice'
    kubernetes_sd_configs:
      - role: pod
    relabel_configs:
      - source_labels: [__meta_kubernetes_pod_label_app]
        action: keep
        regex: pingora-slice
      - source_labels: [__meta_kubernetes_pod_ip]
        action: replace
        target_label: __address__
        replacement: $1:9090
```

### Example Alerts

```yaml
groups:
  - name: pingora_slice
    rules:
      - alert: LowCacheHitRate
        expr: cache_hit_rate < 50
        for: 5m
        annotations:
          summary: "Cache hit rate below 50%"
      
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
   grep enable_cache config.yaml
   ```

2. Check cache statistics:
   ```rust
   let stats = proxy.cache_stats();
   println!("Hits: {}, Misses: {}", stats.l1_hits + stats.l2_hits, stats.misses);
   ```

3. Check logs for cache errors

### Health Check Not Responding

1. Check if health check server is running:
   ```bash
   curl http://localhost:8081/health
   ```

2. Check firewall rules:
   ```bash
   sudo iptables -L | grep 8081
   ```

### Performance Issues

1. Enable raw disk cache for better performance
2. Increase L1 cache size
3. Enable O_DIRECT: `use_direct_io: true`
4. Enable compression: `enable_compression: true`

## Next Steps

- Read the [full configuration guide](STREAMING_PROXY_CONFIG.md)
- Learn about [raw disk cache](RAW_DISK_CACHE_DESIGN.md)
- See [deployment guide](DEPLOYMENT.md)
- Check [performance tuning](PERFORMANCE_TUNING.md)

## Support

For issues and questions:
- GitHub Issues: https://github.com/your-org/pingora-slice/issues
- Documentation: https://docs.your-org.com/pingora-slice
