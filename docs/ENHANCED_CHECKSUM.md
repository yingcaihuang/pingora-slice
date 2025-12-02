# Enhanced Checksum and Data Verification

This document describes the enhanced checksum and data verification features in the raw disk cache.

## Overview

The enhanced checksum system provides:

1. **Multiple Checksum Algorithms**: Support for CRC32, XXHash64, and XXHash3
2. **Periodic Data Verification**: Automatic background verification of cache entries
3. **Automatic Repair**: Optional automatic repair of corrupted data
4. **Verification Statistics**: Detailed metrics on data integrity

## Checksum Algorithms

### CRC32 (Legacy)
- **Speed**: Fast
- **Collision Resistance**: Basic
- **Use Case**: Backward compatibility, simple error detection
- **Output**: 32-bit value

### XXHash64
- **Speed**: Very fast
- **Collision Resistance**: Good
- **Use Case**: General purpose, balanced performance
- **Output**: 64-bit value

### XXHash3 (Default)
- **Speed**: Fastest
- **Collision Resistance**: Excellent
- **Use Case**: High-performance applications, recommended for new deployments
- **Output**: 64-bit value

## Configuration

### Basic Configuration

```rust
use pingora_slice::raw_disk::{VerificationConfig, ChecksumAlgorithm};

let mut config = VerificationConfig::default();
config.algorithm = ChecksumAlgorithm::XxHash3;
config.periodic_verification_enabled = true;
config.verification_interval_secs = 3600; // 1 hour
config.max_entries_per_run = 100;
config.auto_repair_enabled = true;
config.keep_backup_on_repair = true;
```

### Configuration Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `algorithm` | `ChecksumAlgorithm` | `XxHash3` | Checksum algorithm to use |
| `periodic_verification_enabled` | `bool` | `false` | Enable automatic periodic verification |
| `verification_interval_secs` | `u64` | `3600` | Interval between verification runs (seconds) |
| `max_entries_per_run` | `usize` | `100` | Maximum entries to verify per run |
| `auto_repair_enabled` | `bool` | `false` | Enable automatic repair of corrupted data |
| `keep_backup_on_repair` | `bool` | `true` | Keep backup of data before repair |

## Usage

### Manual Verification

#### Verify a Single Entry

```rust
let is_valid = cache.verify_entry("my_key").await?;
if !is_valid {
    println!("Entry is corrupted!");
}
```

#### Verify All Entries

```rust
let result = cache.verify_all_entries().await?;
println!("Verified: {}", result.verified);
println!("Corrupted: {}", result.corrupted);
println!("Repaired: {}", result.repaired);
```

### Periodic Verification

#### Start Periodic Verification

```rust
// Update config to enable periodic verification
let mut config = VerificationConfig::default();
config.periodic_verification_enabled = true;
config.verification_interval_secs = 3600; // 1 hour
cache.update_verification_config(config);

// Start the background task
cache.start_periodic_verification();
```

#### Stop Periodic Verification

```rust
cache.stop_periodic_verification();
```

### Repair Corrupted Data

```rust
// Attempt to repair a corrupted entry
let repaired = cache.repair_entry("corrupted_key").await?;
if repaired {
    println!("Entry repaired successfully");
} else {
    println!("Repair failed - no backup available");
}
```

### Monitoring

#### Get Verification Statistics

```rust
let stats = cache.verification_stats().await;
println!("Total runs: {}", stats.total_runs);
println!("Total verified: {}", stats.total_verified);
println!("Corrupted found: {}", stats.corrupted_found);
println!("Repaired: {}", stats.repaired);
println!("Corruption rate: {:.4}%", stats.corruption_rate() * 100.0);
println!("Repair success rate: {:.2}%", stats.repair_success_rate() * 100.0);
```

#### Include in Cache Statistics

```rust
let cache_stats = cache.stats().await;
if let Some(ver_stats) = cache_stats.verification_stats {
    println!("Verification runs: {}", ver_stats.total_runs);
    println!("Entries verified: {}", ver_stats.total_verified);
}
```

## Performance Considerations

### Algorithm Performance

Benchmark results (approximate, varies by hardware):

