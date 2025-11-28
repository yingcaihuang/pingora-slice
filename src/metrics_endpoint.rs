//! Metrics HTTP Endpoint
//!
//! This module provides an HTTP endpoint to expose metrics in Prometheus format.
//! The endpoint can be started on a separate port and provides real-time metrics
//! about the slice module's operation.
//!
//! # Requirements
//! Validates: Requirements 9.5

use crate::metrics::{SliceMetrics, MetricsSnapshot};
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info};

/// Metrics endpoint server
///
/// Provides an HTTP server that exposes metrics in Prometheus format.
pub struct MetricsEndpoint {
    metrics: Arc<SliceMetrics>,
    addr: SocketAddr,
}

impl MetricsEndpoint {
    /// Create a new metrics endpoint
    ///
    /// # Arguments
    /// * `metrics` - Shared metrics collector
    /// * `addr` - Address to bind the HTTP server to
    ///
    /// # Example
    /// ```no_run
    /// use pingora_slice::metrics::SliceMetrics;
    /// use pingora_slice::metrics_endpoint::MetricsEndpoint;
    /// use std::sync::Arc;
    ///
    /// let metrics = Arc::new(SliceMetrics::new());
    /// let endpoint = MetricsEndpoint::new(metrics, "127.0.0.1:9090".parse().unwrap());
    /// ```
    pub fn new(metrics: Arc<SliceMetrics>, addr: SocketAddr) -> Self {
        Self { metrics, addr }
    }

    /// Start the metrics endpoint server
    ///
    /// This method starts an HTTP server that listens on the configured address
    /// and serves metrics in Prometheus format at the `/metrics` endpoint.
    ///
    /// The server runs indefinitely until the process is terminated.
    ///
    /// # Requirements
    /// Validates: Requirements 9.5
    ///
    /// # Example
    /// ```no_run
    /// use pingora_slice::metrics::SliceMetrics;
    /// use pingora_slice::metrics_endpoint::MetricsEndpoint;
    /// use std::sync::Arc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let metrics = Arc::new(SliceMetrics::new());
    ///     let endpoint = MetricsEndpoint::new(metrics, "127.0.0.1:9090".parse().unwrap());
    ///     endpoint.start().await.unwrap();
    /// }
    /// ```
    pub async fn start(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let listener = TcpListener::bind(self.addr).await?;
        info!("Metrics endpoint listening on http://{}", self.addr);
        info!("Metrics available at http://{}/metrics", self.addr);

        loop {
            let (stream, _) = listener.accept().await?;
            let io = TokioIo::new(stream);
            let metrics = Arc::clone(&self.metrics);

            tokio::task::spawn(async move {
                let service = service_fn(move |req| {
                    let metrics = Arc::clone(&metrics);
                    async move { handle_request(req, metrics).await }
                });

                if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                    error!("Error serving connection: {:?}", err);
                }
            });
        }
    }
}

/// Handle incoming HTTP requests
async fn handle_request(
    req: Request<hyper::body::Incoming>,
    metrics: Arc<SliceMetrics>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    match req.uri().path() {
        "/metrics" => Ok(metrics_response(metrics)),
        "/health" => Ok(health_response()),
        "/" => Ok(index_response()),
        _ => Ok(not_found_response()),
    }
}

/// Generate the metrics response in Prometheus format
///
/// # Requirements
/// Validates: Requirements 9.5
fn metrics_response(metrics: Arc<SliceMetrics>) -> Response<Full<Bytes>> {
    let snapshot = metrics.get_stats();
    let body = format_prometheus_metrics(&snapshot);

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/plain; version=0.0.4; charset=utf-8")
        .body(Full::new(Bytes::from(body)))
        .unwrap()
}

