# Raw Disk Cache Migration Guide

## Overview

This guide provides step-by-step instructions for migrating between file-based and raw disk cache backends in production environments.

## Table of Contents

1. [Pre-Migration Checklist](#pre-migration-checklist)
2. [File to Raw Disk Migration](#file-to-raw-disk-migration)
3. [Raw Disk to File Migration](#raw-disk-to-file-migration)
4. [Rollback Procedures](#rollback-procedures)
5. [Validation](#validation)
6. [Troubleshooting](#troubleshooting)

## Pre-Migration Checklist

### Planning

- [ ] Determine target cache size
- [ ] Calculate required disk space (target size + 20% buffer)
- [ ] Choose appropriate block size for workload
- [ ] Schedule maintenance window
- [ ] Notify stakeholders
- [ ] Prepare rollback plan

### Infrastructure

- [ ] Verify disk space availability
- [ ] Check disk I/O performance
- [ ] Ensure backup storage if needed
- [ ] Test configuration in staging environment
- [ ] Document current cache statistics

### Monitoring

- [ ] Set up monitoring for new backend
- [ ] Configure alerts for cache metrics
- [ ] Prepare dashboard for migration tracking
- [ ] Test metrics collection

## File to Raw Disk Migration

### Step 1: Preparation

#### 1.1 Collect Current Statistics

```bash
# Get current cache statistics
curl http://localhost:9090/stats > pre-migration-stats.json

# Check current cache size
du -sh /var/cache/pingora-slice

# Monitor current hit rate
watch -n 5 'curl -s http://localhost:9090/stats | jq ".l1_hits, .l2_hits, .misses"'
```

#### 1.2 Calculate Required Size

```bash
# Current cache usage
CURRENT_SIZE=$(du -sb /var/cache/pingora-slice | cut -f1)

# Add 20% buffer
REQUIRED_SIZE=$(echo "$CURRENT_SIZE * 1.2" | bc | cut -d. -f1)

echo "Current size: $CURRENT_SIZE bytes"
echo "Required size: $REQUIRED_SIZE bytes"
```

#### 1.3 Create Raw Disk Cache File

```bash
# Create cache file
sudo fallocate -l ${REQUIRED_SIZE} /var/cache/pingora-slice-raw

# Set permissions
sudo chown pingora:pingora /var/cache/pingora-slice-raw
sudo chmod 600 /var/cache/pingora-slice-raw

# Verify
ls -lh /var/cache/pingora-slice-raw
```

### Step 2: Configuration Update

#### 2.1 Backup Current Configuration

```bash
# Backup configuration
sudo cp /etc/pingora-slice/config.yaml /etc/pingora-slice/config.yaml.backup

# Backup with timestamp
sudo cp /etc/pingora-slice/config.yaml \
  /etc/pingora-slice/config.yaml.$(date +%Y%m%d_%H%M%S)
```

#### 2.2 Update Configuration

Edit `/etc/pingora-slice/config.yaml`:

```yaml
# Change from:
l2_backend: "file"
l2_cache_dir: "/var/cache/pingora-slice"

# To:
l2_backend: "raw_disk"
raw_disk_cache:
  device_path: "/var/cache/pingora-slice-raw"
  total_size: 10737418240  # Adjust to your calculated size
  block_size: 4096
  use_direct_io: true
  enable_compression: true
  enable_prefetch: true
  enable_zero_copy: true
```

#### 2.3 Validate Configuration

```bash
# Validate YAML syntax
yamllint /etc/pingora-slice/config.yaml

# Test configuration (if supported)
pingora-slice --config /etc/pingora-slice/config.yaml --validate
```

### Step 3: Migration Execution

#### 3.1 Graceful Shutdown

```bash
# Stop accepting new connections
# (implementation depends on your load balancer)

# Wait for in-flight requests to complete
sleep 30

# Stop service
sudo systemctl stop pingora-slice

# Verify stopped
sudo systemctl status pingora-slice
```

#### 3.2 Start with New Backend

```bash
# Start service
sudo systemctl start pingora-slice

# Check status
sudo systemctl status pingora-slice

# Monitor logs
sudo journalctl -u pingora-slice -f
```

#### 3.3 Verify Startup

Look for these log messages:

```
✓ "Initializing raw disk cache backend"
✓ "Two-tier cache initialized"
✓ "L2 (raw_disk): /var/cache/pingora-slice-raw"
✓ "Total size: X GB"
✓ "Block size: X KB"
```

### Step 4: Validation

#### 4.1 Functional Testing

```bash
# Test cache write
curl -X POST http://localhost:8080/test-file

# Test cache read
curl http://localhost:8080/test-file

# Check statistics
curl http://localhost:9090/stats
```

#### 4.2 Performance Monitoring

```bash
# Monitor hit rate
watch -n 5 'curl -s http://localhost:9090/stats | \
  jq "{l1_hits, l2_hits, misses, hit_rate: (.l1_hits + .l2_hits) / (.l1_hits + .l2_hits + .misses)}"'

# Monitor latency
# (use your monitoring tool)

# Check raw disk stats
curl http://localhost:9090/stats | jq '.raw_disk_stats'
```

#### 4.3 Warmup Period

```bash
# Monitor cache warmup
watch -n 60 'curl -s http://localhost:9090/stats | \
  jq "{entries: .raw_disk_stats.entries, used_blocks: .raw_disk_stats.used_blocks}"'

# Expected: Entries should gradually increase
# Typical warmup: 1-4 hours depending on traffic
```

### Step 5: Cleanup (Optional)

After successful migration and warmup:

```bash
# Archive old cache (optional)
sudo tar -czf /backup/pingora-cache-$(date +%Y%m%d).tar.gz \
  /var/cache/pingora-slice

# Remove old cache
sudo rm -rf /var/cache/pingora-slice

# Remove old configuration backup (after verification period)
sudo rm /etc/pingora-slice/config.yaml.backup
```

## Raw Disk to File Migration

### Step 1: Preparation

#### 1.1 Collect Statistics

```bash
# Get raw disk statistics
curl http://localhost:9090/stats | jq '.raw_disk_stats' > pre-migration-raw-stats.json

# Calculate required file cache size
ENTRIES=$(curl -s http://localhost:9090/stats | jq '.raw_disk_stats.entries')
AVG_SIZE=100000  # Adjust based on your data
REQUIRED_SIZE=$(echo "$ENTRIES * $AVG_SIZE * 1.2" | bc | cut -d. -f1)

echo "Estimated required size: $REQUIRED_SIZE bytes"
```

#### 1.2 Prepare File Cache Directory

```bash
# Create directory
sudo mkdir -p /var/cache/pingora-slice

# Set permissions
sudo chown pingora:pingora /var/cache/pingora-slice
sudo chmod 755 /var/cache/pingora-slice

# Verify disk space
df -h /var/cache
```

### Step 2: Configuration Update

#### 2.1 Backup Configuration

```bash
sudo cp /etc/pingora-slice/config.yaml /etc/pingora-slice/config.yaml.backup
```

#### 2.2 Update Configuration

Edit `/etc/pingora-slice/config.yaml`:

```yaml
# Change from:
l2_backend: "raw_disk"
raw_disk_cache:
  device_path: "/var/cache/pingora-slice-raw"
  # ... other settings ...

# To:
l2_backend: "file"
l2_cache_dir: "/var/cache/pingora-slice"
```

### Step 3: Migration Execution

```bash
# Stop service
sudo systemctl stop pingora-slice

# Start service with new configuration
sudo systemctl start pingora-slice

# Verify
sudo systemctl status pingora-slice
sudo journalctl -u pingora-slice -f
```

### Step 4: Validation

```bash
# Check file cache is being used
ls -lh /var/cache/pingora-slice

# Monitor cache population
watch -n 60 'du -sh /var/cache/pingora-slice'

# Check statistics
curl http://localhost:9090/stats
```

### Step 5: Cleanup (Optional)

```bash
# After successful migration
sudo rm /var/cache/pingora-slice-raw
```

## Rollback Procedures

### Rollback from Raw Disk to File

If issues occur during file-to-raw-disk migration:

```bash
# 1. Stop service
sudo systemctl stop pingora-slice

# 2. Restore configuration
sudo cp /etc/pingora-slice/config.yaml.backup \
  /etc/pingora-slice/config.yaml

# 3. Start service
sudo systemctl start pingora-slice

# 4. Verify
sudo systemctl status pingora-slice
curl http://localhost:9090/stats
```

### Rollback from File to Raw Disk

If issues occur during raw-disk-to-file migration:

```bash
# 1. Stop service
sudo systemctl stop pingora-slice

# 2. Restore configuration
sudo cp /etc/pingora-slice/config.yaml.backup \
  /etc/pingora-slice/config.yaml

# 3. Verify raw disk file exists
ls -lh /var/cache/pingora-slice-raw

# 4. Start service
sudo systemctl start pingora-slice

# 5. Verify
sudo systemctl status pingora-slice
curl http://localhost:9090/stats | jq '.raw_disk_stats'
```

## Validation

### Health Checks

```bash
# Service status
sudo systemctl is-active pingora-slice

# Cache backend
curl -s http://localhost:9090/stats | jq '.l2_backend'

# Cache statistics
curl -s http://localhost:9090/stats | jq '{
  l1_entries,
  l1_hits,
  l2_hits,
  misses,
  hit_rate: ((.l1_hits + .l2_hits) / (.l1_hits + .l2_hits + .misses))
}'
```

### Performance Validation

```bash
# Latency check
curl -w "@curl-format.txt" -o /dev/null -s http://localhost:8080/test-file

# Where curl-format.txt contains:
# time_total: %{time_total}s
# time_connect: %{time_connect}s
# time_starttransfer: %{time_starttransfer}s

# Throughput check
ab -n 1000 -c 10 http://localhost:8080/test-file
```

### Comparison Metrics

| Metric | Before | After | Change |
|--------|--------|-------|--------|
| Hit Rate | X% | Y% | ±Z% |
| Avg Latency | Xms | Yms | ±Zms |
| P95 Latency | Xms | Yms | ±Zms |
| Throughput | X req/s | Y req/s | ±Z% |

## Troubleshooting

### Issue: Service Won't Start

**Symptoms:**
```
Failed to start pingora-slice.service
```

**Solution:**
```bash
# Check logs
sudo journalctl -u pingora-slice -n 50

# Common issues:
# 1. Configuration error
sudo pingora-slice --config /etc/pingora-slice/config.yaml --validate

# 2. Permission error
sudo chown pingora:pingora /var/cache/pingora-slice-raw
sudo chmod 600 /var/cache/pingora-slice-raw

# 3. Disk space
df -h /var/cache

# 4. File doesn't exist
ls -lh /var/cache/pingora-slice-raw
```

### Issue: Poor Performance After Migration

**Symptoms:**
- High latency
- Low hit rate
- High CPU usage

**Solution:**
```bash
# 1. Check cache warmup status
curl http://localhost:9090/stats | jq '.raw_disk_stats.entries'

# 2. Monitor disk I/O
iostat -x 1

# 3. Check fragmentation
curl http://localhost:9090/stats | jq '.raw_disk_stats.fragmentation_ratio'

# 4. Adjust configuration
# - Disable O_DIRECT if high latency
# - Increase block size if large files
# - Disable compression if CPU-bound
```

### Issue: High Memory Usage

**Symptoms:**
- OOM errors
- High memory pressure

**Solution:**
```bash
# 1. Check L1 cache size
curl http://localhost:9090/stats | jq '.l1_bytes'

# 2. Reduce L1 cache size
# Edit config.yaml:
l1_cache_size_bytes: 52428800  # Reduce to 50MB

# 3. Restart service
sudo systemctl restart pingora-slice
```

### Issue: Cache Not Persisting

**Symptoms:**
- Empty cache after restart
- L2 hits always 0

**Solution:**
```bash
# 1. Check metadata save on shutdown
sudo journalctl -u pingora-slice | grep "metadata"

# 2. Verify file permissions
ls -lh /var/cache/pingora-slice-raw

# 3. Check disk space
df -h /var/cache

# 4. Manually trigger metadata save (if supported)
# In code: cache.save_metadata().await?
```

## Best Practices

### 1. Test in Staging First

Always test migration in staging environment:
- Same configuration
- Similar traffic patterns
- Monitor for 24-48 hours

### 2. Gradual Rollout

For large deployments:
1. Migrate one instance
2. Monitor for 24 hours
3. Migrate 10% of instances
4. Monitor for 48 hours
5. Complete migration

### 3. Monitoring During Migration

Monitor these metrics closely:
- Hit rate (should recover within 1-4 hours)
- Latency (should be similar or better)
- Error rate (should remain low)
- Disk I/O (should be within limits)

### 4. Maintenance Window

Schedule migration during:
- Low traffic periods
- With sufficient time for rollback
- With team availability

### 5. Documentation

Document:
- Migration date and time
- Configuration changes
- Issues encountered
- Performance comparison
- Lessons learned

## Post-Migration

### Week 1

- [ ] Monitor hit rate daily
- [ ] Check fragmentation
- [ ] Review error logs
- [ ] Validate performance metrics
- [ ] Collect feedback

### Week 2-4

- [ ] Monitor hit rate weekly
- [ ] Run defragmentation if needed
- [ ] Optimize configuration
- [ ] Update documentation
- [ ] Plan cleanup

### Month 1+

- [ ] Review capacity planning
- [ ] Optimize block size if needed
- [ ] Consider compression tuning
- [ ] Update runbooks
- [ ] Share learnings

## Conclusion

Successful migration requires:
- Thorough planning
- Proper testing
- Careful execution
- Continuous monitoring
- Quick rollback capability

Follow this guide to ensure smooth migration between cache backends.
