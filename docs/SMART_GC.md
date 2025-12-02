# Smart Garbage Collection

The Raw Disk Cache implements an intelligent garbage collection system with multiple eviction strategies, adaptive triggering, and performance monitoring.

## Features

### 1. Multiple Eviction Strategies

The smart GC supports three eviction strategies:

#### LRU (Least Recently Used)
- Evicts entries that haven't been accessed recently
- Best for workloads with temporal locality
- Keeps frequently accessed data in cache

#### LFU (Least Frequently Used)
- Evicts entries with the lowest access count
- Best for workloads with access frequency patterns
- Protects popular data from eviction

#### FIFO (First In First Out)
- Evicts oldest entries by insertion time
- Simple and predictable behavior
- Good for time-based caching scenarios

### 2. Adaptive GC Triggering

The adaptive GC automatically adjusts trigger thresholds based on allocation patterns:

- **Monitors allocation success/failure rates**
- **Increases threshold** when allocation failures are frequent (> 10%)
- **Decreases threshold** when allocations succeed consistently (< 1% failure rate)
- **Prevents thrashing** by maintaining optimal free space

### 3. Incremental GC

Incremental GC processes evictions in small batches:

- **Reduces GC pause times** by yielding between batches
- **Allows other operations** to proceed during GC
- **Configurable batch size** for tuning performance
- **Better responsiveness** for latency-sensitive workloads

### 4. TTL-Based Eviction

Entries can be automatically evicted based on time-to-live:

- **Prioritizes expired entries** during GC
- **Automatic cleanup** of stale data
- **Configurable TTL** per cache instance

### 5. Performance Monitoring

Comprehensive metrics for GC performance:

- Total GC runs and entries evicted
- Total bytes freed
- GC duration (total and average)
- Adaptive adjustments count
- Last GC statistics

## Configuration

### Basic Configuration

```rust
use pingora_slice::raw_disk::{GCConfig, GCTriggerConfig, EvictionStrategy};
use std::time::Duration;

let gc_config = GCConfig {
    strategy: EvictionStrategy::LRU,
    trigger: GCTriggerConfig {
        min_free_ratio: 0.2,      // Trigger when < 20% free
        target_free_ratio: 0.3,   // Target 30% free after GC
        adaptive: true,            // Enable adaptive triggering
        min_interval: Duration::from_secs(60), // Min 60s between GC runs
    },
    incremental: true,             // Enable incremental GC
    batch_size: 100,               // Process 100 entries per batch
    ttl_secs: 3600,                // 1 hour TTL
};

cache.update_gc_config(gc_config).await;
```

### Strategy Selection

Choose the eviction strategy based on your workload:

```rust
// For temporal locality (recent accesses matter)
let config = GCConfig {
    strategy: EvictionStrategy::LRU,
    ..Default::default()
};

// For access frequency (popular items matter)
let config = GCConfig {
    strategy: EvictionStrategy::LFU,
    ..Default::default()
};

// For time-based eviction (oldest first)
let config = GCConfig {
    strategy: EvictionStrategy::FIFO,
    ..Default::default()
};
```

### Adaptive Triggering

Enable adaptive GC for dynamic workloads:

```rust
let config = GCConfig {
    trigger: GCTriggerConfig {
        min_free_ratio: 0.2,
        target_free_ratio: 0.3,
        adaptive: true,  // Enable adaptive triggering
        min_interval: Duration::from_secs(60),
    },
    ..Default::default()
};
```

The adaptive GC will:
- Monitor allocation patterns
- Adjust thresholds automatically
- Prevent allocation failures
- Optimize for your workload

### Incremental GC

Configure incremental GC for better responsiveness:

```rust
let config = GCConfig {
    incremental: true,
    batch_size: 50,  // Smaller batches = more responsive
    ..Default::default()
};
```

Trade-offs:
- **Smaller batches**: Lower pause times, more overhead
- **Larger batches**: Higher pause times, less overhead

## Usage

### Manual GC

Trigger GC manually when needed:

```rust
// Run GC to reach target free ratio
let freed = cache.run_smart_gc().await?;
println!("Freed {} entries", freed);
```

### Automatic GC

GC is automatically triggered during store operations:

```rust
// GC runs automatically when free space is low
cache.store("key", data).await?;
```

### Monitoring

Check GC metrics:

```rust
let metrics = cache.gc_metrics().await;
println!("Total GC runs: {}", metrics.total_runs);
println!("Total evicted: {}", metrics.total_evicted);
println!("Total freed: {} bytes", metrics.total_bytes_freed);
println!("Average duration: {:?}", metrics.average_duration());
println!("Adaptive adjustments: {}", metrics.adaptive_adjustments);
```

Include GC metrics in cache stats:

```rust
let stats = cache.stats().await;
if let Some(gc_metrics) = stats.gc_metrics {
    println!("GC runs: {}", gc_metrics.total_runs);
}
```

