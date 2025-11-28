# Pingora Slice Module - Deployment Guide

[English](DEPLOYMENT.md) | [中文](DEPLOYMENT_zh.md)

This guide provides comprehensive instructions for deploying the Pingora Slice Module in production environments.

## Table of Contents

- [System Requirements](#system-requirements)
- [Pre-Deployment Checklist](#pre-deployment-checklist)
- [Installation](#installation)
- [Configuration](#configuration)
- [Service Management](#service-management)
- [Reverse Proxy Setup](#reverse-proxy-setup)
- [Security Hardening](#security-hardening)
- [Monitoring and Logging](#monitoring-and-logging)
- [Backup and Recovery](#backup-and-recovery)
- [Scaling and High Availability](#scaling-and-high-availability)
- [Maintenance](#maintenance)
- [Troubleshooting](#troubleshooting)

## System Requirements

### Minimum Requirements

- **CPU**: 2 cores
- **RAM**: 2 GB
- **Disk**: 10 GB (plus cache storage)
- **OS**: Linux (Ubuntu 20.04+, CentOS 8+, Debian 11+)
- **Network**: 100 Mbps

### Recommended Requirements

- **CPU**: 4+ cores
- **RAM**: 8+ GB
- **Disk**: 50+ GB SSD (for cache)
- **OS**: Linux (Ubuntu 22.04 LTS recommended)
- **Network**: 1 Gbps+

### Software Dependencies

- **Rust**: 1.70 or later
- **Cargo**: Included with Rust
- **systemd**: For service management
- **OpenSSL**: For HTTPS support (optional)

## Pre-Deployment Checklist

Before deploying, ensure you have:

- [ ] Compiled release binary (`cargo build --release`)
- [ ] Tested configuration file
- [ ] Identified upstream origin server address
- [ ] Determined slice size and concurrency settings
- [ ] Planned cache storage location and size
- [ ] Configured firewall rules
- [ ] Set up monitoring infrastructure
- [ ] Prepared backup strategy
- [ ] Documented rollback procedure

## Installation

### Step 1: Build Release Binary

```bash
# On build server or locally
git clone <repository-url>
cd pingora-slice

# Build optimized release binary
cargo build --release

# Verify binary
./target/release/pingora-slice --version

# Run tests to ensure correctness
cargo test --release
```

### Step 2: Create Deployment User

```bash
# Create dedicated user for running the service
sudo useradd -r -s /bin/false -d /opt/pingora-slice pingora-slice

# Create home directory
sudo mkdir -p /opt/pingora-slice
sudo chown pingora-slice:pingora-slice /opt/pingora-slice
```

### Step 3: Install Binary and Configuration

```bash
# Copy binary to deployment location
sudo cp target/release/pingora-slice /opt/pingora-slice/
sudo chown pingora-slice:pingora-slice /opt/pingora-slice/pingora-slice
sudo chmod 755 /opt/pingora-slice/pingora-slice

# Copy configuration file
sudo cp pingora_slice.yaml /opt/pingora-slice/
sudo chown pingora-slice:pingora-slice /opt/pingora-slice/pingora_slice.yaml
sudo chmod 644 /opt/pingora-slice/pingora_slice.yaml

# Create cache directory
sudo mkdir -p /var/cache/pingora-slice
sudo chown pingora-slice:pingora-slice /var/cache/pingora-slice
sudo chmod 755 /var/cache/pingora-slice

# Create log directory
sudo mkdir -p /var/log/pingora-slice
sudo chown pingora-slice:pingora-slice /var/log/pingora-slice
sudo chmod 755 /var/log/pingora-slice
```

### Step 4: Verify Installation

```bash
# Test configuration
sudo -u pingora-slice /opt/pingora-slice/pingora-slice --check-config

# Test binary execution (should start and be killable)
sudo -u pingora-slice /opt/pingora-slice/pingora-slice &
sleep 2
sudo pkill pingora-slice
```

## Configuration

### Production Configuration Template

Create `/opt/pingora-slice/pingora_slice.yaml`:

```yaml
# Production configuration for Pingora Slice Module

# Slice size: 1MB for balanced performance
slice_size: 1048576

# Concurrency: 4 concurrent subrequests
max_concurrent_subrequests: 4

# Retries: 3 attempts with exponential backoff
max_retries: 3

# URL patterns: Adjust based on your content
slice_patterns:
  - "^/downloads/.*"
  - "^/files/.*\\.(bin|iso|zip|tar\\.gz)$"

# Cache: Enabled with 1 hour TTL
enable_cache: true
cache_ttl: 3600

# Upstream: Your origin server
upstream_address: "origin.example.com:80"

# Metrics: Enabled on localhost only
metrics_endpoint:
  enabled: true
  address: "127.0.0.1:9090"
```

### Environment-Specific Configurations

#### Development
```yaml
slice_size: 524288  # 512KB for faster testing
max_concurrent_subrequests: 2
max_retries: 1
cache_ttl: 300  # 5 minutes
```

#### Staging
```yaml
slice_size: 1048576  # 1MB
max_concurrent_subrequests: 4
max_retries: 3
cache_ttl: 1800  # 30 minutes
```

#### Production
```yaml
slice_size: 2097152  # 2MB for high performance
max_concurrent_subrequests: 8
max_retries: 3
cache_ttl: 7200  # 2 hours
```

## Service Management

### Systemd Service Configuration

Create `/etc/systemd/system/pingora-slice.service`:

```ini
[Unit]
Description=Pingora Slice Module
After=network.target
Wants=network-online.target

[Service]
Type=simple
User=pingora-slice
Group=pingora-slice
WorkingDirectory=/opt/pingora-slice
ExecStart=/opt/pingora-slice/pingora-slice /opt/pingora-slice/pingora_slice.yaml
ExecReload=/bin/kill -HUP $MAINPID
Restart=on-failure
RestartSec=5s
StandardOutput=journal
StandardError=journal
SyslogIdentifier=pingora-slice

# Security settings
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/cache/pingora-slice /var/log/pingora-slice

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
# Reload systemd configuration
sudo systemctl daemon-reload

# Enable service to start on boot
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

# Reload configuration (if supported)
sudo systemctl reload pingora-slice
```

## Reverse Proxy Setup

### Nginx as Reverse Proxy

Create `/etc/nginx/sites-available/pingora-slice`:

```nginx
upstream pingora_slice {
    server 127.0.0.1:8080;
    keepalive 32;
}

server {
    listen 80;
    server_name cdn.example.com;

    # Redirect to HTTPS
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name cdn.example.com;

    # SSL configuration
    ssl_certificate /etc/ssl/certs/cdn.example.com.crt;
    ssl_certificate_key /etc/ssl/private/cdn.example.com.key;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;
    ssl_prefer_server_ciphers on;

    # Logging
    access_log /var/log/nginx/pingora-slice-access.log;
    error_log /var/log/nginx/pingora-slice-error.log;

    # Proxy settings
    location / {
        proxy_pass http://pingora_slice;
        proxy_http_version 1.1;
        proxy_set_header Connection "";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # Timeouts
        proxy_connect_timeout 60s;
        proxy_send_timeout 60s;
        proxy_read_timeout 300s;

        # Buffering
        proxy_buffering off;
        proxy_request_buffering off;
    }

    # Metrics endpoint (restrict access)
    location /metrics {
        proxy_pass http://127.0.0.1:9090/metrics;
        allow 10.0.0.0/8;  # Internal network only
        deny all;
    }

    # Health check
    location /health {
        proxy_pass http://127.0.0.1:9090/health;
        access_log off;
    }
}
```

Enable the site:

```bash
sudo ln -s /etc/nginx/sites-available/pingora-slice /etc/nginx/sites-enabled/
sudo nginx -t
sudo systemctl reload nginx
```

### HAProxy as Load Balancer

Create `/etc/haproxy/haproxy.cfg`:

```
global
    log /dev/log local0
    log /dev/log local1 notice
    chroot /var/lib/haproxy
    stats socket /run/haproxy/admin.sock mode 660 level admin
    stats timeout 30s
    user haproxy
    group haproxy
    daemon

defaults
    log     global
    mode    http
    option  httplog
    option  dontlognull
    timeout connect 5000
    timeout client  300000
    timeout server  300000

frontend http_front
    bind *:80
    default_backend pingora_slice_backend

backend pingora_slice_backend
    balance roundrobin
    option httpchk GET /health
    http-check expect status 200
    server slice1 127.0.0.1:8080 check
    server slice2 127.0.0.1:8081 check
    server slice3 127.0.0.1:8082 check
```

## Security Hardening

### Firewall Configuration

```bash
# UFW (Ubuntu)
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp
sudo ufw allow from 10.0.0.0/8 to any port 9090  # Metrics (internal only)
sudo ufw enable

# firewalld (CentOS/RHEL)
sudo firewall-cmd --permanent --add-service=http
sudo firewall-cmd --permanent --add-service=https
sudo firewall-cmd --permanent --add-rich-rule='rule family="ipv4" source address="10.0.0.0/8" port port="9090" protocol="tcp" accept'
sudo firewall-cmd --reload
```

### File Permissions

```bash
# Ensure correct ownership
sudo chown -R pingora-slice:pingora-slice /opt/pingora-slice
sudo chown -R pingora-slice:pingora-slice /var/cache/pingora-slice
sudo chown -R pingora-slice:pingora-slice /var/log/pingora-slice

# Restrict permissions
sudo chmod 755 /opt/pingora-slice
sudo chmod 755 /opt/pingora-slice/pingora-slice
sudo chmod 644 /opt/pingora-slice/pingora_slice.yaml
sudo chmod 700 /var/cache/pingora-slice
sudo chmod 755 /var/log/pingora-slice
```

### SELinux Configuration (CentOS/RHEL)

```bash
# Set SELinux context
sudo semanage fcontext -a -t bin_t "/opt/pingora-slice/pingora-slice"
sudo restorecon -v /opt/pingora-slice/pingora-slice

# Allow network connections
sudo setsebool -P httpd_can_network_connect 1
```

### Rate Limiting

Configure rate limiting in Nginx:

```nginx
# In http block
limit_req_zone $binary_remote_addr zone=slice_limit:10m rate=10r/s;

# In location block
location / {
    limit_req zone=slice_limit burst=20 nodelay;
    proxy_pass http://pingora_slice;
}
```

## Monitoring and Logging

### Prometheus Configuration

Add to `/etc/prometheus/prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'pingora-slice'
    static_configs:
      - targets: ['localhost:9090']
    scrape_interval: 15s
    scrape_timeout: 10s
```

### Grafana Dashboard

Import the provided Grafana dashboard or create panels for:

1. **Request Rate**
   - Query: `rate(pingora_slice_requests_total[5m])`
   - Type: Graph

2. **Cache Hit Rate**
   - Query: `pingora_slice_cache_hit_rate`
   - Type: Gauge

3. **Subrequest Failure Rate**
   - Query: `pingora_slice_subrequest_failure_rate`
   - Type: Gauge

4. **Bandwidth Usage**
   - Query: `rate(pingora_slice_bytes_to_client_total[5m])`
   - Type: Graph

5. **Latency**
   - Query: `pingora_slice_request_duration_ms_avg`
   - Type: Graph

### Log Rotation

Create `/etc/logrotate.d/pingora-slice`:

```
/var/log/pingora-slice/*.log {
    daily
    rotate 14
    compress
    delaycompress
    notifempty
    create 0644 pingora-slice pingora-slice
    sharedscripts
    postrotate
        systemctl reload pingora-slice > /dev/null 2>&1 || true
    endscript
}
```

### Alerting Rules

Create Prometheus alerting rules:

```yaml
groups:
  - name: pingora_slice
    rules:
      - alert: HighCacheMissRate
        expr: pingora_slice_cache_hit_rate < 50
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "High cache miss rate"
          description: "Cache hit rate is {{ $value }}%"

      - alert: HighSubrequestFailureRate
        expr: pingora_slice_subrequest_failure_rate > 5
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "High subrequest failure rate"
          description: "Failure rate is {{ $value }}%"

      - alert: ServiceDown
        expr: up{job="pingora-slice"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Pingora Slice service is down"
```

## Backup and Recovery

### Configuration Backup

```bash
# Create backup script
cat > /opt/pingora-slice/backup.sh << 'EOF'
#!/bin/bash
BACKUP_DIR="/var/backups/pingora-slice"
DATE=$(date +%Y%m%d_%H%M%S)

mkdir -p $BACKUP_DIR
tar -czf $BACKUP_DIR/config_$DATE.tar.gz \
    /opt/pingora-slice/pingora_slice.yaml \
    /etc/systemd/system/pingora-slice.service

# Keep only last 30 days
find $BACKUP_DIR -name "config_*.tar.gz" -mtime +30 -delete
EOF

chmod +x /opt/pingora-slice/backup.sh

# Add to crontab
echo "0 2 * * * /opt/pingora-slice/backup.sh" | sudo crontab -u root -
```

### Cache Backup (Optional)

```bash
# Backup cache directory
sudo tar -czf /var/backups/pingora-slice/cache_$(date +%Y%m%d).tar.gz \
    /var/cache/pingora-slice/

# Restore cache
sudo tar -xzf /var/backups/pingora-slice/cache_20240101.tar.gz -C /
```

### Disaster Recovery

1. **Backup critical files:**
   - Configuration: `/opt/pingora-slice/pingora_slice.yaml`
   - Service file: `/etc/systemd/system/pingora-slice.service`
   - Binary: `/opt/pingora-slice/pingora-slice`

2. **Recovery procedure:**
   ```bash
   # Restore from backup
   sudo tar -xzf config_backup.tar.gz -C /
   
   # Reload systemd
   sudo systemctl daemon-reload
   
   # Start service
   sudo systemctl start pingora-slice
   
   # Verify
   sudo systemctl status pingora-slice
   curl http://localhost:9090/health
   ```

## Scaling and High Availability

### Horizontal Scaling

Run multiple instances behind a load balancer:

```bash
# Instance 1
/opt/pingora-slice/pingora-slice --port 8080

# Instance 2
/opt/pingora-slice/pingora-slice --port 8081

# Instance 3
/opt/pingora-slice/pingora-slice --port 8082
```

Configure load balancer (HAProxy, Nginx, etc.) to distribute traffic.

### Shared Cache

For multiple instances, use a shared cache backend:
- Redis
- Memcached
- Distributed file system (NFS, GlusterFS)

### Health Checks

Configure health checks in load balancer:

```
# HAProxy
option httpchk GET /health
http-check expect status 200

# Nginx
upstream pingora_slice {
    server 127.0.0.1:8080 max_fails=3 fail_timeout=30s;
    server 127.0.0.1:8081 max_fails=3 fail_timeout=30s;
}
```

## Maintenance

### Updating the Service

```bash
# 1. Build new version
cargo build --release

# 2. Stop service
sudo systemctl stop pingora-slice

# 3. Backup current binary
sudo cp /opt/pingora-slice/pingora-slice /opt/pingora-slice/pingora-slice.backup

# 4. Deploy new binary
sudo cp target/release/pingora-slice /opt/pingora-slice/
sudo chown pingora-slice:pingora-slice /opt/pingora-slice/pingora-slice

# 5. Start service
sudo systemctl start pingora-slice

# 6. Verify
sudo systemctl status pingora-slice
curl http://localhost:9090/health

# 7. If issues, rollback
# sudo cp /opt/pingora-slice/pingora-slice.backup /opt/pingora-slice/pingora-slice
# sudo systemctl restart pingora-slice
```

### Cache Maintenance

```bash
# Clear cache
sudo rm -rf /var/cache/pingora-slice/*

# Check cache size
du -sh /var/cache/pingora-slice/

# Set cache size limit (in systemd service)
# Add to [Service] section:
# ReadWritePaths=/var/cache/pingora-slice
# LimitFSIZE=10G
```

### Log Maintenance

```bash
# View recent logs
sudo journalctl -u pingora-slice -n 100

# View logs since specific time
sudo journalctl -u pingora-slice --since "1 hour ago"

# Follow logs in real-time
sudo journalctl -u pingora-slice -f

# Clear old journal logs
sudo journalctl --vacuum-time=7d
```

## Troubleshooting

### Service Won't Start

```bash
# Check service status
sudo systemctl status pingora-slice

# View detailed logs
sudo journalctl -u pingora-slice -n 50 --no-pager

# Test configuration
sudo -u pingora-slice /opt/pingora-slice/pingora-slice --check-config

# Check file permissions
ls -la /opt/pingora-slice/
ls -la /var/cache/pingora-slice/

# Check port availability
sudo lsof -i :8080
sudo lsof -i :9090
```

### High Memory Usage

```bash
# Monitor memory usage
watch -n 1 'ps aux | grep pingora-slice'

# Check for memory leaks
valgrind --leak-check=full /opt/pingora-slice/pingora-slice

# Restart service to clear memory
sudo systemctl restart pingora-slice
```

### Performance Issues

```bash
# Check system resources
top
htop
iostat -x 1

# Check network connections
netstat -an | grep 8080
ss -s

# Review metrics
curl http://localhost:9090/metrics | grep -E "(cache_hit_rate|failure_rate|duration)"

# Enable debug logging temporarily
sudo systemctl stop pingora-slice
sudo -u pingora-slice RUST_LOG=debug /opt/pingora-slice/pingora-slice
```

### Connection Issues

```bash
# Test connectivity to origin
curl -I http://origin.example.com/test-file

# Test proxy
curl -v http://localhost:8080/test-file

# Check firewall
sudo iptables -L -n
sudo ufw status verbose

# Check DNS resolution
nslookup origin.example.com
dig origin.example.com
```

## Support

For additional support:
- Review logs: `sudo journalctl -u pingora-slice -f`
- Check metrics: `curl http://localhost:9090/metrics`
- Consult documentation: See README.md and docs/
- Report issues: GitHub issues with logs and configuration

## Appendix

### Quick Reference Commands

```bash
# Service management
sudo systemctl start|stop|restart|status pingora-slice

# View logs
sudo journalctl -u pingora-slice -f

# Check metrics
curl http://localhost:9090/metrics

# Test configuration
sudo -u pingora-slice /opt/pingora-slice/pingora-slice --check-config

# Check health
curl http://localhost:9090/health
```

### Configuration Checklist

- [ ] Slice size appropriate for file sizes
- [ ] Concurrency limit set based on origin capacity
- [ ] Cache TTL configured for content update frequency
- [ ] URL patterns match target content
- [ ] Upstream address correct and reachable
- [ ] Metrics endpoint enabled and accessible
- [ ] Firewall rules configured
- [ ] Service enabled to start on boot
- [ ] Monitoring and alerting configured
- [ ] Backup strategy in place
