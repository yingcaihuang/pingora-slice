//! Pingora Slice Module Server with HTTP PURGE Support
//!
//! This is the main entry point for the Pingora Slice proxy server.
//! It provides a complete HTTP server with:
//! - Two-tier cache (L1 memory + L2 disk)
//! - HTTP PURGE support for cache invalidation
//! - Prometheus metrics endpoint
//! - Cache statistics endpoint

use bytes::Bytes;
use http::{Request, Response, StatusCode};
use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use pingora_slice::config::SliceConfig;
use pingora_slice::models::ByteRange;
use pingora_slice::purge_handler::PurgeHandler;
use pingora_slice::purge_metrics::PurgeMetrics;
use pingora_slice::tiered_cache::TieredCache;
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info, warn};

/// Server state containing cache and PURGE handler
struct ServerState {
    cache: Arc<TieredCache>,
    purge_handler: Arc<PurgeHandler>,
    purge_metrics: Arc<PurgeMetrics>,
    config: Arc<SliceConfig>,
}

impl ServerState {
    /// Create new server state from configuration
    async fn new(config: SliceConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let config = Arc::new(config);

        // Get L1 cache size from config
        let l1_size = config.l1_cache_size_bytes;
        let ttl = std::time::Duration::from_secs(config.cache_ttl);

        // Create tiered cache based on backend configuration
        let cache = if config.l2_backend == "raw_disk" {
            // Use raw disk cache backend
            if let Some(raw_disk_config) = &config.raw_disk_cache {
                info!("Initializing raw disk cache backend");
                
                Arc::new(
                    TieredCache::new_with_raw_disk(
                        ttl,
                        l1_size,
                        &raw_disk_config.device_path,
                        raw_disk_config.total_size,
                        raw_disk_config.block_size,
                        raw_disk_config.use_direct_io,
                    )
                    .await?,
                )
            } else {
                return Err(anyhow::anyhow!(
                    "raw_disk_cache configuration required when l2_backend is 'raw_disk'"
                ));
            }
        } else {
            // Use file-based cache backend (default)
            let cache_dir = &config.l2_cache_dir;

            // Create cache directory if it doesn't exist
            std::fs::create_dir_all(cache_dir)?;
            
            Arc::new(
                TieredCache::new(ttl, l1_size, cache_dir).await?,
            )
        };

        info!("Two-tier cache initialized:");
        info!("  - L1 (memory): {} MB", l1_size / 1024 / 1024);
        if config.l2_backend == "raw_disk" {
            if let Some(raw_disk_config) = &config.raw_disk_cache {
                info!("  - L2 (raw_disk): {}", raw_disk_config.device_path);
                info!("    - Total size: {} GB", raw_disk_config.total_size / (1024 * 1024 * 1024));
                info!("    - Block size: {} KB", raw_disk_config.block_size / 1024);
                info!("    - Direct I/O: {}", raw_disk_config.use_direct_io);
            }
        } else {
            info!("  - L2 (file): {}", config.l2_cache_dir);
        }
        info!("  - TTL: {} seconds", config.cache_ttl);

        // Create PURGE metrics
        let purge_metrics = Arc::new(PurgeMetrics::new()?);
        info!("PURGE metrics enabled");

        // Create PURGE handler with optional authentication
        let purge_handler = if let Ok(token) = env::var("PURGE_TOKEN") {
            info!("PURGE authentication enabled");
            Arc::new(
                PurgeHandler::with_auth(cache.clone(), token)
                    .with_metrics(purge_metrics.clone()),
            )
        } else {
            info!("PURGE authentication disabled (set PURGE_TOKEN env var to enable)");
            Arc::new(PurgeHandler::new(cache.clone()).with_metrics(purge_metrics.clone()))
        };

        Ok(Self {
            cache,
            purge_handler,
            purge_metrics,
            config,
        })
    }
}

/// Handle incoming HTTP requests
async fn handle_request(
    state: Arc<ServerState>,
    req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let path = uri.path();

    info!("{} {}", method, uri);

    // Route requests
    match (method.as_str(), path) {
        // PURGE requests
        ("PURGE", _) => handle_purge(state, req).await,

        // Cache statistics endpoint
        ("GET", "/stats") => handle_stats(state).await,

        // Prometheus metrics endpoint
        ("GET", "/metrics") => handle_metrics().await,

        // Health check endpoint
        ("GET", "/health") => Ok(Response::builder()
            .status(StatusCode::OK)
            .body(Full::new(Bytes::from("OK")))
            .unwrap()),

        // Regular GET requests (for testing cache)
        ("GET", _) => handle_get(state, uri.to_string()).await,

        // Method not allowed
        _ => Ok(Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .body(Full::new(Bytes::from("Method not allowed")))
            .unwrap()),
    }
}

/// Handle PURGE requests
async fn handle_purge(
    state: Arc<ServerState>,
    req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
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
}

