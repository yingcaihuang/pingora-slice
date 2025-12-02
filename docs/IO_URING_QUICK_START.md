# io_uring Quick Start Guide

## Prerequisites

- Linux kernel 5.1 or later
- Rust 1.70 or later
- tokio-uring crate (automatically included)

## Installation

The io_uring support is included in the pingora-slice crate. No additional installation is required.

```toml
[dependencies]
pingora-slice = "0.2.3"
```

## Basic Usage

### 1. Create a Cache with io_uring

```rust
use pingora_slice::raw_disk::{RawDiskCache, IoUringConfig};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use default configuration
    let config = IoUringConfig::default();
    
    let cache = RawDiskCache::new_with_io_uring(
        "/tmp/cache.dat",
        100 * 1024 * 1024,  // 100MB
        4096,                // 4KB blocks
        Duration::from_secs(3600),  // 1 hour TTL
        config,
    ).await?;
    
    Ok(())
}
```

### 2. Store Data

```rust
use bytes::Bytes;

let key = "my_key";
let data = Bytes::from("Hello, io_uring!");

cache.store_with_io_uring(key, data).await?;
```

### 3. Lookup Data

```rust
let result = cache.lookup_with_io_uring(key).await?;

if let Some(data) = result {
    println!("Found: {}", String::from_utf8_lossy(&data));
}
```

### 4. Batch Operations

```rust
let keys = vec![
    "key1".to_string(),
    "key2".to_string(),
    "key3".to_string(),
];

let results = cache.lookup_batch(&keys).await?;

for (key, result) in keys.iter().zip(results.iter()) {
    if let Some(data) = result {
        println!("{}: {} bytes", key, data.len());
    }
}
```

## Configuration Examples

### Default (Recommended)

```rust
let config = IoUringConfig::default();
// queue_depth: 128
// use_sqpoll: false
// use_iopoll: false
// block_size: 4096
```

### High Throughput

```rust
let config = IoUringConfig {
    queue_depth: 1024,
    use_sqpoll: false,
    use_iopoll: true,  // For NVMe
    block_size: 4096,
};
```

### Low Latency

```rust
let config = IoUringConfig {
    queue_depth: 64,
    use_sqpoll: true,  // Requires privileges
    use_iopoll: true,
    block_size: 4096,
};
```

### Memory Constrained

```rust
let config = IoUringConfig {
    queue_depth: 32,
    use_sqpoll: false,
    use_iopoll: false,
    block_size: 4096,
};
```

## Complete Example

```rust
use pingora_slice::raw_disk::{RawDiskCache, IoUringConfig};
use bytes::Bytes;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Configure io_uring
    let config = IoUringConfig {
        queue_depth: 256,
        use_sqpoll: false,
        use_iopoll: false,
        block_size: 4096,
    };
    
    // Create cache
    let cache = RawDiskCache::new_with_io_uring(
        "/tmp/my_cache.dat",
        100 * 1024 * 1024,
        4096,
        Duration::from_secs(3600),
        config,
    ).await?;
    
    // Store some data
    for i in 0..10 {
        let key = format!("key_{}", i);
        let data = Bytes::from(format!("Data for entry {}", i));
        cache.store_with_io_uring(&key, data).await?;
    }
    
    // Lookup data
    let result = cache.lookup_with_io_uring("key_5").await?;
    println!("Found: {:?}", result);
    
    // Batch lookup
    let keys: Vec<String> = (0..10).map(|i| format!("key_{}", i)).collect();
    let results = cache.lookup_batch(&keys).await?;
    println!("Retrieved {} entries", results.iter().filter(|r| r.is_some()).count());
    
    // Get statistics
    let stats = cache.stats().await;
    println!("Cache entries: {}", stats.entries);
    println!("Cache hits: {}", stats.hits);
    println!("Cache misses: {}", stats.misses);
    
    Ok(())
}
```

## Running the Example

```bash
# Development
cargo run --example io_uring_example

# Release (for performance testing)
cargo run --example io_uring_example --release
```

## Testing

```bash
# Run all io_uring tests
cargo test --test test_io_uring

# Run with output
cargo test --test test_io_uring -- --nocapture
```

## Common Patterns

### Error Handling

```rust
match cache.store_with_io_uring(key, data).await {
    Ok(()) => println!("Stored successfully"),
    Err(e) => eprintln!("Store failed: {}", e),
}
```

### Conditional io_uring Usage

```rust
#[cfg(target_os = "linux")]
{
    // Use io_uring on Linux
    cache.store_with_io_uring(key, data).await?;
}

#[cfg(not(target_os = "linux"))]
{
    // Fall back to standard I/O on other platforms
    cache.store(key, data).await?;
}
```

### Performance Monitoring

```rust
use std::time::Instant;

let start = Instant::now();
cache.store_with_io_uring(key, data).await?;
let duration = start.elapsed();

println!("Store took: {:?}", duration);
```

## Troubleshooting

### "io_uring is only supported on Linux"

You're running on a non-Linux platform. Use standard I/O instead:

```rust
let cache = RawDiskCache::new(
    path,
    size,
    block_size,
    ttl,
).await?;
```

### "Permission denied" with SQPOLL

SQPOLL requires elevated privileges. Either:

1. Run with sudo
2. Add CAP_SYS_NICE capability
3. Disable SQPOLL: `use_sqpoll: false`

### Poor Performance

1. Check queue depth - try increasing it
2. Enable IOPOLL for NVMe devices
3. Use batch operations
4. Monitor with `cache.stats().await`

## Next Steps

- Read the [Implementation Guide](IO_URING_IMPLEMENTATION.md)
- Check the [Tuning Guide](IO_URING_TUNING.md)
- Review the [Summary](IO_URING_SUMMARY.md)

## Support

For issues or questions:
- Check the documentation in `docs/`
- Review the example in `examples/io_uring_example.rs`
- Run the tests in `tests/test_io_uring.rs`
