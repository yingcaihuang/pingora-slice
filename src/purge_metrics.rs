//! Prometheus metrics for cache purge operations

use prometheus::{
    register_counter_vec, register_histogram_vec, CounterVec, HistogramVec, Registry,
};
use std::sync::Arc;

/// Metrics for purge operations
#[derive(Clone)]
pub struct PurgeMetrics {
    /// Total number of purge requests
    pub purge_requests_total: Arc<CounterVec>,

    /// Total number of purge requests by result (success/failure)
    pub purge_requests_by_result: Arc<CounterVec>,

    /// Total number of cache items purged
    pub purge_items_total: Arc<CounterVec>,

    /// Duration of purge operations
    pub purge_duration_seconds: Arc<HistogramVec>,

    /// Authentication failures
    pub purge_auth_failures_total: Arc<CounterVec>,
}

impl PurgeMetrics {
    /// Create new purge metrics
    pub fn new() -> Result<Self, prometheus::Error> {
        let purge_requests_total = register_counter_vec!(
            "pingora_slice_purge_requests_total",
            "Total number of cache purge requests",
            &["method"] // method: single, url, all
        )?;

        let purge_requests_by_result = register_counter_vec!(
            "pingora_slice_purge_requests_by_result",
            "Total number of purge requests by result",
            &["method", "result"] // result: success, failure, auth_failure
        )?;

        let purge_items_total = register_counter_vec!(
            "pingora_slice_purge_items_total",
            "Total number of cache items purged",
            &["method"]
        )?;

        let purge_duration_seconds = register_histogram_vec!(
            "pingora_slice_purge_duration_seconds",
            "Duration of purge operations in seconds",
            &["method"],
            vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]
        )?;

        let purge_auth_failures_total = register_counter_vec!(
            "pingora_slice_purge_auth_failures_total",
            "Total number of purge authentication failures",
            &["reason"] // reason: missing_token, invalid_token
        )?;

        Ok(Self {
            purge_requests_total: Arc::new(purge_requests_total),
            purge_requests_by_result: Arc::new(purge_requests_by_result),
            purge_items_total: Arc::new(purge_items_total),
            purge_duration_seconds: Arc::new(purge_duration_seconds),
            purge_auth_failures_total: Arc::new(purge_auth_failures_total),
        })
    }

    /// Create metrics with custom registry
    pub fn with_registry(registry: &Registry) -> Result<Self, prometheus::Error> {
        let purge_requests_total = CounterVec::new(
            prometheus::Opts::new(
                "pingora_slice_purge_requests_total",
                "Total number of cache purge requests",
            ),
            &["method"],
        )?;
        registry.register(Box::new(purge_requests_total.clone()))?;

        let purge_requests_by_result = CounterVec::new(
            prometheus::Opts::new(
                "pingora_slice_purge_requests_by_result",
                "Total number of purge requests by result",
            ),
            &["method", "result"],
        )?;
        registry.register(Box::new(purge_requests_by_result.clone()))?;

        let purge_items_total = CounterVec::new(
            prometheus::Opts::new(
                "pingora_slice_purge_items_total",
                "Total number of cache items purged",
            ),
            &["method"],
        )?;
        registry.register(Box::new(purge_items_total.clone()))?;

        let purge_duration_seconds = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "pingora_slice_purge_duration_seconds",
                "Duration of purge operations in seconds",
            )
            .buckets(vec![
                0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0,
            ]),
            &["method"],
        )?;
        registry.register(Box::new(purge_duration_seconds.clone()))?;

        let purge_auth_failures_total = CounterVec::new(
            prometheus::Opts::new(
                "pingora_slice_purge_auth_failures_total",
                "Total number of purge authentication failures",
            ),
            &["reason"],
        )?;
        registry.register(Box::new(purge_auth_failures_total.clone()))?;

        Ok(Self {
            purge_requests_total: Arc::new(purge_requests_total),
            purge_requests_by_result: Arc::new(purge_requests_by_result),
            purge_items_total: Arc::new(purge_items_total),
            purge_duration_seconds: Arc::new(purge_duration_seconds),
            purge_auth_failures_total: Arc::new(purge_auth_failures_total),
        })
    }

    /// Record a purge request
    pub fn record_request(&self, method: &str) {
        self.purge_requests_total
            .with_label_values(&[method])
            .inc();
    }

    /// Record purge result
    pub fn record_result(&self, method: &str, success: bool) {
        let result = if success { "success" } else { "failure" };
        self.purge_requests_by_result
            .with_label_values(&[method, result])
            .inc();
    }

    /// Record purged items count
    pub fn record_purged_items(&self, method: &str, count: usize) {
        self.purge_items_total
            .with_label_values(&[method])
            .inc_by(count as f64);
    }

    /// Record purge duration
    pub fn record_duration(&self, method: &str, duration_secs: f64) {
        self.purge_duration_seconds
            .with_label_values(&[method])
            .observe(duration_secs);
    }

    /// Record authentication failure
    pub fn record_auth_failure(&self, reason: &str) {
        self.purge_auth_failures_total
            .with_label_values(&[reason])
            .inc();
    }
}

impl Default for PurgeMetrics {
    fn default() -> Self {
        Self::new().expect("Failed to create purge metrics")
    }
}
