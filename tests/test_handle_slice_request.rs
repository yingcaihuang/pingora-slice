//! Integration tests for handle_slice_request
//!
//! These tests verify the complete flow of the handle_slice_request method,
//! including fetching slices from origin, caching, and streaming to client.

use pingora_slice::{SliceProxy, SliceConfig, SliceContext, FileMetadata, SliceSpec, ByteRange};
use std::sync::Arc;
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path, header};
use http::StatusCode;

fn create_test_proxy() -> SliceProxy {
    let config = Arc::new(SliceConfig {
        slice_size: 1024,
        max_concurrent_subrequests: 4,
        max_retries: 3,
        cache_ttl: 3600,
        ..Default::default()
    });
    SliceProxy::new(config)
}

#[tokio::test]
async fn test_handle_slice_request_complete_flow() {
    let mock_server = MockServer::start().await;
    
    // Mock GET requests for two slices
    Mock::given(method("GET"))
        .and(path("/test.bin"))
        .and(header("range", "bytes=0-1023"))
        .respond_with(
            ResponseTemplate::new(206)
                .insert_header("Content-Range", "bytes 0-1023/2048")
                .insert_header("Content-Type", "application/octet-stream")
                .set_body_bytes(vec![0xAA; 1024])
        )
        .mount(&mock_server)
        .await;
    
    Mock::given(method("GET"))
        .and(path("/test.bin"))
        .and(header("range", "bytes=1024-2047"))
        .respond_with(
            ResponseTemplate::new(206)
                .insert_header("Content-Range", "bytes 1024-2047/2048")
                .insert_header("Content-Type", "application/octet-stream")
                .set_body_bytes(vec![0xBB; 1024])
        )
        .mount(&mock_server)
        .await;
    
    let proxy = create_test_proxy();
    
    // Create context with metadata and slices
    let mut ctx = SliceContext::new();
    let metadata = FileMetadata::with_headers(
        2048,
        true,
        Some("application/octet-stream".to_string()),
        None,
        None,
    );
    ctx.set_metadata(metadata);
    
    // Create slices
    let range1 = ByteRange::new(0, 1023).unwrap();
    let range2 = ByteRange::new(1024, 2047).unwrap();
    let slice1 = SliceSpec::new(0, range1);
    let slice2 = SliceSpec::new(1, range2);
    
    ctx.set_slices(vec![slice1, slice2]);
    ctx.enable_slicing();
    
    // Handle the request
    let url = format!("{}/test.bin", mock_server.uri());
    let result = proxy.handle_slice_request(&url, &ctx).await;
    
    assert!(result.is_ok(), "handle_slice_request should succeed");
    
    let (status, headers, slices) = result.unwrap();
    
    // Verify status code
    assert_eq!(status, StatusCode::OK, "Status should be 200 OK");
    
    // Verify headers
    assert_eq!(
        headers.get("content-length").unwrap().to_str().unwrap(),
        "2048",
        "Content-Length should be 2048"
    );
    assert_eq!(
        headers.get("content-type").unwrap().to_str().unwrap(),
        "application/octet-stream",
        "Content-Type should be preserved"
    );
    assert_eq!(
        headers.get("accept-ranges").unwrap().to_str().unwrap(),
        "bytes",
        "Accept-Ranges should be bytes"
    );
    
    // Verify slices
    assert_eq!(slices.len(), 2, "Should have 2 slices");
    assert_eq!(slices[0].len(), 1024, "First slice should be 1024 bytes");
    assert_eq!(slices[1].len(), 1024, "Second slice should be 1024 bytes");
    
    // Verify slice content
    assert!(slices[0].iter().all(|&b| b == 0xAA), "First slice should contain 0xAA");
    assert!(slices[1].iter().all(|&b| b == 0xBB), "Second slice should contain 0xBB");
    
    // Verify metrics
    let stats = proxy.metrics().get_stats();
    assert_eq!(stats.total_subrequests, 2, "Should have made 2 subrequests");
    assert_eq!(stats.bytes_from_origin, 2048, "Should have fetched 2048 bytes from origin");
    assert_eq!(stats.bytes_to_client, 2048, "Should have sent 2048 bytes to client");
}

