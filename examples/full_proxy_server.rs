//! Full proxy server with origin fetch support
//!
//! This example demonstrates a complete proxy server that:
//! - Accepts HTTP requests
//! - Checks cache for content
//! - Fetches from origin if cache miss
//! - Stores fetched content in cache
//! - Returns response to client

use bytes::Bytes;
use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use pingora_slice::{ByteRange, SliceConfig, TieredCache};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tracing::{error, info, warn};

struct ProxyState {
    cache: Arc<TieredCache>,
    upstream_base: String,
    http_client: reqwest::Client,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("=== Full Proxy Server with Origin Fetch ===");
    info!("");

    // Load configuration
    let args: Vec<String> = std::env::args().collect();
    let config_path = args.get(1).map(|s| s.as_str()).unwrap_or("pingora_slice_raw_disk_full.yaml");
    
    info!("Loading configuration from: {}", config_path);
    let config = SliceConfig::from_file(config_path)?;
    
    // Create cache
    let ttl = Duration::from_secs(config.cache_ttl);
    let l1_size = config.l1_cache_size_bytes;
    
    let cache = if config.l2_backend == "raw_disk" {
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
            return Err("raw_disk_cache configuration required".into());
        }
    } else {
        Arc::new(TieredCache::new(ttl, l1_size, &config.l2_cache_dir).await?)
    };

    info!("Cache initialized:");
    info!("  - L1 (memory): {} MB", l1_size / 1024 / 1024);
    if config.l2_backend == "raw_disk" {
        if let Some(raw_disk_config) = &config.raw_disk_cache {
            info!("  - L2 (raw_disk): {}", raw_disk_config.device_path);
        }
    } else {
        info!("  - L2 (file): {}", config.l2_cache_dir);
    }

    // Create HTTP client for upstream requests
    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    // Create proxy state
    let state = Arc::new(ProxyState {
        cache,
        upstream_base: config.upstream_address.clone(),
        http_client,
    });

    // Start server
    let addr: SocketAddr = "127.0.0.1:8080".parse()?;
    let listener = TcpListener::bind(addr).await?;
    
    info!("");
    info!("Proxy server listening on http://{}", addr);
    info!("");
    info!("Try these commands:");
    info!("  # Fetch a file (will fetch from origin on first request)");
    info!("  curl http://localhost:8080/dl/15m.iso -o /dev/null");
    info!("");
    info!("  # Check cache stats");
    info!("  curl http://localhost:8080/stats");
    info!("");

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let state = state.clone();

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(move |req| handle_request(state.clone(), req)))
                .await
            {
                error!("Error serving connection: {:?}", err);
            }
        });
    }
}

async fn handle_request(
    state: Arc<ProxyState>,
    req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let path = uri.path();

    info!("{} {}", method, uri);

    match (method.as_str(), path) {
        ("GET", "/stats") => handle_stats(state).await,
        ("GET", "/health") => Ok(Response::builder()
            .status(StatusCode::OK)
            .body(Full::new(Bytes::from("OK")))
            .unwrap()),
        ("GET", _) => handle_proxy_request(state, uri.to_string()).await,
        _ => Ok(Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .body(Full::new(Bytes::from("Method not allowed")))
            .unwrap()),
    }
}

async fn handle_proxy_request(
    state: Arc<ProxyState>,
    url: String,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    // Try cache first
    let range = match ByteRange::new(0, 1024 * 1024 - 1) {
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
            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header("x-cache", "HIT")
                .header("content-type", "application/octet-stream")
                .body(Full::new(data))
                .unwrap());
        }
        Ok(None) => {
            info!("Cache MISS: {}, fetching from origin", url);
        }
        Err(e) => {
            warn!("Cache lookup error: {}, fetching from origin", e);
        }
    }

    // Cache miss - fetch from origin
    // If url is already a full URL, use it directly; otherwise construct it
    let upstream_url = if url.starts_with("http://") || url.starts_with("https://") {
        url.clone()
    } else {
        format!("http://{}{}", state.upstream_base, url)
    };
    info!("Fetching from upstream: {}", upstream_url);

    match state.http_client.get(&upstream_url).send().await {
        Ok(response) => {
            let status = response.status();
            info!("Upstream response: {}", status);

            if !status.is_success() {
                return Ok(Response::builder()
                    .status(status)
                    .header("x-cache", "MISS")
                    .body(Full::new(Bytes::from(format!("Upstream error: {}", status))))
                    .unwrap());
            }

            match response.bytes().await {
                Ok(data) => {
                    info!("Fetched {} bytes from upstream", data.len());

                    // Store in cache (async operation handled internally)
                    if let Err(e) = state.cache.store(&url, &range, data.clone()) {
                        warn!("Failed to store in cache: {}", e);
                    } else {
                        info!("Stored in cache: {}", url);
                    }

                    Ok(Response::builder()
                        .status(StatusCode::OK)
                        .header("x-cache", "MISS")
                        .header("content-type", "application/octet-stream")
                        .body(Full::new(data))
                        .unwrap())
                }
                Err(e) => {
                    error!("Failed to read response body: {}", e);
                    Ok(Response::builder()
                        .status(StatusCode::BAD_GATEWAY)
                        .body(Full::new(Bytes::from(format!("Failed to read response: {}", e))))
                        .unwrap())
                }
            }
        }
        Err(e) => {
            error!("Failed to fetch from upstream: {}", e);
            Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .header("x-cache", "MISS")
                .body(Full::new(Bytes::from(format!("Upstream error: {}", e))))
                .unwrap())
        }
    }
}

async fn handle_stats(
    state: Arc<ProxyState>,
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
        }
    });

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(json.to_string())))
        .unwrap())
}
