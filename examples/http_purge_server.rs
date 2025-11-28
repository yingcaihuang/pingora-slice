//! HTTP PURGE Server Example
//!
//! This example demonstrates how to set up an HTTP server that handles
//! PURGE requests for cache invalidation.
//!
//! Usage:
//!   cargo run --example http_purge_server
//!
//! Then test with curl:
//!   # Purge specific URL
//!   curl -X PURGE http://localhost:8080/test.dat
//!
//!   # Purge all cache
//!   curl -X PURGE http://localhost:8080/* -H "X-Purge-All: true"
//!
//!   # Purge with authentication
//!   curl -X PURGE http://localhost:8080/test.dat -H "Authorization: Bearer secret-token"

use bytes::Bytes;
use http::{Request, Response, StatusCode};
use http_body_util::{BodyExt, Empty, Full};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use pingora_slice::config::SliceConfig;
use pingora_slice::models::ByteRange;
use pingora_slice::purge_handler::PurgeHandler;
use pingora_slice::purge_metrics::PurgeMetrics;
use pingora_slice::tiered_cache::TieredCache;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tracing::{error, info};

/// Simple HTTP server state
struct ServerState {
    cache: Arc<TieredCache>,
    purge_handler: Arc<PurgeHandler>,
    purge_metrics: Option<Arc<PurgeMetrics>>,
}

impl ServerState {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Create cache directory
        let cache_dir = tempfile::tempdir()?;
        info!("Cache directory: {:?}", cache_dir.path());

        // Create tiered cache
        let cache = Arc::new(
            TieredCache::new(
                Duration::from_secs(3600), // 1 hour TTL
                10 * 1024 * 1024,          // 10MB L1
                cache_dir.path(),
            )
            .await?,
        );

        // Create PURGE metrics
        let purge_metrics = Arc::new(PurgeMetrics::new().expect("Failed to create purge metrics"));
        info!("PURGE metrics enabled");

        // Create PURGE handler (with optional authentication)
        let purge_handler = if std::env::var("PURGE_TOKEN").is_ok() {
            let token = std::env::var("PURGE_TOKEN").unwrap();
            info!("PURGE authentication enabled");
            Arc::new(
                PurgeHandler::with_auth(cache.clone(), token)
                    .with_metrics(purge_metrics.clone())
            )
        } else {
            info!("PURGE authentication disabled (set PURGE_TOKEN env var to enable)");
            Arc::new(
                PurgeHandler::new(cache.clone())
                    .with_metrics(purge_metrics.clone())
            )
        };

        // Pre-populate some test data
        Self::populate_test_data(&cache).await?;

        // Prevent cache_dir from being dropped
        std::mem::forget(cache_dir);

        Ok(Self {
            cache,
            purge_handler,
            purge_metrics: Some(purge_metrics),
        })
    }

    async fn populate_test_data(cache: &TieredCache) -> Result<(), Box<dyn std::error::Error>> {
        info!("Populating test data...");

        let test_data = vec![
            ("http://localhost:8080/test.dat", 5),
            ("http://localhost:8080/video.mp4", 10),
            ("http://localhost:8080/image.jpg", 3),
        ];

        for (url, slice_count) in test_data {
            for i in 0..slice_count {
                let start = i * 1024;
                let end = start + 1023;
                let range = ByteRange::new(start, end)?;
                let data = Bytes::from(vec![(i % 256) as u8; 1024]);
                cache.store(url, &range, data)?;
            }
            info!("  Cached: {} ({} slices)", url, slice_count);
        }

        let stats = cache.get_stats();
        info!(
            "Cache populated: {} entries, {} bytes",
            stats.l1_entries, stats.l1_bytes
        );

        Ok(())
    }
}

