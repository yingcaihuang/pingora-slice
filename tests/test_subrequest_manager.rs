//! Integration tests for SubrequestManager
//!
//! Note: These tests require a server that supports Range requests.
//! httpbin.org does not support Range requests, so these tests are ignored by default.
//! To run these tests, set up a local server that supports Range requests.

use pingora_slice::{ByteRange, SliceSpec, SubrequestManager};

#[tokio::test]
#[ignore = "Requires a server that supports Range requests"]
async fn test_subrequest_manager_basic() {
    let manager = SubrequestManager::new(4, 3);
    
    // Create a simple slice
    let slice = SliceSpec::new(0, ByteRange::new(0, 1023).unwrap());
    
    // Use httpbin.org which supports Range requests
    let url = "https://httpbin.org/bytes/2048";
    
    let result = manager.fetch_single_slice(&slice, url).await;
    
    // Should succeed
    assert!(result.is_ok(), "Failed to fetch slice: {:?}", result.err());
    
    let result = result.unwrap();
    assert_eq!(result.slice_index, 0);
    assert_eq!(result.status, 206);
    assert_eq!(result.data.len(), 1024); // Should be exactly 1024 bytes
}

#[tokio::test]
#[ignore = "Requires a server that supports Range requests"]
async fn test_subrequest_manager_multiple_slices() {
    let manager = SubrequestManager::new(4, 3);
    
    // Create multiple slices
    let slices = vec![
        SliceSpec::new(0, ByteRange::new(0, 1023).unwrap()),
        SliceSpec::new(1, ByteRange::new(1024, 2047).unwrap()),
        SliceSpec::new(2, ByteRange::new(2048, 3071).unwrap()),
    ];
    
    let url = "https://httpbin.org/bytes/4096";
    
    let results = manager.fetch_slices(slices, url).await;
    
    assert!(results.is_ok(), "Failed to fetch slices: {:?}", results.err());
    
    let results = results.unwrap();
    assert_eq!(results.len(), 3);
    
    // Verify results are in order
    for (i, result) in results.iter().enumerate() {
        assert_eq!(result.slice_index, i);
        assert_eq!(result.status, 206);
        assert_eq!(result.data.len(), 1024);
    }
}

#[tokio::test]
#[ignore = "Requires a server that supports Range requests"]
async fn test_subrequest_manager_invalid_url() {
    let manager = SubrequestManager::new(4, 3);
    
    let slice = SliceSpec::new(0, ByteRange::new(0, 1023).unwrap());
    let url = "https://httpbin.org/status/404";
    
    let result = manager.fetch_single_slice(&slice, url).await;
    
    // Should fail because 404 is not 206
    assert!(result.is_err());
}

#[tokio::test]
#[ignore = "Requires a server that supports Range requests"]
async fn test_subrequest_manager_content_range_validation() {
    let manager = SubrequestManager::new(4, 3);
    
    // Request a specific range
    let slice = SliceSpec::new(0, ByteRange::new(100, 199).unwrap());
    let url = "https://httpbin.org/bytes/1000";
    
    let result = manager.fetch_single_slice(&slice, url).await;
    
    assert!(result.is_ok(), "Failed to fetch slice: {:?}", result.err());
    
    let result = result.unwrap();
    assert_eq!(result.data.len(), 100); // Should be exactly 100 bytes
    
    // Verify Content-Range header is present
    assert!(result.headers.contains_key("content-range"));
}

#[tokio::test]
#[ignore = "Requires a server that supports Range requests"]
async fn test_subrequest_manager_concurrent_limit() {
    let manager = SubrequestManager::new(2, 3); // Only 2 concurrent
    
    // Create 5 slices
    let slices = vec![
        SliceSpec::new(0, ByteRange::new(0, 1023).unwrap()),
        SliceSpec::new(1, ByteRange::new(1024, 2047).unwrap()),
        SliceSpec::new(2, ByteRange::new(2048, 3071).unwrap()),
        SliceSpec::new(3, ByteRange::new(3072, 4095).unwrap()),
        SliceSpec::new(4, ByteRange::new(4096, 5119).unwrap()),
    ];
    
    let url = "https://httpbin.org/bytes/6144";
    
    let results = manager.fetch_slices(slices, url).await;
    
    assert!(results.is_ok(), "Failed to fetch slices: {:?}", results.err());
    
    let results = results.unwrap();
    assert_eq!(results.len(), 5);
}

#[test]
fn test_retry_policy_exponential_backoff() {
    use pingora_slice::RetryPolicy;
    use std::time::Duration;
    
    let policy = RetryPolicy::new(4);
    
    // Verify exponential backoff
    assert_eq!(policy.backoff_duration(0), Duration::from_millis(100));
    assert_eq!(policy.backoff_duration(1), Duration::from_millis(200));
    assert_eq!(policy.backoff_duration(2), Duration::from_millis(400));
    assert_eq!(policy.backoff_duration(3), Duration::from_millis(800));
}