/// Format metrics in Prometheus exposition format
///
/// This function converts the metrics snapshot into Prometheus text format.
/// Each metric includes a HELP comment describing what it measures and a TYPE
/// comment indicating the metric type (counter, gauge, etc.).
///
/// # Requirements
/// Validates: Requirements 9.5
fn format_prometheus_metrics(snapshot: &MetricsSnapshot) -> String {
    let mut output = String::new();

    // Request metrics
    output.push_str("# HELP pingora_slice_requests_total Total number of requests processed\n");
    output.push_str("# TYPE pingora_slice_requests_total counter\n");
    output.push_str(&format!("pingora_slice_requests_total {}\n", snapshot.total_requests));
    output.push_str("\n");

    output.push_str("# HELP pingora_slice_sliced_requests_total Number of requests handled with slicing\n");
    output.push_str("# TYPE pingora_slice_sliced_requests_total counter\n");
    output.push_str(&format!("pingora_slice_sliced_requests_total {}\n", snapshot.sliced_requests));
    output.push_str("\n");

    output.push_str("# HELP pingora_slice_passthrough_requests_total Number of requests passed through without slicing\n");
    output.push_str("# TYPE pingora_slice_passthrough_requests_total counter\n");
    output.push_str(&format!("pingora_slice_passthrough_requests_total {}\n", snapshot.passthrough_requests));
    output.push_str("\n");

    // Cache metrics
    output.push_str("# HELP pingora_slice_cache_hits_total Number of cache hits\n");
    output.push_str("# TYPE pingora_slice_cache_hits_total counter\n");
    output.push_str(&format!("pingora_slice_cache_hits_total {}\n", snapshot.cache_hits));
    output.push_str("\n");

    output.push_str("# HELP pingora_slice_cache_misses_total Number of cache misses\n");
    output.push_str("# TYPE pingora_slice_cache_misses_total counter\n");
    output.push_str(&format!("pingora_slice_cache_misses_total {}\n", snapshot.cache_misses));
    output.push_str("\n");

    output.push_str("# HELP pingora_slice_cache_errors_total Number of cache errors\n");
    output.push_str("# TYPE pingora_slice_cache_errors_total counter\n");
    output.push_str(&format!("pingora_slice_cache_errors_total {}\n", snapshot.cache_errors));
    output.push_str("\n");

    output.push_str("# HELP pingora_slice_cache_hit_rate Cache hit rate percentage\n");
    output.push_str("# TYPE pingora_slice_cache_hit_rate gauge\n");
    output.push_str(&format!("pingora_slice_cache_hit_rate {:.2}\n", snapshot.cache_hit_rate()));
    output.push_str("\n");

    // Subrequest metrics
    output.push_str("# HELP pingora_slice_subrequests_total Total number of subrequests sent\n");
    output.push_str("# TYPE pingora_slice_subrequests_total counter\n");
    output.push_str(&format!("pingora_slice_subrequests_total {}\n", snapshot.total_subrequests));
    output.push_str("\n");

    output.push_str("# HELP pingora_slice_failed_subrequests_total Number of failed subrequests\n");
    output.push_str("# TYPE pingora_slice_failed_subrequests_total counter\n");
    output.push_str(&format!("pingora_slice_failed_subrequests_total {}\n", snapshot.failed_subrequests));
    output.push_str("\n");

    output.push_str("# HELP pingora_slice_retried_subrequests_total Number of retried subrequests\n");
    output.push_str("# TYPE pingora_slice_retried_subrequests_total counter\n");
    output.push_str(&format!("pingora_slice_retried_subrequests_total {}\n", snapshot.retried_subrequests));
    output.push_str("\n");

    output.push_str("# HELP pingora_slice_subrequest_failure_rate Subrequest failure rate percentage\n");
    output.push_str("# TYPE pingora_slice_subrequest_failure_rate gauge\n");
    output.push_str(&format!("pingora_slice_subrequest_failure_rate {:.2}\n", snapshot.subrequest_failure_rate()));
    output.push_str("\n");

    // Byte metrics
    output.push_str("# HELP pingora_slice_bytes_from_origin_total Total bytes received from origin\n");
    output.push_str("# TYPE pingora_slice_bytes_from_origin_total counter\n");
    output.push_str(&format!("pingora_slice_bytes_from_origin_total {}\n", snapshot.bytes_from_origin));
    output.push_str("\n");

    output.push_str("# HELP pingora_slice_bytes_from_cache_total Total bytes received from cache\n");
    output.push_str("# TYPE pingora_slice_bytes_from_cache_total counter\n");
    output.push_str(&format!("pingora_slice_bytes_from_cache_total {}\n", snapshot.bytes_from_cache));
    output.push_str("\n");

    output.push_str("# HELP pingora_slice_bytes_to_client_total Total bytes sent to client\n");
    output.push_str("# TYPE pingora_slice_bytes_to_client_total counter\n");
    output.push_str(&format!("pingora_slice_bytes_to_client_total {}\n", snapshot.bytes_to_client));
    output.push_str("\n");

    // Latency metrics (in milliseconds)
    output.push_str("# HELP pingora_slice_request_duration_ms_avg Average request duration in milliseconds\n");
    output.push_str("# TYPE pingora_slice_request_duration_ms_avg gauge\n");
    output.push_str(&format!("pingora_slice_request_duration_ms_avg {:.2}\n", snapshot.avg_request_duration_ms()));
    output.push_str("\n");

    output.push_str("# HELP pingora_slice_subrequest_duration_ms_avg Average subrequest duration in milliseconds\n");
    output.push_str("# TYPE pingora_slice_subrequest_duration_ms_avg gauge\n");
    output.push_str(&format!("pingora_slice_subrequest_duration_ms_avg {:.2}\n", snapshot.avg_subrequest_duration_ms()));
    output.push_str("\n");

    output.push_str("# HELP pingora_slice_assembly_duration_ms_avg Average assembly duration in milliseconds\n");
    output.push_str("# TYPE pingora_slice_assembly_duration_ms_avg gauge\n");
    output.push_str(&format!("pingora_slice_assembly_duration_ms_avg {:.2}\n", snapshot.avg_assembly_duration_ms()));
    output.push_str("\n");

    output
}

