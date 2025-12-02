# Raw Disk Cache Configuration Management Implementation

## Overview

This document describes the implementation of configuration management for the raw disk cache, including validation and hot reload capabilities.

## Implementation Summary

### 1. Configuration Structure

Added comprehensive configuration support for raw disk cache in `src/config.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawDiskCacheConfig {
    pub device_path: String,
    pub total_size: u64,
    pub block_size: usize,
    pub use_direct_io: bool,
    pub enable_compression: bool,
    pub enable_prefetch: bool,
    pub enable_zero_copy: bool,
}
```

### 2. Configuration Validation

Implemented comprehensive validation for all configuration parameters:

#### Device Path Validation
- Must not be empty
- Can be a block device or regular file path

#### Total Size Validation
- Minimum: 1MB (1,048,576 bytes)
- Must be at least 10x the block size

#### Block Size Validation
- Must be a power of 2
- Range: 512 bytes to 1MB
- Common values: 4KB, 8KB, 16KB, 32KB, 64KB

#### L2 Backend Validation
- Validates backend type ("file" or "raw_disk")
- Ensures raw_disk_cache config exists when backend is "raw_disk"
- Validates raw_disk_cache configuration

### 3. Hot Reload Support

Implemented hot reload functionality with change tracking:

```rust
pub struct ConfigChanges {
    pub slice_size_changed: bool,
    pub max_concurrent_changed: bool,
    pub cache_ttl_changed: bool,
    pub raw_disk_config_changed: bool,
    // ... other fields
}
```

#### Key Features:
- **Change Detection**: Tracks which configuration parameters changed
- **Validation**: Validates new configuration before applying
- **Rollback**: Keeps current config if validation fails
- **Restart Detection**: Identifies changes requiring cache restart

#### API Methods:

```rust
// Update from another config
pub fn update_from(&mut self, new_config: &SliceConfig) -> Result<ConfigChanges>

// Reload from file
pub fn reload_from_file<P: AsRef<Path>>(&mut self, path: P) -> Result<ConfigChanges>
```

### 4. Configuration Changes Analysis

The `ConfigChanges` struct provides methods to analyze updates:

```rust
// Check if any changes occurred
pub fn has_changes(&self) -> bool

// Check if cache restart is required
pub fn requires_cache_restart(&self) -> bool

// Get list of changed parameters
pub fn summary(&self) -> Vec<String>
```

## Files Modified

### Core Implementation
- `src/config.rs` - Added validation and hot reload support

### Documentation
- `docs/RAW_DISK_CONFIGURATION.md` - Comprehensive configuration guide
- `docs/RAW_DISK_CONFIG_IMPLEMENTATION.md` - This implementation document

### Examples
- `examples/pingora_slice_raw_disk_full.yaml` - Full configuration example
- `examples/config_hot_reload_example.rs` - Hot reload demonstration

## Validation Rules

### 1. Device Path
```rust
if device_path.is_empty() {
    return Err("device_path must not be empty");
}
```

### 2. Total Size
```rust
const MIN_TOTAL_SIZE: u64 = 1024 * 1024; // 1MB
if total_size < MIN_TOTAL_SIZE {
    return Err("total_size must be at least 1MB");
}
```

### 3. Block Size
```rust
const MIN_BLOCK_SIZE: usize = 512;
const MAX_BLOCK_SIZE: usize = 1024 * 1024;

if block_size < MIN_BLOCK_SIZE || block_size > MAX_BLOCK_SIZE {
    return Err("block_size must be between 512 bytes and 1MB");
}

if !block_size.is_power_of_two() {
    return Err("block_size must be a power of 2");
}
```

### 4. Size Relationship
```rust
let min_total_for_blocks = (block_size as u64) * 10;
if total_size < min_total_for_blocks {
    return Err("total_size must be at least 10x block_size");
}
```

## Hot Reload Workflow

```
┌─────────────────────────────────────────────────────────────┐
│ 1. Load New Configuration from File                         │
│    - Read YAML file                                          │
│    - Parse into SliceConfig                                  │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│ 2. Validate New Configuration                                │
│    - Check all validation rules                              │
│    - Verify raw_disk_cache if backend is "raw_disk"          │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│ 3. Compare with Current Configuration                        │
│    - Detect changed parameters                               │
│    - Build ConfigChanges struct                              │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│ 4. Apply Changes                                             │
│    - Update configuration fields                             │
│    - Return ConfigChanges for analysis                       │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│ 5. Handle Changes                                            │
│    - Check if cache restart needed                           │
│    - Apply immediate changes                                 │
│    - Schedule restart if required                            │
└─────────────────────────────────────────────────────────────┘
```

