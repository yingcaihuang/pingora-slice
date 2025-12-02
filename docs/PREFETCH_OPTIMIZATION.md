# Prefetch Optimization

## Overview

The prefetch optimization feature reduces read latency by predicting and pre-loading data based on detected access patterns. This is particularly effective for sequential and temporal access patterns.

## Features

### Access Pattern Detection

The system automatically detects three types of access patterns:

1. **Sequential Access**: Keys are accessed in order (e.g., key_1, key_2, key_3, ...)
   - Detected when 70% or more of accesses show increasing offsets
   - Triggers prefetching of subsequent keys

2. **Temporal Access**: Same keys are accessed repeatedly
   - Detected when 50% or more of accesses are repeats
   - Triggers prefetching of frequently accessed keys

3. **Random Access**: No clear pattern
   - Default when neither sequential nor temporal patterns are detected
   - Minimal prefetching to avoid wasting resources

### Prefetch Cache

A separate in-memory cache stores prefetched data:
- Configurable size (default: 100 entries)
- LRU eviction policy
- Independent from main cache directory
- Tracks hit/miss statistics

### Prefetch Strategy

Based on the detected pattern:

- **Sequential**: Prefetch next N keys in order
- **Temporal**: Prefetch most frequently accessed keys
- **Random**: No prefetching

## Configuration

```rust
use pingora_slice::raw_disk::{PrefetchConfig, RawDiskCache};
use std::time::Duration;

let prefetch_config = PrefetchConfig {
    enabled: true,                    // Enable/disable prefetch
    max_prefetch_entries: 5,          // Max keys to prefetch per trigger
    cache_size: 100,                  // Prefetch cache size
    pattern_window_size: 20,          // History size for pattern detection
    sequential_threshold: 0.7,        // Threshold for sequential pattern (70%)
    temporal_threshold: 0.5,          // Threshold for temporal pattern (50%)
};

let cache = RawDiskCache::new_with_prefetch(
    "/path/to/cache",
    100 * 1024 * 1024,  // 100MB
    4096,               // 4KB blocks
    Duration::from_secs(3600),
    prefetch_config,
)
.await?;
```

## Usage

### Basic Usage

Prefetch works automatically once configured:

```rust
// Store data
for i in 0..100 {
    let key = format!("key_{}", i);
    let data = Bytes::from(format!("data_{}", i));
    cache.store(&key, data).await?;
}

// Access sequentially - prefetch will kick in
for i in 0..50 {
    let key = format!("key_{}", i);
    let result = cache.lookup(&key).await?;
    // Subsequent lookups may hit prefetch cache
}
```

### Monitoring

Check prefetch statistics:

```rust
// Get prefetch-specific stats
let prefetch_stats = cache.prefetch_stats().await;
println!("Prefetch cache size: {}", prefetch_stats.cache_size);
println!("Prefetch hits: {}", prefetch_stats.hits);
println!("Prefetch misses: {}", prefetch_stats.misses);
println!("Hit rate: {:.2}%", prefetch_stats.hit_rate * 100.0);

// Get current access pattern
let pattern = cache.access_pattern().await;
println!("Current pattern: {:?}", pattern);

// Get overall cache stats (includes prefetch)
let stats = cache.stats().await;
if let Some(prefetch_stats) = stats.prefetch_stats {
    println!("Prefetch stats: {:?}", prefetch_stats);
}
```

### Manual Control

Clear prefetch cache when needed:

```rust
// Clear prefetch cache (e.g., when access pattern changes)
cache.clear_prefetch_cache().await;
```

## Performance Characteristics

### Benefits

- **Reduced Latency**: Prefetched data is served from memory
- **Sequential Access**: 2-5x faster for sequential reads
- **Temporal Access**: Significant improvement for hot keys
- **Adaptive**: Automatically adjusts to access patterns

### Overhead

- **Memory**: Prefetch cache uses additional memory
- **CPU**: Pattern detection has minimal overhead
- **I/O**: Background prefetch may increase disk I/O
- **Accuracy**: Prefetch effectiveness depends on pattern predictability

### Tuning Guidelines

1. **cache_size**: 
   - Larger = more prefetched data, more memory
   - Start with 100, adjust based on working set size

2. **max_prefetch_entries**:
   - Higher = more aggressive prefetching
   - Start with 3-5, increase for strong sequential patterns

3. **pattern_window_size**:
   - Larger = more stable pattern detection
   - Smaller = faster adaptation to pattern changes
   - Default 20 works well for most cases

4. **Thresholds**:
   - Lower = more sensitive pattern detection
   - Higher = more conservative, fewer false positives
   - Adjust based on your access patterns

## Examples

### Sequential Access

```rust
// Configure for sequential workload
let config = PrefetchConfig {
    enabled: true,
    max_prefetch_entries: 10,  // Aggressive prefetch
    cache_size: 200,
    sequential_threshold: 0.6,  // Lower threshold
    ..Default::default()
};
```

### Temporal Access

```rust
// Configure for hot key workload
let config = PrefetchConfig {
    enabled: true,
    max_prefetch_entries: 5,
    cache_size: 50,  // Smaller cache for hot keys
    temporal_threshold: 0.4,  // Lower threshold
    ..Default::default()
};
```

### Mixed Workload

```rust
// Balanced configuration
let config = PrefetchConfig::default();
```

## Implementation Details

### Pattern Detection Algorithm

1. Maintain a sliding window of recent accesses
2. For each access, record key and offset
3. Calculate scores:
   - Sequential: ratio of increasing offsets
   - Temporal: ratio of repeated keys
4. Select pattern based on highest score above threshold

### Prefetch Trigger

1. On each lookup, record access for pattern detection
2. After serving data, predict next keys based on pattern
3. Spawn background task to prefetch predicted keys
4. Store prefetched data in prefetch cache

### Cache Integration

1. Check prefetch cache first on lookup
2. If hit, serve from prefetch cache (fast path)
3. If miss, read from disk (normal path)
4. Update pattern detection and trigger prefetch

## Limitations

- Prefetch is most effective for predictable access patterns
- Random access patterns see minimal benefit
- Prefetch cache uses additional memory
- Background prefetch may increase disk I/O
- Pattern detection requires history (cold start period)

## Best Practices

1. **Enable for sequential/temporal workloads**: Prefetch works best when access patterns are predictable

2. **Monitor statistics**: Track hit rates to validate effectiveness

3. **Tune configuration**: Adjust based on your specific workload

4. **Consider memory**: Ensure sufficient memory for prefetch cache

5. **Test performance**: Benchmark with and without prefetch for your use case

## See Also

- [Raw Disk Cache Design](RAW_DISK_CACHE_DESIGN.md)
- [Batch I/O Implementation](BATCH_IO_IMPLEMENTATION.md)
- [Performance Tuning](PERFORMANCE_TUNING.md)