/// Handle incoming HTTP requests
async fn handle_request(
    state: Arc<ServerState>,
    req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    let method = req.method().clone();
    let uri = req.uri().clone();

    info!("{} {}", method, uri);

    // Check if this is a PURGE request
    if method.as_str() == "PURGE" {
        // Handle PURGE request
        match state.purge_handler.handle_purge(req).await {
            Ok(response) => Ok(response),
            Err(e) => {
                error!("PURGE request failed: {}", e);
                Ok(Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Full::new(Bytes::from(format!("Error: {}", e))))
                    .unwrap())
            }
        }
    } else if method == hyper::Method::GET && uri.path() == "/stats" {
        // Return cache statistics
        let stats = state.cache.get_stats();
        let json = serde_json::json!({
            "l1_entries": stats.l1_entries,
            "l1_bytes": stats.l1_bytes,
            "l1_hits": stats.l1_hits,
            "l2_hits": stats.l2_hits,
            "misses": stats.misses,
            "disk_writes": stats.disk_writes,
            "disk_errors": stats.disk_errors,
        });

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Full::new(Bytes::from(json.to_string())))
            .unwrap())
    } else if method == hyper::Method::GET && uri.path() == "/metrics" {
        // Return Prometheus metrics
        use prometheus::Encoder;
        let encoder = prometheus::TextEncoder::new();
        let metric_families = prometheus::gather();
        let mut buffer = vec![];
        encoder.encode(&metric_families, &mut buffer).unwrap();

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", encoder.format_type())
            .body(Full::new(Bytes::from(buffer)))
            .unwrap())
    } else if method == hyper::Method::GET {
        // Simple GET handler (for testing)
        let path = uri.path();
        let url = format!("http://localhost:8080{}", path);

        // Try to get from cache
        let range = ByteRange::new(0, 1023).unwrap();
        match state.cache.lookup(&url, &range).await {
            Ok(Some(data)) => {
                info!("Cache HIT: {}", url);
                Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header("x-cache", "HIT")
                    .body(Full::new(data))
                    .unwrap())
            }
            Ok(None) => {
                info!("Cache MISS: {}", url);
                Ok(Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .header("x-cache", "MISS")
                    .body(Full::new(Bytes::from("Not found in cache")))
                    .unwrap())
            }
            Err(e) => {
                error!("Cache lookup error: {}", e);
                Ok(Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Full::new(Bytes::from(format!("Error: {}", e))))
                    .unwrap())
            }
        }
    } else {
        // Method not allowed
        Ok(Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .body(Full::new(Bytes::from("Method not allowed")))
            .unwrap())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting HTTP PURGE server...");

    // Create server state
    let state = Arc::new(ServerState::new().await?);

    // Bind to address
    let addr: SocketAddr = "127.0.0.1:8080".parse()?;
    let listener = TcpListener::bind(addr).await?;

    info!("Server listening on http://{}", addr);
    info!("");
    info!("Try these commands:");
    info!("  # Get cache stats");
    info!("  curl http://localhost:8080/stats");
    info!("");
    info!("  # Get Prometheus metrics");
    info!("  curl http://localhost:8080/metrics");
    info!("");
    info!("  # Get cached file (should HIT)");
    info!("  curl http://localhost:8080/test.dat");
    info!("");
    info!("  # Purge specific URL");
    info!("  curl -X PURGE http://localhost:8080/test.dat");
    info!("");
    info!("  # Verify it's purged (should MISS)");
    info!("  curl http://localhost:8080/test.dat");
    info!("");
    info!("  # Purge all cache");
    info!("  curl -X PURGE http://localhost:8080/* -H 'X-Purge-All: true'");
    info!("");
    if std::env::var("PURGE_TOKEN").is_ok() {
        info!("  # Purge with authentication");
        info!(
            "  curl -X PURGE http://localhost:8080/test.dat -H 'Authorization: Bearer {}'",
            std::env::var("PURGE_TOKEN").unwrap()
        );
    } else {
        info!("  # Enable authentication:");
        info!("  PURGE_TOKEN=secret cargo run --example http_purge_server");
    }
    info!("");

    // Accept connections
    loop {
        let (stream, peer_addr) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let state = state.clone();

        tokio::spawn(async move {
            let service = service_fn(move |req| {
                let state = state.clone();
                async move { handle_request(state, req).await }
            });

            if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                error!("Connection error from {}: {}", peer_addr, err);
            }
        });
    }
}
