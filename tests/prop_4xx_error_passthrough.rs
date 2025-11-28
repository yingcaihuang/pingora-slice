// Feature: pingora-slice, Property 15: 4xx 错误透传
// **Validates: Requirements 8.1**
//
// Property: For any 4xx status code returned by the origin server for a HEAD request,
// the same status code should be returned to the client without retry

use pingora_slice::error::SliceError;
use pingora_slice::metadata_fetcher::MetadataFetcher;
use proptest::prelude::*;
use tokio::runtime::Runtime;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property 15: 4xx error pass-through
    /// 
    /// For any 4xx status code returned by the origin server,
    /// the error should:
    /// 1. Not be retried
    /// 2. Return the same status code to the client
    /// 3. Be categorized as OriginClientError
    #[test]
    fn prop_4xx_error_passthrough(status_code in prop::sample::select(vec![400, 401, 402, 403, 404, 405, 406, 407, 408, 409, 410, 411, 412, 413, 414, 415, 416, 417, 418, 421, 422, 423, 424, 425, 426, 428, 429, 431, 451])) {
        let rt = Runtime::new().unwrap();
        
        let result: Result<(), TestCaseError> = rt.block_on(async {
            // Setup mock server that returns a 4xx error
            let mock_server = MockServer::start().await;
            let request_counter = Arc::new(AtomicUsize::new(0));
            let counter_clone = request_counter.clone();
            
            // Mock HEAD endpoint that returns the specified 4xx status
            Mock::given(method("HEAD"))
                .and(path("/test-file"))
                .respond_with(move |_req: &wiremock::Request| {
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                    ResponseTemplate::new(status_code)
                        .insert_header("Content-Type", "application/octet-stream")
                })
                .expect(1) // Should only be called once (no retries)
                .mount(&mock_server)
                .await;
            
            // Create metadata fetcher
            let fetcher = MetadataFetcher::new().unwrap();
            
            // Attempt to fetch metadata (should fail with 4xx error)
            let url = format!("{}/test-file", mock_server.uri());
            let result = fetcher.fetch_metadata(&url).await;
            
            // Verify the request failed
            prop_assert!(
                result.is_err(),
                "Request should fail with 4xx error"
            );
            
            // Verify it's the correct error type (OriginClientError)
            let error = match result {
                Err(SliceError::OriginClientError { status, message: _ }) => {
                    // Property 1: Status code should be passed through unchanged
                    prop_assert_eq!(
                        status,
                        status_code,
                        "Status code should be passed through: expected {}, got {}",
                        status_code,
                        status
                    );
                    SliceError::OriginClientError { status, message: String::new() }
                }
                Err(ref other) => {
                    prop_assert!(
                        false,
                        "Expected OriginClientError for 4xx status, got: {:?}",
                        other
                    );
                    return Ok(());
                }
                Ok(_) => {
                    prop_assert!(false, "Expected error for 4xx status");
                    return Ok(());
                }
            };
            
            // Verify the error is not retryable
            {
                prop_assert!(
                    !error.should_retry(),
                    "4xx errors should not be retryable (status: {})",
                    status_code
                );
                
                // Property 2: HTTP status should match original
                let http_status = error.to_http_status();
                prop_assert_eq!(
                    http_status,
                    status_code,
                    "to_http_status() should return original 4xx code: expected {}, got {}",
                    status_code,
                    http_status
                );
                
                // Property 3: Should not fallback to normal proxy
                prop_assert!(
                    !error.fallback_to_normal_proxy(),
                    "4xx errors should not trigger fallback to normal proxy"
                );
            }
            
            // Verify only one request was made (no retries)
            let actual_requests = request_counter.load(Ordering::SeqCst);
            prop_assert_eq!(
                actual_requests,
                1,
                "4xx errors should not be retried: expected 1 request, got {}",
                actual_requests
            );
            
            Ok(())
        });
        
        result?;
    }

    /// Property 15 (specific codes): Common 4xx status codes
    /// 
    /// Test specific common 4xx status codes to ensure they all behave correctly.
    #[test]
    fn prop_common_4xx_codes(status_code in prop::sample::select(vec![400, 401, 403, 404, 405, 409, 410, 416, 429])) {
        let rt = Runtime::new().unwrap();
        
        let result: Result<(), TestCaseError> = rt.block_on(async {
            let mock_server = MockServer::start().await;
            let request_counter = Arc::new(AtomicUsize::new(0));
            let counter_clone = request_counter.clone();
            
            Mock::given(method("HEAD"))
                .and(path("/test-file"))
                .respond_with(move |_req: &wiremock::Request| {
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                    ResponseTemplate::new(status_code)
                })
                .expect(1)
                .mount(&mock_server)
                .await;
            
            let fetcher = MetadataFetcher::new().unwrap();
            let url = format!("{}/test-file", mock_server.uri());
            let result = fetcher.fetch_metadata(&url).await;
            
            prop_assert!(result.is_err());
            
            if let Err(error) = result {
                // Verify it's an OriginClientError
                match &error {
                    SliceError::OriginClientError { status, .. } => {
                        prop_assert_eq!(*status, status_code);
                    }
                    _ => prop_assert!(false, "Expected OriginClientError"),
                }
                
                // Verify not retryable
                prop_assert!(!error.should_retry());
                
                // Verify status code passthrough
                prop_assert_eq!(error.to_http_status(), status_code);
            }
            
            // Verify no retries
            let actual_requests = request_counter.load(Ordering::SeqCst);
            prop_assert_eq!(actual_requests, 1);
            
            Ok(())
        });
        
        result?;
    }

    /// Property 15 (contrast with 5xx): 5xx errors should behave differently
    /// 
    /// This test verifies that 5xx errors are categorized differently and marked as retryable,
    /// contrasting with 4xx behavior.
    #[test]
    fn prop_5xx_errors_are_retryable(status_code in prop::sample::select(vec![500, 501, 502, 503, 504, 505, 506, 507, 508, 510, 511])) {
        let rt = Runtime::new().unwrap();
        
        let result: Result<(), TestCaseError> = rt.block_on(async {
            let mock_server = MockServer::start().await;
            let request_counter = Arc::new(AtomicUsize::new(0));
            let counter_clone = request_counter.clone();
            
            // Mock that returns 5xx error
            Mock::given(method("HEAD"))
                .and(path("/test-file"))
                .respond_with(move |_req: &wiremock::Request| {
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                    ResponseTemplate::new(status_code)
                })
                .expect(1)
                .mount(&mock_server)
                .await;
            
            let fetcher = MetadataFetcher::new().unwrap();
            let url = format!("{}/test-file", mock_server.uri());
            let result = fetcher.fetch_metadata(&url).await;
            
            prop_assert!(result.is_err());
            
            if let Err(error) = result {
                // Verify it's an OriginServerError (not OriginClientError)
                match &error {
                    SliceError::OriginServerError { status, .. } => {
                        prop_assert_eq!(*status, status_code);
                    }
                    _ => prop_assert!(false, "Expected OriginServerError for 5xx"),
                }
                
                // Verify it IS retryable (contrast with 4xx)
                prop_assert!(
                    error.should_retry(),
                    "5xx errors should be retryable (unlike 4xx)"
                );
                
                // Verify status code is converted to 502
                prop_assert_eq!(
                    error.to_http_status(),
                    502,
                    "5xx errors should return 502 Bad Gateway"
                );
                
                // Verify it does not fallback to normal proxy
                prop_assert!(
                    !error.fallback_to_normal_proxy(),
                    "5xx errors should not trigger fallback"
                );
            }
            
            // Verify only one request was made (retry logic is at higher level)
            let actual_requests = request_counter.load(Ordering::SeqCst);
            prop_assert_eq!(
                actual_requests,
                1,
                "MetadataFetcher makes single request, retry logic is at higher level"
            );
            
            Ok(())
        });
        
        result?;
    }

    /// Property 15 (error creation): from_http_status correctly categorizes errors
    /// 
    /// Verify that the from_http_status helper correctly creates 4xx errors.
    #[test]
    fn prop_from_http_status_4xx(status_code in 400u16..500, message in "[a-zA-Z ]{5,30}") {
        let error = SliceError::from_http_status(status_code, message.clone());
        
        // Should create OriginClientError
        match &error {
            SliceError::OriginClientError { status, message: msg } => {
                prop_assert_eq!(*status, status_code);
                prop_assert_eq!(msg, &message);
            }
            _ => prop_assert!(false, "from_http_status should create OriginClientError for 4xx"),
        }
        
        // Should not be retryable
        prop_assert!(!error.should_retry());
        
        // Should pass through status code
        prop_assert_eq!(error.to_http_status(), status_code);
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    /// Unit test: Verify 404 Not Found is handled correctly
    #[test]
    fn test_404_not_found() {
        let error = SliceError::origin_client_error(404, "Not Found");
        
        assert!(!error.should_retry(), "404 should not be retried");
        assert_eq!(error.to_http_status(), 404, "404 should be passed through");
        assert!(!error.fallback_to_normal_proxy(), "404 should not fallback");
        
        match error {
            SliceError::OriginClientError { status, message } => {
                assert_eq!(status, 404);
                assert_eq!(message, "Not Found");
            }
            _ => panic!("Expected OriginClientError"),
        }
    }

    /// Unit test: Verify 403 Forbidden is handled correctly
    #[test]
    fn test_403_forbidden() {
        let error = SliceError::origin_client_error(403, "Forbidden");
        
        assert!(!error.should_retry());
        assert_eq!(error.to_http_status(), 403);
        assert!(!error.fallback_to_normal_proxy());
    }

    /// Unit test: Verify 400 Bad Request is handled correctly
    #[test]
    fn test_400_bad_request() {
        let error = SliceError::origin_client_error(400, "Bad Request");
        
        assert!(!error.should_retry());
        assert_eq!(error.to_http_status(), 400);
        assert!(!error.fallback_to_normal_proxy());
    }

    /// Unit test: Verify 416 Range Not Satisfiable is handled correctly
    #[test]
    fn test_416_range_not_satisfiable() {
        let error = SliceError::origin_client_error(416, "Range Not Satisfiable");
        
        assert!(!error.should_retry());
        assert_eq!(error.to_http_status(), 416);
        assert!(!error.fallback_to_normal_proxy());
    }

    /// Unit test: Verify 429 Too Many Requests is handled correctly
    #[test]
    fn test_429_too_many_requests() {
        let error = SliceError::origin_client_error(429, "Too Many Requests");
        
        assert!(!error.should_retry());
        assert_eq!(error.to_http_status(), 429);
        assert!(!error.fallback_to_normal_proxy());
    }

    /// Unit test: Contrast 4xx with 5xx behavior
    #[test]
    fn test_4xx_vs_5xx_behavior() {
        let error_4xx = SliceError::origin_client_error(404, "Not Found");
        let error_5xx = SliceError::origin_server_error(500, "Internal Server Error");
        
        // 4xx should not retry, 5xx should retry
        assert!(!error_4xx.should_retry());
        assert!(error_5xx.should_retry());
        
        // 4xx passes through status, 5xx returns 502
        assert_eq!(error_4xx.to_http_status(), 404);
        assert_eq!(error_5xx.to_http_status(), 502);
        
        // Neither should fallback
        assert!(!error_4xx.fallback_to_normal_proxy());
        assert!(!error_5xx.fallback_to_normal_proxy());
    }

    /// Unit test: Verify from_http_status categorization
    #[test]
    fn test_from_http_status_categorization() {
        // 4xx should create OriginClientError
        let error_404 = SliceError::from_http_status(404, "Not Found");
        assert!(matches!(error_404, SliceError::OriginClientError { .. }));
        assert!(!error_404.should_retry());
        assert_eq!(error_404.to_http_status(), 404);
        
        let error_400 = SliceError::from_http_status(400, "Bad Request");
        assert!(matches!(error_400, SliceError::OriginClientError { .. }));
        
        // 5xx should create OriginServerError
        let error_500 = SliceError::from_http_status(500, "Internal Server Error");
        assert!(matches!(error_500, SliceError::OriginServerError { .. }));
        assert!(error_500.should_retry());
        assert_eq!(error_500.to_http_status(), 502);
        
        let error_503 = SliceError::from_http_status(503, "Service Unavailable");
        assert!(matches!(error_503, SliceError::OriginServerError { .. }));
    }
}