#[tokio::test]
async fn test_handle_slice_request_with_range() {
    let mock_server = MockServer::start().await;
    
    // Mock GET request for single slice (client requested range)
    Mock::given(method("GET"))
        .and(path("/test.bin"))
        .and(header("range", "bytes=1024-2047"))
        .respond_with(
            ResponseTemplate::new(206)
                .insert_header("Content-Range", "bytes 1024-2047/10240")
                .insert_header("Content-Type", "application/octet-stream")
                .set_body_bytes(vec![0xCC; 1024])
        )
        .mount(&mock_server)
        .await;
    
    let proxy = create_test_proxy();
    
    // Create context with metadata and client range
    let mut ctx = SliceContext::new();
    let metadata = FileMetadata::with_headers(
        10240,
        true,
        Some("application/octet-stream".to_string()),
        None,
        None,
    );
    ctx.set_metadata(metadata);
    
    // Client requested bytes 1024-2047
    let client_range = ByteRange::new(1024, 2047).unwrap();
    ctx.set_client_range(client_range);
    
    // Create single slice for the client range
    let slice1 = SliceSpec::new(0, client_range);
    ctx.set_slices(vec![slice1]);
    ctx.enable_slicing();
    
    // Handle the request
    let url = format!("{}/test.bin", mock_server.uri());
    let result = proxy.handle_slice_request(&url, &ctx).await;
    
    assert!(result.is_ok(), "handle_slice_request should succeed");
    
    let (status, headers, slices) = result.unwrap();
    
    // Verify status code for range request
    assert_eq!(status, StatusCode::PARTIAL_CONTENT, "Status should be 206 Partial Content");
    
    // Verify Content-Range header
    assert_eq!(
        headers.get("content-range").unwrap().to_str().unwrap(),
        "bytes 1024-2047/10240",
        "Content-Range should match requested range"
    );
    
    // Verify content length
    assert_eq!(
        headers.get("content-length").unwrap().to_str().unwrap(),
        "1024",
        "Content-Length should be 1024"
    );
    
    // Verify slices
    assert_eq!(slices.len(), 1, "Should have 1 slice");
    assert_eq!(slices[0].len(), 1024, "Slice should be 1024 bytes");
    assert!(slices[0].iter().all(|&b| b == 0xCC), "Slice should contain 0xCC");
}

#[tokio::test]
async fn test_handle_slice_request_concurrent_fetching() {
    let mock_server = MockServer::start().await;
    
    // Mock GET requests for 4 slices
    for i in 0..4 {
        let start = i * 1024;
        let end = start + 1023;
        let range_header = format!("bytes={}-{}", start, end);
        let content_range = format!("bytes {}-{}/4096", start, end);
        let byte_value = i as u8;
        
        Mock::given(method("GET"))
            .and(path("/test.bin"))
            .and(header("range", range_header.as_str()))
            .respond_with(
                ResponseTemplate::new(206)
                    .insert_header("Content-Range", content_range.as_str())
                    .set_body_bytes(vec![byte_value; 1024])
            )
            .mount(&mock_server)
            .await;
    }
    
    let proxy = create_test_proxy();
    
    // Create context with metadata and 4 slices
    let mut ctx = SliceContext::new();
    let metadata = FileMetadata::new(4096, true);
    ctx.set_metadata(metadata);
    
    let mut slices = Vec::new();
    for i in 0..4 {
        let start = i * 1024;
        let end = start + 1023;
        let range = ByteRange::new(start, end).unwrap();
        slices.push(SliceSpec::new(i as usize, range));
    }
    
    ctx.set_slices(slices);
    ctx.enable_slicing();
    
    // Handle the request
    let url = format!("{}/test.bin", mock_server.uri());
    let result = proxy.handle_slice_request(&url, &ctx).await;
    
    assert!(result.is_ok(), "handle_slice_request should succeed");
    
    let (status, _headers, slices) = result.unwrap();
    
    // Verify response
    assert_eq!(status, StatusCode::OK);
    assert_eq!(slices.len(), 4, "Should have 4 slices");
    
    // Verify each slice has correct content
    for (i, slice) in slices.iter().enumerate() {
        assert_eq!(slice.len(), 1024, "Slice {} should be 1024 bytes", i);
        assert!(
            slice.iter().all(|&b| b == i as u8),
            "Slice {} should contain byte value {}",
            i, i
        );
    }
    
    // Verify metrics
    let stats = proxy.metrics().get_stats();
    assert_eq!(stats.total_subrequests, 4, "Should have made 4 subrequests");
    assert_eq!(stats.bytes_from_origin, 4096, "Should have fetched 4096 bytes");
}

#[tokio::test]
async fn test_handle_slice_request_error_handling() {
    let mock_server = MockServer::start().await;
    
    // Mock GET request that returns 500 error
    Mock::given(method("GET"))
        .and(path("/test.bin"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;
    
    let proxy = create_test_proxy();
    
    // Create context with metadata and slices
    let mut ctx = SliceContext::new();
    let metadata = FileMetadata::new(1024, true);
    ctx.set_metadata(metadata);
    
    let range = ByteRange::new(0, 1023).unwrap();
    let slice = SliceSpec::new(0, range);
    ctx.set_slices(vec![slice]);
    ctx.enable_slicing();
    
    // Handle the request - should fail after retries
    let url = format!("{}/test.bin", mock_server.uri());
    let result = proxy.handle_slice_request(&url, &ctx).await;
    
    assert!(result.is_err(), "handle_slice_request should fail");
    
    // Verify that failed subrequests were recorded
    let stats = proxy.metrics().get_stats();
    assert!(stats.failed_subrequests > 0, "Should have recorded failed subrequests");
}
