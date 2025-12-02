//! Metrics collection for Raw Disk Cache
//!
//! This module provides thread-safe metrics collection for monitoring
//! raw disk cache performance, health, and resource utilization.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Metrics collector for Raw Disk Cache
///
/// All operations are thread-safe using atomic operations.
#[derive(Debug, Default)]
pub struct RawDiskMetrics {
    // Operation counters
    store_operations: AtomicU64,
    lookup_operations: AtomicU64,
    remove_operations: AtomicU64,
    
    // Success/failure counters
    store_successes: AtomicU64,
    store_failures: AtomicU64,
    lookup_hits: AtomicU64,
    lookup_misses: AtomicU64,
    
    // I/O metrics
    bytes_written: AtomicU64,
    bytes_read: AtomicU64,
    disk_writes: AtomicU64,
    disk_reads: AtomicU64,
    
    // Latency metrics (stored as microseconds)
    total_store_duration_us: AtomicU64,
    total_lookup_duration_us: AtomicU64,
    total_remove_duration_us: AtomicU64,
    
    // Cache state metrics (updated periodically)
    current_entries: AtomicU64,
    used_blocks: AtomicU64,
    free_blocks: AtomicU64,
    
    // GC metrics
    gc_runs: AtomicU64,
    gc_entries_evicted: AtomicU64,
    gc_bytes_freed: AtomicU64,
}

/// Snapshot of metrics at a point in time
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawDiskMetricsSnapshot {
    // Operation counters
    pub store_operations: u64,
    pub lookup_operations: u64,
    pub remove_operations: u64,
    
    // Success/failure counters
    pub store_successes: u64,
    pub store_failures: u64,
    pub lookup_hits: u64,
    pub lookup_misses: u64,
    
    // I/O metrics
    pub bytes_written: u64,
    pub bytes_read: u64,
    pub disk_writes: u64,
    pub disk_reads: u64,
    
    // Latency metrics
    pub total_store_duration_us: u64,
    pub total_lookup_duration_us: u64,
    pub total_remove_duration_us: u64,
    
    // Cache state metrics
    pub current_entries: u64,
    pub used_blocks: u64,
    pub free_blocks: u64,
    
    // GC metrics
    pub gc_runs: u64,
    pub gc_entries_evicted: u64,
    pub gc_bytes_freed: u64,
}

impl RawDiskMetrics {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Record a store operation
    pub fn record_store(&self, success: bool, duration: Duration, bytes: u64) {
        self.store_operations.fetch_add(1, Ordering::Relaxed);
        if success {
            self.store_successes.fetch_add(1, Ordering::Relaxed);
            self.bytes_written.fetch_add(bytes, Ordering::Relaxed);
        } else {
            self.store_failures.fetch_add(1, Ordering::Relaxed);
        }
        self.total_store_duration_us
            .fetch_add(duration.as_micros() as u64, Ordering::Relaxed);
    }

    /// Record a lookup operation
    pub fn record_lookup(&self, hit: bool, duration: Duration, bytes: u64) {
        self.lookup_operations.fetch_add(1, Ordering::Relaxed);
        if hit {
            self.lookup_hits.fetch_add(1, Ordering::Relaxed);
            self.bytes_read.fetch_add(bytes, Ordering::Relaxed);
        } else {
            self.lookup_misses.fetch_add(1, Ordering::Relaxed);
        }
        self.total_lookup_duration_us
            .fetch_add(duration.as_micros() as u64, Ordering::Relaxed);
    }
    
    /// Record a remove operation
    pub fn record_remove(&self, duration: Duration) {
        self.remove_operations.fetch_add(1, Ordering::Relaxed);
        self.total_remove_duration_us
            .fetch_add(duration.as_micros() as u64, Ordering::Relaxed);
    }
    
    /// Record a disk write operation
    pub fn record_disk_write(&self, bytes: u64) {
        self.disk_writes.fetch_add(1, Ordering::Relaxed);
        self.bytes_written.fetch_add(bytes, Ordering::Relaxed);
    }
    
    /// Record a disk read operation
    pub fn record_disk_read(&self, bytes: u64) {
        self.disk_reads.fetch_add(1, Ordering::Relaxed);
        self.bytes_read.fetch_add(bytes, Ordering::Relaxed);
    }
    
