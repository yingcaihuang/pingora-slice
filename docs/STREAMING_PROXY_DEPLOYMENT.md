# Pingora Streaming Proxy - Production Deployment Guide

## Overview

This guide provides comprehensive instructions for deploying the Pingora streaming proxy in production environments. The streaming proxy provides edge caching with real-time streaming capabilities, supporting both file-based and high-performance raw disk cache backends.

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [System Requirements](#system-requirements)
- [Pre-Deployment Planning](#pre-deployment-planning)
- [Installation](#installation)
- [Configuration](#configuration)
- [Service Management](#service-management)
- [Monitoring and Observability](#monitoring-and-observability)
- [Performance Tuning](#performance-tuning)
- [Security](#security)
- [High Availability](#high-availability)
- [Troubleshooting](#troubleshooting)
- [Maintenance](#maintenance)

## Architecture Overview

### Streaming Proxy Architecture

```
┌─────────┐         ┌──────────────────┐         ┌──────────┐
│ Client  │────────▶│ Streaming Proxy  │────────▶│ Origin   │
└─────────┘         │                  │         └──────────┘
                    │  ┌────────────┐  │
                    │  │ L1 (Memory)│  │
                    │  └────────────┘  │
                    │  ┌────────────┐  │
                    │  │ L2 (Disk)  │  │
                    │  │ - File     │  │
                    │  │ - Raw Disk │  │
                    │  └────────────┘  │
                    └──────────────────┘
                           │
                           ▼
                    ┌──────────────┐
                    │  Prometheus  │
                    │   Metrics    │
                    └──────────────┘
```

### Key Features

- **Real-time Streaming**: Edge-to-client streaming with <1ms TTFB
- **Background Caching**: Non-blocking cache writes during streaming
- **Tiered Cache**: L1 (memory) + L2 (file or raw disk)
- **Graceful Degradation**: Cache failures don't stop proxying
- **Health Checks**: Built-in health check endpoints
- **Metrics**: Prometheus-compatible metrics

## System Requirements

### Minimum Requirements

- **CPU**: 2 cores (x86_64 or ARM64)
- **RAM**: 2 GB
- **Disk**: 20 GB (10 GB for cache)
- **OS**: Linux (Ubuntu 20.04+, CentOS 8+, Debian 11+)
- **Network**: 100 Mbps

### Recommended Production Requirements

- **CPU**: 4+ cores (x86_64)
- **RAM**: 8+ GB
- **Disk**: 100+ GB NVMe SSD (for raw disk cache)
- **OS**: Ubuntu 22.04 LTS or Rocky Linux 9
- **Network**: 1+ Gbps

### Software Dependencies

- **Rust**: 1.70+ (for building from source)
- **systemd**: For service management
- **OpenSSL**: 1.1.1+ (for HTTPS support)

## Pre-Deployment Planning

### 1. Capacity Planning

#### Cache Size Calculation

```
L1 Cache Size = Hot Data Size × 1.2
L2 Cache Size = (Total Content Size × Cache Hit Target) × 1.5

Example:
- Hot data: 50 MB → L1 = 60 MB
- Total content: 100 GB, 80% hit rate → L2 = 120 GB
```

#### Concurrent Connections

```
Max Connections = (Available RAM - OS - L1 Cache) / 10 MB

Example:
- 8 GB RAM
- OS: 2 GB
- L1 Cache: 100 MB
- Max connections: (8000 - 2000 - 100) / 10 = 590
```

### 2. Backend Selection

| Backend | Use Case | Performance | Complexity |
|---------|----------|-------------|------------|
| **File** | Development, small deployments | Good | Low |
| **Raw Disk** | Production, high-performance | Excellent | Medium |

### 3. Network Planning

- **Upstream**: Ensure stable, low-latency connection to origin
- **Firewall**: Plan port access (8080 for proxy, 8081 for health, 9090 for metrics)
- **Load Balancer**: Consider HAProxy or Nginx for multi-instance deployments

## Installation

### Option 1: Binary Installation (Recommended)

```bash
# Download latest release
VERSION="0.2.3"
wget https://github.com/your-org/pingora-slice/releases/download/v${VERSION}/pingora-slice-${VERSION}-linux-x86_64.tar.gz

# Extract
tar -xzf pingora-slice-${VERSION}-linux-x86_64.tar.gz

# Install
sudo cp pingora-slice /usr/local/bin/
sudo chmod +x /usr/local/bin/pingora-slice

# Verify
pingora-slice --version
```

### Option 2: Build from Source

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Clone repository
git clone https://github.com/your-org/pingora-slice.git
cd pingora-slice

# Build release binary
cargo build --release

# Install
sudo cp target/release/pingora-slice /usr/local/bin/
sudo chmod +x /usr/local/bin/pingora-slice
```

### Create Service User

```bash
# Create dedicated user
sudo useradd -r -s /bin/false -d /var/lib/pingora-slice pingora-slice

# Create directories
sudo mkdir -p /etc/pingora-slice
sudo mkdir -p /var/lib/pingora-slice
sudo mkdir -p /var/cache/pingora-slice
sudo mkdir -p /var/log/pingora-slice

# Set ownership
sudo chown -R pingora-slice:pingora-slice /var/lib/pingora-slice
sudo chown -R pingora-slice:pingora-slice /var/cache/pingora-slice
sudo chown -R pingora-slice:pingora-slice /var/log/pingora-slice
```

## Configuration

### Basic Configuration (File-Based Cache)

Create `/etc/pingora-slice/config.yaml`:

```yaml
# Cache configuration
enable_cache: true
cache_ttl: 3600  # 1 hour

# L1 (memory) cache
l1_cache_size_bytes: 104857600  # 100 MB

# L2 (file-based) cache
enable_l2_cache: true
l2_backend: "file"
l2_cache_dir: "/var/cache/pingora-slice"

# Upstream server
upstream_address: "origin.example.com:80"

# Metrics endpoint
metrics_endpoint:
  enabled: true
  address: "127.0.0.1:9090"
```

### Production Configuration (Raw Disk Cache)

Create `/etc/pingora-slice/config.yaml`:

```yaml
# Cache configuration
enable_cache: true
cache_ttl: 3600  # 1 hour

# L1 (memory) cache - 100 MB for hot data
l1_cache_size_bytes: 104857600

# L2 (raw disk) cache - High performance
enable_l2_cache: true
l2_backend: "raw_disk"

# Raw disk cache configuration
raw_disk_cache:
  # Cache device/file path
  device_path: "/var/cache/pingora-slice-raw"
  
  # Total cache size: 100 GB
  total_size: 107374182400
  
  # Block size: 4 KB (optimal for most workloads)
  block_size: 4096
  
  # Performance optimizations
  use_direct_io: true        # Bypass OS page cache
  enable_compression: true   # Compress cached data
  enable_prefetch: true      # Predictive prefetching
  enable_zero_copy: true     # Reduce memory copies

# Upstream server
upstream_address: "origin.example.com:80"

# Metrics endpoint
metrics_endpoint:
  enabled: true
  address: "0.0.0.0:9090"
```

### Create Raw Disk Cache File

```bash
# Create 100 GB cache file
sudo fallocate -l 100G /var/cache/pingora-slice-raw

# Set ownership
sudo chown pingora-slice:pingora-slice /var/cache/pingora-slice-raw

# Set permissions
sudo chmod 600 /var/cache/pingora-slice-raw
```

### Configuration Validation

```bash
# Test configuration
sudo -u pingora-slice /usr/local/bin/pingora-slice --check-config /etc/pingora-slice/config.yaml
```

## Service Management

### Systemd Service Configuration

Create `/etc/systemd/system/pingora-slice.service`:

```ini
[Unit]
Description=Pingora Slice Streaming Proxy
Documentation=https://github.com/your-org/pingora-slice
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=pingora-slice
Group=pingora-slice
WorkingDirectory=/var/lib/pingora-slice

# Main service
ExecStart=/usr/local/bin/pingora-slice /etc/pingora-slice/config.yaml

# Graceful reload
ExecReload=/bin/kill -HUP $MAINPID

# Restart policy
Restart=on-failure
RestartSec=5s

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=pingora-slice

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/cache/pingora-slice /var/log/pingora-slice /var/lib/pingora-slice

# Resource limits
LimitNOFILE=65536
LimitNPROC=4096

# Environment
Environment="RUST_LOG=info"
Environment="RUST_BACKTRACE=1"

[Install]
WantedBy=multi-user.target
```

### Service Management Commands

```bash
# Reload systemd
sudo systemctl daemon-reload

# Enable service
sudo systemctl enable pingora-slice

# Start service
sudo systemctl start pingora-slice

# Check status
sudo systemctl status pingora-slice

# View logs
sudo journalctl -u pingora-slice -f

# Stop service
sudo systemctl stop pingora-slice

# Restart service
sudo systemctl restart pingora-slice
```

## Monitoring and Observability

### Health Check Endpoints

The streaming proxy provides built-in health check endpoints:

```bash
# Health status
curl http://localhost:8081/health
# Response: {"status":"healthy"}

# Readiness check
curl http://localhost:8081/ready

# Liveness check
curl http://localhost:8081/live
```

### Prometheus Metrics

Configure Prometheus to scrape metrics:

```yaml
# /etc/prometheus/prometheus.yml
scrape_configs:
  - job_name: 'pingora-slice'
    static_configs:
      - targets: ['localhost:9090']
    scrape_interval: 15s
    scrape_timeout: 10s
```

### Key Metrics

| Metric | Description | Alert Threshold |
|--------|-------------|-----------------|
| `cache_hit_rate` | Cache hit rate percentage | < 50% |
| `cache_l1_hits_total` | L1 cache hits | - |
| `cache_l2_hits_total` | L2 cache hits | - |
| `cache_misses_total` | Cache misses | - |
| `raw_disk_fragmentation_rate` | Disk fragmentation | > 0.4 |
| `raw_disk_used_blocks` | Used cache blocks | > 90% |

### Grafana Dashboard

Import the provided Grafana dashboard or create panels for:

1. **Cache Performance**
   - Hit rate over time
   - L1 vs L2 hit distribution
   - Miss rate trends

2. **Raw Disk Health**
   - Fragmentation rate
   - Space utilization
   - I/O operations

3. **Request Metrics**
   - Request rate
   - Response time (TTFB)
   - Error rate

4. **System Resources**
   - CPU usage
   - Memory usage
   - Network throughput

### Alerting Rules

Create `/etc/prometheus/rules/pingora-slice.yml`:

```yaml
groups:
  - name: pingora_slice_alerts
    rules:
      - alert: LowCacheHitRate
        expr: cache_hit_rate < 50
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "Low cache hit rate"
          description: "Cache hit rate is {{ $value }}% (threshold: 50%)"

      - alert: HighFragmentation
        expr: raw_disk_fragmentation_rate > 0.4
        for: 15m
        labels:
          severity: warning
        annotations:
          summary: "High disk fragmentation"
          description: "Fragmentation rate is {{ $value }} (threshold: 0.4)"

      - alert: CacheAlmostFull
        expr: (raw_disk_used_blocks / raw_disk_total_blocks) > 0.9
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Cache almost full"
          description: "Cache usage is {{ $value | humanizePercentage }}"

      - alert: ServiceDown
        expr: up{job="pingora-slice"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Pingora Slice service is down"
          description: "Service has been down for more than 1 minute"
```

## Performance Tuning

### L1 Cache Sizing

```yaml
# Small deployments (< 1000 req/s)
l1_cache_size_bytes: 52428800  # 50 MB

# Medium deployments (1000-10000 req/s)
l1_cache_size_bytes: 104857600  # 100 MB

# Large deployments (> 10000 req/s)
l1_cache_size_bytes: 524288000  # 500 MB
```

### Raw Disk Cache Tuning

#### Block Size Selection

```yaml
# Small files (< 100 KB average)
block_size: 2048  # 2 KB

# Medium files (100 KB - 10 MB average)
block_size: 4096  # 4 KB (default)

# Large files (> 10 MB average)
block_size: 8192  # 8 KB
```

#### O_DIRECT Configuration

```yaml
# Enable for dedicated cache storage
use_direct_io: true

# Disable for shared storage or NFS
use_direct_io: false
```

#### Compression Settings

```yaml
# Enable for text content (HTML, CSS, JS, JSON)
enable_compression: true

# Disable for already compressed content (JPEG, PNG, MP4, ZIP)
enable_compression: false
```

### System Tuning

#### Kernel Parameters

Add to `/etc/sysctl.conf`:

```bash
# Network tuning
net.core.somaxconn = 65535
net.ipv4.tcp_max_syn_backlog = 8192
net.ipv4.tcp_tw_reuse = 1
net.ipv4.ip_local_port_range = 1024 65535

# File descriptor limits
fs.file-max = 2097152

# Memory management
vm.swappiness = 10
vm.dirty_ratio = 15
vm.dirty_background_ratio = 5

# Apply changes
sudo sysctl -p
```

#### File Descriptor Limits

Add to `/etc/security/limits.conf`:

```
pingora-slice soft nofile 65536
pingora-slice hard nofile 65536
pingora-slice soft nproc 4096
pingora-slice hard nproc 4096
```

## Security

### Firewall Configuration

```bash
# UFW (Ubuntu/Debian)
sudo ufw allow 8080/tcp comment 'Pingora Slice Proxy'
sudo ufw allow from 10.0.0.0/8 to any port 8081 comment 'Health Check (internal)'
sudo ufw allow from 10.0.0.0/8 to any port 9090 comment 'Metrics (internal)'
sudo ufw enable

# firewalld (CentOS/RHEL)
sudo firewall-cmd --permanent --add-port=8080/tcp
sudo firewall-cmd --permanent --add-rich-rule='rule family="ipv4" source address="10.0.0.0/8" port port="8081" protocol="tcp" accept'
sudo firewall-cmd --permanent --add-rich-rule='rule family="ipv4" source address="10.0.0.0/8" port port="9090" protocol="tcp" accept'
sudo firewall-cmd --reload
```

### TLS/SSL Configuration

Use a reverse proxy (Nginx or HAProxy) for TLS termination:

```nginx
# /etc/nginx/sites-available/pingora-slice
upstream pingora_backend {
    server 127.0.0.1:8080;
    keepalive 32;
}

server {
    listen 443 ssl http2;
    server_name cdn.example.com;

    ssl_certificate /etc/ssl/certs/cdn.example.com.crt;
    ssl_certificate_key /etc/ssl/private/cdn.example.com.key;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;

    location / {
        proxy_pass http://pingora_backend;
        proxy_http_version 1.1;
        proxy_set_header Connection "";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # Disable buffering for streaming
        proxy_buffering off;
        proxy_request_buffering off;
    }
}
```

### File Permissions

```bash
# Configuration files
sudo chmod 640 /etc/pingora-slice/config.yaml
sudo chown root:pingora-slice /etc/pingora-slice/config.yaml

# Cache files
sudo chmod 700 /var/cache/pingora-slice
sudo chmod 600 /var/cache/pingora-slice-raw

# Log files
sudo chmod 755 /var/log/pingora-slice
```

## High Availability

### Multi-Instance Deployment

#### Instance Configuration

Run multiple instances on different ports:

```yaml
# Instance 1: /etc/pingora-slice/config-1.yaml
# (Use port 8080)

# Instance 2: /etc/pingora-slice/config-2.yaml
# (Use port 8081)

# Instance 3: /etc/pingora-slice/config-3.yaml
# (Use port 8082)
```

#### Load Balancer Configuration (HAProxy)

```
# /etc/haproxy/haproxy.cfg
frontend http_front
    bind *:80
    default_backend pingora_backend

backend pingora_backend
    balance roundrobin
    option httpchk GET /health
    http-check expect status 200
    server slice1 127.0.0.1:8080 check inter 5s fall 3 rise 2
    server slice2 127.0.0.1:8081 check inter 5s fall 3 rise 2
    server slice3 127.0.0.1:8082 check inter 5s fall 3 rise 2
```

### Kubernetes Deployment

#### Deployment Manifest

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: pingora-slice
  labels:
    app: pingora-slice
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
        image: your-registry/pingora-slice:0.2.3
        ports:
        - containerPort: 8080
          name: http
        - containerPort: 8081
          name: health
        - containerPort: 9090
          name: metrics
        env:
        - name: RUST_LOG
          value: "info"
        livenessProbe:
          httpGet:
            path: /live
            port: 8081
          initialDelaySeconds: 10
          periodSeconds: 10
          timeoutSeconds: 5
          failureThreshold: 3
        readinessProbe:
          httpGet:
            path: /ready
            port: 8081
          initialDelaySeconds: 5
          periodSeconds: 5
          timeoutSeconds: 3
          failureThreshold: 2
        resources:
          requests:
            memory: "2Gi"
            cpu: "1000m"
          limits:
            memory: "4Gi"
            cpu: "2000m"
        volumeMounts:
        - name: config
          mountPath: /etc/pingora-slice
          readOnly: true
        - name: cache
          mountPath: /var/cache/pingora-slice
      volumes:
      - name: config
        configMap:
          name: pingora-slice-config
      - name: cache
        emptyDir:
          sizeLimit: 10Gi
```

#### Service Manifest

```yaml
apiVersion: v1
kind: Service
metadata:
  name: pingora-slice
  labels:
    app: pingora-slice
spec:
  type: ClusterIP
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

#### ConfigMap

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

## Troubleshooting

### Service Won't Start

```bash
# Check service status
sudo systemctl status pingora-slice

# View detailed logs
sudo journalctl -u pingora-slice -n 100 --no-pager

# Test configuration
sudo -u pingora-slice /usr/local/bin/pingora-slice --check-config /etc/pingora-slice/config.yaml

# Check file permissions
ls -la /etc/pingora-slice/
ls -la /var/cache/pingora-slice/

# Check port availability
sudo ss -tlnp | grep -E '8080|8081|9090'
```

### Cache Not Working

```bash
# Check cache statistics
curl http://localhost:9090/metrics | grep cache

# Verify cache directory
ls -la /var/cache/pingora-slice/

# Check disk space
df -h /var/cache

# Review logs for cache errors
sudo journalctl -u pingora-slice | grep -i cache
```

### High Memory Usage

```bash
# Monitor memory usage
watch -n 1 'ps aux | grep pingora-slice'

# Check L1 cache size
grep l1_cache_size_bytes /etc/pingora-slice/config.yaml

# Reduce L1 cache size if needed
sudo vi /etc/pingora-slice/config.yaml
# Set: l1_cache_size_bytes: 52428800  # 50 MB

# Restart service
sudo systemctl restart pingora-slice
```

### Performance Issues

```bash
# Check system resources
top
htop
iostat -x 1

# Check network connections
ss -s
netstat -an | grep 8080 | wc -l

# Review metrics
curl http://localhost:9090/metrics

# Enable debug logging temporarily
sudo systemctl stop pingora-slice
sudo -u pingora-slice RUST_LOG=debug /usr/local/bin/pingora-slice /etc/pingora-slice/config.yaml
```

### Raw Disk Cache Issues

```bash
# Check raw disk cache file
ls -lh /var/cache/pingora-slice-raw

# Check fragmentation
curl http://localhost:9090/metrics | grep fragmentation

# Check disk I/O
iostat -x 1 10

# Recreate cache file if corrupted
sudo systemctl stop pingora-slice
sudo rm /var/cache/pingora-slice-raw
sudo fallocate -l 100G /var/cache/pingora-slice-raw
sudo chown pingora-slice:pingora-slice /var/cache/pingora-slice-raw
sudo systemctl start pingora-slice
```

## Maintenance

### Updating the Service

```bash
# 1. Download new version
wget https://github.com/your-org/pingora-slice/releases/download/v0.2.4/pingora-slice-0.2.4-linux-x86_64.tar.gz

# 2. Extract
tar -xzf pingora-slice-0.2.4-linux-x86_64.tar.gz

# 3. Backup current binary
sudo cp /usr/local/bin/pingora-slice /usr/local/bin/pingora-slice.backup

# 4. Stop service
sudo systemctl stop pingora-slice

# 5. Install new binary
sudo cp pingora-slice /usr/local/bin/
sudo chmod +x /usr/local/bin/pingora-slice

# 6. Start service
sudo systemctl start pingora-slice

# 7. Verify
sudo systemctl status pingora-slice
curl http://localhost:8081/health

# 8. Rollback if needed
# sudo cp /usr/local/bin/pingora-slice.backup /usr/local/bin/pingora-slice
# sudo systemctl restart pingora-slice
```

### Cache Maintenance

```bash
# View cache statistics
curl http://localhost:9090/metrics | grep -E '(cache|raw_disk)'

# Clear cache (if needed)
sudo systemctl stop pingora-slice
sudo rm -rf /var/cache/pingora-slice/*
sudo rm /var/cache/pingora-slice-raw
sudo fallocate -l 100G /var/cache/pingora-slice-raw
sudo chown pingora-slice:pingora-slice /var/cache/pingora-slice-raw
sudo systemctl start pingora-slice

# Monitor cache growth
watch -n 5 'du -sh /var/cache/pingora-slice*'
```

### Log Management

```bash
# View recent logs
sudo journalctl -u pingora-slice -n 100

# View logs since specific time
sudo journalctl -u pingora-slice --since "1 hour ago"

# Follow logs in real-time
sudo journalctl -u pingora-slice -f

# Export logs
sudo journalctl -u pingora-slice --since "2024-01-01" > pingora-slice.log

# Clear old journal logs
sudo journalctl --vacuum-time=7d
sudo journalctl --vacuum-size=1G
```

### Backup and Recovery

```bash
# Backup configuration
sudo tar -czf /backup/pingora-slice-config-$(date +%Y%m%d).tar.gz \
    /etc/pingora-slice/ \
    /etc/systemd/system/pingora-slice.service

# Backup cache (optional, large)
sudo tar -czf /backup/pingora-slice-cache-$(date +%Y%m%d).tar.gz \
    /var/cache/pingora-slice/

# Restore configuration
sudo tar -xzf /backup/pingora-slice-config-20240101.tar.gz -C /

# Restore cache
sudo tar -xzf /backup/pingora-slice-cache-20240101.tar.gz -C /
sudo chown -R pingora-slice:pingora-slice /var/cache/pingora-slice/
```

## Best Practices

### 1. Capacity Planning

- Monitor cache hit rate (target: >80%)
- Plan for 20% free space in raw disk cache
- Size L1 cache for hot data (typically 50-500 MB)
- Size L2 cache based on content size and hit rate target

### 2. Performance Optimization

- Use raw disk cache for production deployments
- Enable O_DIRECT for dedicated cache storage
- Enable compression for text content
- Tune block size based on average file size
- Monitor and address fragmentation (target: <20%)

### 3. Reliability

- Deploy multiple instances behind load balancer
- Configure health checks in load balancer
- Set up monitoring and alerting
- Implement automated failover
- Regular backups of configuration

### 4. Security

- Run service as dedicated user
- Restrict metrics endpoint to internal network
- Use TLS termination at reverse proxy
- Keep software up to date
- Regular security audits

### 5. Monitoring

- Track cache hit rate
- Monitor fragmentation
- Alert on service downtime
- Monitor resource usage (CPU, memory, disk)
- Track error rates

## Quick Reference

### Service Commands

```bash
# Start/stop/restart
sudo systemctl start|stop|restart pingora-slice

# View status
sudo systemctl status pingora-slice

# View logs
sudo journalctl -u pingora-slice -f

# Reload configuration
sudo systemctl reload pingora-slice
```

### Health Checks

```bash
# Health status
curl http://localhost:8081/health

# Readiness
curl http://localhost:8081/ready

# Liveness
curl http://localhost:8081/live
```

### Metrics

```bash
# All metrics
curl http://localhost:9090/metrics

# Cache metrics
curl http://localhost:9090/metrics | grep cache

# Raw disk metrics
curl http://localhost:9090/metrics | grep raw_disk
```

## Support and Resources

### Documentation

- [Streaming Proxy Overview](STREAMING_PROXY.md)
- [Configuration Guide](STREAMING_PROXY_CONFIG.md)
- [Performance Analysis](STREAMING_PROXY_PERFORMANCE.md)
- [Error Handling](STREAMING_PROXY_ERROR_HANDLING.md)
- [Quick Start Guide](STREAMING_PROXY_QUICK_START.md)

### Getting Help

- **GitHub Issues**: https://github.com/your-org/pingora-slice/issues
- **Documentation**: https://github.com/your-org/pingora-slice/tree/main/docs
- **Logs**: `sudo journalctl -u pingora-slice -f`
- **Metrics**: `curl http://localhost:9090/metrics`

## Appendix

### Configuration Template

See [examples/pingora_slice_raw_disk_full.yaml](../examples/pingora_slice_raw_disk_full.yaml) for a complete configuration template with all available options.

### Example Deployments

- **Development**: File-based cache, 1 GB, single instance
- **Small Production**: Raw disk cache, 10 GB, 2 instances
- **Large Production**: Raw disk cache, 100 GB, 3+ instances, load balanced

### Performance Benchmarks

See [STREAMING_PROXY_PERFORMANCE.md](STREAMING_PROXY_PERFORMANCE.md) for detailed performance analysis and benchmarks.