/// Generate health check response
fn health_response() -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(r#"{"status":"healthy"}"#)))
        .unwrap()
}

/// Generate index page response
fn index_response() -> Response<Full<Bytes>> {
    let body = r#"<!DOCTYPE html>
<html>
<head>
    <title>Pingora Slice Metrics</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; }
        h1 { color: #333; }
        a { color: #0066cc; text-decoration: none; }
        a:hover { text-decoration: underline; }
        .endpoint { margin: 10px 0; padding: 10px; background: #f5f5f5; border-radius: 4px; }
    </style>
</head>
<body>
    <h1>Pingora Slice Metrics Endpoint</h1>
    <p>Available endpoints:</p>
    <div class="endpoint">
        <strong><a href="/metrics">/metrics</a></strong> - Prometheus format metrics
    </div>
    <div class="endpoint">
        <strong><a href="/health">/health</a></strong> - Health check endpoint
    </div>
</body>
</html>"#;

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/html; charset=utf-8")
        .body(Full::new(Bytes::from(body)))
        .unwrap()
}

/// Generate 404 response
fn not_found_response() -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("Content-Type", "text/plain")
        .body(Full::new(Bytes::from("404 Not Found")))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_prometheus_metrics() {
        let metrics = SliceMetrics::new();
        
        // Record some test data
        metrics.record_request(true);
        metrics.record_request(true);
        metrics.record_request(false);
        metrics.record_cache_hit();
        metrics.record_cache_miss();
        metrics.record_subrequest(true);
        metrics.record_subrequest(false);
        metrics.record_bytes_from_origin(1000);
        metrics.record_bytes_from_cache(500);
        metrics.record_bytes_to_client(1500);

        let snapshot = metrics.get_stats();
        let output = format_prometheus_metrics(&snapshot);

        // Verify the output contains expected metrics
        assert!(output.contains("pingora_slice_requests_total 3"));
        assert!(output.contains("pingora_slice_sliced_requests_total 2"));
        assert!(output.contains("pingora_slice_passthrough_requests_total 1"));
        assert!(output.contains("pingora_slice_cache_hits_total 1"));
        assert!(output.contains("pingora_slice_cache_misses_total 1"));
        assert!(output.contains("pingora_slice_subrequests_total 2"));
        assert!(output.contains("pingora_slice_failed_subrequests_total 1"));
        assert!(output.contains("pingora_slice_bytes_from_origin_total 1000"));
        assert!(output.contains("pingora_slice_bytes_from_cache_total 500"));
        assert!(output.contains("pingora_slice_bytes_to_client_total 1500"));
        
        // Verify HELP and TYPE comments are present
        assert!(output.contains("# HELP pingora_slice_requests_total"));
        assert!(output.contains("# TYPE pingora_slice_requests_total counter"));
        assert!(output.contains("# HELP pingora_slice_cache_hit_rate"));
        assert!(output.contains("# TYPE pingora_slice_cache_hit_rate gauge"));
    }

    #[test]
    fn test_format_prometheus_metrics_empty() {
        let metrics = SliceMetrics::new();
        let snapshot = metrics.get_stats();
        let output = format_prometheus_metrics(&snapshot);

        // Verify all metrics are present with zero values
        assert!(output.contains("pingora_slice_requests_total 0"));
        assert!(output.contains("pingora_slice_cache_hits_total 0"));
        assert!(output.contains("pingora_slice_subrequests_total 0"));
        assert!(output.contains("pingora_slice_cache_hit_rate 0.00"));
    }

    #[test]
    fn test_format_prometheus_metrics_calculated_values() {
        let metrics = SliceMetrics::new();
        
        // Record data to test calculated metrics
        metrics.record_cache_hit();
        metrics.record_cache_hit();
        metrics.record_cache_hit();
        metrics.record_cache_miss();
        
        let snapshot = metrics.get_stats();
        let output = format_prometheus_metrics(&snapshot);

        // Cache hit rate should be 75%
        assert!(output.contains("pingora_slice_cache_hit_rate 75.00"));
    }

    #[test]
    fn test_health_response() {
        let response = health_response();
        assert_eq!(response.status(), StatusCode::OK);
        
        let content_type = response.headers().get("Content-Type").unwrap();
        assert_eq!(content_type, "application/json");
    }

    #[test]
    fn test_index_response() {
        let response = index_response();
        assert_eq!(response.status(), StatusCode::OK);
        
        let content_type = response.headers().get("Content-Type").unwrap();
        assert_eq!(content_type, "text/html; charset=utf-8");
    }

    #[test]
    fn test_not_found_response() {
        let response = not_found_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