    /// Update cache state metrics
    pub fn update_cache_state(&self, entries: u64, used_blocks: u64, free_blocks: u64) {
        self.current_entries.store(entries, Ordering::Relaxed);
        self.used_blocks.store(used_blocks, Ordering::Relaxed);
        self.free_blocks.store(free_blocks, Ordering::Relaxed);
    }
    
    /// Record a GC run
    pub fn record_gc_run(&self, entries_evicted: u64, bytes_freed: u64) {
        self.gc_runs.fetch_add(1, Ordering::Relaxed);
        self.gc_entries_evicted.fetch_add(entries_evicted, Ordering::Relaxed);
        self.gc_bytes_freed.fetch_add(bytes_freed, Ordering::Relaxed);
    }

    /// Get a snapshot of current metrics
    pub fn get_stats(&self) -> RawDiskMetricsSnapshot {
        RawDiskMetricsSnapshot {
            store_operations: self.store_operations.load(Ordering::Relaxed),
            lookup_operations: self.lookup_operations.load(Ordering::Relaxed),
            remove_operations: self.remove_operations.load(Ordering::Relaxed),
            store_successes: self.store_successes.load(Ordering::Relaxed),
            store_failures: self.store_failures.load(Ordering::Relaxed),
            lookup_hits: self.lookup_hits.load(Ordering::Relaxed),
            lookup_misses: self.lookup_misses.load(Ordering::Relaxed),
            bytes_written: self.bytes_written.load(Ordering::Relaxed),
            bytes_read: self.bytes_read.load(Ordering::Relaxed),
            disk_writes: self.disk_writes.load(Ordering::Relaxed),
            disk_reads: self.disk_reads.load(Ordering::Relaxed),
            total_store_duration_us: self.total_store_duration_us.load(Ordering::Relaxed),
            total_lookup_duration_us: self.total_lookup_duration_us.load(Ordering::Relaxed),
            total_remove_duration_us: self.total_remove_duration_us.load(Ordering::Relaxed),
            current_entries: self.current_entries.load(Ordering::Relaxed),
            used_blocks: self.used_blocks.load(Ordering::Relaxed),
            free_blocks: self.free_blocks.load(Ordering::Relaxed),
            gc_runs: self.gc_runs.load(Ordering::Relaxed),
            gc_entries_evicted: self.gc_entries_evicted.load(Ordering::Relaxed),
            gc_bytes_freed: self.gc_bytes_freed.load(Ordering::Relaxed),
        }
    }
    
    /// Reset all metrics to zero
    pub fn reset(&self) {
        self.store_operations.store(0, Ordering::Relaxed);
        self.lookup_operations.store(0, Ordering::Relaxed);
        self.remove_operations.store(0, Ordering::Relaxed);
        self.store_successes.store(0, Ordering::Relaxed);
        self.store_failures.store(0, Ordering::Relaxed);
        self.lookup_hits.store(0, Ordering::Relaxed);
        self.lookup_misses.store(0, Ordering::Relaxed);
        self.bytes_written.store(0, Ordering::Relaxed);
        self.bytes_read.store(0, Ordering::Relaxed);
        self.disk_writes.store(0, Ordering::Relaxed);
        self.disk_reads.store(0, Ordering::Relaxed);
        self.total_store_duration_us.store(0, Ordering::Relaxed);
        self.total_lookup_duration_us.store(0, Ordering::Relaxed);
        self.total_remove_duration_us.store(0, Ordering::Relaxed);
        self.current_entries.store(0, Ordering::Relaxed);
        self.used_blocks.store(0, Ordering::Relaxed);
        self.free_blocks.store(0, Ordering::Relaxed);
        self.gc_runs.store(0, Ordering::Relaxed);
        self.gc_entries_evicted.store(0, Ordering::Relaxed);
        self.gc_bytes_freed.store(0, Ordering::Relaxed);
    }
}

impl RawDiskMetricsSnapshot {
    /// Calculate cache hit rate as a percentage (0.0 to 100.0)
    pub fn cache_hit_rate(&self) -> f64 {
        let total = self.lookup_hits + self.lookup_misses;
        if total == 0 {
            0.0
        } else {
            (self.lookup_hits as f64 / total as f64) * 100.0
        }
    }
    
    /// Calculate store success rate as a percentage (0.0 to 100.0)
    pub fn store_success_rate(&self) -> f64 {
        if self.store_operations == 0 {
            0.0
        } else {
            (self.store_successes as f64 / self.store_operations as f64) * 100.0
        }
    }
    
