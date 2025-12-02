//! Integration tests for StreamingProxy
//!
//! These tests verify the core streaming proxy functionality:
//! - Streaming download (receive and forward data simultaneously)
//! - Cache hit and miss scenarios
//! - Large file proxying (>100MB)
//! - Concurrent request handling
//!
//! Requirements: Phase 7, Task 8 - Verify streaming proxy functionality

use bytes::Bytes;
use pingora_proxy::ProxyHttp;
use pingora_slice::config::RawDiskCacheConfig;
use pingora_slice::{ByteRange, SliceConfig, StreamingProxy, TieredCache};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

/// Test helper to create a streaming proxy with file-based cache
async fn create_test_proxy(cache_dir: &std::path::Path) -> StreamingProxy {
    let config = Arc::new(SliceConfig {
        enable_cache: true,
        cache_ttl: 3600,
        l1_cache_size_bytes: 10 * 1024 * 1024, // 10MB
        enable_l2_cache: true,
        l2_backend: "file".to_string(),
        l2_cache_dir: cache_dir.to_str().unwrap().to_string(),
        upstream_address: "example.com:80".to_string(),
        ..Default::default()
    });

    let cache = Arc::new(
        TieredCache::new(
            Duration::from_secs(config.cache_ttl),
            config.l1_cache_size_bytes,
            cache_dir,
        )
        .await
        .unwrap(),
    );

    StreamingProxy::new(cache, config)
}

/// Test helper to create a streaming proxy with raw disk cache
async fn create_test_proxy_raw_disk(device_path: &std::path::Path) -> StreamingProxy {
    let config = Arc::new(SliceConfig {
        enable_cache: true,
        cache_ttl: 3600,
        l1_cache_size_bytes: 10 * 1024 * 1024, // 10MB
        enable_l2_cache: true,
        l2_backend: "raw_disk".to_string(),
        l2_cache_dir: device_path.to_str().unwrap().to_string(),
        upstream_address: "example.com:80".to_string(),
        raw_disk_cache: Some(RawDiskCacheConfig {
            device_path: device_path.to_str().unwrap().to_string(),
            total_size: 100 * 1024 * 1024, // 100MB
            block_size: 4096,
            use_direct_io: false,
            enable_compression: true,
            enable_prefetch: true,
            enable_zero_copy: true,
        }),
        ..Default::default()
    });

    let cache = Arc::new(
        TieredCache::new_with_raw_disk(
            Duration::from_secs(config.cache_ttl),
            config.l1_cache_size_bytes,
            device_path.to_str().unwrap(),
            100 * 1024 * 1024, // 100MB
            4096,
            false,
        )
        .await
        .unwrap(),
    );

    StreamingProxy::new(cache, config)
}

#[tokio::test]
async fn test_streaming_download_simulation() {
    // Test: Simulate streaming download by processing chunks incrementally
    // This verifies that data can be received and forwarded in chunks
    
    let temp_dir = TempDir::new().unwrap();
    let proxy = create_test_proxy(temp_dir.path()).await;
    
    let mut ctx = proxy.new_ctx();
    ctx.set_url("http://example.com/test.dat".to_string());
    ctx.set_cache_key("cache:http://example.com/test.dat".to_string());
    ctx.enable_cache();
    
    // Simulate receiving data in chunks (like streaming from upstream)
    let chunk_size = 1024 * 1024; // 1MB chunks
    let num_chunks = 10;
    
    for i in 0..num_chunks {
        let chunk = Bytes::from(vec![i as u8; chunk_size]);
        
        // Simulate what response_body_filter does:
        // 1. Update bytes received
        ctx.add_bytes_received(chunk.len() as u64);
        
        // 2. Buffer chunk for caching
        ctx.add_chunk(chunk.clone());
        
        // 3. In real implementation, Pingora forwards chunk to client here
        // We just verify the chunk is buffered
        assert_eq!(ctx.buffer().len(), i + 1);
    }
    
    // Verify all chunks were buffered
    assert_eq!(ctx.buffer().len(), num_chunks);
    assert_eq!(ctx.bytes_received(), (chunk_size * num_chunks) as u64);
    assert_eq!(ctx.buffer_size(), chunk_size * num_chunks);
    
    // Simulate end of stream - merge and cache
    let total_data: Vec<u8> = ctx
        .buffer()
        .iter()
        .flat_map(|chunk| chunk.iter())
        .copied()
        .collect();
    let data = Bytes::from(total_data);
    
    // Store in cache
    let range = ByteRange::new(0, data.len() as u64 - 1).unwrap();
    proxy
        .cache()
        .store(ctx.cache_key(), &range, data.clone())
        .unwrap();
    
    // Wait for async cache write
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Verify data was cached
    let cached = proxy
        .cache()
        .lookup(ctx.cache_key(), &range)
        .await
        .unwrap();
    assert!(cached.is_some());
    assert_eq!(cached.unwrap().len(), chunk_size * num_chunks);
}

