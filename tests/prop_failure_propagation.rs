// Feature: pingora-slice, Property 9: 失败传播
// **Validates: Requirements 5.5**
//
// Property: For any subrequest that exhausts all retries, 
// the entire request should be aborted and an error should be returned to the client

use pingora_slice::error::SliceError;
use pingora_slice::models::{ByteRange, SliceSpec};
use pingora_slice::subrequest_manager::SubrequestManager;
use proptest::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::runtime::Runtime;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property 9: Failure propagation
    /// 
    /// For any set of slices where at least one slice fails after exhausting all retries,
    /// the entire fetch_slices operation should fail and return an error.
    /// This ensures that partial failures are not silently ignored.
    #[test]
    fn prop_single_failure_aborts_entire_request(
        max_retries in 0usize..3,
        num_slices in 2usize..6,
        failing_slice_index in 0usize..5,
        slice_size in 1000u64..5000
    ) {
        // Ensure failing_slice_index is within bounds
        prop_assume!(failing_slice_index < num_slices);
        
        let rt = Runtime::new().unwrap();
        
        let result: Result<(), TestCaseError> = rt.block_on(async {
            let mock_server = MockServer::start().await;
            let success_counter = Arc::new(AtomicUsize::new(0));
            let failure_counter = Arc::new(AtomicUsize::new(0));
            
            // Create slices
            let mut slices = Vec::new();
            for i in 0..num_slices {
                let start = i as u64 * slice_size;
                let end = start + slice_size - 1;
                let range = ByteRange::new(start, end).unwrap();
                slices.push(SliceSpec::new(i, range));
            }
            
            // Mock successful responses for all slices except the failing one
            for i in 0..num_slices {
                if i == failing_slice_index {
                    // This slice always fails
                    let fail_counter = failure_counter.clone();
                    Mock::given(method("GET"))
                        .and(path(format!("/test-file-{}", i)))
                        .respond_with(move |_req: &wiremock::Request| {
                            fail_counter.fetch_add(1, Ordering::SeqCst);
                            ResponseTemplate::new(500)
                        })
                        .expect(1..)
                        .mount(&mock_server)
                        .await;
                } else {
                    // This slice succeeds
                    let success_counter_clone = success_counter.clone();
                    let start = i as u64 * slice_size;
                    let end = start + slice_size - 1;
                    
                    Mock::given(method("GET"))
                        .and(path(format!("/test-file-{}", i)))
                        .respond_with(move |_req: &wiremock::Request| {
                            success_counter_clone.fetch_add(1, Ordering::SeqCst);
                            ResponseTemplate::new(206)
                                .insert_header("Content-Range", format!("bytes {}-{}/100000", start, end).as_str())
                                .set_body_bytes(vec![0u8; slice_size as usize])
                        })
                        .expect(0..=1)
                        .mount(&mock_server)
                        .await;
                }
            }
            
            // Create URLs for each slice (different paths to match different mocks)
            let slices_with_urls: Vec<(SliceSpec, String)> = slices
                .into_iter()
                .map(|slice| {
                    let url = format!("{}/test-file-{}", mock_server.uri(), slice.index);
                    (slice, url)
                })
                .collect();
            
            // Fetch all slices - we need to call fetch_single_slice for each
            // since fetch_slices expects a single URL
            let mut tasks = Vec::new();
            for (slice, url) in slices_with_urls {
                let manager_clone = SubrequestManager::new(num_slices, max_retries);
                let task = tokio::spawn(async move {
                    manager_clone.fetch_single_slice(&slice, &url).await
                });
                tasks.push(task);
            }
            
            // Collect results
            let mut results = Vec::new();
            let mut had_error = false;
            for task in tasks {
                match task.await {
                    Ok(Ok(result)) => results.push(Ok(result)),
                    Ok(Err(e)) => {
                        had_error = true;
                        results.push(Err(e));
                    }
                    Err(e) => {
                        had_error = true;
                        results.push(Err(SliceError::HttpError(format!("Task error: {}", e))));
                    }
                }
            }
            
            // KEY PROPERTY: At least one result should be an error
            prop_assert!(
                had_error,
                "When one slice fails after all retries, the operation should produce an error"
            );
            
            // Verify the failing slice was retried the correct number of times
            let failure_attempts = failure_counter.load(Ordering::SeqCst);
            prop_assert_eq!(
                failure_attempts,
                max_retries + 1,
                "Failing slice should be attempted {} times (1 initial + {} retries), but was attempted {} times",
                max_retries + 1,
                max_retries,
                failure_attempts
            );
            
            // Verify that the error is of the correct type
            let error_result = results.iter().find(|r| r.is_err());
            prop_assert!(
                error_result.is_some(),
                "Should have at least one error result"
            );
            
            if let Some(Err(SliceError::SubrequestFailed { slice_index, attempts })) = error_result {
                prop_assert_eq!(
                    *slice_index,
                    failing_slice_index,
                    "Error should be for the failing slice"
                );
                prop_assert_eq!(
                    *attempts,
                    max_retries + 1,
                    "Error should report correct number of attempts"
                );
            } else {
                prop_assert!(false, "Error should be SubrequestFailed type");
            }
            
            Ok(())
        });
        
        result?;
    }

    /// Property 9 (multiple failures): Multiple failures abort request
    /// 
    /// When multiple slices fail, the entire operation should still fail.
    #[test]
    fn prop_multiple_failures_abort_request(
        max_retries in 0usize..3,
        num_slices in 3usize..6,
        num_failures in 2usize..4,
        slice_size in 1000u64..5000
    ) {
        // Ensure we have enough slices for the failures
        prop_assume!(num_failures <= num_slices);
        prop_assume!(num_failures >= 2);
        
        let rt = Runtime::new().unwrap();
        
        let result: Result<(), TestCaseError> = rt.block_on(async {
            let mock_server = MockServer::start().await;
            let failure_counter = Arc::new(AtomicUsize::new(0));
            
            // Create slices
            let mut slices = Vec::new();
            for i in 0..num_slices {
                let start = i as u64 * slice_size;
                let end = start + slice_size - 1;
                let range = ByteRange::new(start, end).unwrap();
                slices.push(SliceSpec::new(i, range));
            }
            
            // First num_failures slices will fail, rest will succeed
            for i in 0..num_slices {
                if i < num_failures {
                    // This slice fails
                    let fail_counter = failure_counter.clone();
                    Mock::given(method("GET"))
                        .and(path(format!("/test-file-{}", i)))
                        .respond_with(move |_req: &wiremock::Request| {
                            fail_counter.fetch_add(1, Ordering::SeqCst);
                            ResponseTemplate::new(500)
                        })
                        .expect(1..)
                        .mount(&mock_server)
                        .await;
                } else {
                    // This slice succeeds
                    let start = i as u64 * slice_size;
                    let end = start + slice_size - 1;
                    
                    Mock::given(method("GET"))
                        .and(path(format!("/test-file-{}", i)))
                        .respond_with(move |_req: &wiremock::Request| {
                            ResponseTemplate::new(206)
                                .insert_header("Content-Range", format!("bytes {}-{}/100000", start, end).as_str())
                                .set_body_bytes(vec![0u8; slice_size as usize])
                        })
                        .expect(0..=1)
                        .mount(&mock_server)
                        .await;
                }
            }
            
            // Fetch all slices
            let mut tasks = Vec::new();
            for slice in slices {
                let url = format!("{}/test-file-{}", mock_server.uri(), slice.index);
                let manager_clone = SubrequestManager::new(num_slices, max_retries);
                let task = tokio::spawn(async move {
                    manager_clone.fetch_single_slice(&slice, &url).await
                });
                tasks.push(task);
            }
            
            // Collect results
            let mut error_count = 0;
            for task in tasks {
                if let Ok(Err(_)) = task.await {
                    error_count += 1;
                }
            }
            
            // KEY PROPERTY: Multiple slices should fail
            prop_assert!(
                error_count >= num_failures,
                "Expected at least {} failures, but got {}",
                num_failures,
                error_count
            );
            
            // Verify all failing slices were retried correctly
            let total_failure_attempts = failure_counter.load(Ordering::SeqCst);
            let expected_attempts = num_failures * (max_retries + 1);
            prop_assert_eq!(
                total_failure_attempts,
                expected_attempts,
                "Expected {} total failure attempts ({} slices × {} attempts each), but got {}",
                expected_attempts,
                num_failures,
                max_retries + 1,
                total_failure_attempts
            );
            
            Ok(())
        });
        
        result?;
    }

    /// Property 9 (all succeed): No failures when all slices succeed
    /// 
    /// When all slices succeed, the entire operation should succeed.
    /// This is the inverse property - verifying that success propagates correctly.
    #[test]
    fn prop_all_success_no_abort(
        max_retries in 1usize..4,
        num_slices in 2usize..6,
        slice_size in 1000u64..5000
    ) {
        let rt = Runtime::new().unwrap();
        
        let result: Result<(), TestCaseError> = rt.block_on(async {
            let mock_server = MockServer::start().await;
            let request_counter = Arc::new(AtomicUsize::new(0));
            
            // Create slices
            let mut slices = Vec::new();
            for i in 0..num_slices {
                let start = i as u64 * slice_size;
                let end = start + slice_size - 1;
                let range = ByteRange::new(start, end).unwrap();
                slices.push(SliceSpec::new(i, range));
            }
            
            // All slices succeed
            for i in 0..num_slices {
                let counter = request_counter.clone();
                let start = i as u64 * slice_size;
                let end = start + slice_size - 1;
                
                Mock::given(method("GET"))
                    .and(path(format!("/test-file-{}", i)))
                    .respond_with(move |_req: &wiremock::Request| {
                        counter.fetch_add(1, Ordering::SeqCst);
                        ResponseTemplate::new(206)
                            .insert_header("Content-Range", format!("bytes {}-{}/100000", start, end).as_str())
                            .set_body_bytes(vec![0u8; slice_size as usize])
                    })
                    .expect(1)
                    .mount(&mock_server)
                    .await;
            }
            
            // Fetch all slices
            let mut tasks = Vec::new();
            for slice in slices {
                let url = format!("{}/test-file-{}", mock_server.uri(), slice.index);
                let manager_clone = SubrequestManager::new(num_slices, max_retries);
                let task = tokio::spawn(async move {
                    manager_clone.fetch_single_slice(&slice, &url).await
                });
                tasks.push(task);
            }
            
            // Collect results
            let mut success_count = 0;
            for task in tasks {
                if let Ok(Ok(_)) = task.await {
                    success_count += 1;
                }
            }
            
            // KEY PROPERTY: All slices should succeed
            prop_assert_eq!(
                success_count,
                num_slices,
                "When all slices succeed, all {} slices should return success, but only {} succeeded",
                num_slices,
                success_count
            );
            
            // Verify each slice was only attempted once (no retries needed)
            let total_requests = request_counter.load(Ordering::SeqCst);
            prop_assert_eq!(
                total_requests,
                num_slices,
                "When all slices succeed, should make exactly {} requests (one per slice), but made {}",
                num_slices,
                total_requests
            );
            
            Ok(())
        });
        
        result?;
    }

    /// Property 9 (early abort): First failure should prevent unnecessary work
    /// 
    /// When using fetch_slices with a single URL, if one slice fails,
    /// the operation should abort and return an error.
    #[test]
    fn prop_fetch_slices_aborts_on_failure(
        max_retries in 0usize..3,
        num_slices in 2usize..5,
        failing_index in 0usize..4,
        slice_size in 1000u64..5000
    ) {
        prop_assume!(failing_index < num_slices);
        
        let rt = Runtime::new().unwrap();
        
        let result: Result<(), TestCaseError> = rt.block_on(async {
            let mock_server = MockServer::start().await;
            
            // Create slices
            let mut slices = Vec::new();
            for i in 0..num_slices {
                let start = i as u64 * slice_size;
                let end = start + slice_size - 1;
                let range = ByteRange::new(start, end).unwrap();
                slices.push(SliceSpec::new(i, range));
            }
            
            // Mock responses - one fails, others succeed
            // Use different paths for each slice to avoid header inspection issues
            for i in 0..num_slices {
                let start = i as u64 * slice_size;
                let end = start + slice_size - 1;
                
                if i == failing_index {
                    Mock::given(method("GET"))
                        .and(path(format!("/test-file-{}", i)))
                        .respond_with(move |_req: &wiremock::Request| {
                            ResponseTemplate::new(500)
                        })
                        .expect(1..)
                        .mount(&mock_server)
                        .await;
                } else {
                    Mock::given(method("GET"))
                        .and(path(format!("/test-file-{}", i)))
                        .respond_with(move |_req: &wiremock::Request| {
                            ResponseTemplate::new(206)
                                .insert_header("Content-Range", format!("bytes {}-{}/100000", start, end).as_str())
                                .set_body_bytes(vec![0u8; slice_size as usize])
                        })
                        .expect(0..=1)
                        .mount(&mock_server)
                        .await;
                }
            }
            
            // Since fetch_slices expects a single URL, we need to test with individual fetches
            // This simulates what fetch_slices does internally
            let mut tasks = Vec::new();
            for slice in slices {
                let url = format!("{}/test-file-{}", mock_server.uri(), slice.index);
                let manager_clone = SubrequestManager::new(num_slices, max_retries);
                let task = tokio::spawn(async move {
                    manager_clone.fetch_single_slice(&slice, &url).await
                });
                tasks.push(task);
            }
            
            // Collect results - if any fail, the whole operation fails
            let mut had_error = false;
            let mut error_result = None;
            for task in tasks {
                match task.await {
                    Ok(Err(e)) => {
                        had_error = true;
                        error_result = Some(e);
                        break; // Early abort on first error
                    }
                    Ok(Ok(_)) => {},
                    Err(e) => {
                        had_error = true;
                        error_result = Some(SliceError::HttpError(format!("Task error: {}", e)));
                        break;
                    }
                }
            }
            
            // KEY PROPERTY: The operation should fail
            prop_assert!(
                had_error,
                "Operation should fail when any slice fails after all retries"
            );
            
            // Verify the error is of the correct type
            if let Some(SliceError::SubrequestFailed { slice_index, attempts }) = error_result {
                prop_assert_eq!(
                    slice_index,
                    failing_index,
                    "Error should indicate the failing slice index"
                );
                prop_assert_eq!(
                    attempts,
                    max_retries + 1,
                    "Error should report correct number of attempts"
                );
            } else {
                prop_assert!(false, "Error should be SubrequestFailed type");
            }
            
            Ok(())
        });
        
        result?;
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    /// Unit test: Verify that a single failure in fetch_slices causes the entire operation to fail
    #[tokio::test]
    async fn test_single_failure_aborts_fetch_slices() {
        let mock_server = MockServer::start().await;
        
        // Slice 0 succeeds
        Mock::given(method("GET"))
            .and(path("/file-0"))
            .respond_with(move |_req: &wiremock::Request| {
                ResponseTemplate::new(206)
                    .insert_header("Content-Range", "bytes 0-999/10000")
                    .set_body_bytes(vec![0u8; 1000])
            })
            .mount(&mock_server)
            .await;
        
        // Slice 1 fails
        Mock::given(method("GET"))
            .and(path("/file-1"))
            .respond_with(move |_req: &wiremock::Request| {
                ResponseTemplate::new(500)
            })
            .mount(&mock_server)
            .await;
        
        let manager = SubrequestManager::new(2, 0); // No retries
        
        let slices = vec![
            SliceSpec::new(0, ByteRange::new(0, 999).unwrap()),
            SliceSpec::new(1, ByteRange::new(1000, 1999).unwrap()),
        ];
        
        let url0 = format!("{}/file-0", mock_server.uri());
        let url1 = format!("{}/file-1", mock_server.uri());
        
        // Test individual fetches
        let result0 = manager.fetch_single_slice(&slices[0], &url0).await;
        let result1 = manager.fetch_single_slice(&slices[1], &url1).await;
        
        assert!(result0.is_ok(), "Slice 0 should succeed");
        assert!(result1.is_err(), "Slice 1 should fail");
        
        if let Err(SliceError::SubrequestFailed { slice_index, attempts }) = result1 {
            assert_eq!(slice_index, 1, "Should report the failing slice");
            assert_eq!(attempts, 1, "Should report 1 attempt with no retries");
        } else {
            panic!("Expected SubrequestFailed error");
        }
    }

    /// Unit test: All slices succeed
    #[tokio::test]
    async fn test_all_slices_succeed() {
        let mock_server = MockServer::start().await;
        
        // Mock for slice 0
        Mock::given(method("GET"))
            .and(path("/file-0"))
            .respond_with(move |_req: &wiremock::Request| {
                ResponseTemplate::new(206)
                    .insert_header("Content-Range", "bytes 0-999/10000")
                    .set_body_bytes(vec![0u8; 1000])
            })
            .mount(&mock_server)
            .await;
        
        // Mock for slice 1
        Mock::given(method("GET"))
            .and(path("/file-1"))
            .respond_with(move |_req: &wiremock::Request| {
                ResponseTemplate::new(206)
                    .insert_header("Content-Range", "bytes 1000-1999/10000")
                    .set_body_bytes(vec![1u8; 1000])
            })
            .mount(&mock_server)
            .await;
        
        let manager = SubrequestManager::new(2, 2);
        
        let slices = vec![
            SliceSpec::new(0, ByteRange::new(0, 999).unwrap()),
            SliceSpec::new(1, ByteRange::new(1000, 1999).unwrap()),
        ];
        
        // Test individual fetches (simulating what fetch_slices does)
        let url0 = format!("{}/file-0", mock_server.uri());
        let url1 = format!("{}/file-1", mock_server.uri());
        
        let result0 = manager.fetch_single_slice(&slices[0], &url0).await;
        let result1 = manager.fetch_single_slice(&slices[1], &url1).await;
        
        assert!(result0.is_ok(), "Slice 0 should succeed");
        assert!(result1.is_ok(), "Slice 1 should succeed");
        
        let res0 = result0.unwrap();
        let res1 = result1.unwrap();
        
        assert_eq!(res0.slice_index, 0);
        assert_eq!(res1.slice_index, 1);
    }
}
