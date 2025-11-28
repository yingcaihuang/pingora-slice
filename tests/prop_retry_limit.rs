// Feature: pingora-slice, Property 8: 重试次数限制
// **Validates: Requirements 5.4**
//
// Property: For any failed subrequest and configured max retry count M, 
// the subrequest should be retried at most M times before giving up

use pingora_slice::error::SliceError;
use pingora_slice::models::{ByteRange, SliceSpec};
use pingora_slice::subrequest_manager::{RetryPolicy, SubrequestManager};
use proptest::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::runtime::Runtime;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]

    /// Property 8: Retry limit enforcement
    /// 
    /// For any configured max_retries value M, when a subrequest fails,
    /// it should be attempted at most M+1 times (initial attempt + M retries).
    #[test]
    fn prop_retry_limit_enforcement(
        max_retries in 0usize..5,
        slice_index in 0usize..10,
        start in 0u64..10000,
        size in 1u64..1000
    ) {
        let rt = Runtime::new().unwrap();
        
        let result: Result<(), TestCaseError> = rt.block_on(async {
            // Setup mock server that always fails
            let mock_server = MockServer::start().await;
            let request_counter = Arc::new(AtomicUsize::new(0));
            let counter_clone = request_counter.clone();
            
            // Mock endpoint that counts requests and always returns 500
            Mock::given(method("GET"))
                .and(path("/test-file"))
                .respond_with(move |_req: &wiremock::Request| {
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                    ResponseTemplate::new(500)
                })
                .expect(1..)
                .mount(&mock_server)
                .await;
            
            // Create subrequest manager with the specified max_retries
            let manager = SubrequestManager::new(1, max_retries);
            
            // Create a slice specification
            let end = start + size - 1;
            let range = ByteRange::new(start, end).unwrap();
            let slice = SliceSpec::new(slice_index, range);
            
            // Attempt to fetch the slice (should fail after retries)
            let url = format!("{}/test-file", mock_server.uri());
            let result = manager.fetch_single_slice(&slice, &url).await;
            
            // Verify the request failed
            prop_assert!(
                result.is_err(),
                "Request should fail after exhausting retries"
            );
            
            // Verify it's the correct error type
            if let Err(SliceError::SubrequestFailed { slice_index: idx, attempts }) = result {
                prop_assert_eq!(
                    idx,
                    slice_index,
                    "Error should contain correct slice index"
                );
                
                // The key property: attempts should be exactly max_retries + 1
                // (initial attempt + max_retries retry attempts)
                prop_assert_eq!(
                    attempts,
                    max_retries + 1,
                    "Should attempt exactly {} times (1 initial + {} retries), but got {} attempts",
                    max_retries + 1,
                    max_retries,
                    attempts
                );
            } else {
                prop_assert!(false, "Expected SubrequestFailed error");
            }
            
            // Verify the actual number of HTTP requests made
            let actual_requests = request_counter.load(Ordering::SeqCst);
            prop_assert_eq!(
                actual_requests,
                max_retries + 1,
                "Should make exactly {} HTTP requests (1 initial + {} retries), but made {}",
                max_retries + 1,
                max_retries,
                actual_requests
            );
            
            Ok(())
        });
        
        result?;
    }

    /// Property 8 (boundary test): Zero retries
    /// 
    /// When max_retries is 0, only the initial attempt should be made.
    #[test]
    fn prop_zero_retries(
        slice_index in 0usize..10,
        start in 0u64..10000,
        size in 1u64..1000
    ) {
        let rt = Runtime::new().unwrap();
        
        let result: Result<(), TestCaseError> = rt.block_on(async {
            let mock_server = MockServer::start().await;
            let request_counter = Arc::new(AtomicUsize::new(0));
            let counter_clone = request_counter.clone();
            
            Mock::given(method("GET"))
                .and(path("/test-file"))
                .respond_with(move |_req: &wiremock::Request| {
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                    ResponseTemplate::new(500)
                })
                .expect(1)
                .mount(&mock_server)
                .await;
            
            let manager = SubrequestManager::new(1, 0); // max_retries = 0
            
            let end = start + size - 1;
            let range = ByteRange::new(start, end).unwrap();
            let slice = SliceSpec::new(slice_index, range);
            
            let url = format!("{}/test-file", mock_server.uri());
            let result = manager.fetch_single_slice(&slice, &url).await;
            
            prop_assert!(result.is_err());
            
            if let Err(SliceError::SubrequestFailed { attempts, .. }) = result {
                prop_assert_eq!(
                    attempts,
                    1,
                    "With max_retries=0, should attempt exactly once"
                );
            }
            
            let actual_requests = request_counter.load(Ordering::SeqCst);
            prop_assert_eq!(
                actual_requests,
                1,
                "With max_retries=0, should make exactly 1 HTTP request"
            );
            
            Ok(())
        });
        
        result?;
    }

    /// Property 8 (success case): Successful request doesn't retry
    /// 
    /// When a request succeeds on the first attempt, no retries should occur.
    #[test]
    fn prop_success_no_retry(
        max_retries in 1usize..5,
        slice_index in 0usize..10,
        start in 0u64..10000,
        size in 1u64..1000
    ) {
        let rt = Runtime::new().unwrap();
        
        let result: Result<(), TestCaseError> = rt.block_on(async {
            let mock_server = MockServer::start().await;
            let request_counter = Arc::new(AtomicUsize::new(0));
            let counter_clone = request_counter.clone();
            
            let end = start + size - 1;
            
            // Mock endpoint that succeeds immediately
            Mock::given(method("GET"))
                .and(path("/test-file"))
                .respond_with(move |_req: &wiremock::Request| {
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                    ResponseTemplate::new(206)
                        .insert_header("Content-Range", format!("bytes {}-{}/100000", start, end).as_str())
                        .set_body_bytes(vec![0u8; size as usize])
                })
                .expect(1)
                .mount(&mock_server)
                .await;
            
            let manager = SubrequestManager::new(1, max_retries);
            
            let range = ByteRange::new(start, end).unwrap();
            let slice = SliceSpec::new(slice_index, range);
            
            let url = format!("{}/test-file", mock_server.uri());
            let result = manager.fetch_single_slice(&slice, &url).await;
            
            prop_assert!(
                result.is_ok(),
                "Request should succeed"
            );
            
            // Verify only one request was made (no retries)
            let actual_requests = request_counter.load(Ordering::SeqCst);
            prop_assert_eq!(
                actual_requests,
                1,
                "Successful request should not retry, expected 1 request but got {}",
                actual_requests
            );
            
            Ok(())
        });
        
        result?;
    }

    /// Property 8 (eventual success): Success after N retries
    /// 
    /// If a request succeeds after N failed attempts (where N < max_retries),
    /// exactly N+1 attempts should be made.
    #[test]
    fn prop_eventual_success(
        max_retries in 2usize..5,
        fail_count in 1usize..3,
        slice_index in 0usize..10,
        start in 0u64..10000,
        size in 1u64..1000
    ) {
        // Only test cases where fail_count < max_retries
        prop_assume!(fail_count < max_retries);
        
        let rt = Runtime::new().unwrap();
        
        let result: Result<(), TestCaseError> = rt.block_on(async {
            let mock_server = MockServer::start().await;
            let request_counter = Arc::new(AtomicUsize::new(0));
            let counter_clone = request_counter.clone();
            let fail_count_clone = fail_count;
            
            let end = start + size - 1;
            
            // Mock endpoint that fails N times then succeeds
            Mock::given(method("GET"))
                .and(path("/test-file"))
                .respond_with(move |_req: &wiremock::Request| {
                    let count = counter_clone.fetch_add(1, Ordering::SeqCst);
                    
                    if count < fail_count_clone {
                        // Fail for the first N attempts
                        ResponseTemplate::new(500)
                    } else {
                        // Succeed on attempt N+1
                        ResponseTemplate::new(206)
                            .insert_header("Content-Range", format!("bytes {}-{}/100000", start, end).as_str())
                            .set_body_bytes(vec![0u8; size as usize])
                    }
                })
                .expect(1..)
                .mount(&mock_server)
                .await;
            
            let manager = SubrequestManager::new(1, max_retries);
            
            let range = ByteRange::new(start, end).unwrap();
            let slice = SliceSpec::new(slice_index, range);
            
            let url = format!("{}/test-file", mock_server.uri());
            let result = manager.fetch_single_slice(&slice, &url).await;
            
            prop_assert!(
                result.is_ok(),
                "Request should eventually succeed after {} failures",
                fail_count
            );
            
            // Verify exactly fail_count + 1 requests were made
            let actual_requests = request_counter.load(Ordering::SeqCst);
            prop_assert_eq!(
                actual_requests,
                fail_count + 1,
                "Should make exactly {} requests ({} failures + 1 success), but made {}",
                fail_count + 1,
                fail_count,
                actual_requests
            );
            
            Ok(())
        });
        
        result?;
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    /// Unit test: Verify RetryPolicy correctly limits retries
    #[test]
    fn test_retry_policy_limits() {
        let policy = RetryPolicy::new(3);
        let error = SliceError::HttpError("test".to_string());
        
        // Should allow retries for attempts 0, 1, 2 (total 3 retries)
        assert!(policy.should_retry(0, &error), "Should retry on attempt 0");
        assert!(policy.should_retry(1, &error), "Should retry on attempt 1");
        assert!(policy.should_retry(2, &error), "Should retry on attempt 2");
        
        // Should NOT allow retry on attempt 3 (would be 4th total attempt)
        assert!(!policy.should_retry(3, &error), "Should NOT retry on attempt 3");
        assert!(!policy.should_retry(4, &error), "Should NOT retry on attempt 4");
    }

    /// Unit test: Zero retries means only initial attempt
    #[test]
    fn test_zero_retries_policy() {
        let policy = RetryPolicy::new(0);
        let error = SliceError::HttpError("test".to_string());
        
        // Should NOT allow any retries
        assert!(!policy.should_retry(0, &error), "Should NOT retry with max_retries=0");
    }

    /// Unit test: Non-retryable errors are not retried
    #[test]
    fn test_non_retryable_error() {
        let policy = RetryPolicy::new(3);
        let error = SliceError::ConfigError("test".to_string());
        
        // ConfigError is not retryable
        assert!(!policy.should_retry(0, &error), "Should NOT retry non-retryable errors");
    }
}
