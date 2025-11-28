//! Example: Metrics Endpoint
//!
//! This example demonstrates how to start the metrics HTTP endpoint
//! to expose slice module metrics in Prometheus format.
//!
//! # Usage
//! ```bash
//! cargo run --example metrics_endpoint_example
//! ```
//!
//! Then visit:
//! - http://127.0.0.1:9090/ - Index page with available endpoints
//! - http://127.0.0.1:9090/metrics - Prometheus format metrics
//! - http://127.0.0.1:9090/health - Health check endpoint
//!
//! # Requirements
//! Validates: Requirements 9.5

use pingora_slice::{MetricsEndpoint, SliceMetrics};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .init();

    info!("=== Pingora Slice Metrics Endpoint Example ===");
    info!("");

    // Create shared metrics instance
    let metrics = Arc::new(SliceMetrics::new());
    info!("Created metrics collector");

    // Start a background task to simulate metric updates
    let metrics_clone = Arc::clone(&metrics);
    tokio::spawn(async move {
        simulate_traffic(metrics_clone).await;
    });

    // Create and start the metrics endpoint
    let addr = match "127.0.0.1:9090".parse() {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!("Failed to parse address: {}", e);
            return;
        }
    };
    let endpoint = MetricsEndpoint::new(Arc::clone(&metrics), addr);

    info!("");
    info!("Starting metrics endpoint server...");
    info!("Metrics available at:");
    info!("  - http://127.0.0.1:9090/");
    info!("  - http://127.0.0.1:9090/metrics (Prometheus format)");
    info!("  - http://127.0.0.1:9090/health");
    info!("");
    info!("Press Ctrl+C to stop");
    info!("");

    // Start the endpoint (this runs forever)
    if let Err(e) = endpoint.start().await {
        eprintln!("Metrics endpoint error: {}", e);
    }
}

/// Simulate traffic by updating metrics periodically
async fn simulate_traffic(metrics: Arc<SliceMetrics>) {
    let mut counter = 0;

    loop {
        sleep(Duration::from_secs(2)).await;
        counter += 1;

        // Simulate various operations
        match counter % 5 {
            0 => {
                // Simulate a sliced request with cache hits
                metrics.record_request(true);
                metrics.record_cache_hit();
                metrics.record_cache_hit();
                metrics.record_subrequest(true);
                metrics.record_subrequest(true);
                metrics.record_bytes_from_cache(512 * 1024);
                metrics.record_bytes_from_origin(512 * 1024);
                metrics.record_bytes_to_client(1024 * 1024);
                metrics.record_request_duration(Duration::from_millis(150));
                info!("Simulated: Sliced request with cache hits");
            }
            1 => {
                // Simulate a sliced request with cache misses
                metrics.record_request(true);
                metrics.record_cache_miss();
                metrics.record_cache_miss();
                metrics.record_cache_miss();
                metrics.record_subrequest(true);
                metrics.record_subrequest(true);
                metrics.record_subrequest(true);
                metrics.record_bytes_from_origin(1024 * 1024);
                metrics.record_bytes_to_client(1024 * 1024);
                metrics.record_request_duration(Duration::from_millis(300));
                metrics.record_subrequest_duration(Duration::from_millis(100));
                info!("Simulated: Sliced request with cache misses");
            }
            2 => {
                // Simulate a passthrough request
                metrics.record_request(false);
                metrics.record_bytes_from_origin(2048 * 1024);
                metrics.record_bytes_to_client(2048 * 1024);
                metrics.record_request_duration(Duration::from_millis(200));
                info!("Simulated: Passthrough request");
            }
            3 => {
                // Simulate a request with subrequest failures
                metrics.record_request(true);
                metrics.record_cache_miss();
                metrics.record_subrequest(false);
                metrics.record_subrequest_retry();
                metrics.record_subrequest(true);
                metrics.record_bytes_from_origin(512 * 1024);
                metrics.record_bytes_to_client(512 * 1024);
                metrics.record_request_duration(Duration::from_millis(500));
                info!("Simulated: Request with subrequest retry");
            }
            _ => {
                // Simulate a cache error
                metrics.record_request(true);
                metrics.record_cache_error();
                metrics.record_subrequest(true);
                metrics.record_bytes_from_origin(256 * 1024);
                metrics.record_bytes_to_client(256 * 1024);
                metrics.record_request_duration(Duration::from_millis(180));
                info!("Simulated: Request with cache error");
            }
        }

        // Print current stats
        let stats = metrics.get_stats();
        info!("Current stats: {} total requests, {:.1}% cache hit rate",
              stats.total_requests, stats.cache_hit_rate());
    }
}