| Algorithm | Speed (GB/s) | Collision Resistance | Recommended Use |
|-----------|--------------|---------------------|-----------------|
| CRC32 | ~1-2 | Basic | Legacy compatibility |
| XXHash64 | ~10-15 | Good | General purpose |
| XXHash3 | ~15-30 | Excellent | High performance |

### Verification Impact

- **Periodic Verification**: Runs in background, minimal impact on cache operations
- **Manual Verification**: Blocks until complete, use for maintenance windows
- **Max Entries Per Run**: Limits verification batch size to control resource usage

### Best Practices

1. **Choose the Right Algorithm**:
   - Use XXHash3 for new deployments (best performance and collision resistance)
   - Use CRC32 only for backward compatibility
   - Use XXHash64 for balanced performance

2. **Configure Periodic Verification**:
   - Enable for critical data
   - Set interval based on data importance (1-24 hours typical)
   - Limit max entries per run to avoid performance impact

3. **Enable Auto-Repair**:
   - Enable for non-critical data
   - Keep backups enabled for important data
   - Monitor repair statistics

4. **Monitor Corruption Rates**:
   - Track corruption rate over time
   - Investigate if rate exceeds 0.1%
   - Check hardware if rate is increasing

## Error Handling

### Verification Errors

```rust
match cache.verify_entry("my_key").await {
    Ok(true) => println!("Entry is valid"),
    Ok(false) => println!("Entry is corrupted"),
    Err(e) => eprintln!("Verification failed: {}", e),
}
```

### Repair Errors

```rust
match cache.repair_entry("corrupted_key").await {
    Ok(true) => println!("Repaired successfully"),
    Ok(false) => println!("No backup available"),
    Err(e) => eprintln!("Repair failed: {}", e),
}
```

## Migration from CRC32

### Backward Compatibility

The system maintains backward compatibility with CRC32 checksums:

```rust
use pingora_slice::raw_disk::Checksum;

// Convert from legacy CRC32
let legacy_crc32: u32 = 0x12345678;
let checksum = Checksum::from_crc32(legacy_crc32);

// Convert to legacy CRC32 (if needed)
let crc32_value = checksum.to_crc32();
```

### Migration Strategy

1. **Phase 1**: Deploy with XXHash3 enabled for new entries
2. **Phase 2**: Run verification to identify old CRC32 entries
3. **Phase 3**: Gradually re-write old entries with new checksums
4. **Phase 4**: Remove CRC32 support (optional)

## Examples

See `examples/enhanced_checksum_example.rs` for a complete working example.

## Testing

Run the enhanced checksum tests:

```bash
cargo test test_enhanced_checksum
```

Run all verification tests:

```bash
cargo test verification
```

## Troubleshooting

### High Corruption Rate

**Symptoms**: Corruption rate > 0.1%

**Possible Causes**:
- Hardware issues (disk, memory)
- Concurrent access without proper locking
- Power failures during writes

**Solutions**:
- Check hardware health
- Enable auto-repair
- Increase verification frequency
- Review application logs

### Repair Failures

**Symptoms**: High repair failure rate

**Possible Causes**:
- Backups not enabled
- Backup storage cleared
- Original data lost

**Solutions**:
- Enable `keep_backup_on_repair`
- Increase backup retention
- Implement external backup strategy

### Performance Impact

**Symptoms**: Slow cache operations during verification

**Possible Causes**:
- Too many entries verified per run
- Verification interval too short
- Slow disk I/O

**Solutions**:
- Reduce `max_entries_per_run`
- Increase `verification_interval_secs`
- Use faster storage
- Run verification during off-peak hours

## Future Enhancements

Planned improvements:

1. **Additional Algorithms**: SHA-256, BLAKE3
2. **Incremental Verification**: Verify only recently modified entries
3. **Distributed Verification**: Coordinate verification across multiple nodes
4. **Smart Repair**: Use redundancy or parity data for repair
5. **Compression-Aware Verification**: Verify compressed data without decompression

## References

- [XXHash Homepage](https://xxhash.com/)
- [CRC32 Wikipedia](https://en.wikipedia.org/wiki/Cyclic_redundancy_check)
- [Data Integrity Best Practices](https://en.wikipedia.org/wiki/Data_integrity)
