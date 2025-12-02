# Raw Disk Cache Monitoring and Metrics

This document describes the monitoring and metrics functionality for the raw disk cache.

## Overview

The raw disk cache provides comprehensive monitoring capabilities including:
- Performance metrics (operations, latency, throughput)
- Resource utilization (space, blocks, entries)
- Health checks
- Prometheus-compatible metrics export

## Metrics Collection

### Operation Metrics

The cache tracks all operations with detailed counters:

- **Store Operations**: Total number of store operations (successes and failures)
- **Lookup Operations**: Total number of lookup operations (hits and misses)
- **Remove Operations**: Total number of remove operations

### Performance Metrics

- **Latency Tracking**: Average duration for store, lookup, and remove operations
- **Throughput**: Bytes written and read from disk
- **I/O Operations**: Number of disk read and write operations

### Resource Metrics

- **Cache State**: Current number of entries, used blocks, free blocks
- **Space Utilization**: Percentage of disk space used
- **GC Metrics**: Garbage collection runs, entries evicted, bytes freed

## Usage

### Basic Metrics Access

```rust
use pingora_slice::raw_disk::RawDiskCache;

// Create cache
let cache = RawDiskCache::new(path, size, block_size, ttl).await?;

// Get metrics snapshot (sync)
let snapshot = cache.metrics_snapshot();
println!("Store operations: {}", snapshot.store_operations);
println!("Cache hit rate: {:.2}%", snapshot.cache_hit_rate());

// Get metrics snapshot with updated cache state (async)
let snapshot = cache.metrics_snapshot_async().await;
println!("Current entries: {}", snapshot.current_entries);
```

### Health Check

```rust
// Perform health check
let is_healthy = cache.health_check().await;
if !is_healthy {
    eprintln!("Cache is unhealthy!");
}
```

### Prometheus Export

```rust
use pingora_slice::raw_disk::metrics::format_prometheus_metrics;

// Get metrics in Prometheus format
let snapshot = cache.metrics_snapshot_async().await;
let prometheus_output = format_prometheus_metrics(&snapshot);
println!("{}", prometheus_output);
```

## Prometheus Metrics

The following metrics are exported in Prometheus format:

### Counters

- `raw_disk_cache_store_operations_total` - Total store operations
- `raw_disk_cache_lookup_operations_total` - Total lookup operations
- `raw_disk_cache_remove_operations_total` - Total remove operations
- `raw_disk_cache_store_successes_total` - Successful store operations
- `raw_disk_cache_store_failures_total` - Failed store operations
- `raw_disk_cache_lookup_hits_total` - Cache hits
- `raw_disk_cache_lookup_misses_total` - Cache misses
- `raw_disk_cache_bytes_written_total` - Total bytes written
- `raw_disk_cache_bytes_read_total` - Total bytes read
- `raw_disk_cache_disk_writes_total` - Total disk write operations
- `raw_disk_cache_disk_reads_total` - Total disk read operations
- `raw_disk_cache_gc_runs_total` - Total GC runs
- `raw_disk_cache_gc_entries_evicted_total` - Total entries evicted by GC
- `raw_disk_cache_gc_bytes_freed_total` - Total bytes freed by GC

### Gauges

- `raw_disk_cache_hit_rate` - Cache hit rate percentage
- `raw_disk_cache_store_success_rate` - Store success rate percentage
- `raw_disk_cache_store_duration_ms_avg` - Average store duration (ms)
- `raw_disk_cache_lookup_duration_ms_avg` - Average lookup duration (ms)
- `raw_disk_cache_remove_duration_ms_avg` - Average remove duration (ms)
- `raw_disk_cache_entries` - Current number of entries
- `raw_disk_cache_used_blocks` - Number of used blocks
- `raw_disk_cache_free_blocks` - Number of free blocks
- `raw_disk_cache_space_utilization` - Space utilization percentage

## Health Check Criteria

The health check verifies:

1. **Disk I/O Accessibility**: Can read the superblock
2. **Free Space**: Has available free blocks
3. **Store Success Rate**: If >100 operations, success rate must be >50%

## Example

See `examples/raw_disk_metrics_example.rs` for a complete example demonstrating:
- Metrics collection
- Health checks
- Prometheus export
- Detailed statistics

Run the example:
```bash
cargo run --example raw_disk_metrics_example
```

## Integration

The metrics are automatically collected during normal cache operations. No additional configuration is required. Metrics are thread-safe and use atomic operations for minimal performance impact.