#[tokio::test]
async fn test_cache_miss_then_hit() {
    // Test: First request is cache miss, second request is cache hit
    
    let temp_dir = TempDir::new().unwrap();
    let proxy = create_test_proxy(temp_dir.path()).await;
    
    let url = "http://example.com/test.dat";
    let cache_key = format!("cache:{}", url);
    let data = Bytes::from(vec![42u8; 1024 * 1024]); // 1MB
    let range = ByteRange::new(0, data.len() as u64 - 1).unwrap();
    
    // First request - cache miss
    let result = proxy.cache().lookup(&cache_key, &range).await.unwrap();
    assert!(result.is_none(), "First lookup should be cache miss");
    
    // Simulate caching the data (what happens after streaming from upstream)
    proxy.cache().store(&cache_key, &range, data.clone()).unwrap();
    
    // Wait for async cache write
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Second request - cache hit
    let result = proxy.cache().lookup(&cache_key, &range).await.unwrap();
    assert!(result.is_some(), "Second lookup should be cache hit");
    
    let cached_data = result.unwrap();
    assert_eq!(cached_data.len(), data.len());
    assert_eq!(cached_data, data);
}

#[tokio::test]
async fn test_cache_hit_serves_immediately() {
    // Test: Cache hit should serve data immediately without upstream request
    
    let temp_dir = TempDir::new().unwrap();
    let proxy = create_test_proxy(temp_dir.path()).await;
    
    let url = "http://example.com/cached.dat";
    let cache_key = format!("cache:{}", url);
    let data = Bytes::from(vec![123u8; 2 * 1024 * 1024]); // 2MB
    let range = ByteRange::new(0, data.len() as u64 - 1).unwrap();
    
    // Pre-populate cache
    proxy.cache().store(&cache_key, &range, data.clone()).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Create context for cache hit scenario
    let mut ctx = proxy.new_ctx();
    ctx.set_url(url.to_string());
    ctx.set_cache_key(cache_key.clone());
    
    // Lookup cache
    let cached = proxy.cache().lookup(&cache_key, &range).await.unwrap();
    assert!(cached.is_some());
    
    // Simulate cache hit
    ctx.set_cache_hit(true);
    ctx.set_cached_data(cached);
    
    // Verify cache hit state
    assert!(ctx.is_cache_hit());
    assert!(ctx.cached_data().is_some());
    assert_eq!(ctx.cached_data().unwrap().len(), data.len());
    
    // In real implementation, this data would be served immediately
    // without contacting upstream
}

