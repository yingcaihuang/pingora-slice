# TTL (Time-To-Live) Support

## Overview

The raw disk cache now supports automatic expiration of cached entries based on Time-To-Live (TTL). This feature helps manage cache freshness and automatically removes stale data.

## Features

### 1. Configurable TTL

When creating a cache, you can specify a TTL duration:

```rust
use std::time::Duration;
use pingora_slice::raw_disk::RawDiskCache;

// Create cache with 1 hour TTL
let cache = RawDiskCache::new(
    "/path/to/cache",
    10 * 1024 * 1024,  // 10MB
    4096,              // 4KB blocks
    Duration::from_secs(3600),  // 1 hour TTL
).await?;
```

### 2. Automatic Expiration on Lookup

When you lookup an entry, the cache automatically checks if it has expired:

```rust
// If the entry is expired, lookup returns None and removes it
let result = cache.lookup("key").await?;
if result.is_none() {
    println!("Entry not found or expired");
}
```

This works for all lookup methods:
- `lookup()` - Standard lookup
- `lookup_zero_copy()` - Zero-copy lookup with mmap
- `lookup_with_io_uring()` - io_uring-based lookup (Linux only)

### 3. Manual Cleanup

You can manually trigger cleanup of all expired entries:

```rust
// Remove all expired entries
let removed_count = cache.cleanup_expired().await?;
println!("Removed {} expired entries", removed_count);
```

Or run cleanup in the background:

```rust
// Non-blocking cleanup
cache.cleanup_expired_background().await;
```

### 4. Integration with Garbage Collection

The smart GC system prioritizes expired entries for eviction:

```rust
// GC will first remove expired entries, then use the configured strategy (LRU/LFU/FIFO)
let removed = cache.run_smart_gc().await?;
```

## Implementation Details

### Timestamp Storage

Each cache entry stores a Unix timestamp when it's created:

```rust
pub struct DiskLocation {
    pub offset: u64,
    pub size: u32,
    pub checksum: u32,
    pub timestamp: u64,  // Unix timestamp in seconds
}
```

### Expiration Check

The `is_expired()` method checks if an entry has exceeded its TTL:

```rust
impl DiskLocation {
    pub fn is_expired(&self, ttl_secs: u64) -> bool {
        // TTL of 0 means no expiration
        if ttl_secs == 0 {
            return false;
        }
        
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        now - self.timestamp > ttl_secs
    }
}
```

### GC Priority

When GC runs, it follows this priority:

1. **Expired entries** - Removed first if TTL is configured
2. **Strategy-based selection** - LRU/LFU/FIFO for remaining evictions

```rust
// GC selects victims in this order:
// 1. Expired entries (if TTL > 0)
// 2. LRU/LFU/FIFO victims (if more evictions needed)
let victims = gc.select_victims(&directory, target_count, block_size);
```

## Usage Examples

### Basic TTL Usage

```rust
use bytes::Bytes;
use std::time::Duration;
use pingora_slice::raw_disk::RawDiskCache;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create cache with 5 second TTL
    let cache = RawDiskCache::new(
        "/tmp/cache",
        10 * 1024 * 1024,
        4096,
        Duration::from_secs(5),
    ).await?;

    // Store data
    cache.store("key", Bytes::from("data")).await?;

    // Lookup immediately - succeeds
    assert!(cache.lookup("key").await?.is_some());

    // Wait for expiration
    tokio::time::sleep(Duration::from_secs(6)).await;

    // Lookup after expiration - returns None
    assert!(cache.lookup("key").await?.is_none());

    Ok(())
}
```

### Periodic Cleanup

```rust
use std::time::Duration;
use tokio::time::interval;

// Run cleanup every 5 minutes
let mut cleanup_interval = interval(Duration::from_secs(300));

loop {
    cleanup_interval.tick().await;
    
    let removed = cache.cleanup_expired().await?;
    if removed > 0 {
        println!("Cleaned up {} expired entries", removed);
    }
}
```

### TTL with Different Strategies

```rust
use pingora_slice::raw_disk::{GCConfig, EvictionStrategy};

// Configure GC with TTL
let mut gc_config = GCConfig::default();
gc_config.ttl_secs = 3600;  // 1 hour
gc_config.strategy = EvictionStrategy::LRU;

cache.update_gc_config(gc_config).await;

// GC will prioritize expired entries, then use LRU
cache.run_smart_gc().await?;
```

## Disabling TTL

To disable TTL-based expiration, set TTL to 0:

```rust
// No expiration
let cache = RawDiskCache::new(
    "/tmp/cache",
    10 * 1024 * 1024,
    4096,
    Duration::from_secs(0),  // TTL disabled
).await?;
```

## Performance Considerations

### Lookup Performance

- Expiration check adds minimal overhead (single timestamp comparison)
- Expired entries are removed lazily on lookup
- No background scanning unless explicitly triggered

### Cleanup Performance

- `cleanup_expired()` scans all entries once
- Removal is done in batches to avoid blocking
- Background cleanup is non-blocking

### Memory Usage

- Each entry stores an 8-byte timestamp
- No additional memory overhead for TTL tracking

## Best Practices

1. **Choose appropriate TTL**: Balance between freshness and cache hit rate
2. **Periodic cleanup**: Run `cleanup_expired()` periodically to free space proactively
3. **Monitor metrics**: Check GC metrics to see how many expired entries are being removed
4. **Combine with GC**: Let GC handle expired entries automatically during space pressure

## Testing

Comprehensive tests are available in `tests/test_ttl.rs`:

```bash
cargo test --test test_ttl
```

Tests cover:
- Basic expiration on lookup
- Manual cleanup
- Partial cleanup (mixed expired/valid entries)
- TTL disabled (0 seconds)
- GC prioritization of expired entries
- Zero-copy lookup with TTL
- Batch lookup with TTL
- Disk space reclamation

## Example

Run the TTL example to see it in action:

```bash
cargo run --example ttl_example
```

This demonstrates:
- Creating a cache with TTL
- Automatic expiration on lookup
- Manual cleanup
- Integration with GC