    /// Calculate average store duration in milliseconds
    pub fn avg_store_duration_ms(&self) -> f64 {
        if self.store_operations == 0 {
            0.0
        } else {
            (self.total_store_duration_us as f64 / self.store_operations as f64) / 1000.0
        }
    }
    
    /// Calculate average lookup duration in milliseconds
    pub fn avg_lookup_duration_ms(&self) -> f64 {
        if self.lookup_operations == 0 {
            0.0
        } else {
            (self.total_lookup_duration_us as f64 / self.lookup_operations as f64) / 1000.0
        }
    }
    
    /// Calculate average remove duration in milliseconds
    pub fn avg_remove_duration_ms(&self) -> f64 {
        if self.remove_operations == 0 {
            0.0
        } else {
            (self.total_remove_duration_us as f64 / self.remove_operations as f64) / 1000.0
        }
    }
    
    /// Calculate total blocks
    pub fn total_blocks(&self) -> u64 {
        self.used_blocks + self.free_blocks
    }
    
    /// Calculate space utilization as a percentage (0.0 to 100.0)
    pub fn space_utilization(&self) -> f64 {
        let total = self.total_blocks();
        if total == 0 {
            0.0
        } else {
            (self.used_blocks as f64 / total as f64) * 100.0
        }
    }
}