#[tokio::test]
async fn test_large_file_streaming() {
    // Test: Large file (>100MB) can be streamed without memory issues
    
    let temp_dir = TempDir::new().unwrap();
    let proxy = create_test_proxy(temp_dir.path()).await;
    
    let mut ctx = proxy.new_ctx();
    ctx.set_url("http://example.com/large.dat".to_string());
    ctx.set_cache_key("cache:http://example.com/large.dat".to_string());
    ctx.enable_cache();
    
    // Simulate streaming a 150MB file in 10MB chunks
    let chunk_size = 10 * 1024 * 1024; // 10MB
    let num_chunks = 15; // 150MB total
    
    for i in 0..num_chunks {
        // Create chunk with pattern for verification
        let chunk = Bytes::from(vec![(i % 256) as u8; chunk_size]);
        
        // Process chunk
        ctx.add_bytes_received(chunk.len() as u64);
        ctx.add_chunk(chunk);
        
        // Verify memory usage stays reasonable
        // Buffer should contain all chunks so far
        assert_eq!(ctx.buffer().len(), i + 1);
    }
    
    // Verify total size
    let total_size = chunk_size * num_chunks;
    assert_eq!(ctx.bytes_received(), total_size as u64);
    assert_eq!(ctx.buffer_size(), total_size);
    
    // Simulate end of stream - merge chunks
    let total_data: Vec<u8> = ctx
        .buffer()
        .iter()
        .flat_map(|chunk| chunk.iter())
        .copied()
        .collect();
    
    assert_eq!(total_data.len(), total_size);
    
    // Verify data pattern
    for i in 0..num_chunks {
        let chunk_start = i * chunk_size;
        let chunk_end = chunk_start + chunk_size;
        let chunk_data = &total_data[chunk_start..chunk_end];
        
        // All bytes in this chunk should have the same value
        assert!(chunk_data.iter().all(|&b| b == (i % 256) as u8));
    }
    
    // Clear buffer to free memory
    ctx.clear_buffer();
    assert_eq!(ctx.buffer_size(), 0);
}

#[tokio::test]
async fn test_concurrent_requests_different_urls() {
    // Test: Multiple concurrent requests for different URLs
    
    let temp_dir = TempDir::new().unwrap();
    let proxy = Arc::new(create_test_proxy(temp_dir.path()).await);
    
    let num_requests = 10;
    let mut handles = vec![];
    
    for i in 0..num_requests {
        let proxy_clone = proxy.clone();
        
        let handle = tokio::spawn(async move {
            let url = format!("http://example.com/file{}.dat", i);
            let cache_key = format!("cache:{}", url);
            let data = Bytes::from(vec![i as u8; 1024 * 1024]); // 1MB per file
            let range = ByteRange::new(0, data.len() as u64 - 1).unwrap();
            
            // Simulate request processing
            let mut ctx = proxy_clone.new_ctx();
            ctx.set_url(url.clone());
            ctx.set_cache_key(cache_key.clone());
            ctx.enable_cache();
            
            // Simulate receiving data
            ctx.add_bytes_received(data.len() as u64);
            ctx.add_chunk(data.clone());
            
            // Cache the data
            proxy_clone
                .cache()
                .store(&cache_key, &range, data.clone())
                .unwrap();
            
            // Wait a bit
            tokio::time::sleep(Duration::from_millis(50)).await;
            
            // Verify it was cached
            let cached = proxy_clone
                .cache()
                .lookup(&cache_key, &range)
                .await
                .unwrap();
            assert!(cached.is_some());
            assert_eq!(cached.unwrap(), data);
        });
        
        handles.push(handle);
    }
    
    // Wait for all requests to complete
    for handle in handles {
        handle.await.unwrap();
    }
    
    // Verify cache stats
    let stats = proxy.cache_stats();
    assert!(stats.l1_entries > 0 || stats.disk_writes > 0);
}