## Performance Considerations

### Strategy Performance

| Strategy | Overhead | Best For |
|----------|----------|----------|
| LRU | Low | Temporal locality |
| LFU | Medium | Frequency-based |
| FIFO | Very Low | Simple time-based |

### Adaptive GC Overhead

- Minimal overhead (< 1%)
- Tracks allocation success/failure
- Adjusts thresholds every 100 allocations
- Worth the cost for dynamic workloads

### Incremental GC Trade-offs

- **Pros**: Lower pause times, better responsiveness
- **Cons**: Slightly higher total GC time
- **Recommendation**: Use for latency-sensitive workloads

## Best Practices

### 1. Choose the Right Strategy

```rust
// Web cache with temporal locality
let config = GCConfig {
    strategy: EvictionStrategy::LRU,
    ..Default::default()
};

// CDN cache with popularity patterns
let config = GCConfig {
    strategy: EvictionStrategy::LFU,
    ..Default::default()
};

// Log cache with time-based retention
let config = GCConfig {
    strategy: EvictionStrategy::FIFO,
    ttl_secs: 86400, // 24 hours
    ..Default::default()
};
```

### 2. Tune Trigger Thresholds

```rust
// Conservative (more free space)
let config = GCConfig {
    trigger: GCTriggerConfig {
        min_free_ratio: 0.3,  // Trigger at 30% free
        target_free_ratio: 0.5, // Target 50% free
        ..Default::default()
    },
    ..Default::default()
};

// Aggressive (less free space)
let config = GCConfig {
    trigger: GCTriggerConfig {
        min_free_ratio: 0.1,  // Trigger at 10% free
        target_free_ratio: 0.2, // Target 20% free
        ..Default::default()
    },
    ..Default::default()
};
```

### 3. Enable Adaptive GC for Dynamic Workloads

```rust
let config = GCConfig {
    trigger: GCTriggerConfig {
        adaptive: true,  // Let GC adapt to workload
        ..Default::default()
    },
    ..Default::default()
};
```

### 4. Use Incremental GC for Latency-Sensitive Applications

```rust
let config = GCConfig {
    incremental: true,
    batch_size: 50,  // Tune based on latency requirements
    ..Default::default()
};
```

### 5. Monitor GC Performance

```rust
// Periodically check GC metrics
tokio::spawn(async move {
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
        let metrics = cache.gc_metrics().await;
        
        // Alert if GC is running too frequently
        if metrics.total_runs > 100 {
            warn!("High GC frequency: {} runs", metrics.total_runs);
        }
        
        // Alert if GC is taking too long
        if metrics.average_duration() > Duration::from_millis(100) {
            warn!("High GC latency: {:?}", metrics.average_duration());
        }
    }
});
```

## Examples

See `examples/smart_gc_example.rs` for a complete demonstration of:
- All eviction strategies
- Adaptive GC triggering
- Incremental GC
- Performance monitoring

Run the example:
```bash
cargo run --example smart_gc_example
```

## Troubleshooting

### High GC Frequency

If GC runs too frequently:
1. Increase cache size
2. Lower `min_free_ratio` threshold
3. Enable adaptive GC
4. Review data retention policies

### High GC Latency

If GC takes too long:
1. Enable incremental GC
2. Reduce batch size
3. Consider faster storage
4. Review eviction strategy

### Allocation Failures

If allocations fail despite GC:
1. Enable adaptive GC
2. Increase `target_free_ratio`
3. Increase cache size
4. Review data size distribution

### Memory Pressure

If system memory is high:
1. Reduce cache size
2. Enable TTL-based eviction
3. Lower free space thresholds
4. Use more aggressive GC

## Implementation Details

### LRU Implementation

- Uses LRU queue in `CacheDirectory`
- O(1) access and update
- Evicts from front of queue

### LFU Implementation

- Tracks access frequency per key
- O(n log n) victim selection
- Evicts entries with lowest count

### FIFO Implementation

- Tracks insertion order
- O(1) victim selection
- Evicts oldest entries first

### Adaptive Algorithm

```
Every 100 allocations:
  failure_rate = failures / total
  
  if failure_rate > 10%:
    threshold *= 1.2  // Increase threshold
  else if failure_rate < 1%:
    threshold *= 0.9  // Decrease threshold
```

### Incremental Algorithm

```
while entries_to_evict > 0:
  batch = min(batch_size, entries_to_evict)
  evict(batch)
  entries_to_evict -= batch
  yield()  // Allow other operations
```

## Future Enhancements

Potential improvements for future versions:

1. **Multi-level eviction**: Combine strategies (e.g., LRU + TTL)
2. **Cost-aware eviction**: Consider entry size and access cost
3. **Predictive GC**: Use ML to predict optimal GC timing
4. **Concurrent GC**: Run GC in parallel with operations
5. **Compaction**: Reduce fragmentation during GC
