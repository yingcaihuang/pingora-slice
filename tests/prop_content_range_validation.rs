// Feature: pingora-slice, Property 16: Content-Range 验证
// **Validates: Requirements 8.3, 8.4**
//
// Property: For any 206 response received for a subrequest, if the Content-Range 
// header does not match the requested range, the response should be treated as an error

use pingora_slice::error::SliceError;
use pingora_slice::models::{ByteRange, SliceSpec};
use pingora_slice::subrequest_manager::SubrequestManager;
use proptest::prelude::*;
use tokio::runtime::Runtime;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property 16: Content-Range validation correctness
    /// 
    /// For any requested byte range, when the server returns a 206 response
    /// with a Content-Range header that matches the request, the subrequest
    /// should succeed. When the Content-Range doesn't match, it should fail.
    #[test]
    fn prop_content_range_validation_matching(
        slice_index in 0usize..10,
        start in 0u64..10_000_000,
        size in 1u64..1_000_000,
        total_size in 10_000_000u64..100_000_000,
    ) {
        let rt = Runtime::new().unwrap();
        
        let result: Result<(), TestCaseError> = rt.block_on(async {
            let mock_server = MockServer::start().await;
            let end = start + size - 1;
            
            // Ensure total_size is at least end + 1
            let total_size = std::cmp::max(total_size, end + 1);
            
            // Mock endpoint that returns matching Content-Range
            Mock::given(method("GET"))
                .and(path("/test-file"))
                .respond_with(move |_req: &wiremock::Request| {
                    ResponseTemplate::new(206)
                        .insert_header(
                            "Content-Range",
                            format!("bytes {}-{}/{}", start, end, total_size).as_str()
                        )
                        .set_body_bytes(vec![0u8; size as usize])
                })
                .expect(1)
                .mount(&mock_server)
                .await;
            
            let manager = SubrequestManager::new(1, 0);
            let range = ByteRange::new(start, end).unwrap();
            let slice = SliceSpec::new(slice_index, range);
            
            let url = format!("{}/test-file", mock_server.uri());
            let result = manager.fetch_single_slice(&slice, &url).await;
            
            // When Content-Range matches the request, it should succeed
            prop_assert!(
                result.is_ok(),
                "Request should succeed when Content-Range matches: expected {}-{}, got matching response",
                start,
                end
            );
            
            let subrequest_result = result.unwrap();
            prop_assert_eq!(
                subrequest_result.status,
                206,
                "Status should be 206"
            );
            prop_assert_eq!(
                subrequest_result.slice_index,
                slice_index,
                "Slice index should match"
            );
            
            Ok(())
        });
        
        result?;
    }

    /// Property 16: Content-Range mismatch detection
    /// 
    /// For any requested byte range, when the server returns a Content-Range
    /// that doesn't match the request, the subrequest should fail with an error.
    #[test]
    fn prop_content_range_validation_mismatch(
        slice_index in 0usize..10,
        requested_start in 0u64..10_000_000,
        requested_size in 1u64..1_000_000,
        returned_start in 0u64..10_000_000,
        returned_size in 1u64..1_000_000,
        total_size in 10_000_000u64..100_000_000,
    ) {
        // Only test cases where the ranges actually differ
        let requested_end = requested_start + requested_size - 1;
        let returned_end = returned_start + returned_size - 1;
        
        prop_assume!(
            requested_start != returned_start || requested_end != returned_end
        );
        
        let rt = Runtime::new().unwrap();
        
        let result: Result<(), TestCaseError> = rt.block_on(async {
            let mock_server = MockServer::start().await;
            
            // Ensure total_size is large enough
            let total_size = std::cmp::max(
                total_size,
                std::cmp::max(requested_end, returned_end) + 1
            );
            
            // Mock endpoint that returns mismatched Content-Range
            Mock::given(method("GET"))
                .and(path("/test-file"))
                .respond_with(move |_req: &wiremock::Request| {
                    ResponseTemplate::new(206)
                        .insert_header(
                            "Content-Range",
                            format!("bytes {}-{}/{}", returned_start, returned_end, total_size).as_str()
                        )
                        .set_body_bytes(vec![0u8; returned_size as usize])
                })
                .expect(1)
                .mount(&mock_server)
                .await;
            
            let manager = SubrequestManager::new(1, 0); // No retries
            let range = ByteRange::new(requested_start, requested_end).unwrap();
            let slice = SliceSpec::new(slice_index, range);
            
            let url = format!("{}/test-file", mock_server.uri());
            let result = manager.fetch_single_slice(&slice, &url).await;
            
            // When Content-Range doesn't match, it should fail
            prop_assert!(
                result.is_err(),
                "Request should fail when Content-Range doesn't match: requested {}-{}, got {}-{}",
                requested_start,
                requested_end,
                returned_start,
                returned_end
            );
            
            // Verify it's the correct error type
            match result {
                Err(SliceError::SubrequestFailed { slice_index: idx, attempts }) => {
                    prop_assert_eq!(
                        idx,
                        slice_index,
                        "Error should contain correct slice index"
                    );
                    prop_assert_eq!(
                        attempts,
                        1,
                        "Should have made exactly 1 attempt (no retries)"
                    );
                }
                Err(e) => {
                    prop_assert!(
                        false,
                        "Expected SubrequestFailed error, got: {:?}",
                        e
                    );
                }
                Ok(_) => {
                    prop_assert!(
                        false,
                        "Expected error but got success"
                    );
                }
            }
            
            Ok(())
        });
        
        result?;
    }

    /// Property 16: Missing Content-Range header
    /// 
    /// For any 206 response without a Content-Range header, the request should fail.
    #[test]
    fn prop_content_range_validation_missing_header(
        slice_index in 0usize..10,
        start in 0u64..10_000_000,
        size in 1u64..1_000_000,
    ) {
        let rt = Runtime::new().unwrap();
        
        let result: Result<(), TestCaseError> = rt.block_on(async {
            let mock_server = MockServer::start().await;
            let end = start + size - 1;
            
            // Mock endpoint that returns 206 but without Content-Range header
            Mock::given(method("GET"))
                .and(path("/test-file"))
                .respond_with(move |_req: &wiremock::Request| {
                    ResponseTemplate::new(206)
                        .set_body_bytes(vec![0u8; size as usize])
                    // Intentionally not setting Content-Range header
                })
                .expect(1)
                .mount(&mock_server)
                .await;
            
            let manager = SubrequestManager::new(1, 0);
            let range = ByteRange::new(start, end).unwrap();
            let slice = SliceSpec::new(slice_index, range);
            
            let url = format!("{}/test-file", mock_server.uri());
            let result = manager.fetch_single_slice(&slice, &url).await;
            
            // Missing Content-Range header should cause failure
            prop_assert!(
                result.is_err(),
                "Request should fail when Content-Range header is missing"
            );
            
            Ok(())
        });
        
        result?;
    }

    /// Property 16: Invalid Content-Range format
    /// 
    /// For any 206 response with an invalid Content-Range format, the request should fail.
    #[test]
    fn prop_content_range_validation_invalid_format(
        slice_index in 0usize..10,
        start in 0u64..10_000_000,
        size in 1u64..1_000_000,
        invalid_format in prop::sample::select(vec![
            "invalid".to_string(),
            "0-1023".to_string(),
            "bytes 0:1023/10000".to_string(),
            "bytes 0-1023".to_string(),
            "bytes /10000".to_string(),
            "range 0-1023/10000".to_string(),
        ]),
    ) {
        let rt = Runtime::new().unwrap();
        
        let result: Result<(), TestCaseError> = rt.block_on(async {
            let mock_server = MockServer::start().await;
            let end = start + size - 1;
            
            let invalid_format_clone = invalid_format.clone();
            
            // Mock endpoint that returns invalid Content-Range format
            Mock::given(method("GET"))
                .and(path("/test-file"))
                .respond_with(move |_req: &wiremock::Request| {
                    ResponseTemplate::new(206)
                        .insert_header("Content-Range", invalid_format_clone.as_str())
                        .set_body_bytes(vec![0u8; size as usize])
                })
                .expect(1)
                .mount(&mock_server)
                .await;
            
            let manager = SubrequestManager::new(1, 0);
            let range = ByteRange::new(start, end).unwrap();
            let slice = SliceSpec::new(slice_index, range);
            
            let url = format!("{}/test-file", mock_server.uri());
            let result = manager.fetch_single_slice(&slice, &url).await;
            
            // Invalid Content-Range format should cause failure
            prop_assert!(
                result.is_err(),
                "Request should fail when Content-Range format is invalid: {}",
                invalid_format
            );
            
            Ok(())
        });
        
        result?;
    }

    /// Property 16: Boundary values validation
    /// 
    /// Content-Range validation should work correctly for boundary values
    /// (zero start, maximum values, single byte ranges).
    #[test]
    fn prop_content_range_validation_boundaries(
        slice_index in 0usize..10,
        use_zero_start in proptest::bool::ANY,
        use_single_byte in proptest::bool::ANY,
        total_size in 1_000_000u64..100_000_000,
    ) {
        let rt = Runtime::new().unwrap();
        
        let result: Result<(), TestCaseError> = rt.block_on(async {
            let mock_server = MockServer::start().await;
            
            let (start, end) = if use_single_byte {
                let pos = if use_zero_start { 0 } else { total_size / 2 };
                (pos, pos)
            } else if use_zero_start {
                (0, 1023)
            } else {
                let start = total_size - 1024;
                (start, total_size - 1)
            };
            
            let size = end - start + 1;
            
            // Mock endpoint with matching Content-Range
            Mock::given(method("GET"))
                .and(path("/test-file"))
                .respond_with(move |_req: &wiremock::Request| {
                    ResponseTemplate::new(206)
                        .insert_header(
                            "Content-Range",
                            format!("bytes {}-{}/{}", start, end, total_size).as_str()
                        )
                        .set_body_bytes(vec![0u8; size as usize])
                })
                .expect(1)
                .mount(&mock_server)
                .await;
            
            let manager = SubrequestManager::new(1, 0);
            let range = ByteRange::new(start, end).unwrap();
            let slice = SliceSpec::new(slice_index, range);
            
            let url = format!("{}/test-file", mock_server.uri());
            let result = manager.fetch_single_slice(&slice, &url).await;
            
            // Boundary values should work correctly
            prop_assert!(
                result.is_ok(),
                "Request should succeed for boundary values: {}-{}/{}",
                start,
                end,
                total_size
            );
            
            Ok(())
        });
        
        result?;
    }

    /// Property 16: Whitespace tolerance in Content-Range
    /// 
    /// Content-Range validation should handle whitespace correctly.
    #[test]
    fn prop_content_range_validation_whitespace(
        slice_index in 0usize..10,
        start in 0u64..10_000_000,
        size in 1u64..1_000_000,
        total_size in 10_000_000u64..100_000_000,
        leading_spaces in 0usize..3,
        trailing_spaces in 0usize..3,
    ) {
        let rt = Runtime::new().unwrap();
        
        let result: Result<(), TestCaseError> = rt.block_on(async {
            let mock_server = MockServer::start().await;
            let end = start + size - 1;
            let total_size = std::cmp::max(total_size, end + 1);
            
            // Create Content-Range with whitespace
            let leading = " ".repeat(leading_spaces);
            let trailing = " ".repeat(trailing_spaces);
            let content_range = format!(
                "{}bytes {}-{}/{}{}",
                leading, start, end, total_size, trailing
            );
            
            let content_range_clone = content_range.clone();
            
            // Mock endpoint with whitespace in Content-Range
            Mock::given(method("GET"))
                .and(path("/test-file"))
                .respond_with(move |_req: &wiremock::Request| {
                    ResponseTemplate::new(206)
                        .insert_header("Content-Range", content_range_clone.as_str())
                        .set_body_bytes(vec![0u8; size as usize])
                })
                .expect(1)
                .mount(&mock_server)
                .await;
            
            let manager = SubrequestManager::new(1, 0);
            let range = ByteRange::new(start, end).unwrap();
            let slice = SliceSpec::new(slice_index, range);
            
            let url = format!("{}/test-file", mock_server.uri());
            let result = manager.fetch_single_slice(&slice, &url).await;
            
            // Should handle whitespace correctly
            prop_assert!(
                result.is_ok(),
                "Request should succeed with whitespace in Content-Range: '{}'",
                content_range
            );
            
            Ok(())
        });
        
        result?;
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    /// Unit test: Exact match should succeed
    #[tokio::test]
    async fn test_content_range_exact_match() {
        let mock_server = MockServer::start().await;
        
        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(206)
                .insert_header("Content-Range", "bytes 0-1023/10000")
                .set_body_bytes(vec![0u8; 1024]))
            .expect(1)
            .mount(&mock_server)
            .await;
        
        let manager = SubrequestManager::new(1, 0);
        let range = ByteRange::new(0, 1023).unwrap();
        let slice = SliceSpec::new(0, range);
        
        let url = format!("{}/test", mock_server.uri());
        let result = manager.fetch_single_slice(&slice, &url).await;
        
        assert!(result.is_ok(), "Exact match should succeed");
    }

    /// Unit test: Start mismatch should fail
    #[tokio::test]
    async fn test_content_range_start_mismatch() {
        let mock_server = MockServer::start().await;
        
        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(206)
                .insert_header("Content-Range", "bytes 100-1123/10000")
                .set_body_bytes(vec![0u8; 1024]))
            .expect(1)
            .mount(&mock_server)
            .await;
        
        let manager = SubrequestManager::new(1, 0);
        let range = ByteRange::new(0, 1023).unwrap();
        let slice = SliceSpec::new(0, range);
        
        let url = format!("{}/test", mock_server.uri());
        let result = manager.fetch_single_slice(&slice, &url).await;
        
        assert!(result.is_err(), "Start mismatch should fail");
    }

    /// Unit test: End mismatch should fail
    #[tokio::test]
    async fn test_content_range_end_mismatch() {
        let mock_server = MockServer::start().await;
        
        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(206)
                .insert_header("Content-Range", "bytes 0-2047/10000")
                .set_body_bytes(vec![0u8; 2048]))
            .expect(1)
            .mount(&mock_server)
            .await;
        
        let manager = SubrequestManager::new(1, 0);
        let range = ByteRange::new(0, 1023).unwrap();
        let slice = SliceSpec::new(0, range);
        
        let url = format!("{}/test", mock_server.uri());
        let result = manager.fetch_single_slice(&slice, &url).await;
        
        assert!(result.is_err(), "End mismatch should fail");
    }

    /// Unit test: Missing Content-Range header should fail
    #[tokio::test]
    async fn test_content_range_missing() {
        let mock_server = MockServer::start().await;
        
        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(206)
                .set_body_bytes(vec![0u8; 1024]))
            .expect(1)
            .mount(&mock_server)
            .await;
        
        let manager = SubrequestManager::new(1, 0);
        let range = ByteRange::new(0, 1023).unwrap();
        let slice = SliceSpec::new(0, range);
        
        let url = format!("{}/test", mock_server.uri());
        let result = manager.fetch_single_slice(&slice, &url).await;
        
        assert!(result.is_err(), "Missing Content-Range should fail");
    }

    /// Unit test: Invalid format should fail
    #[tokio::test]
    async fn test_content_range_invalid_format() {
        let mock_server = MockServer::start().await;
        
        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(206)
                .insert_header("Content-Range", "invalid-format")
                .set_body_bytes(vec![0u8; 1024]))
            .expect(1)
            .mount(&mock_server)
            .await;
        
        let manager = SubrequestManager::new(1, 0);
        let range = ByteRange::new(0, 1023).unwrap();
        let slice = SliceSpec::new(0, range);
        
        let url = format!("{}/test", mock_server.uri());
        let result = manager.fetch_single_slice(&slice, &url).await;
        
        assert!(result.is_err(), "Invalid format should fail");
    }

    /// Unit test: Single byte range
    #[tokio::test]
    async fn test_content_range_single_byte() {
        let mock_server = MockServer::start().await;
        
        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(206)
                .insert_header("Content-Range", "bytes 0-0/10000")
                .set_body_bytes(vec![0u8; 1]))
            .expect(1)
            .mount(&mock_server)
            .await;
        
        let manager = SubrequestManager::new(1, 0);
        let range = ByteRange::new(0, 0).unwrap();
        let slice = SliceSpec::new(0, range);
        
        let url = format!("{}/test", mock_server.uri());
        let result = manager.fetch_single_slice(&slice, &url).await;
        
        assert!(result.is_ok(), "Single byte range should succeed");
    }

    /// Unit test: Large byte values
    #[tokio::test]
    async fn test_content_range_large_values() {
        let mock_server = MockServer::start().await;
        
        let start = 1_000_000_000u64;
        let end = 1_000_001_023u64;
        let total = 10_000_000_000u64;
        
        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(206)
                .insert_header("Content-Range", format!("bytes {}-{}/{}", start, end, total).as_str())
                .set_body_bytes(vec![0u8; 1024]))
            .expect(1)
            .mount(&mock_server)
            .await;
        
        let manager = SubrequestManager::new(1, 0);
        let range = ByteRange::new(start, end).unwrap();
        let slice = SliceSpec::new(0, range);
        
        let url = format!("{}/test", mock_server.uri());
        let result = manager.fetch_single_slice(&slice, &url).await;
        
        assert!(result.is_ok(), "Large values should work correctly");
    }
}