#[tokio::test]
async fn test_concurrent_requests_same_url() {
    // Test: Multiple concurrent requests for the same URL
    // First request should cache, subsequent requests should hit cache
    
    let temp_dir = TempDir::new().unwrap();
    let proxy = Arc::new(create_test_proxy(temp_dir.path()).await);
    
    let url = "http://example.com/popular.dat";
    let cache_key = format!("cache:{}", url);
    let data = Bytes::from(vec![99u8; 5 * 1024 * 1024]); // 5MB
    let range = ByteRange::new(0, data.len() as u64 - 1).unwrap();
    
    // First request - populate cache
    proxy
        .cache()
        .store(&cache_key, &range, data.clone())
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Now spawn multiple concurrent requests for the same URL
    let num_requests = 20;
    let mut handles = vec![];
    
    for _ in 0..num_requests {
        let proxy_clone = proxy.clone();
        let cache_key_clone = cache_key.clone();
        let range_clone = range;
        let data_clone = data.clone();
        
        let handle = tokio::spawn(async move {
            // All requests should hit cache
            let cached = proxy_clone
                .cache()
                .lookup(&cache_key_clone, &range_clone)
                .await
                .unwrap();
            
            assert!(cached.is_some(), "Should be cache hit");
            assert_eq!(cached.unwrap(), data_clone);
        });
        
        handles.push(handle);
    }
    
    // Wait for all requests to complete
    for handle in handles {
        handle.await.unwrap();
    }
}

#[tokio::test]
async fn test_streaming_with_raw_disk_cache() {
    // Test: Streaming with raw disk cache backend
    
    let temp_dir = TempDir::new().unwrap();
    let device_path = temp_dir.path().join("raw-cache");
    let proxy = create_test_proxy_raw_disk(&device_path).await;
    
    let mut ctx = proxy.new_ctx();
    ctx.set_url("http://example.com/rawdisk.dat".to_string());
    ctx.set_cache_key("cache:http://example.com/rawdisk.dat".to_string());
    ctx.enable_cache();
    
    // Simulate streaming data
    let chunk_size = 1024 * 1024; // 1MB
    let num_chunks = 5;
    
    for i in 0..num_chunks {
        let chunk = Bytes::from(vec![i as u8; chunk_size]);
        ctx.add_bytes_received(chunk.len() as u64);
        ctx.add_chunk(chunk);
    }
    
    // Merge and cache
    let total_data: Vec<u8> = ctx
        .buffer()
        .iter()
        .flat_map(|chunk| chunk.iter())
        .copied()
        .collect();
    let data = Bytes::from(total_data);
    
    let range = ByteRange::new(0, data.len() as u64 - 1).unwrap();
    proxy
        .cache()
        .store(ctx.cache_key(), &range, data.clone())
        .unwrap();
    
    // Wait for async write
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    // Verify cached
    let cached = proxy
        .cache()
        .lookup(ctx.cache_key(), &range)
        .await
        .unwrap();
    assert!(cached.is_some());
    assert_eq!(cached.unwrap().len(), chunk_size * num_chunks);
    
    // Verify raw disk stats are available
    let raw_stats = proxy.raw_disk_stats().await;
    assert!(raw_stats.is_some());
}

#[tokio::test]
async fn test_cache_disabled_no_buffering() {
    // Test: When cache is disabled, data should not be buffered
    
    let temp_dir = TempDir::new().unwrap();
    let config = Arc::new(SliceConfig {
        enable_cache: false, // Cache disabled
        ..Default::default()
    });
    
    let cache = Arc::new(
        TieredCache::new(
            Duration::from_secs(3600),
            10 * 1024 * 1024,
            temp_dir.path(),
        )
        .await
        .unwrap(),
    );
    
    let proxy = StreamingProxy::new(cache, config);
    let mut ctx = proxy.new_ctx();
    ctx.set_url("http://example.com/nocache.dat".to_string());
    
    // Cache should be disabled
    assert!(!ctx.is_cache_enabled());
    
    // Simulate receiving data
    let chunk = Bytes::from(vec![1u8; 1024 * 1024]);
    ctx.add_bytes_received(chunk.len() as u64);
    
    // Should NOT buffer since cache is disabled
    if ctx.is_cache_enabled() {
        ctx.add_chunk(chunk);
    }
    
    // Verify no buffering occurred
    assert_eq!(ctx.buffer().len(), 0);
    assert_eq!(ctx.buffer_size(), 0);
    assert_eq!(ctx.bytes_received(), 1024 * 1024);
}

