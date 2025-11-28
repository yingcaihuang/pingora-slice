# Metrics Implementation

## Overview

The `SliceMetrics` module provides comprehensive, thread-safe metrics collection for the Pingora Slice Module. It tracks requests, cache operations, subrequests, byte transfers, and latencies using atomic operations to ensure thread safety without locks.

## Architecture

### Core Components

1. **SliceMetrics**: The main metrics collector using atomic counters
2. **MetricsSnapshot**: A point-in-time snapshot of all metrics
3. **Atomic Operations**: All metrics use `AtomicU64` with relaxed ordering for thread-safe updates

## Data Structures

### SliceMetrics

```rust
pub struct SliceMetrics {
    // Request statistics
    total_requests: AtomicU64,
    sliced_requests: AtomicU64,
    passthrough_requests: AtomicU64,
    
    // Cache statistics
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
    cache_errors: AtomicU64,
    
    // Subrequest statistics
    total_subrequests: AtomicU64,
    failed_subrequests: AtomicU64,
    retried_subrequests: AtomicU64,
    
    // Byte statistics
    bytes_from_origin: AtomicU64,
    bytes_from_cache: AtomicU64,
    bytes_to_client: AtomicU64,
    
    // Latency statistics (stored as microseconds)
    total_request_duration_us: AtomicU64,
    total_subrequest_duration_us: AtomicU64,
    total_assembly_duration_us: AtomicU64,
}
```

### MetricsSnapshot

```rust
pub struct MetricsSnapshot {
    pub total_requests: u64,
    pub sliced_requests: u64,
    pub passthrough_requests: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub cache_errors: u64,
    pub total_subrequests: u64,
    pub failed_subrequests: u64,
    pub retried_subrequests: u64,
    pub bytes_from_origin: u64,
    pub bytes_from_cache: u64,
    pub bytes_to_client: u64,
    pub total_request_duration_us: u64,
    pub total_subrequest_duration_us: u64,
    pub total_assembly_duration_us: u64,
}
```

## API Reference

### Recording Metrics

#### record_request(sliced: bool)
Records a request, tracking whether it was handled with slicing or passthrough.

**Requirements**: Validates Requirements 9.1

```rust
metrics.record_request(true);  // Sliced request
metrics.record_request(false); // Passthrough request
```

#### record_cache_hit() / record_cache_miss()
Records cache hit or miss events.

**Requirements**: Validates Requirements 9.1

```rust
metrics.record_cache_hit();
metrics.record_cache_miss();
```

#### record_subrequest(success: bool)
Records a subrequest and whether it succeeded.

**Requirements**: Validates Requirements 9.2

```rust
metrics.record_subrequest(true);  // Success
metrics.record_subrequest(false); // Failure
```

#### record_subrequest_retry()
Records a subrequest retry attempt.

**Requirements**: Validates Requirements 9.2

```rust
metrics.record_subrequest_retry();
```

#### record_bytes_from_origin(bytes: u64)
Records bytes received from the origin server.

```rust
metrics.record_bytes_from_origin(1024 * 1024); // 1 MB
```

#### record_bytes_from_cache(bytes: u64)
Records bytes received from cache.

```rust
metrics.record_bytes_from_cache(512 * 1024); // 512 KB
```

#### record_bytes_to_client(bytes: u64)
Records bytes sent to the client.

```rust
metrics.record_bytes_to_client(1536 * 1024); // 1.5 MB
```

#### record_request_duration(duration: Duration)
Records the total duration of a request.

**Requirements**: Validates Requirements 9.2

```rust
metrics.record_request_duration(Duration::from_millis(150));
```

#### record_subrequest_duration(duration: Duration)
Records the duration of a subrequest.

**Requirements**: Validates Requirements 9.2

```rust
metrics.record_subrequest_duration(Duration::from_millis(50));
```

#### record_assembly_duration(duration: Duration)
Records the duration of the response assembly process.

```rust
metrics.record_assembly_duration(Duration::from_millis(10));
```

