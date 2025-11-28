// Feature: pingora-slice, Property 7: 并发限制遵守
// **Validates: Requirements 5.2**
//
// Property: For any configured concurrency limit N and set of subrequests,
// at no point should there be more than N concurrent active subrequests

use pingora_slice::models::{ByteRange, SliceSpec};
use pingora_slice::subrequest_manager::SubrequestManager;
use proptest::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property 7: Concurrent limit enforcement
    /// 
    /// For any configured concurrency limit N and set of subrequests,
    /// at no point should there be more than N concurrent active subrequests.
    #[test]
    fn prop_concurrent_limit_enforcement(
        max_concurrent in 1usize..8,
        num_slices in 5usize..20,
        _slice_size in 1000u64..5000
    ) {
        // Only test cases where we have more slices than concurrent limit
        // to actually test the limiting behavior
        prop_assume!(num_slices > max_concurrent);
        
        let rt = Runtime::new().unwrap();
        
        let result: Result<(), TestCaseError> = rt.block_on(async {
            // Setup mock server with tracking
            let mock_server = MockServer::start().await;
            
            // Track concurrent requests
            let concurrent_count = Arc::new(AtomicUsize::new(0));
            let max_observed_concurrent = Arc::new(AtomicUsize::new(0));
            let total_requests = Arc::new(AtomicUsize::new(0));
            
            let concurrent_clone = concurrent_count.clone();
            let max_observed_clone = max_observed_concurrent.clone();
            let total_clone = total_requests.clone();
            
            // Mock endpoint that tracks concurrent requests
            Mock::given(method("GET"))
                .and(path("/test-file"))
                .respond_with(move |_req: &wiremock::Request| {
                    // Increment concurrent counter
                    let current = concurrent_clone.fetch_add(1, Ordering::SeqCst) + 1;
                    total_clone.fetch_add(1, Ordering::SeqCst);
                    
                    // Update max observed concurrent requests
                    max_observed_clone.fetch_max(current, Ordering::SeqCst);
                    
                    // Simulate some processing time to ensure concurrent requests overlap
                    std::thread::sleep(Duration::from_millis(50));
                    
                    // Decrement concurrent counter before responding
                    concurrent_clone.fetch_sub(1, Ordering::SeqCst);
                    
                    // Return a generic 206 response with a fixed range
                    // The actual range doesn't matter for concurrency testing
                    ResponseTemplate::new(206)
                        .insert_header("Content-Range", "bytes 0-999/100000")
                        .set_body_bytes(vec![0u8; 1000])
                })
                .expect(1..)
                .mount(&mock_server)
                .await;
            
            // Create subrequest manager with the specified concurrency limit
            let manager = SubrequestManager::new(max_concurrent, 0);
            
            // Create slice specifications
            // All slices use the same range (0-999) to match the mock response
            // This is fine for testing concurrency - we only care about the number
            // of concurrent requests, not the actual data
            let mut slices = Vec::new();
            for i in 0..num_slices {
                let range = ByteRange::new(0, 999).unwrap();
                slices.push(SliceSpec::new(i, range));
            }
            
            // Fetch all slices concurrently
            let url = format!("{}/test-file", mock_server.uri());
            let result = manager.fetch_slices(slices, &url).await;
            
            // Verify all requests succeeded
            prop_assert!(
                result.is_ok(),
                "All requests should succeed"
            );
            
            let results = result.unwrap();
            prop_assert_eq!(
                results.len(),
                num_slices,
                "Should receive results for all slices"
            );
            
            // Verify total requests made
            let total = total_requests.load(Ordering::SeqCst);
            prop_assert_eq!(
                total,
                num_slices,
                "Should make exactly {} requests, but made {}",
                num_slices,
                total
            );
            
            // THE KEY PROPERTY: Verify concurrent limit was never exceeded
            let max_observed = max_observed_concurrent.load(Ordering::SeqCst);
            prop_assert!(
                max_observed <= max_concurrent,
                "Concurrent limit violated! Max concurrent: {}, Observed: {}",
                max_concurrent,
                max_observed
            );
            
            // Additional check: if we have more slices than the limit,
            // we should have observed at least some concurrency
            if num_slices > max_concurrent {
                prop_assert!(
                    max_observed >= 1,
                    "Should observe at least some concurrent requests when num_slices > max_concurrent"
                );
            }
            
            Ok(())
        });
        
        result?;
    }

    /// Property 7 (boundary test): Single concurrent request
    /// 
    /// When max_concurrent is 1, requests should be processed sequentially.
    #[test]
    fn prop_sequential_processing(
        num_slices in 3usize..10,
        _slice_size in 1000u64..5000
    ) {
        let rt = Runtime::new().unwrap();
        
        let result: Result<(), TestCaseError> = rt.block_on(async {
            let mock_server = MockServer::start().await;
            
            let concurrent_count = Arc::new(AtomicUsize::new(0));
            let max_observed_concurrent = Arc::new(AtomicUsize::new(0));
            
            let concurrent_clone = concurrent_count.clone();
            let max_observed_clone = max_observed_concurrent.clone();
            
            Mock::given(method("GET"))
                .and(path("/test-file"))
                .respond_with(move |_req: &wiremock::Request| {
                    let current = concurrent_clone.fetch_add(1, Ordering::SeqCst) + 1;
                    max_observed_clone.fetch_max(current, Ordering::SeqCst);
                    
                    std::thread::sleep(Duration::from_millis(50));
                    concurrent_clone.fetch_sub(1, Ordering::SeqCst);
                    
                    ResponseTemplate::new(206)
                        .insert_header("Content-Range", "bytes 0-999/100000")
                        .set_body_bytes(vec![0u8; 1000])
                })
                .expect(1..)
                .mount(&mock_server)
                .await;
            
            // max_concurrent = 1 means sequential processing
            let manager = SubrequestManager::new(1, 0);
            
            let mut slices = Vec::new();
            for i in 0..num_slices {
                // All slices use the same range to match the mock response
                let range = ByteRange::new(0, 999).unwrap();
                slices.push(SliceSpec::new(i, range));
            }
            
            let url = format!("{}/test-file", mock_server.uri());
            let result = manager.fetch_slices(slices, &url).await;
            
            prop_assert!(result.is_ok());
            
            // With max_concurrent=1, we should never see more than 1 concurrent request
            let max_observed = max_observed_concurrent.load(Ordering::SeqCst);
            prop_assert_eq!(
                max_observed,
                1,
                "With max_concurrent=1, should never exceed 1 concurrent request, but observed {}",
                max_observed
            );
            
            Ok(())
        });
        
        result?;
    }

    /// Property 7 (stress test): High concurrency with many slices
    /// 
    /// Even with high concurrency limits and many slices, the limit should be respected.
    #[test]
    fn prop_high_concurrency_limit(
        max_concurrent in 8usize..16,
        num_slices in 20usize..50,
        _slice_size in 500u64..2000
    ) {
        prop_assume!(num_slices > max_concurrent);
        
        let rt = Runtime::new().unwrap();
        
        let result: Result<(), TestCaseError> = rt.block_on(async {
            let mock_server = MockServer::start().await;
            
            let concurrent_count = Arc::new(AtomicUsize::new(0));
            let max_observed_concurrent = Arc::new(AtomicUsize::new(0));
            
            let concurrent_clone = concurrent_count.clone();
            let max_observed_clone = max_observed_concurrent.clone();
            
            Mock::given(method("GET"))
                .and(path("/test-file"))
                .respond_with(move |_req: &wiremock::Request| {
                    let current = concurrent_clone.fetch_add(1, Ordering::SeqCst) + 1;
                    max_observed_clone.fetch_max(current, Ordering::SeqCst);
                    
                    // Shorter sleep for high concurrency test
                    std::thread::sleep(Duration::from_millis(20));
                    concurrent_clone.fetch_sub(1, Ordering::SeqCst);
                    
                    ResponseTemplate::new(206)
                        .insert_header("Content-Range", "bytes 0-999/100000")
                        .set_body_bytes(vec![0u8; 1000])
                })
                .expect(1..)
                .mount(&mock_server)
                .await;
            
            let manager = SubrequestManager::new(max_concurrent, 0);
            
            let mut slices = Vec::new();
            for i in 0..num_slices {
                // All slices use the same range to match the mock response
                let range = ByteRange::new(0, 999).unwrap();
                slices.push(SliceSpec::new(i, range));
            }
            
            let url = format!("{}/test-file", mock_server.uri());
            let result = manager.fetch_slices(slices, &url).await;
            
            prop_assert!(result.is_ok());
            
            let max_observed = max_observed_concurrent.load(Ordering::SeqCst);
            prop_assert!(
                max_observed <= max_concurrent,
                "Concurrent limit violated! Max concurrent: {}, Observed: {}",
                max_concurrent,
                max_observed
            );
            
            Ok(())
        });
        
        result?;
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    /// Unit test: Verify semaphore-based concurrency control
    #[tokio::test]
    async fn test_concurrent_limit_basic() {
        let mock_server = MockServer::start().await;
        
        let concurrent_count = Arc::new(AtomicUsize::new(0));
        let max_observed = Arc::new(AtomicUsize::new(0));
        
        let concurrent_clone = concurrent_count.clone();
        let max_clone = max_observed.clone();
        
        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(move |_req: &wiremock::Request| {
                let current = concurrent_clone.fetch_add(1, Ordering::SeqCst) + 1;
                max_clone.fetch_max(current, Ordering::SeqCst);
                
                std::thread::sleep(Duration::from_millis(100));
                concurrent_clone.fetch_sub(1, Ordering::SeqCst);
                
                ResponseTemplate::new(206)
                    .insert_header("Content-Range", "bytes 0-999/10000")
                    .set_body_bytes(vec![0u8; 1000])
            })
            .mount(&mock_server)
            .await;
        
        let manager = SubrequestManager::new(2, 0); // max_concurrent = 2
        
        let mut slices = Vec::new();
        for i in 0..5 {
            // All slices use the same range to match the mock response
            let range = ByteRange::new(0, 999).unwrap();
            slices.push(SliceSpec::new(i, range));
        }
        
        let url = format!("{}/test", mock_server.uri());
        let result = manager.fetch_slices(slices, &url).await;
        
        assert!(result.is_ok());
        
        let max = max_observed.load(Ordering::SeqCst);
        assert!(
            max <= 2,
            "Should never exceed max_concurrent=2, but observed {}",
            max
        );
    }

    /// Unit test: Verify manager can be created with various concurrent limits
    #[test]
    fn test_manager_creation() {
        // Test that manager can be created with various concurrent limits
        let manager1 = SubrequestManager::new(1, 0);
        let manager2 = SubrequestManager::new(4, 3);
        let manager3 = SubrequestManager::new(10, 5);
        
        // Just verify they can be created without panicking
        // The actual concurrency enforcement is tested in property tests
        drop(manager1);
        drop(manager2);
        drop(manager3);
    }
}
