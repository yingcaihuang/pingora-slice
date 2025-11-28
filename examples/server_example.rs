//! Example demonstrating server startup and configuration
//!
//! This example shows how to programmatically create and configure
//! a SliceProxy instance, similar to what the main server does.

use pingora_slice::{SliceConfig, SliceProxy};
use std::sync::Arc;

fn main() {
    println!("=== Pingora Slice Server Example ===\n");
    
    // Example 1: Create with default configuration
    println!("1. Creating SliceProxy with default configuration:");
    let default_config = SliceConfig::default();
    let proxy1 = SliceProxy::new(Arc::new(default_config.clone()));
    
    println!("   Slice size: {} KB", default_config.slice_size / 1024);
    println!("   Max concurrent: {}", default_config.max_concurrent_subrequests);
    println!("   Max retries: {}", default_config.max_retries);
    println!("   Cache enabled: {}", default_config.enable_cache);
    println!("   Cache TTL: {} seconds", default_config.cache_ttl);
    println!("   Upstream: {}\n", default_config.upstream_address);
    
    // Example 2: Create with custom configuration
    println!("2. Creating SliceProxy with custom configuration:");
    let custom_config = SliceConfig {
        slice_size: 512 * 1024, // 512KB
        max_concurrent_subrequests: 8,
        max_retries: 5,
        slice_patterns: vec![
            "/large-files/.*".to_string(),
            "/downloads/.*\\.bin$".to_string(),
        ],
        enable_cache: true,
        cache_ttl: 7200, // 2 hours
        upstream_address: "origin.example.com:80".to_string(),
        metrics_endpoint: None,
    };
    
    let _proxy2 = SliceProxy::new(Arc::new(custom_config.clone()));
    
    println!("   Slice size: {} KB", custom_config.slice_size / 1024);
    println!("   Max concurrent: {}", custom_config.max_concurrent_subrequests);
    println!("   Max retries: {}", custom_config.max_retries);
    println!("   Patterns: {:?}", custom_config.slice_patterns);
    println!("   Cache TTL: {} seconds", custom_config.cache_ttl);
    println!("   Upstream: {}\n", custom_config.upstream_address);
    
    // Example 3: Load from YAML file
    println!("3. Loading configuration from YAML file:");
    match SliceConfig::from_file("examples/pingora_slice.yaml") {
        Ok(yaml_config) => {
            let proxy3 = SliceProxy::new(Arc::new(yaml_config.clone()));
            
            println!("   Configuration loaded successfully!");
            println!("   Slice size: {} KB", yaml_config.slice_size / 1024);
            println!("   Max concurrent: {}", yaml_config.max_concurrent_subrequests);
            println!("   Patterns: {:?}", yaml_config.slice_patterns);
            println!("   Upstream: {}\n", yaml_config.upstream_address);
            
            // Access metrics
            let stats = proxy3.metrics().get_stats();
            println!("   Initial metrics:");
            println!("     Total requests: {}", stats.total_requests);
            println!("     Cache hits: {}", stats.cache_hits);
        }
        Err(e) => {
            println!("   Error loading config: {}", e);
        }
    }
    
    // Example 4: Configuration validation
    println!("\n4. Testing configuration validation:");
    
    // Valid configuration
    match SliceConfig::new(1024 * 1024, 4, 3) {
        Ok(_) => println!("   ✓ Valid config (1MB slice size) accepted"),
        Err(e) => println!("   ✗ Error: {}", e),
    }
    
    // Invalid configuration (slice too small)
    match SliceConfig::new(1024, 4, 3) {
        Ok(_) => println!("   ✗ Invalid config should have been rejected"),
        Err(e) => println!("   ✓ Invalid config (1KB slice size) rejected: {}", e),
    }
    
    // Invalid configuration (slice too large)
    match SliceConfig::new(20 * 1024 * 1024, 4, 3) {
        Ok(_) => println!("   ✗ Invalid config should have been rejected"),
        Err(e) => println!("   ✓ Invalid config (20MB slice size) rejected: {}", e),
    }
    
    // Example 5: Accessing proxy components
    println!("\n5. Accessing proxy components:");
    println!("   Config slice size: {} bytes", proxy1.config().slice_size);
    println!("   Config max concurrent: {}", proxy1.config().max_concurrent_subrequests);
    
    // Record some metrics
    proxy1.metrics().record_request(true);
    proxy1.metrics().record_cache_hit();
    proxy1.metrics().record_subrequest(true);
    
    let stats = proxy1.metrics().get_stats();
    println!("   Metrics after recording:");
    println!("     Total requests: {}", stats.total_requests);
    println!("     Sliced requests: {}", stats.sliced_requests);
    println!("     Cache hits: {}", stats.cache_hits);
    println!("     Total subrequests: {}", stats.total_subrequests);
    
    // Example 6: Creating request contexts
    println!("\n6. Creating request contexts:");
    let ctx = proxy1.new_ctx();
    println!("   New context created");
    println!("   Slice enabled: {}", ctx.is_slice_enabled());
    println!("   Has metadata: {}", ctx.metadata().is_some());
    println!("   Has slices: {}", ctx.has_slices());
    
    println!("\n=== Example completed successfully ===");
}