### Retrieving Metrics

#### get_stats() -> MetricsSnapshot
Returns a point-in-time snapshot of all metrics.

**Requirements**: Validates Requirements 9.1, 9.2

```rust
let stats = metrics.get_stats();
println!("Total requests: {}", stats.total_requests);
```

#### reset()
Resets all metrics to zero. Primarily useful for testing.

```rust
metrics.reset();
```

## MetricsSnapshot Helper Methods

The `MetricsSnapshot` provides several helper methods for calculating derived metrics:

### cache_hit_rate() -> f64
Returns the cache hit rate as a percentage (0.0 to 100.0).

```rust
let rate = snapshot.cache_hit_rate();
println!("Cache hit rate: {:.2}%", rate);
```

### avg_request_duration_ms() -> f64
Returns the average request duration in milliseconds.

```rust
let avg = snapshot.avg_request_duration_ms();
println!("Average request duration: {:.2} ms", avg);
```

### avg_subrequest_duration_ms() -> f64
Returns the average subrequest duration in milliseconds.

```rust
let avg = snapshot.avg_subrequest_duration_ms();
println!("Average subrequest duration: {:.2} ms", avg);
```

### avg_assembly_duration_ms() -> f64
Returns the average assembly duration in milliseconds.

```rust
let avg = snapshot.avg_assembly_duration_ms();
println!("Average assembly duration: {:.2} ms", avg);
```

### subrequest_failure_rate() -> f64
Returns the subrequest failure rate as a percentage (0.0 to 100.0).

```rust
let rate = snapshot.subrequest_failure_rate();
println!("Subrequest failure rate: {:.2}%", rate);
```

## Thread Safety

All metrics operations are thread-safe using atomic operations with relaxed memory ordering. This provides:

1. **Lock-free updates**: No mutex contention
2. **High performance**: Minimal overhead for metric recording
3. **Concurrent access**: Multiple threads can safely update metrics simultaneously

### Example: Concurrent Access

```rust
use std::sync::Arc;
use std::thread;

let metrics = Arc::new(SliceMetrics::new());

let mut handles = vec![];
for _ in 0..10 {
    let metrics_clone = Arc::clone(&metrics);
    let handle = thread::spawn(move || {
        for _ in 0..100 {
            metrics_clone.record_request(true);
        }
    });
    handles.push(handle);
}

for handle in handles {
    handle.join().unwrap();
}

let stats = metrics.get_stats();
assert_eq!(stats.total_requests, 1000);
```

## Usage Patterns

### Pattern 1: Request Lifecycle Tracking

```rust
use std::time::Instant;

let start = Instant::now();

// Process request
let sliced = should_use_slicing();
metrics.record_request(sliced);

if sliced {
    // Handle sliced request
    let assembly_start = Instant::now();
    // ... assembly logic ...
    metrics.record_assembly_duration(assembly_start.elapsed());
}

metrics.record_request_duration(start.elapsed());
```

### Pattern 2: Cache Operation Tracking

```rust
match cache.lookup(&key).await {
    Ok(Some(data)) => {
        metrics.record_cache_hit();
        metrics.record_bytes_from_cache(data.len() as u64);
        Ok(data)
    }
    Ok(None) => {
        metrics.record_cache_miss();
        // Fetch from origin
        let data = fetch_from_origin().await?;
        metrics.record_bytes_from_origin(data.len() as u64);
        Ok(data)
    }
    Err(_) => {
        metrics.record_cache_error();
        Err(error)
    }
}
```

### Pattern 3: Subrequest Tracking

```rust
let start = Instant::now();
let mut attempts = 0;

loop {
    match try_subrequest().await {
        Ok(result) => {
            metrics.record_subrequest(true);
            metrics.record_subrequest_duration(start.elapsed());
            if attempts > 0 {
                metrics.record_subrequest_retry();
            }
            return Ok(result);
        }
        Err(e) if attempts < max_retries => {
            attempts += 1;
            metrics.record_subrequest_retry();
            continue;
        }
        Err(e) => {
            metrics.record_subrequest(false);
            return Err(e);
        }
    }
}
```