/// Format metrics in Prometheus exposition format
pub fn format_prometheus_metrics(snapshot: &RawDiskMetricsSnapshot) -> String {
    let mut output = String::new();

    // Operation counters
    output.push_str("# HELP raw_disk_cache_store_operations_total Total number of store operations\n");
    output.push_str("# TYPE raw_disk_cache_store_operations_total counter\n");
    output.push_str(&format!("raw_disk_cache_store_operations_total {}\n", snapshot.store_operations));
    output.push_str("\n");

    output.push_str("# HELP raw_disk_cache_lookup_operations_total Total number of lookup operations\n");
    output.push_str("# TYPE raw_disk_cache_lookup_operations_total counter\n");
    output.push_str(&format!("raw_disk_cache_lookup_operations_total {}\n", snapshot.lookup_operations));
    output.push_str("\n");

    output.push_str("# HELP raw_disk_cache_remove_operations_total Total number of remove operations\n");
    output.push_str("# TYPE raw_disk_cache_remove_operations_total counter\n");
    output.push_str(&format!("raw_disk_cache_remove_operations_total {}\n", snapshot.remove_operations));
    output.push_str("\n");

    // Success/failure metrics
    output.push_str("# HELP raw_disk_cache_store_successes_total Number of successful store operations\n");
    output.push_str("# TYPE raw_disk_cache_store_successes_total counter\n");
    output.push_str(&format!("raw_disk_cache_store_successes_total {}\n", snapshot.store_successes));
    output.push_str("\n");

    output.push_str("# HELP raw_disk_cache_store_failures_total Number of failed store operations\n");
    output.push_str("# TYPE raw_disk_cache_store_failures_total counter\n");
    output.push_str(&format!("raw_disk_cache_store_failures_total {}\n", snapshot.store_failures));
    output.push_str("\n");

    output.push_str("# HELP raw_disk_cache_lookup_hits_total Number of cache hits\n");
    output.push_str("# TYPE raw_disk_cache_lookup_hits_total counter\n");
    output.push_str(&format!("raw_disk_cache_lookup_hits_total {}\n", snapshot.lookup_hits));
    output.push_str("\n");

    output.push_str("# HELP raw_disk_cache_lookup_misses_total Number of cache misses\n");
    output.push_str("# TYPE raw_disk_cache_lookup_misses_total counter\n");
    output.push_str(&format!("raw_disk_cache_lookup_misses_total {}\n", snapshot.lookup_misses));
    output.push_str("\n");

    // Calculated rates
    output.push_str("# HELP raw_disk_cache_hit_rate Cache hit rate percentage\n");
    output.push_str("# TYPE raw_disk_cache_hit_rate gauge\n");
    output.push_str(&format!("raw_disk_cache_hit_rate {:.2}\n", snapshot.cache_hit_rate()));
    output.push_str("\n");

    output.push_str("# HELP raw_disk_cache_store_success_rate Store success rate percentage\n");
    output.push_str("# TYPE raw_disk_cache_store_success_rate gauge\n");
    output.push_str(&format!("raw_disk_cache_store_success_rate {:.2}\n", snapshot.store_success_rate()));
    output.push_str("\n");

    // I/O metrics
    output.push_str("# HELP raw_disk_cache_bytes_written_total Total bytes written to disk\n");
    output.push_str("# TYPE raw_disk_cache_bytes_written_total counter\n");
    output.push_str(&format!("raw_disk_cache_bytes_written_total {}\n", snapshot.bytes_written));
    output.push_str("\n");

    output.push_str("# HELP raw_disk_cache_bytes_read_total Total bytes read from disk\n");
    output.push_str("# TYPE raw_disk_cache_bytes_read_total counter\n");
    output.push_str(&format!("raw_disk_cache_bytes_read_total {}\n", snapshot.bytes_read));
    output.push_str("\n");

    output.push_str("# HELP raw_disk_cache_disk_writes_total Total number of disk write operations\n");
    output.push_str("# TYPE raw_disk_cache_disk_writes_total counter\n");
    output.push_str(&format!("raw_disk_cache_disk_writes_total {}\n", snapshot.disk_writes));
    output.push_str("\n");

    output.push_str("# HELP raw_disk_cache_disk_reads_total Total number of disk read operations\n");
    output.push_str("# TYPE raw_disk_cache_disk_reads_total counter\n");
    output.push_str(&format!("raw_disk_cache_disk_reads_total {}\n", snapshot.disk_reads));
    output.push_str("\n");

    // Latency metrics
    output.push_str("# HELP raw_disk_cache_store_duration_ms_avg Average store duration in milliseconds\n");
    output.push_str("# TYPE raw_disk_cache_store_duration_ms_avg gauge\n");
    output.push_str(&format!("raw_disk_cache_store_duration_ms_avg {:.2}\n", snapshot.avg_store_duration_ms()));
    output.push_str("\n");

    output.push_str("# HELP raw_disk_cache_lookup_duration_ms_avg Average lookup duration in milliseconds\n");
    output.push_str("# TYPE raw_disk_cache_lookup_duration_ms_avg gauge\n");
    output.push_str(&format!("raw_disk_cache_lookup_duration_ms_avg {:.2}\n", snapshot.avg_lookup_duration_ms()));
    output.push_str("\n");

    output.push_str("# HELP raw_disk_cache_remove_duration_ms_avg Average remove duration in milliseconds\n");
    output.push_str("# TYPE raw_disk_cache_remove_duration_ms_avg gauge\n");
    output.push_str(&format!("raw_disk_cache_remove_duration_ms_avg {:.2}\n", snapshot.avg_remove_duration_ms()));
    output.push_str("\n");

    // Cache state metrics
    output.push_str("# HELP raw_disk_cache_entries Current number of cache entries\n");
    output.push_str("# TYPE raw_disk_cache_entries gauge\n");
    output.push_str(&format!("raw_disk_cache_entries {}\n", snapshot.current_entries));
    output.push_str("\n");

    output.push_str("# HELP raw_disk_cache_used_blocks Number of used blocks\n");
    output.push_str("# TYPE raw_disk_cache_used_blocks gauge\n");
    output.push_str(&format!("raw_disk_cache_used_blocks {}\n", snapshot.used_blocks));
    output.push_str("\n");

    output.push_str("# HELP raw_disk_cache_free_blocks Number of free blocks\n");
    output.push_str("# TYPE raw_disk_cache_free_blocks gauge\n");
    output.push_str(&format!("raw_disk_cache_free_blocks {}\n", snapshot.free_blocks));
    output.push_str("\n");

    output.push_str("# HELP raw_disk_cache_space_utilization Space utilization percentage\n");
    output.push_str("# TYPE raw_disk_cache_space_utilization gauge\n");
    output.push_str(&format!("raw_disk_cache_space_utilization {:.2}\n", snapshot.space_utilization()));
    output.push_str("\n");

    // GC metrics
    output.push_str("# HELP raw_disk_cache_gc_runs_total Total number of GC runs\n");
    output.push_str("# TYPE raw_disk_cache_gc_runs_total counter\n");
    output.push_str(&format!("raw_disk_cache_gc_runs_total {}\n", snapshot.gc_runs));
    output.push_str("\n");

    output.push_str("# HELP raw_disk_cache_gc_entries_evicted_total Total entries evicted by GC\n");
    output.push_str("# TYPE raw_disk_cache_gc_entries_evicted_total counter\n");
    output.push_str(&format!("raw_disk_cache_gc_entries_evicted_total {}\n", snapshot.gc_entries_evicted));
    output.push_str("\n");

    output.push_str("# HELP raw_disk_cache_gc_bytes_freed_total Total bytes freed by GC\n");
    output.push_str("# TYPE raw_disk_cache_gc_bytes_freed_total counter\n");
    output.push_str(&format!("raw_disk_cache_gc_bytes_freed_total {}\n", snapshot.gc_bytes_freed));
    output.push_str("\n");

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_store() {
        let metrics = RawDiskMetrics::new();
        
        metrics.record_store(true, Duration::from_millis(10), 1024);
        metrics.record_store(true, Duration::from_millis(20), 2048);
        metrics.record_store(false, Duration::from_millis(5), 0);
        
        let stats = metrics.get_stats();
        assert_eq!(stats.store_operations, 3);
        assert_eq!(stats.store_successes, 2);
        assert_eq!(stats.store_failures, 1);
        assert_eq!(stats.bytes_written, 3072);
    }

    #[test]
    fn test_record_lookup() {
        let metrics = RawDiskMetrics::new();
        
        metrics.record_lookup(true, Duration::from_millis(5), 1024);
        metrics.record_lookup(true, Duration::from_millis(10), 2048);
        metrics.record_lookup(false, Duration::from_millis(2), 0);
        
        let stats = metrics.get_stats();
        assert_eq!(stats.lookup_operations, 3);
        assert_eq!(stats.lookup_hits, 2);
        assert_eq!(stats.lookup_misses, 1);
        assert_eq!(stats.bytes_read, 3072);
    }

    #[test]
    fn test_cache_hit_rate() {
        let metrics = RawDiskMetrics::new();
        
        metrics.record_lookup(true, Duration::from_millis(1), 100);
        metrics.record_lookup(true, Duration::from_millis(1), 100);
        metrics.record_lookup(true, Duration::from_millis(1), 100);
        metrics.record_lookup(false, Duration::from_millis(1), 0);
        
        let stats = metrics.get_stats();
        assert_eq!(stats.cache_hit_rate(), 75.0);
    }

    #[test]
    fn test_store_success_rate() {
        let metrics = RawDiskMetrics::new();
        
        metrics.record_store(true, Duration::from_millis(1), 100);
        metrics.record_store(true, Duration::from_millis(1), 100);
        metrics.record_store(false, Duration::from_millis(1), 0);
        metrics.record_store(false, Duration::from_millis(1), 0);
        
        let stats = metrics.get_stats();
        assert_eq!(stats.store_success_rate(), 50.0);
    }

    #[test]
    fn test_update_cache_state() {
        let metrics = RawDiskMetrics::new();
        
        metrics.update_cache_state(100, 800, 200);
        
        let stats = metrics.get_stats();
        assert_eq!(stats.current_entries, 100);
        assert_eq!(stats.used_blocks, 800);
        assert_eq!(stats.free_blocks, 200);
        assert_eq!(stats.space_utilization(), 80.0);
    }

    #[test]
    fn test_record_gc_run() {
        let metrics = RawDiskMetrics::new();
        
        metrics.record_gc_run(50, 102400);
        metrics.record_gc_run(30, 61440);
        
        let stats = metrics.get_stats();
        assert_eq!(stats.gc_runs, 2);
        assert_eq!(stats.gc_entries_evicted, 80);
        assert_eq!(stats.gc_bytes_freed, 163840);
    }

    #[test]
    fn test_format_prometheus_metrics() {
        let metrics = RawDiskMetrics::new();
        
        metrics.record_store(true, Duration::from_millis(10), 1024);
        metrics.record_lookup(true, Duration::from_millis(5), 512);
        metrics.update_cache_state(10, 80, 20);
        
        let snapshot = metrics.get_stats();
        let output = format_prometheus_metrics(&snapshot);
        
        assert!(output.contains("raw_disk_cache_store_operations_total 1"));
        assert!(output.contains("raw_disk_cache_lookup_operations_total 1"));
        assert!(output.contains("raw_disk_cache_entries 10"));
        assert!(output.contains("raw_disk_cache_used_blocks 80"));
        assert!(output.contains("raw_disk_cache_free_blocks 20"));
    }
}
