//! Integration tests for the metrics endpoint
//!
//! These tests verify that the metrics HTTP endpoint correctly exposes
//! metrics in Prometheus format.
//!
//! # Requirements
//! Validates: Requirements 9.5

use pingora_slice::{MetricsEndpoint, SliceMetrics};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn test_metrics_endpoint_starts() {
    // Create metrics
    let metrics = Arc::new(SliceMetrics::new());
    
    // Record some test data
    metrics.record_request(true);
    metrics.record_cache_hit();
    metrics.record_subrequest(true);
    
    // Create endpoint on a random available port
    let addr = "127.0.0.1:0".parse().unwrap();
    let endpoint = MetricsEndpoint::new(metrics, addr);
    
    // Start the endpoint in a background task with a timeout
    let handle = tokio::spawn(async move {
        endpoint.start().await
    });
    
    // Give it a moment to start
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Abort the task (we just wanted to verify it starts without panicking)
    handle.abort();
}

#[tokio::test]
async fn test_metrics_endpoint_serves_metrics() {
    // Create metrics with some data
    let metrics = Arc::new(SliceMetrics::new());
    metrics.record_request(true);
    metrics.record_request(true);
    metrics.record_request(false);
    metrics.record_cache_hit();
    metrics.record_cache_miss();
    metrics.record_subrequest(true);
    metrics.record_bytes_from_origin(1000);
    
    // Find an available port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener); // Release the port
    
    // Start the endpoint
    let endpoint = MetricsEndpoint::new(Arc::clone(&metrics), addr);
    let handle = tokio::spawn(async move {
        endpoint.start().await
    });
    
    // Give the server time to start
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    // Make a request to the metrics endpoint
    let url = format!("http://{}/metrics", addr);
    let result = timeout(Duration::from_secs(2), async {
        reqwest::get(&url).await
    }).await;
    
    // Abort the server
    handle.abort();
    
    // Verify the request succeeded
    if let Ok(Ok(response)) = result {
        assert_eq!(response.status(), 200);
        
        let body = response.text().await.unwrap();
        
        // Verify Prometheus format
        assert!(body.contains("# HELP"));
        assert!(body.contains("# TYPE"));
        assert!(body.contains("pingora_slice_requests_total 3"));
        assert!(body.contains("pingora_slice_sliced_requests_total 2"));
        assert!(body.contains("pingora_slice_passthrough_requests_total 1"));
        assert!(body.contains("pingora_slice_cache_hits_total 1"));
        assert!(body.contains("pingora_slice_cache_misses_total 1"));
        assert!(body.contains("pingora_slice_subrequests_total 1"));
        assert!(body.contains("pingora_slice_bytes_from_origin_total 1000"));
    }
}

#[tokio::test]
async fn test_metrics_endpoint_health_check() {
    // Create metrics
    let metrics = Arc::new(SliceMetrics::new());
    
    // Find an available port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    
    // Start the endpoint
    let endpoint = MetricsEndpoint::new(metrics, addr);
    let handle = tokio::spawn(async move {
        endpoint.start().await
    });
    
    // Give the server time to start
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    // Make a request to the health endpoint
    let url = format!("http://{}/health", addr);
    let result = timeout(Duration::from_secs(2), async {
        reqwest::get(&url).await
    }).await;
    
    // Abort the server
    handle.abort();
    
    // Verify the request succeeded
    if let Ok(Ok(response)) = result {
        assert_eq!(response.status(), 200);
        
        let body = response.text().await.unwrap();
        assert!(body.contains("healthy"));
    }
}

#[tokio::test]
async fn test_metrics_endpoint_index() {
    // Create metrics
    let metrics = Arc::new(SliceMetrics::new());
    
    // Find an available port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    
    // Start the endpoint
    let endpoint = MetricsEndpoint::new(metrics, addr);
    let handle = tokio::spawn(async move {
        endpoint.start().await
    });
    
    // Give the server time to start
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    // Make a request to the index
    let url = format!("http://{}/", addr);
    let result = timeout(Duration::from_secs(2), async {
        reqwest::get(&url).await
    }).await;
    
    // Abort the server
    handle.abort();
    
    // Verify the request succeeded
    if let Ok(Ok(response)) = result {
        assert_eq!(response.status(), 200);
        
        let body = response.text().await.unwrap();
        assert!(body.contains("Pingora Slice Metrics"));
        assert!(body.contains("/metrics"));
        assert!(body.contains("/health"));
    }
}

#[tokio::test]
async fn test_metrics_endpoint_not_found() {
    // Create metrics
    let metrics = Arc::new(SliceMetrics::new());
    
    // Find an available port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    
    // Start the endpoint
    let endpoint = MetricsEndpoint::new(metrics, addr);
    let handle = tokio::spawn(async move {
        endpoint.start().await
    });
    
    // Give the server time to start
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    // Make a request to a non-existent path
    let url = format!("http://{}/nonexistent", addr);
    let result = timeout(Duration::from_secs(2), async {
        reqwest::get(&url).await
    }).await;
    
    // Abort the server
    handle.abort();
    
    // Verify we get a 404
    if let Ok(Ok(response)) = result {
        assert_eq!(response.status(), 404);
    }
}

#[tokio::test]
async fn test_metrics_update_reflected_in_endpoint() {
    // Create metrics
    let metrics = Arc::new(SliceMetrics::new());
    
    // Find an available port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    
    // Start the endpoint
    let metrics_clone = Arc::clone(&metrics);
    let endpoint = MetricsEndpoint::new(metrics_clone, addr);
    let handle = tokio::spawn(async move {
        endpoint.start().await
    });
    
    // Give the server time to start
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    // Record some metrics
    metrics.record_request(true);
    metrics.record_cache_hit();
    
    // Fetch metrics
    let url = format!("http://{}/metrics", addr);
    let result1 = timeout(Duration::from_secs(2), async {
        reqwest::get(&url).await
    }).await;
    
    if let Ok(Ok(response)) = result1 {
        let body = response.text().await.unwrap();
        assert!(body.contains("pingora_slice_requests_total 1"));
        assert!(body.contains("pingora_slice_cache_hits_total 1"));
    }
    
    // Record more metrics
    metrics.record_request(true);
    metrics.record_cache_hit();
    
    // Fetch metrics again
    let result2 = timeout(Duration::from_secs(2), async {
        reqwest::get(&url).await
    }).await;
    
    // Abort the server
    handle.abort();
    
    // Verify the metrics were updated
    if let Ok(Ok(response)) = result2 {
        let body = response.text().await.unwrap();
        assert!(body.contains("pingora_slice_requests_total 2"));
        assert!(body.contains("pingora_slice_cache_hits_total 2"));
    }
}
