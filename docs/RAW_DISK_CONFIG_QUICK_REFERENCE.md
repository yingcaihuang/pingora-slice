# Raw Disk Cache Configuration Quick Reference

## Minimal Configuration

```yaml
enable_l2_cache: true
l2_backend: "raw_disk"

raw_disk_cache:
  device_path: "/var/cache/pingora-slice-raw"
  total_size: 10737418240  # 10GB
  block_size: 4096
```

## Full Configuration

```yaml
enable_l2_cache: true
l2_backend: "raw_disk"

raw_disk_cache:
  device_path: "/var/cache/pingora-slice-raw"
  total_size: 10737418240
  block_size: 4096
  use_direct_io: true
  enable_compression: true
  enable_prefetch: true
  enable_zero_copy: true
```

## Configuration Parameters

| Parameter | Type | Required | Default | Valid Range |
|-----------|------|----------|---------|-------------|
| `device_path` | string | Yes | `/var/cache/pingora-slice-raw` | Non-empty path |
| `total_size` | u64 | Yes | 10GB | ≥ 1MB |
| `block_size` | usize | Yes | 4096 | 512B - 1MB (power of 2) |
| `use_direct_io` | bool | No | true | true/false |
| `enable_compression` | bool | No | true | true/false |
| `enable_prefetch` | bool | No | true | true/false |
| `enable_zero_copy` | bool | No | true | true/false |

## Validation Rules

### Device Path
- ✅ `/var/cache/pingora-slice-raw`
- ✅ `/dev/sdb`
- ❌ `` (empty)

### Total Size
- ✅ `1048576` (1MB minimum)
- ✅ `10737418240` (10GB)
- ❌ `512000` (< 1MB)
- ❌ Must be ≥ 10 × block_size

### Block Size
- ✅ `512`, `1024`, `2048`, `4096`, `8192`, `16384`, `32768`, `65536`, `1048576`
- ❌ `3000` (not power of 2)
- ❌ `256` (< 512)
- ❌ `2097152` (> 1MB)

## Common Configurations

### Small Deployment (1GB)
```yaml
raw_disk_cache:
  device_path: "/var/cache/pingora-slice-raw"
  total_size: 1073741824
  block_size: 4096
```

### Medium Deployment (10GB)
```yaml
raw_disk_cache:
  device_path: "/var/cache/pingora-slice-raw"
  total_size: 10737418240
  block_size: 8192
```

### Large Deployment (100GB)
```yaml
raw_disk_cache:
  device_path: "/dev/nvme0n1p1"
  total_size: 107374182400
  block_size: 16384
```

### Development (100MB)
```yaml
raw_disk_cache:
  device_path: "/tmp/pingora-cache"
  total_size: 104857600
  block_size: 4096
  use_direct_io: false
```

## Hot Reload

### Check for Changes
```rust
let changes = config.reload_from_file("config.yaml")?;
if changes.has_changes() {
    println!("Changed: {:?}", changes.summary());
}
```

### Detect Restart Requirement
```rust
if changes.requires_cache_restart() {
    // Reinitialize cache
}
```

## Changes Requiring Restart

- ✅ `cache_ttl`
- ✅ `l1_cache_size_bytes`
- ✅ `l2_cache_dir`
- ✅ `enable_l2_cache`
- ✅ `l2_backend`
- ✅ All `raw_disk_cache.*` parameters

## Changes Applied Immediately

- ✅ `slice_size`
- ✅ `max_concurrent_subrequests`
- ✅ `max_retries`
- ✅ `slice_patterns`
- ✅ `upstream_address`

## Error Messages

| Error | Cause | Solution |
|-------|-------|----------|
| `device_path must not be empty` | Empty path | Provide valid path |
| `total_size must be at least 1MB` | Size too small | Increase to ≥ 1MB |
| `block_size must be a power of 2` | Invalid block size | Use 512, 1024, 2048, 4096, etc. |
| `total_size must be at least 10x block_size` | Size mismatch | Increase total_size or decrease block_size |
| `raw_disk_cache configuration is required` | Missing config | Add raw_disk_cache section |

## See Also

- [Full Configuration Guide](RAW_DISK_CONFIGURATION.md)
- [Implementation Details](RAW_DISK_CONFIG_IMPLEMENTATION.md)
- [Quick Start Guide](RAW_DISK_QUICK_START.md)