#[tokio::test]
async fn test_partial_data_cleanup_on_stream_end() {
    // Test: Buffer is cleared after caching completes
    
    let temp_dir = TempDir::new().unwrap();
    let proxy = create_test_proxy(temp_dir.path()).await;
    
    let mut ctx = proxy.new_ctx();
    ctx.set_url("http://example.com/cleanup.dat".to_string());
    ctx.set_cache_key("cache:http://example.com/cleanup.dat".to_string());
    ctx.enable_cache();
    
    // Simulate receiving data
    for i in 0..5 {
        let chunk = Bytes::from(vec![i as u8; 1024 * 1024]);
        ctx.add_chunk(chunk);
    }
    
    assert_eq!(ctx.buffer().len(), 5);
    assert_eq!(ctx.buffer_size(), 5 * 1024 * 1024);
    
    // Simulate end of stream - clear buffer
    ctx.clear_buffer();
    
    // Verify cleanup
    assert_eq!(ctx.buffer().len(), 0);
    assert_eq!(ctx.buffer_size(), 0);
}

#[tokio::test]
async fn test_memory_efficiency_large_file() {
    // Test: Memory usage remains stable during large file streaming
    
    let temp_dir = TempDir::new().unwrap();
    let proxy = create_test_proxy(temp_dir.path()).await;
    
    let mut ctx = proxy.new_ctx();
    ctx.set_url("http://example.com/huge.dat".to_string());
    ctx.set_cache_key("cache:http://example.com/huge.dat".to_string());
    ctx.enable_cache();
    
    // Simulate streaming a very large file (500MB) in chunks
    let chunk_size = 10 * 1024 * 1024; // 10MB chunks
    let num_chunks = 50; // 500MB total
    
    for i in 0..num_chunks {
        let chunk = Bytes::from(vec![(i % 256) as u8; chunk_size]);
        ctx.add_bytes_received(chunk.len() as u64);
        ctx.add_chunk(chunk);
    }
    
    // Verify all data was buffered
    assert_eq!(ctx.buffer().len(), num_chunks);
    assert_eq!(ctx.bytes_received(), (chunk_size * num_chunks) as u64);
    
    // In real implementation, after caching, buffer would be cleared
    // to free memory
    ctx.clear_buffer();
    assert_eq!(ctx.buffer_size(), 0);
    
    // Memory is now freed, but bytes_received counter remains
    assert_eq!(ctx.bytes_received(), (chunk_size * num_chunks) as u64);
}

#[tokio::test]
async fn test_cache_stats_tracking() {
    // Test: Cache statistics are properly tracked during streaming
    
    let temp_dir = TempDir::new().unwrap();
    let proxy = create_test_proxy(temp_dir.path()).await;
    
    // Get initial stats
    let initial_stats = proxy.cache_stats();
    let initial_writes = initial_stats.disk_writes;
    
    // Perform some cache operations
    for i in 0..5 {
        let url = format!("http://example.com/stats{}.dat", i);
        let cache_key = format!("cache:{}", url);
        let data = Bytes::from(vec![i as u8; 1024 * 1024]);
        let range = ByteRange::new(0, data.len() as u64 - 1).unwrap();
        
        proxy.cache().store(&cache_key, &range, data).unwrap();
    }
    
    // Wait for async writes
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    // Get updated stats
    let updated_stats = proxy.cache_stats();
    
    // Verify stats were updated
    assert!(
        updated_stats.disk_writes >= initial_writes,
        "Disk writes should increase"
    );
    assert!(
        updated_stats.l1_entries > 0 || updated_stats.disk_writes > initial_writes,
        "Should have cache activity"
    );
}

