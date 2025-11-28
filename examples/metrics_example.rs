//! Example demonstrating the SliceMetrics usage
//!
//! This example shows how to use the metrics collector to track
//! various statistics about the slice module's operation.

use pingora_slice::{SliceMetrics, MetricsSnapshot};
use std::time::Duration;
use std::thread;
use std::sync::Arc;

fn main() {
    println!("=== Pingora Slice Metrics Example ===\n");
    
    // Create a new metrics collector
    let metrics = Arc::new(SliceMetrics::new());
    
    // Example 1: Recording basic request metrics
    println!("Example 1: Recording requests");
    metrics.record_request(true);  // Sliced request
    metrics.record_request(true);  // Sliced request
    metrics.record_request(false); // Passthrough request
    
    let stats = metrics.get_stats();
    println!("Total requests: {}", stats.total_requests);
    println!("Sliced requests: {}", stats.sliced_requests);
    println!("Passthrough requests: {}", stats.passthrough_requests);
    println!();
    
    // Example 2: Recording cache operations
    println!("Example 2: Recording cache operations");
    metrics.record_cache_hit();
    metrics.record_cache_hit();
    metrics.record_cache_hit();
    metrics.record_cache_miss();
    
    let stats = metrics.get_stats();
    println!("Cache hits: {}", stats.cache_hits);
    println!("Cache misses: {}", stats.cache_misses);
    println!("Cache hit rate: {:.2}%", stats.cache_hit_rate());
    println!();
    
    // Example 3: Recording subrequest statistics
    println!("Example 3: Recording subrequests");
    metrics.record_subrequest(true);  // Success
    metrics.record_subrequest(true);  // Success
    metrics.record_subrequest(false); // Failure
    metrics.record_subrequest_retry();
    
    let stats = metrics.get_stats();
    println!("Total subrequests: {}", stats.total_subrequests);
    println!("Failed subrequests: {}", stats.failed_subrequests);
    println!("Retried subrequests: {}", stats.retried_subrequests);
    println!("Failure rate: {:.2}%", stats.subrequest_failure_rate());
    println!();
    
    // Example 4: Recording byte statistics
    println!("Example 4: Recording byte transfers");
    metrics.record_bytes_from_origin(1024 * 1024);  // 1 MB from origin
    metrics.record_bytes_from_cache(512 * 1024);    // 512 KB from cache
    metrics.record_bytes_to_client(1536 * 1024);    // 1.5 MB to client
    
    let stats = metrics.get_stats();
    println!("Bytes from origin: {} KB", stats.bytes_from_origin / 1024);
    println!("Bytes from cache: {} KB", stats.bytes_from_cache / 1024);
    println!("Bytes to client: {} KB", stats.bytes_to_client / 1024);
    println!();
    
    // Example 5: Recording latencies
    println!("Example 5: Recording latencies");
    metrics.record_request_duration(Duration::from_millis(150));
    metrics.record_request_duration(Duration::from_millis(200));
    metrics.record_subrequest_duration(Duration::from_millis(50));
    metrics.record_subrequest_duration(Duration::from_millis(75));
    metrics.record_assembly_duration(Duration::from_millis(10));
    
    let stats = metrics.get_stats();
    println!("Average request duration: {:.2} ms", stats.avg_request_duration_ms());
    println!("Average subrequest duration: {:.2} ms", stats.avg_subrequest_duration_ms());
    println!("Average assembly duration: {:.2} ms", stats.avg_assembly_duration_ms());
    println!();
    
    // Example 6: Thread-safe concurrent access
    println!("Example 6: Concurrent metrics collection");
    let metrics_clone = Arc::clone(&metrics);
    let mut handles = vec![];
    
    // Spawn 5 threads, each recording 10 requests
    for i in 0..5 {
        let metrics_thread = Arc::clone(&metrics_clone);
        let handle = thread::spawn(move || {
            for _ in 0..10 {
                metrics_thread.record_request(true);
                metrics_thread.record_cache_hit();
                thread::sleep(Duration::from_millis(1));
            }
            println!("Thread {} completed", i);
        });
        handles.push(handle);
    }
    
    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }
    
    let stats = metrics.get_stats();
    println!("Total requests after concurrent access: {}", stats.total_requests);
    println!("Total cache hits after concurrent access: {}", stats.cache_hits);
    println!();
    
    // Example 7: Getting a complete snapshot
    println!("Example 7: Complete metrics snapshot");
    print_metrics_snapshot(&stats);
    
    // Example 8: Resetting metrics
    println!("\nExample 8: Resetting metrics");
    metrics.reset();
    let stats = metrics.get_stats();
    println!("Total requests after reset: {}", stats.total_requests);
    println!("Cache hits after reset: {}", stats.cache_hits);
}

fn print_metrics_snapshot(snapshot: &MetricsSnapshot) {
    println!("┌─────────────────────────────────────────┐");
    println!("│         Metrics Snapshot                │");
    println!("├─────────────────────────────────────────┤");
    println!("│ Requests:                               │");
    println!("│   Total: {:>30} │", snapshot.total_requests);
    println!("│   Sliced: {:>29} │", snapshot.sliced_requests);
    println!("│   Passthrough: {:>23} │", snapshot.passthrough_requests);
    println!("├─────────────────────────────────────────┤");
    println!("│ Cache:                                  │");
    println!("│   Hits: {:>30} │", snapshot.cache_hits);
    println!("│   Misses: {:>28} │", snapshot.cache_misses);
    println!("│   Errors: {:>28} │", snapshot.cache_errors);
    println!("│   Hit Rate: {:>23.2}% │", snapshot.cache_hit_rate());
    println!("├─────────────────────────────────────────┤");
    println!("│ Subrequests:                            │");
    println!("│   Total: {:>29} │", snapshot.total_subrequests);
    println!("│   Failed: {:>28} │", snapshot.failed_subrequests);
    println!("│   Retried: {:>27} │", snapshot.retried_subrequests);
    println!("│   Failure Rate: {:>19.2}% │", snapshot.subrequest_failure_rate());
    println!("├─────────────────────────────────────────┤");
    println!("│ Bytes:                                  │");
    println!("│   From Origin: {:>19} KB │", snapshot.bytes_from_origin / 1024);
    println!("│   From Cache: {:>20} KB │", snapshot.bytes_from_cache / 1024);
    println!("│   To Client: {:>21} KB │", snapshot.bytes_to_client / 1024);
    println!("├─────────────────────────────────────────┤");
    println!("│ Latencies:                              │");
    println!("│   Avg Request: {:>18.2} ms │", snapshot.avg_request_duration_ms());
    println!("│   Avg Subrequest: {:>15.2} ms │", snapshot.avg_subrequest_duration_ms());
    println!("│   Avg Assembly: {:>17.2} ms │", snapshot.avg_assembly_duration_ms());
    println!("└─────────────────────────────────────────┘");
}