/// Handle cache statistics requests
async fn handle_stats(
    state: Arc<ServerState>,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    let stats = state.cache.get_stats();
    let json = serde_json::json!({
        "cache": {
            "l1": {
                "entries": stats.l1_entries,
                "bytes": stats.l1_bytes,
                "hits": stats.l1_hits,
            },
            "l2": {
                "hits": stats.l2_hits,
                "writes": stats.disk_writes,
                "errors": stats.disk_errors,
            },
            "misses": stats.misses,
        },
        "config": {
            "slice_size": state.config.slice_size,
            "max_concurrent": state.config.max_concurrent_subrequests,
            "cache_ttl": state.config.cache_ttl,
        }
    });

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(json.to_string())))
        .unwrap())
}

/// Handle Prometheus metrics requests
async fn handle_metrics() -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = vec![];

    if let Err(e) = encoder.encode(&metric_families, &mut buffer) {
        error!("Failed to encode metrics: {}", e);
        return Ok(Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Full::new(Bytes::from("Failed to encode metrics")))
            .unwrap());
    }

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", encoder.format_type())
        .body(Full::new(Bytes::from(buffer)))
        .unwrap())
}

/// Handle GET requests (for testing cache functionality)
async fn handle_get(
    state: Arc<ServerState>,
    url: String,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    // Try to get first slice from cache
    let range = match ByteRange::new(0, 1023) {
        Ok(r) => r,
        Err(e) => {
            error!("Invalid range: {}", e);
            return Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Full::new(Bytes::from("Invalid range")))
                .unwrap());
        }
    };

    match state.cache.lookup(&url, &range).await {
        Ok(Some(data)) => {
            info!("Cache HIT: {}", url);
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("x-cache", "HIT")
                .header("content-type", "application/octet-stream")
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
}

/// Print usage information
fn print_usage() {
    info!("");
    info!("=== Pingora Slice Server ===");
    info!("");
    info!("Available endpoints:");
    info!("  GET  /health          - Health check");
    info!("  GET  /stats           - Cache statistics (JSON)");
    info!("  GET  /metrics         - Prometheus metrics");
    info!("  GET  <any-path>       - Test cache lookup");
    info!("  PURGE <url>           - Purge specific URL");
    info!("  PURGE * + X-Purge-All - Purge all cache");
    info!("");
    info!("Example commands:");
    info!("  # Health check");
    info!("  curl http://localhost:8080/health");
    info!("");
    info!("  # Get cache stats");
    info!("  curl http://localhost:8080/stats");
    info!("");
    info!("  # Get Prometheus metrics");
    info!("  curl http://localhost:8080/metrics");
    info!("");
    info!("  # Purge specific URL");
    info!("  curl -X PURGE http://localhost:8080/test.dat");
    info!("");
    info!("  # Purge all cache");
    info!("  curl -X PURGE http://localhost:8080/* -H 'X-Purge-All: true'");
    info!("");

    if env::var("PURGE_TOKEN").is_ok() {
        info!("  # Purge with authentication");
        info!(
            "  curl -X PURGE http://localhost:8080/test.dat -H 'Authorization: Bearer {}'",
            env::var("PURGE_TOKEN").unwrap()
        );
    } else {
        info!("  # Enable PURGE authentication:");
        info!("  PURGE_TOKEN=secret cargo run -- pingora_slice.yaml");
    }
    info!("");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .with_thread_ids(true)
        .with_line_number(true)
        .init();

    info!("Starting Pingora Slice Module Server");

    // Get config file path from command line or use default
    let config_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "pingora_slice.yaml".to_string());

    info!("Loading configuration from: {}", config_path);

    // Load configuration
    let config = match SliceConfig::from_file(&config_path) {
        Ok(cfg) => {
            info!("Configuration loaded successfully");
            info!("  - Slice size: {} bytes ({} KB)", cfg.slice_size, cfg.slice_size / 1024);
            info!("  - Max concurrent subrequests: {}", cfg.max_concurrent_subrequests);
            info!("  - Max retries: {}", cfg.max_retries);
            info!("  - Cache enabled: {}", cfg.enable_cache);
            info!("  - Cache TTL: {} seconds", cfg.cache_ttl);
            info!("  - Upstream address: {}", cfg.upstream_address);
            cfg
        }
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            error!("Please ensure the configuration file exists and is valid");
            std::process::exit(1);
        }
    };

    // Get listen address from config or use default
    let listen_addr = config
        .listen_address
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or("127.0.0.1:8080");

    // Create server state
    let state = match ServerState::new(config).await {
        Ok(s) => Arc::new(s),
        Err(e) => {
            error!("Failed to initialize server: {}", e);
            std::process::exit(1);
        }
    };

    // Parse and bind to address
    let addr: SocketAddr = match listen_addr.parse() {
        Ok(a) => a,
        Err(e) => {
            error!("Invalid listen address '{}': {}", listen_addr, e);
            std::process::exit(1);
        }
    };

    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            error!("Failed to bind to {}: {}", addr, e);
            std::process::exit(1);
        }
    };

    info!("Server listening on http://{}", addr);
    print_usage();

    // Accept connections
    loop {
        let (stream, peer_addr) = match listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                warn!("Failed to accept connection: {}", e);
                continue;
            }
        };

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