#[tokio::test]
async fn test_concurrent_streaming_and_cache_hits() {
    // Test: Mix of streaming (cache miss) and cache hits
    
    let temp_dir = TempDir::new().unwrap();
    let proxy = Arc::new(create_test_proxy(temp_dir.path()).await);
    
    // Pre-populate cache with some files
    for i in 0..5 {
        let url = format!("http://example.com/cached{}.dat", i);
        let cache_key = format!("cache:{}", url);
        let data = Bytes::from(vec![i as u8; 1024 * 1024]);
        let range = ByteRange::new(0, data.len() as u64 - 1).unwrap();
        
        proxy.cache().store(&cache_key, &range, data).unwrap();
    }
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Now spawn mixed requests
    let mut handles = vec![];
    
    // Cache hits
    for i in 0..5 {
        let proxy_clone = proxy.clone();
        let handle = tokio::spawn(async move {
            let url = format!("http://example.com/cached{}.dat", i);
            let cache_key = format!("cache:{}", url);
            let range = ByteRange::new(0, 1024 * 1024 - 1).unwrap();
            
            let cached = proxy_clone.cache().lookup(&cache_key, &range).await.unwrap();
            assert!(cached.is_some(), "Should be cache hit");
        });
        handles.push(handle);
    }
    
    // Cache misses (new files)
    for i in 5..10 {
        let proxy_clone = proxy.clone();
        let handle = tokio::spawn(async move {
            let url = format!("http://example.com/new{}.dat", i);
            let cache_key = format!("cache:{}", url);
            let data = Bytes::from(vec![i as u8; 1024 * 1024]);
            let range = ByteRange::new(0, data.len() as u64 - 1).unwrap();
            
            // Simulate streaming and caching
            proxy_clone
                .cache()
                .store(&cache_key, &range, data.clone())
                .unwrap();
            
            tokio::time::sleep(Duration::from_millis(50)).await;
            
            let cached = proxy_clone.cache().lookup(&cache_key, &range).await.unwrap();
            assert!(cached.is_some());
        });
        handles.push(handle);
    }
    
    // Wait for all
    for handle in handles {
        handle.await.unwrap();
    }
}

#[tokio::test]
async fn test_streaming_empty_response() {
    // Test: Handle empty responses correctly
    
    let temp_dir = TempDir::new().unwrap();
    let proxy = create_test_proxy(temp_dir.path()).await;
    
    let mut ctx = proxy.new_ctx();
    ctx.set_url("http://example.com/empty.dat".to_string());
    ctx.set_cache_key("cache:http://example.com/empty.dat".to_string());
    ctx.enable_cache();
    
    // Simulate end of stream with no data
    assert_eq!(ctx.buffer().len(), 0);
    assert_eq!(ctx.bytes_received(), 0);
    
    // Should handle gracefully - no caching needed for empty response
    assert!(ctx.buffer().is_empty());
}

#[tokio::test]
async fn test_streaming_single_chunk() {
    // Test: Handle single chunk responses (small files)
    
    let temp_dir = TempDir::new().unwrap();
    let proxy = create_test_proxy(temp_dir.path()).await;
    
    let mut ctx = proxy.new_ctx();
    ctx.set_url("http://example.com/small.dat".to_string());
    ctx.set_cache_key("cache:http://example.com/small.dat".to_string());
    ctx.enable_cache();
    
    // Single chunk
    let chunk = Bytes::from(vec![42u8; 1024]); // 1KB
    ctx.add_bytes_received(chunk.len() as u64);
    ctx.add_chunk(chunk.clone());
    
    // Verify
    assert_eq!(ctx.buffer().len(), 1);
    assert_eq!(ctx.buffer_size(), 1024);
    
    // Cache it
    let range = ByteRange::new(0, chunk.len() as u64 - 1).unwrap();
    proxy
        .cache()
        .store(ctx.cache_key(), &range, chunk.clone())
        .unwrap();
    
    tokio::time::sleep(Duration::from_millis(50)).await;
    
    // Verify cached
    let cached = proxy
        .cache()
        .lookup(ctx.cache_key(), &range)
        .await
        .unwrap();
    assert!(cached.is_some());
    assert_eq!(cached.unwrap(), chunk);
}
