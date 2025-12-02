//! Streaming Proxy Example
//!
//! This example demonstrates how to use the StreamingProxy with Pingora framework.
//! The streaming proxy provides real-time data forwarding while caching in the background.
//!
//! Features:
//! - Real-time streaming: forwards data chunks to client immediately
//! - Background caching: caches data while streaming
//! - Memory efficient: stable memory usage regardless of file size
//! - Cache-first: checks cache before fetching from origin
//!
//! Usage:
//!   cargo run --example streaming_proxy_example
//!
//! Then test with curl:
//!   curl http://localhost:8080/test.dat -o /dev/null

use pingora::prelude::*;
use pingora::proxy::http_proxy_service;
use pingora_slice::{SliceConfig, StreamingProxy, TieredCache};
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("=== Streaming Proxy Example ===");
    info!("");

    // Create Pingora server
    let mut my_server = Server::new(None).unwrap();
    my_server.bootstrap();

    // Create runtime for async initialization
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    let proxy = rt.block_on(async {
        // Load configuration
        let config = SliceConfig {
            slice_size: 1024 * 1024, // 1MB
            max_concurrent_subrequests: 4,
            max_retries: 3,
            slice_patterns: vec![".*".to_string()],
            enable_cache: true,
            cache_ttl: 3600,
            upstream_address: "mirrors.verycloud.cn:80".to_string(),
            l1_cache_size_bytes: 10 * 1024 * 1024, // 10MB
            l2_cache_dir: "/tmp/streaming-cache".to_string(),
            l2_backend: "file".to_string(),
            enable_l2_cache: true,
            raw_disk_cache: None,
            metrics_endpoint: None,
            purge: None,
        };
        
        info!("Configuration:");
        info!("  Upstream: {}", config.upstream_address);
        info!("  Cache enabled: {}", config.enable_cache);
        info!("  L1 cache size: {} MB", config.l1_cache_size_bytes / 1024 / 1024);
        info!("");

        // Create cache
        let cache = Arc::new(
            TieredCache::new(
                Duration::from_secs(config.cache_ttl),
                config.l1_cache_size_bytes,
                &config.l2_cache_dir,
            )
            .await
            .expect("Failed to create cache"),
        );

        info!("Cache initialized:");
        info!("  L1 (memory): {} MB", config.l1_cache_size_bytes / 1024 / 1024);
        info!("  L2 (file): {}", config.l2_cache_dir);
        info!("");

        // Create streaming proxy
        StreamingProxy::new(cache, Arc::new(config))
    });

    // Create proxy service
    let mut proxy_service = http_proxy_service(&my_server.configuration, proxy);
    
    // Listen on port 8080
    proxy_service.add_tcp("0.0.0.0:8080");

    info!("Streaming proxy server started on http://0.0.0.0:8080");
    info!("");
    info!("Try these commands:");
    info!("  # Fetch a file (will stream from origin on first request)");
    info!("  curl http://localhost:8080/dl/15m.iso -o /dev/null");
    info!("");
    info!("  # Fetch again (should be faster from cache)");
    info!("  curl http://localhost:8080/dl/15m.iso -o /dev/null");
    info!("");

    // Add service to server
    my_server.add_service(proxy_service);

    // Run server
    my_server.run_forever();
}