## Testing

### Unit Tests

Comprehensive test coverage in `src/config.rs`:

1. **Raw Disk Config Validation Tests**
   - `test_raw_disk_config_validation` - Tests all validation rules
   - Valid and invalid configurations

2. **L2 Backend Validation Tests**
   - `test_l2_backend_validation` - Tests backend type validation
   - Tests raw_disk_cache requirement

3. **Hot Reload Tests**
   - `test_config_hot_reload` - Tests change detection
   - `test_config_hot_reload_validation` - Tests validation during reload
   - `test_config_changes_summary` - Tests change summary
   - `test_raw_disk_config_changes` - Tests raw disk config updates

### Integration Example

The `config_hot_reload_example.rs` demonstrates:
- Loading initial configuration
- Reloading with changes
- Detecting and handling changes
- Validation error handling
- Configuration rollback on error

## Usage Examples

### Basic Configuration

```yaml
enable_l2_cache: true
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

### Hot Reload in Code

```rust
use pingora_slice::config::SliceConfig;

// Load initial config
let mut config = SliceConfig::from_file("config.yaml")?;

// Later, reload configuration
match config.reload_from_file("config.yaml") {
    Ok(changes) => {
        if changes.has_changes() {
            println!("Configuration updated:");
            for change in changes.summary() {
                println!("  - {}", change);
            }
            
            if changes.requires_cache_restart() {
                println!("Warning: Cache restart required");
                // Reinitialize cache with new config
            }
        }
    }
    Err(e) => {
        eprintln!("Failed to reload config: {}", e);
        // Current config remains active
    }
}
```

## Configuration Changes Requiring Restart

The following changes require cache restart:

| Parameter | Reason |
|-----------|--------|
| `cache_ttl` | TTL is set during cache initialization |
| `l1_cache_size_bytes` | L1 cache size is fixed at creation |
| `l2_cache_dir` | L2 directory is set during initialization |
| `enable_l2_cache` | Requires cache backend initialization |
| `l2_backend` | Requires switching cache backend |
| `raw_disk_cache.*` | Raw disk cache parameters are set at creation |

## Configuration Changes Applied Immediately

The following changes take effect immediately:

| Parameter | Effect |
|-----------|--------|
| `slice_size` | Applied to new requests |
| `max_concurrent_subrequests` | Applied to new requests |
| `max_retries` | Applied to new requests |
| `slice_patterns` | Applied to new requests |
| `upstream_address` | Applied to new connections |
| `metrics_endpoint` | Can restart metrics server |
| `purge` | Can update purge handler |

## Error Handling

### Validation Errors

All validation errors return descriptive messages:

```rust
Err(SliceError::ConfigError(
    "raw_disk block_size must be a power of 2, got 3000"
))
```

### Reload Errors

If reload fails, the current configuration remains active:

```rust
match config.reload_from_file("config.yaml") {
    Ok(changes) => {
        // Apply changes
    }
    Err(e) => {
        // Current config still active
        eprintln!("Reload failed: {}", e);
    }
}
```

## Best Practices

### 1. Validate Before Deployment

Always validate configuration files before deploying:

```bash
# Test configuration loading
cargo run --example config_hot_reload_example
```

### 2. Monitor Configuration Changes

Log all configuration changes:

```rust
if changes.has_changes() {
    info!("Configuration updated: {:?}", changes.summary());
    if changes.requires_cache_restart() {
        warn!("Cache restart required for changes to take effect");
    }
}
```

### 3. Plan for Restarts

Schedule cache restarts during low-traffic periods when configuration changes require restart.

### 4. Use Version Control

Keep configuration files in version control to track changes and enable rollback.

### 5. Test in Staging

Always test configuration changes in a staging environment before production.

## Future Enhancements

Potential improvements for future versions:

1. **Gradual Rollout**: Apply changes gradually to minimize impact
2. **Configuration Profiles**: Support multiple configuration profiles
3. **Dynamic Tuning**: Auto-tune parameters based on workload
4. **Configuration API**: HTTP API for configuration management
5. **Configuration Validation Service**: Separate service to validate configs

## See Also

- [Raw Disk Cache Configuration Guide](RAW_DISK_CONFIGURATION.md)
- [Raw Disk Cache Design](RAW_DISK_CACHE_DESIGN.md)
- [Configuration Reference](CONFIGURATION.md)
