//! Metrics collection for the Slice Module
//!
//! This module provides thread-safe metrics collection using atomic operations.
//! It tracks requests, cache hits/misses, subrequests, and latencies.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Metrics collector for the Slice Module
///
/// All operations are thread-safe using atomic operations.
#[derive(Debug, Default)]
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

/// Snapshot of metrics at a point in time
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricsSnapshot {
    // Request statistics
    pub total_requests: u64,
    pub sliced_requests: u64,
    pub passthrough_requests: u64,
    
    // Cache statistics
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub cache_errors: u64,
    
    // Subrequest statistics
    pub total_subrequests: u64,
    pub failed_subrequests: u64,
    pub retried_subrequests: u64,
    
    // Byte statistics
    pub bytes_from_origin: u64,
    pub bytes_from_cache: u64,
    pub bytes_to_client: u64,
    
    // Latency statistics
    pub total_request_duration_us: u64,
    pub total_subrequest_duration_us: u64,
    pub total_assembly_duration_us: u64,
}

impl SliceMetrics {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Record a request
    ///
    /// # Arguments
    /// * `sliced` - Whether the request was handled using slicing (true) or passthrough (false)
    ///
    /// # Requirements
    /// Validates: Requirements 9.1
    pub fn record_request(&self, sliced: bool) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        if sliced {
            self.sliced_requests.fetch_add(1, Ordering::Relaxed);
        } else {
            self.passthrough_requests.fetch_add(1, Ordering::Relaxed);
        }
    }
    
    /// Record a cache hit
    ///
    /// # Requirements
    /// Validates: Requirements 9.1
    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record a cache miss
    ///
    /// # Requirements
    /// Validates: Requirements 9.1
    pub fn record_cache_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record a cache error
    pub fn record_cache_error(&self) {
        self.cache_errors.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record a subrequest
    ///
    /// # Arguments
    /// * `success` - Whether the subrequest succeeded
    ///
    /// # Requirements
    /// Validates: Requirements 9.2
    pub fn record_subrequest(&self, success: bool) {
        self.total_subrequests.fetch_add(1, Ordering::Relaxed);
        if !success {
            self.failed_subrequests.fetch_add(1, Ordering::Relaxed);
        }
    }
    
    /// Record a subrequest retry
    ///
    /// # Requirements
    /// Validates: Requirements 9.2
    pub fn record_subrequest_retry(&self) {
        self.retried_subrequests.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record bytes received from origin
    ///
    /// # Arguments
    /// * `bytes` - Number of bytes received
    pub fn record_bytes_from_origin(&self, bytes: u64) {
        self.bytes_from_origin.fetch_add(bytes, Ordering::Relaxed);
    }
    
    /// Record bytes received from cache
    ///
    /// # Arguments
    /// * `bytes` - Number of bytes received
    pub fn record_bytes_from_cache(&self, bytes: u64) {
        self.bytes_from_cache.fetch_add(bytes, Ordering::Relaxed);
    }
    
    /// Record bytes sent to client
    ///
    /// # Arguments
    /// * `bytes` - Number of bytes sent
    pub fn record_bytes_to_client(&self, bytes: u64) {
        self.bytes_to_client.fetch_add(bytes, Ordering::Relaxed);
    }
    
    /// Record request duration
    ///
    /// # Arguments
    /// * `duration` - Duration of the request
    ///
    /// # Requirements
    /// Validates: Requirements 9.2
    pub fn record_request_duration(&self, duration: Duration) {
        self.total_request_duration_us
            .fetch_add(duration.as_micros() as u64, Ordering::Relaxed);
    }
    
    /// Record subrequest duration
    ///
    /// # Arguments
    /// * `duration` - Duration of the subrequest
    ///
    /// # Requirements
    /// Validates: Requirements 9.2
    pub fn record_subrequest_duration(&self, duration: Duration) {
        self.total_subrequest_duration_us
            .fetch_add(duration.as_micros() as u64, Ordering::Relaxed);
    }
    
    /// Record assembly duration
    ///
    /// # Arguments
    /// * `duration` - Duration of the assembly process
    pub fn record_assembly_duration(&self, duration: Duration) {
        self.total_assembly_duration_us
            .fetch_add(duration.as_micros() as u64, Ordering::Relaxed);
    }
    
    /// Get a snapshot of current metrics
    ///
    /// Returns a point-in-time snapshot of all metrics. Note that due to the
    /// concurrent nature of the system, the snapshot may not be perfectly
    /// consistent across all fields.
    ///
    /// # Requirements
    /// Validates: Requirements 9.1, 9.2
    pub fn get_stats(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            total_requests: self.total_requests.load(Ordering::Relaxed),
            sliced_requests: self.sliced_requests.load(Ordering::Relaxed),
            passthrough_requests: self.passthrough_requests.load(Ordering::Relaxed),
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
            cache_misses: self.cache_misses.load(Ordering::Relaxed),
            cache_errors: self.cache_errors.load(Ordering::Relaxed),
            total_subrequests: self.total_subrequests.load(Ordering::Relaxed),
            failed_subrequests: self.failed_subrequests.load(Ordering::Relaxed),
            retried_subrequests: self.retried_subrequests.load(Ordering::Relaxed),
            bytes_from_origin: self.bytes_from_origin.load(Ordering::Relaxed),
            bytes_from_cache: self.bytes_from_cache.load(Ordering::Relaxed),
            bytes_to_client: self.bytes_to_client.load(Ordering::Relaxed),
            total_request_duration_us: self.total_request_duration_us.load(Ordering::Relaxed),
            total_subrequest_duration_us: self.total_subrequest_duration_us.load(Ordering::Relaxed),
            total_assembly_duration_us: self.total_assembly_duration_us.load(Ordering::Relaxed),
        }
    }
    
    /// Reset all metrics to zero
    ///
    /// This is primarily useful for testing.
    pub fn reset(&self) {
        self.total_requests.store(0, Ordering::Relaxed);
        self.sliced_requests.store(0, Ordering::Relaxed);
        self.passthrough_requests.store(0, Ordering::Relaxed);
        self.cache_hits.store(0, Ordering::Relaxed);
        self.cache_misses.store(0, Ordering::Relaxed);
        self.cache_errors.store(0, Ordering::Relaxed);
        self.total_subrequests.store(0, Ordering::Relaxed);
        self.failed_subrequests.store(0, Ordering::Relaxed);
        self.retried_subrequests.store(0, Ordering::Relaxed);
        self.bytes_from_origin.store(0, Ordering::Relaxed);
        self.bytes_from_cache.store(0, Ordering::Relaxed);
        self.bytes_to_client.store(0, Ordering::Relaxed);
        self.total_request_duration_us.store(0, Ordering::Relaxed);
        self.total_subrequest_duration_us.store(0, Ordering::Relaxed);
        self.total_assembly_duration_us.store(0, Ordering::Relaxed);
    }
}

