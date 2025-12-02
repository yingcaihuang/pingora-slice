# Enhanced Checksum Quick Start Guide

## Quick Setup

### 1. Basic Usage (Default XXHash3)

```rust
use pingora_slice::raw_disk::RawDiskCache;
use std::time::Duration;

// Create cache with default verification (XXHash3)
let cache = RawDiskCache::new(
    "/path/to/cache",
    100 * 1024 * 1024,  // 100MB
    4096,                // 4KB blocks
    Duration::from_secs(3600)  // 1 hour TTL
).await?;

// Store data (automatically uses XXHash3)
cache.store("key", data).await?;

// Verify a single entry
let is_valid = cache.verify_entry("key").await?;
```

### 2. Enable Periodic Verification

```rust
use pingora_slice::raw_disk::{RawDiskCache, VerificationConfig};

let mut cache = RawDiskCache::new(path, size, block_size, ttl).await?;

// Configure periodic verification
let mut config = VerificationConfig::default();
config.periodic_verification_enabled = true;
config.verification_interval_secs = 3600;  // Every hour
config.max_entries_per_run = 100;
config.auto_repair_enabled = true;

cache.update_verification_config(config);
cache.start_periodic_verification();
```

### 3. Manual Verification

```rust
// Verify all entries
let result = cache.verify_all_entries().await?;
println!("Verified: {}, Corrupted: {}, Repaired: {}", 
    result.verified, result.corrupted, result.repaired);

// Get statistics
let stats = cache.verification_stats().await;
println!("Corruption rate: {:.4}%", stats.corruption_rate() * 100.0);
```

### 4. Choose Different Algorithm

```rust
use pingora_slice::raw_disk::{ChecksumAlgorithm, VerificationConfig};

let mut config = VerificationConfig::default();
config.algorithm = ChecksumAlgorithm::XxHash64;  // or Crc32, XxHash3

cache.update_verification_config(config);
```

## Common Patterns

### Pattern 1: High-Reliability Cache

```rust
// Enable all verification features
let mut config = VerificationConfig::default();
config.algorithm = ChecksumAlgorithm::XxHash3;
config.periodic_verification_enabled = true;
config.verification_interval_secs = 1800;  // 30 minutes
config.max_entries_per_run = 200;
config.auto_repair_enabled = true;
config.keep_backup_on_repair = true;

cache.update_verification_config(config);
cache.start_periodic_verification();
```

### Pattern 2: Performance-Optimized

```rust
// Minimal overhead, manual verification only
let mut config = VerificationConfig::default();
config.algorithm = ChecksumAlgorithm::XxHash3;  // Fastest
config.periodic_verification_enabled = false;   // No background tasks

cache.update_verification_config(config);

// Verify during maintenance windows
tokio::spawn(async move {
    loop {
        tokio::time::sleep(Duration::from_secs(86400)).await;  // Daily
        let _ = cache.verify_all_entries().await;
    }
});
```

### Pattern 3: Legacy Compatibility

```rust
// Use CRC32 for backward compatibility
let mut config = VerificationConfig::default();
config.algorithm = ChecksumAlgorithm::Crc32;

cache.update_verification_config(config);
```

## Monitoring

### Check Verification Status

```rust
let stats = cache.verification_stats().await;
println!("Total runs: {}", stats.total_runs);
println!("Total verified: {}", stats.total_verified);
println!("Corrupted found: {}", stats.corrupted_found);
println!("Repaired: {}", stats.repaired);
println!("Corruption rate: {:.4}%", stats.corruption_rate() * 100.0);
println!("Repair success: {:.2}%", stats.repair_success_rate() * 100.0);
```

### Include in Cache Stats

```rust
let cache_stats = cache.stats().await;
if let Some(ver_stats) = cache_stats.verification_stats {
    println!("Verification runs: {}", ver_stats.total_runs);
    println!("Entries verified: {}", ver_stats.total_verified);
}
```

## Troubleshooting

### High Corruption Rate

```rust
// Check corruption rate
let stats = cache.verification_stats().await;
if stats.corruption_rate() > 0.001 {  // > 0.1%
    eprintln!("WARNING: High corruption rate detected!");
    
    // Enable more frequent verification
    let mut config = cache.verification_config().clone();
    config.verification_interval_secs = 600;  // Every 10 minutes
    cache.update_verification_config(config);
}
```

### Repair Failures

```rust
// Check repair success rate
let stats = cache.verification_stats().await;
if stats.repair_success_rate() < 0.5 {  // < 50%
    eprintln!("WARNING: Low repair success rate!");
    
    // Ensure backups are enabled
    let mut config = cache.verification_config().clone();
    config.keep_backup_on_repair = true;
    cache.update_verification_config(config);
}
```

## Performance Tips

1. **Choose the Right Algorithm**:
   - XXHash3: Best for most use cases (default)
   - XXHash64: Good balance
   - CRC32: Only for legacy compatibility

2. **Tune Verification Frequency**:
   - Critical data: 30-60 minutes
   - Normal data: 1-6 hours
   - Cold data: Daily or weekly

3. **Limit Batch Size**:
   - High traffic: 50-100 entries per run
   - Low traffic: 200-500 entries per run

4. **Schedule Verification**:
   - Run during off-peak hours
   - Avoid overlapping with GC or defragmentation

## Examples

See `examples/enhanced_checksum_example.rs` for a complete working example.

## Testing

```bash
# Run all checksum tests
cargo test --test test_enhanced_checksum

# Run the example
cargo run --example enhanced_checksum_example
```

## API Reference

See `docs/ENHANCED_CHECKSUM.md` for complete API documentation.
