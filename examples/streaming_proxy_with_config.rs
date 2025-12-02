//! Streaming Proxy with Configuration and Monitoring Example
//!
//! This example demonstrates the full integration of:
//! - Configuration loading from YAML file
//! - TieredCache with raw disk support
//! - Prometheus metrics
//! - Health check endpoint
//!
//! Usage:
//!   cargo run --example streaming_proxy_with_config
//!
//! Then test with:
//!   # Fetch a file
//!   curl http://localhost:8080/test.dat -o /dev/null
//!
//!   # Check health
//!   curl http://localhost:8081/health
//!
//!   # Check metrics (if enabled)
//!   curl http://localhost:9090/metrics

use pingora::prelude::*;
use pingora::proxy::http_proxy_service;
use pingora_slice::{HealthCheckService, StreamingProxy};
use std::sync::Arc;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("=== Streaming Proxy with Configuration and Monitoring ===");
    info!("");

    // Load configuration from file
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "pingora_slice.yaml".to_string());

    info!("Loading configuration from: {}", config_path);

    // Create streaming proxy from configuration
    let proxy = StreamingProxy::from_config(&config_path).await?;

    info!("Streaming proxy initialized successfully");
    info!("");

    // Print configuration summary
    {
        let config = proxy.config();
        info!("Configuration Summary:");
        info!("  Upstream: {}", config.upstream_address);
        info!("  Cache enabled: {}", config.enable_cache);
        info!("  L1 cache size: {} MB", config.l1_cache_size_bytes / 1024 / 1024);
        info!("  L2 cache enabled: {}", config.enable_l2_cache);
        info!("  L2 backend: {}", config.l2_backend);
        
        if let Some(ref raw_disk_config) = config.raw_disk_cache {
            info!("  Raw disk cache:");
            info!("    Device: {}", raw_disk_config.device_path);
            info!("    Size: {} GB", raw_disk_config.total_size / (1024 * 1024 * 1024));
            info!("    Block size: {} KB", raw_disk_config.block_size / 1024);
            info!("    O_DIRECT: {}", raw_disk_config.use_direct_io);
            info!("    Compression: {}", raw_disk_config.enable_compression);
        }
        info!("");
    }

    // Start health check endpoint
    let health_service = Arc::new(HealthCheckService::new());
    let health_addr = "127.0.0.1:8081".parse::<std::net::SocketAddr>().unwrap();
    
    info!("Starting health check endpoint on http://{}", health_addr);
    let health_service_clone = health_service.clone();
    tokio::spawn(async move {
        if let Err(e) = health_service_clone.start(health_addr).await {
            eprintln!("Health check server error: {}", e);
        }
    });

    // Print initial cache statistics
    let cache_stats = proxy.cache_stats();
    info!("Initial Cache Statistics:");
    info!("  L1 entries: {}", cache_stats.l1_entries);
    info!("  L1 bytes: {} KB", cache_stats.l1_bytes / 1024);
    info!("  L1 hits: {}", cache_stats.l1_hits);
    info!("  L2 hits: {}", cache_stats.l2_hits);
    info!("  Misses: {}", cache_stats.misses);
    info!("");

    // Print raw disk statistics if available
    if let Some(raw_stats) = proxy.raw_disk_stats().await {
        info!("Raw Disk Cache Statistics:");
        info!("  Total blocks: {}", raw_stats.total_blocks);
        info!("  Used blocks: {}", raw_stats.used_blocks);
        info!("  Free blocks: {}", raw_stats.free_blocks);
        info!("  Fragmentation: {:.2}%", raw_stats.fragmentation_ratio * 100.0);
        info!("  Cache entries: {}", raw_stats.entries);
        let total_requests = raw_stats.hits + raw_stats.misses;
        let hit_rate = if total_requests > 0 {
            (raw_stats.hits as f64 / total_requests as f64) * 100.0
        } else {
            0.0
        };
        info!("  Hit rate: {:.2}%", hit_rate);
        info!("");
    }

    // Create Pingora server
    let mut my_server = Server::new(None)?;
    my_server.bootstrap();

    // Create proxy service
    let mut proxy_service = http_proxy_service(&my_server.configuration, proxy);
    
    // Listen on port 8080
    proxy_service.add_tcp("0.0.0.0:8080");

    info!("Streaming proxy server started on http://0.0.0.0:8080");
    info!("");
    info!("Available endpoints:");
    info!("  Proxy:  http://localhost:8080/<path>");
    info!("  Health: http://localhost:8081/health");
    info!("  Ready:  http://localhost:8081/ready");
    info!("  Live:   http://localhost:8081/live");
    info!("");
    info!("Try these commands:");
    info!("  # Fetch a file (will stream from origin on first request)");
    info!("  curl http://localhost:8080/dl/15m.iso -o /dev/null");
    info!("");
    info!("  # Fetch again (should be faster from cache)");
    info!("  curl http://localhost:8080/dl/15m.iso -o /dev/null");
    info!("");
    info!("  # Check health status");
    info!("  curl http://localhost:8081/health");
    info!("");

    // Add service to server
    my_server.add_service(proxy_service);

    // Run server
    my_server.run_forever();
}