impl MetricsSnapshot {
    /// Calculate cache hit rate as a percentage (0.0 to 100.0)
    pub fn cache_hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            0.0
        } else {
            (self.cache_hits as f64 / total as f64) * 100.0
        }
    }
    
    /// Calculate average request duration in milliseconds
    pub fn avg_request_duration_ms(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            (self.total_request_duration_us as f64 / self.total_requests as f64) / 1000.0
        }
    }
    
    /// Calculate average subrequest duration in milliseconds
    pub fn avg_subrequest_duration_ms(&self) -> f64 {
        if self.total_subrequests == 0 {
            0.0
        } else {
            (self.total_subrequest_duration_us as f64 / self.total_subrequests as f64) / 1000.0
        }
    }
    
    /// Calculate average assembly duration in milliseconds
    pub fn avg_assembly_duration_ms(&self) -> f64 {
        if self.sliced_requests == 0 {
            0.0
        } else {
            (self.total_assembly_duration_us as f64 / self.sliced_requests as f64) / 1000.0
        }
    }
    
    /// Calculate subrequest failure rate as a percentage (0.0 to 100.0)
    pub fn subrequest_failure_rate(&self) -> f64 {
        if self.total_subrequests == 0 {
            0.0
        } else {
            (self.failed_subrequests as f64 / self.total_subrequests as f64) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::sync::Arc;
    
    #[test]
    fn test_record_request() {
        let metrics = SliceMetrics::new();
        
        metrics.record_request(true);
        metrics.record_request(true);
        metrics.record_request(false);
        
        let stats = metrics.get_stats();
        assert_eq!(stats.total_requests, 3);
        assert_eq!(stats.sliced_requests, 2);
        assert_eq!(stats.passthrough_requests, 1);
    }
    
    #[test]
    fn test_record_cache_operations() {
        let metrics = SliceMetrics::new();
        
        metrics.record_cache_hit();
        metrics.record_cache_hit();
        metrics.record_cache_miss();
        metrics.record_cache_error();
        
        let stats = metrics.get_stats();
        assert_eq!(stats.cache_hits, 2);
        assert_eq!(stats.cache_misses, 1);
        assert_eq!(stats.cache_errors, 1);
    }
    
    #[test]
    fn test_record_subrequest() {
        let metrics = SliceMetrics::new();
        
        metrics.record_subrequest(true);
        metrics.record_subrequest(true);
        metrics.record_subrequest(false);
        metrics.record_subrequest_retry();
        
        let stats = metrics.get_stats();
        assert_eq!(stats.total_subrequests, 3);
        assert_eq!(stats.failed_subrequests, 1);
        assert_eq!(stats.retried_subrequests, 1);
    }
    
    #[test]
    fn test_record_bytes() {
        let metrics = SliceMetrics::new();
        
        metrics.record_bytes_from_origin(1000);
        metrics.record_bytes_from_cache(500);
        metrics.record_bytes_to_client(1500);
        
        let stats = metrics.get_stats();
        assert_eq!(stats.bytes_from_origin, 1000);
        assert_eq!(stats.bytes_from_cache, 500);
        assert_eq!(stats.bytes_to_client, 1500);
    }
    
    #[test]
    fn test_record_durations() {
        let metrics = SliceMetrics::new();
        
        metrics.record_request_duration(Duration::from_millis(100));
        metrics.record_subrequest_duration(Duration::from_millis(50));
        metrics.record_assembly_duration(Duration::from_millis(10));
        
        let stats = metrics.get_stats();
        assert_eq!(stats.total_request_duration_us, 100_000);
        assert_eq!(stats.total_subrequest_duration_us, 50_000);
        assert_eq!(stats.total_assembly_duration_us, 10_000);
    }
    
    #[test]
    fn test_cache_hit_rate() {
        let metrics = SliceMetrics::new();
        
        metrics.record_cache_hit();
        metrics.record_cache_hit();
        metrics.record_cache_hit();
        metrics.record_cache_miss();
        
        let stats = metrics.get_stats();
        assert_eq!(stats.cache_hit_rate(), 75.0);
    }
    
    #[test]
    fn test_cache_hit_rate_no_operations() {
        let metrics = SliceMetrics::new();
        let stats = metrics.get_stats();
        assert_eq!(stats.cache_hit_rate(), 0.0);
    }
    
    #[test]
    fn test_avg_request_duration() {
        let metrics = SliceMetrics::new();
        
        metrics.record_request(true);
        metrics.record_request_duration(Duration::from_millis(100));
        metrics.record_request(true);
        metrics.record_request_duration(Duration::from_millis(200));
        
        let stats = metrics.get_stats();
        assert_eq!(stats.avg_request_duration_ms(), 150.0);
    }
    
    #[test]
    fn test_subrequest_failure_rate() {
        let metrics = SliceMetrics::new();
        
        metrics.record_subrequest(true);
        metrics.record_subrequest(true);
        metrics.record_subrequest(false);
        metrics.record_subrequest(false);
        
        let stats = metrics.get_stats();
        assert_eq!(stats.subrequest_failure_rate(), 50.0);
    }
    
    #[test]
    fn test_reset() {
        let metrics = SliceMetrics::new();
        
        metrics.record_request(true);
        metrics.record_cache_hit();
        metrics.record_subrequest(true);
        
        metrics.reset();
        
        let stats = metrics.get_stats();
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.cache_hits, 0);
        assert_eq!(stats.total_subrequests, 0);
    }
    
    #[test]
    fn test_thread_safety() {
        let metrics = Arc::new(SliceMetrics::new());
        let mut handles = vec![];
        
        // Spawn 10 threads, each recording 100 requests
        for _ in 0..10 {
            let metrics_clone = Arc::clone(&metrics);
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    metrics_clone.record_request(true);
                    metrics_clone.record_cache_hit();
                    metrics_clone.record_subrequest(true);
                }
            });
            handles.push(handle);
        }
        
        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
        
        let stats = metrics.get_stats();
        assert_eq!(stats.total_requests, 1000);
        assert_eq!(stats.cache_hits, 1000);
        assert_eq!(stats.total_subrequests, 1000);
    }
}