### Pattern 4: Periodic Metrics Reporting

```rust
use tokio::time::{interval, Duration};

async fn metrics_reporter(metrics: Arc<SliceMetrics>) {
    let mut ticker = interval(Duration::from_secs(60));
    
    loop {
        ticker.tick().await;
        let stats = metrics.get_stats();
        
        println!("=== Metrics Report ===");
        println!("Requests: {} (sliced: {}, passthrough: {})",
            stats.total_requests,
            stats.sliced_requests,
            stats.passthrough_requests
        );
        println!("Cache hit rate: {:.2}%", stats.cache_hit_rate());
        println!("Avg request duration: {:.2} ms", stats.avg_request_duration_ms());
        println!("Subrequest failure rate: {:.2}%", stats.subrequest_failure_rate());
    }
}
```

## Integration with Monitoring Systems

### Prometheus Format

```rust
impl MetricsSnapshot {
    pub fn to_prometheus(&self) -> String {
        format!(
            "# HELP slice_requests_total Total number of requests\n\
             # TYPE slice_requests_total counter\n\
             slice_requests_total{{type=\"sliced\"}} {}\n\
             slice_requests_total{{type=\"passthrough\"}} {}\n\
             \n\
             # HELP slice_cache_operations_total Cache operations\n\
             # TYPE slice_cache_operations_total counter\n\
             slice_cache_operations_total{{result=\"hit\"}} {}\n\
             slice_cache_operations_total{{result=\"miss\"}} {}\n\
             \n\
             # HELP slice_subrequests_total Total subrequests\n\
             # TYPE slice_subrequests_total counter\n\
             slice_subrequests_total{{result=\"success\"}} {}\n\
             slice_subrequests_total{{result=\"failure\"}} {}\n\
             \n\
             # HELP slice_bytes_total Bytes transferred\n\
             # TYPE slice_bytes_total counter\n\
             slice_bytes_total{{source=\"origin\"}} {}\n\
             slice_bytes_total{{source=\"cache\"}} {}\n\
             slice_bytes_total{{destination=\"client\"}} {}\n",
            self.sliced_requests,
            self.passthrough_requests,
            self.cache_hits,
            self.cache_misses,
            self.total_subrequests - self.failed_subrequests,
            self.failed_subrequests,
            self.bytes_from_origin,
            self.bytes_from_cache,
            self.bytes_to_client,
        )
    }
}
```

## Performance Considerations

1. **Relaxed Ordering**: Uses `Ordering::Relaxed` for atomic operations, which is sufficient for metrics and provides the best performance.

2. **No Locks**: All operations are lock-free, avoiding contention in high-concurrency scenarios.

3. **Minimal Overhead**: Each metric update is a single atomic operation, typically just a few CPU cycles.

4. **Snapshot Consistency**: The `get_stats()` method reads all metrics atomically, but the snapshot may not be perfectly consistent across all fields due to concurrent updates. This is acceptable for monitoring purposes.

## Testing

The module includes comprehensive unit tests covering:

- Basic metric recording
- Cache operations
- Subrequest tracking
- Byte counting
- Duration recording
- Derived metrics (hit rates, averages)
- Thread safety
- Reset functionality

Run tests with:
```bash
cargo test --lib metrics
```

## Requirements Validation

This implementation validates the following requirements:

- **Requirement 9.1**: Records metrics including total requests, sliced requests, and cache hits
- **Requirement 9.2**: Records the number of subrequests and their latencies

## Future Enhancements

1. **Histogram Support**: Add histogram tracking for latency distributions
2. **Percentile Calculations**: P50, P95, P99 latency metrics
3. **Rate Calculations**: Requests per second, bytes per second
4. **Alerting Thresholds**: Built-in threshold checking for alerts
5. **Time-windowed Metrics**: Rolling window statistics (last 5 minutes, etc.)
6. **Metric Labels**: Support for custom labels/tags on metrics
