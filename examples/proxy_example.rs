//! Example demonstrating the usage of SliceProxy and SliceContext
//!
//! This example shows how to create and use the main SliceProxy structure
//! along with the per-request SliceContext.

use pingora_slice::{SliceConfig, SliceProxy, ByteRange, FileMetadata, SliceSpec};
use std::sync::Arc;

fn main() {
    println!("=== SliceProxy Example ===\n");
    
    // 1. Create configuration
    println!("1. Creating configuration...");
    let mut config = SliceConfig::default();
    config.slice_size = 512 * 1024; // 512KB slices
    config.max_concurrent_subrequests = 8;
    config.max_retries = 5;
    config.slice_patterns = vec![
        "/large-files/*".to_string(),
        "/downloads/*.bin".to_string(),
    ];
    
    println!("   Slice size: {} bytes", config.slice_size);
    println!("   Max concurrent: {}", config.max_concurrent_subrequests);
    println!("   Max retries: {}", config.max_retries);
    println!("   Patterns: {:?}\n", config.slice_patterns);
    
    // 2. Create SliceProxy
    println!("2. Creating SliceProxy...");
    let proxy = SliceProxy::new(Arc::new(config));
    println!("   SliceProxy created successfully\n");
    
    // 3. Create a request context
    println!("3. Creating request context...");
    let mut ctx = proxy.new_ctx();
    println!("   Initial state:");
    println!("     - Slice enabled: {}", ctx.is_slice_enabled());
    println!("     - Has metadata: {}", ctx.metadata().is_some());
    println!("     - Has slices: {}\n", ctx.has_slices());
    
    // 4. Simulate request processing
    println!("4. Simulating request processing...");
    
    // Enable slicing
    ctx.enable_slicing();
    println!("   Slicing enabled");
    
    // Set file metadata
    let metadata = FileMetadata::with_headers(
        10 * 1024 * 1024, // 10MB file
        true,
        Some("application/octet-stream".to_string()),
        Some("\"abc123\"".to_string()),
        Some("Wed, 21 Oct 2015 07:28:00 GMT".to_string()),
    );
    ctx.set_metadata(metadata.clone());
    println!("   Metadata set:");
    println!("     - Content length: {} bytes", metadata.content_length);
    println!("     - Supports range: {}", metadata.supports_range);
    println!("     - Content type: {:?}", metadata.content_type);
    
    // Set client range (optional)
    let client_range = ByteRange::new(0, 1024 * 1024 - 1).unwrap(); // First 1MB
    ctx.set_client_range(client_range);
    println!("   Client range set: {}-{}\n", client_range.start, client_range.end);
    
    // 5. Calculate slices
    println!("5. Calculating slices...");
    let slice_size = proxy.config().slice_size as u64;
    let file_size = metadata.content_length;
    let mut slices = Vec::new();
    
    let mut offset = 0;
    let mut index = 0;
    while offset < file_size {
        let end = std::cmp::min(offset + slice_size - 1, file_size - 1);
        let range = ByteRange::new(offset, end).unwrap();
        slices.push(SliceSpec::new(index, range));
        offset = end + 1;
        index += 1;
    }
    
    ctx.set_slices(slices);
    println!("   Calculated {} slices", ctx.slice_count());
    println!("   First 3 slices:");
    for (i, slice) in ctx.slices().iter().take(3).enumerate() {
        println!("     Slice {}: bytes {}-{} ({} bytes)",
                 i, slice.range.start, slice.range.end, slice.range.size());
    }
    println!();
    
    // 6. Simulate cache hits
    println!("6. Simulating cache operations...");
    ctx.slices_mut()[0].cached = true;
    ctx.slices_mut()[2].cached = true;
    ctx.slices_mut()[4].cached = true;
    
    println!("   Total slices: {}", ctx.slice_count());
    println!("   Cached slices: {}", ctx.cached_slice_count());
    println!("   Uncached slices: {}\n", ctx.uncached_slice_count());
    
    // 7. Record metrics
    println!("7. Recording metrics...");
    proxy.metrics().record_request(true);
    proxy.metrics().record_cache_hit();
    proxy.metrics().record_cache_hit();
    proxy.metrics().record_cache_hit();
    proxy.metrics().record_cache_miss();
    proxy.metrics().record_subrequest(true);
    proxy.metrics().record_subrequest(true);
    
    let stats = proxy.metrics().get_stats();
    println!("   Metrics snapshot:");
    println!("     - Total requests: {}", stats.total_requests);
    println!("     - Sliced requests: {}", stats.sliced_requests);
    println!("     - Cache hits: {}", stats.cache_hits);
    println!("     - Cache misses: {}", stats.cache_misses);
    println!("     - Cache hit rate: {:.1}%", stats.cache_hit_rate());
    println!("     - Total subrequests: {}", stats.total_subrequests);
    println!();
    
    // 8. Test upstream_peer method
    println!("8. Testing upstream_peer method...");
    
    // Test with normal proxy mode (slicing disabled)
    let normal_ctx = proxy.new_ctx();
    match proxy.upstream_peer(&normal_ctx) {
        Ok(upstream) => println!("   Normal mode upstream: {}", upstream),
        Err(e) => println!("   Error: {:?}", e),
    }
    
    // Test with slice mode (should error)
    match proxy.upstream_peer(&ctx) {
        Ok(upstream) => println!("   Slice mode upstream: {}", upstream),
        Err(e) => println!("   Slice mode error (expected): {:?}", e),
    }
    println!();
    
    // 9. Test logging method
    println!("9. Testing logging method...");
    use http::Method;
    
    // Log a successful slice request
    println!("   Logging successful slice request:");
    proxy.logging(
        &Method::GET,
        "http://example.com/large-files/video.mp4",
        &ctx,
        None,
        250, // 250ms duration
    );
    
    // Log a normal proxy request
    println!("   Logging normal proxy request:");
    proxy.logging(
        &Method::GET,
        "http://example.com/small-file.txt",
        &normal_ctx,
        None,
        50, // 50ms duration
    );
    
    // Log a failed request
    println!("   Logging failed request:");
    use pingora_slice::SliceError;
    let error = SliceError::MetadataFetchError("Connection refused".to_string());
    proxy.logging(
        &Method::GET,
        "http://example.com/unreachable.bin",
        &normal_ctx,
        Some(&error),
        100, // 100ms duration
    );
    println!();
    
    // 10. Context state summary
    println!("10. Final context state:");
    println!("   - Slice enabled: {}", ctx.is_slice_enabled());
    println!("   - File size: {} bytes", ctx.metadata().unwrap().content_length);
    println!("   - Client range: {:?}", ctx.client_range());
    println!("   - Total slices: {}", ctx.slice_count());
    println!("   - Cached: {}, Uncached: {}", 
             ctx.cached_slice_count(), ctx.uncached_slice_count());
    
    println!("\n=== Example completed successfully ===");
}
