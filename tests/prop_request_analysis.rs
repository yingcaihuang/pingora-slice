// Feature: pingora-slice, Property 2: Range 请求透传
// **Validates: Requirements 2.3**
//
// Property: For any client request containing a Range header, the request 
// should bypass the slicing logic and be passed through to the origin server directly

use pingora_slice::config::SliceConfig;
use pingora_slice::request_analyzer::RequestAnalyzer;
use http::{Method, HeaderMap, HeaderValue};
use proptest::prelude::*;
use std::sync::Arc;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property 2: Range request passthrough
    /// 
    /// For any request that contains a Range header (regardless of the range value),
    /// the should_slice() method must return false, indicating that slicing should
    /// NOT be enabled and the request should be passed through to the origin.
    #[test]
    fn prop_range_request_passthrough(
        start in 0u64..=u64::MAX / 2,
        end in 0u64..=u64::MAX / 2,
        uri in "[a-z/]{1,50}",
        use_lowercase in proptest::bool::ANY,
    ) {
        // Ensure start <= end
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        
        // Create a Range header value
        let range_value = format!("bytes={}-{}", start, end);
        
        // Create headers with Range header (test both lowercase and capitalized)
        let mut headers = HeaderMap::new();
        let header_name = if use_lowercase { "range" } else { "Range" };
        headers.insert(
            header_name,
            HeaderValue::from_str(&range_value).unwrap()
        );
        
        // Create config with no patterns (would normally slice everything)
        let config = Arc::new(SliceConfig::default());
        let analyzer = RequestAnalyzer::new(config);
        
        // Test with GET method (which would normally enable slicing)
        let result = analyzer.should_slice(&Method::GET, &uri, &headers);
        
        // Property: should_slice MUST return false when Range header is present
        prop_assert!(
            !result,
            "should_slice() must return false for requests with Range header. \
             Range: {}, URI: {}, Header name: {}",
            range_value,
            uri,
            header_name
        );
    }

    /// Property 2 (with patterns): Range passthrough regardless of URL patterns
    /// 
    /// Even if the URL matches configured slice patterns, requests with Range
    /// headers should still bypass slicing.
    #[test]
    fn prop_range_passthrough_with_patterns(
        start in 0u64..=1_000_000u64,
        end in 0u64..=1_000_000u64,
        pattern_idx in 0usize..3,
    ) {
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        
        // Create config with various patterns
        let patterns = vec![
            "/large-files/".to_string(),
            "*.bin".to_string(),
            "/downloads/*".to_string(),
        ];
        
        let mut config = SliceConfig::default();
        config.slice_patterns = patterns.clone();
        let config = Arc::new(config);
        let analyzer = RequestAnalyzer::new(config);
        
        // Create URIs that match the patterns
        let uris = vec![
            "/large-files/test.bin",
            "/path/to/file.bin",
            "/downloads/archive.zip",
        ];
        
        let uri = uris[pattern_idx % uris.len()];
        
        // Create Range header
        let range_value = format!("bytes={}-{}", start, end);
        let mut headers = HeaderMap::new();
        headers.insert("range", HeaderValue::from_str(&range_value).unwrap());
        
        // Even though URI matches pattern, Range header should prevent slicing
        let result = analyzer.should_slice(&Method::GET, uri, &headers);
        
        prop_assert!(
            !result,
            "should_slice() must return false even when URI matches pattern. \
             URI: {}, Pattern: {}, Range: {}",
            uri,
            patterns[pattern_idx % patterns.len()],
            range_value
        );
    }

    /// Property 2 (various methods): Range passthrough for all HTTP methods
    /// 
    /// Range headers should cause passthrough regardless of HTTP method.
    #[test]
    fn prop_range_passthrough_all_methods(
        start in 0u64..=10000u64,
        end in 0u64..=10000u64,
        method_idx in 0usize..5,
    ) {
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        
        let methods = vec![
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::HEAD,
            Method::DELETE,
        ];
        
        let method = &methods[method_idx % methods.len()];
        
        // Create Range header
        let range_value = format!("bytes={}-{}", start, end);
        let mut headers = HeaderMap::new();
        headers.insert("range", HeaderValue::from_str(&range_value).unwrap());
        
        let config = Arc::new(SliceConfig::default());
        let analyzer = RequestAnalyzer::new(config);
        
        // should_slice should return false for any method with Range header
        let result = analyzer.should_slice(method, "/test.bin", &headers);
        
        prop_assert!(
            !result,
            "should_slice() must return false for any method with Range header. \
             Method: {}, Range: {}",
            method,
            range_value
        );
    }

    /// Property 2 (edge cases): Range passthrough with various range formats
    /// 
    /// Test that various valid Range header formats all trigger passthrough.
    #[test]
    fn prop_range_passthrough_various_formats(
        start in 0u64..=1000u64,
        end in 0u64..=1000u64,
        add_whitespace in proptest::bool::ANY,
    ) {
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        
        // Create Range header with optional whitespace
        let range_value = if add_whitespace {
            format!(" bytes={}-{} ", start, end)
        } else {
            format!("bytes={}-{}", start, end)
        };
        
        let mut headers = HeaderMap::new();
        headers.insert("range", HeaderValue::from_str(&range_value).unwrap());
        
        let config = Arc::new(SliceConfig::default());
        let analyzer = RequestAnalyzer::new(config);
        
        let result = analyzer.should_slice(&Method::GET, "/test.bin", &headers);
        
        prop_assert!(
            !result,
            "should_slice() must return false for Range header with format: '{}'",
            range_value
        );
    }

    /// Property 2 (case sensitivity): Both "range" and "Range" headers
    /// 
    /// HTTP headers are case-insensitive, so both lowercase and capitalized
    /// Range headers should trigger passthrough.
    #[test]
    fn prop_range_passthrough_case_insensitive(
        start in 0u64..=10000u64,
        end in 0u64..=10000u64,
    ) {
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        
        let range_value = format!("bytes={}-{}", start, end);
        let config = Arc::new(SliceConfig::default());
        let analyzer = RequestAnalyzer::new(config);
        
        // Test lowercase "range"
        let mut headers_lower = HeaderMap::new();
        headers_lower.insert("range", HeaderValue::from_str(&range_value).unwrap());
        let result_lower = analyzer.should_slice(&Method::GET, "/test.bin", &headers_lower);
        
        // Test capitalized "Range"
        let mut headers_upper = HeaderMap::new();
        headers_upper.insert("Range", HeaderValue::from_str(&range_value).unwrap());
        let result_upper = analyzer.should_slice(&Method::GET, "/test.bin", &headers_upper);
        
        prop_assert!(
            !result_lower,
            "should_slice() must return false for lowercase 'range' header"
        );
        
        prop_assert!(
            !result_upper,
            "should_slice() must return false for capitalized 'Range' header"
        );
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_range_header_prevents_slicing() {
        let config = Arc::new(SliceConfig::default());
        let analyzer = RequestAnalyzer::new(config);
        
        let mut headers = HeaderMap::new();
        headers.insert("range", HeaderValue::from_str("bytes=0-1023").unwrap());
        
        let result = analyzer.should_slice(&Method::GET, "/test.bin", &headers);
        assert!(!result, "Range header should prevent slicing");
    }

    #[test]
    fn test_range_header_case_insensitive() {
        let config = Arc::new(SliceConfig::default());
        let analyzer = RequestAnalyzer::new(config);
        
        // Test lowercase
        let mut headers = HeaderMap::new();
        headers.insert("range", HeaderValue::from_str("bytes=0-1023").unwrap());
        assert!(!analyzer.should_slice(&Method::GET, "/test.bin", &headers));
        
        // Test capitalized
        let mut headers = HeaderMap::new();
        headers.insert("Range", HeaderValue::from_str("bytes=0-1023").unwrap());
        assert!(!analyzer.should_slice(&Method::GET, "/test.bin", &headers));
    }

    #[test]
    fn test_range_header_overrides_pattern_match() {
        let mut config = SliceConfig::default();
        config.slice_patterns = vec!["/large-files/".to_string()];
        let config = Arc::new(config);
        let analyzer = RequestAnalyzer::new(config);
        
        let mut headers = HeaderMap::new();
        headers.insert("range", HeaderValue::from_str("bytes=0-1023").unwrap());
        
        // Even though URI matches pattern, Range header should prevent slicing
        let result = analyzer.should_slice(&Method::GET, "/large-files/test.bin", &headers);
        assert!(!result, "Range header should override pattern matching");
    }

    #[test]
    fn test_no_range_header_allows_slicing() {
        let config = Arc::new(SliceConfig::default());
        let analyzer = RequestAnalyzer::new(config);
        
        let headers = HeaderMap::new();
        
        // Without Range header, GET request should enable slicing
        let result = analyzer.should_slice(&Method::GET, "/test.bin", &headers);
        assert!(result, "GET request without Range header should enable slicing");
    }

    #[test]
    fn test_range_header_with_post_method() {
        let config = Arc::new(SliceConfig::default());
        let analyzer = RequestAnalyzer::new(config);
        
        let mut headers = HeaderMap::new();
        headers.insert("range", HeaderValue::from_str("bytes=0-1023").unwrap());
        
        // POST with Range header should not slice (POST never slices anyway)
        let result = analyzer.should_slice(&Method::POST, "/test.bin", &headers);
        assert!(!result, "POST request should not enable slicing");
    }
}

// Feature: pingora-slice, Property 3: URL 模式匹配一致性
// **Validates: Requirements 2.4**
//
// Property: For any request URL and configured pattern list, the pattern matching 
// result should be deterministic and consistent across multiple evaluations

#[cfg(test)]
mod prop_url_pattern_matching {
    use super::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property 3: URL pattern matching consistency
        /// 
        /// For any URL and pattern configuration, calling should_slice() multiple times
        /// with the same inputs must always produce the same result. This ensures
        /// deterministic behavior and no hidden state affecting pattern matching.
        #[test]
        fn prop_url_pattern_matching_deterministic(
            uri in "[a-z0-9/_.-]{1,100}",
            pattern_count in 0usize..5,
            evaluation_count in 2usize..10,
        ) {
            // Generate random patterns
            let patterns: Vec<String> = (0..pattern_count)
                .map(|i| match i % 4 {
                    0 => format!("/path{}/", i),
                    1 => format!("*.ext{}", i),
                    2 => format!("/prefix{}/*", i),
                    _ => format!("/*/middle{}/*/", i),
                })
                .collect();

            let mut config = SliceConfig::default();
            config.slice_patterns = patterns;
            let config = Arc::new(config);
            let analyzer = RequestAnalyzer::new(config);

            let headers = HeaderMap::new();
            
            // Evaluate the same input multiple times
            let mut results = Vec::new();
            for _ in 0..evaluation_count {
                let result = analyzer.should_slice(&Method::GET, &uri, &headers);
                results.push(result);
            }

            // All results must be identical
            let first_result = results[0];
            for (idx, result) in results.iter().enumerate() {
                prop_assert_eq!(
                    *result,
                    first_result,
                    "Pattern matching must be deterministic. Evaluation {} returned {} but first evaluation returned {}. URI: {}",
                    idx,
                    result,
                    first_result,
                    uri
                );
            }
        }

        /// Property 3 (with various patterns): Deterministic matching with different pattern types
        /// 
        /// Test determinism with various pattern types including wildcards, prefixes, and exact matches.
        #[test]
        fn prop_pattern_matching_various_types(
            path_segment in "[a-z]{1,20}",
            extension in "[a-z]{2,4}",
            evaluation_count in 2usize..5,
        ) {
            // Create URIs that might match different pattern types
            let uris = vec![
                format!("/{}/file.{}", path_segment, extension),
                format!("/downloads/{}.{}", path_segment, extension),
                format!("/large-files/{}/data.{}", path_segment, extension),
                format!("/{}/nested/path/file.{}", path_segment, extension),
            ];

            // Create various pattern types
            let patterns = vec![
                format!("*.{}", extension),                    // Wildcard suffix
                format!("/downloads/*"),                       // Prefix with wildcard
                format!("/large-files/*/"),                    // Middle wildcard
                format!("/{}/*/", path_segment),               // Multiple wildcards
            ];

            let mut config = SliceConfig::default();
            config.slice_patterns = patterns;
            let config = Arc::new(config);
            let analyzer = RequestAnalyzer::new(config);

            let headers = HeaderMap::new();

            // Test each URI
            for uri in &uris {
                let mut results = Vec::new();
                
                // Evaluate multiple times
                for _ in 0..evaluation_count {
                    let result = analyzer.should_slice(&Method::GET, uri, &headers);
                    results.push(result);
                }

                // All results must be identical
                let first_result = results[0];
                for (idx, result) in results.iter().enumerate() {
                    prop_assert_eq!(
                        *result,
                        first_result,
                        "Pattern matching must be deterministic for URI: {}. Evaluation {} returned {} but first returned {}",
                        uri,
                        idx,
                        result,
                        first_result
                    );
                }
            }
        }

        /// Property 3 (empty patterns): Deterministic behavior with empty pattern list
        /// 
        /// When no patterns are configured, all GET requests should consistently enable slicing.
        #[test]
        fn prop_empty_patterns_deterministic(
            uri in "[a-z0-9/_.-]{1,100}",
            evaluation_count in 2usize..10,
        ) {
            // Empty pattern list means slice everything
            let config = Arc::new(SliceConfig::default());
            let analyzer = RequestAnalyzer::new(config);

            let headers = HeaderMap::new();

            let mut results = Vec::new();
            for _ in 0..evaluation_count {
                let result = analyzer.should_slice(&Method::GET, &uri, &headers);
                results.push(result);
            }

            // All results must be true (slice everything when no patterns)
            for (idx, result) in results.iter().enumerate() {
                prop_assert!(
                    *result,
                    "With empty patterns, should_slice must always return true. Evaluation {} returned false for URI: {}",
                    idx,
                    uri
                );
            }

            // All results must be identical
            let first_result = results[0];
            for result in &results {
                prop_assert_eq!(*result, first_result);
            }
        }

        /// Property 3 (pattern order independence): Pattern matching should not depend on evaluation order
        /// 
        /// The order in which we evaluate patterns should not affect the result.
        #[test]
        fn prop_pattern_order_independence(
            uri in "[a-z0-9/_.-]{1,50}",
            shuffle_seed in 0usize..100,
        ) {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};

            // Create a set of patterns
            let patterns = vec![
                "/large-files/".to_string(),
                "*.bin".to_string(),
                "/downloads/*".to_string(),
                "/media/*.mp4".to_string(),
            ];

            // Create first config with original order
            let mut config1 = SliceConfig::default();
            config1.slice_patterns = patterns.clone();
            let config1 = Arc::new(config1);
            let analyzer1 = RequestAnalyzer::new(config1);

            // Create second config with shuffled order (deterministic shuffle based on seed)
            let mut hasher = DefaultHasher::new();
            shuffle_seed.hash(&mut hasher);
            let hash = hasher.finish();
            
            // Clone patterns for second config
            let mut patterns2 = patterns.clone();
            
            // Simple deterministic shuffle
            if hash % 2 == 0 {
                patterns2.reverse();
            }
            if hash % 3 == 0 && patterns2.len() > 2 {
                let last_idx = patterns2.len() - 1;
                patterns2.swap(0, last_idx);
            }

            let mut config2 = SliceConfig::default();
            config2.slice_patterns = patterns2;
            let config2 = Arc::new(config2);
            let analyzer2 = RequestAnalyzer::new(config2);

            let headers = HeaderMap::new();

            // Both analyzers should produce the same result
            let result1 = analyzer1.should_slice(&Method::GET, &uri, &headers);
            let result2 = analyzer2.should_slice(&Method::GET, &uri, &headers);

            prop_assert_eq!(
                result1,
                result2,
                "Pattern matching result should not depend on pattern order. URI: {}, Result1: {}, Result2: {}",
                uri,
                result1,
                result2
            );
        }

        /// Property 3 (concurrent evaluation): Pattern matching should be thread-safe
        /// 
        /// Multiple threads evaluating the same pattern matching should get consistent results.
        #[test]
        fn prop_concurrent_pattern_matching(
            uri in "[a-z0-9/_.-]{1,50}",
            thread_count in 2usize..8,
        ) {
            let patterns = vec![
                "/large-files/".to_string(),
                "*.bin".to_string(),
                "/downloads/*".to_string(),
            ];

            let mut config = SliceConfig::default();
            config.slice_patterns = patterns;
            let config = Arc::new(config);
            let analyzer = Arc::new(RequestAnalyzer::new(config));

            let headers = HeaderMap::new();

            // Spawn multiple threads to evaluate pattern matching concurrently
            let mut handles = vec![];
            for _ in 0..thread_count {
                let analyzer_clone = Arc::clone(&analyzer);
                let uri_clone = uri.clone();
                let headers_clone = headers.clone();
                
                let handle = std::thread::spawn(move || {
                    analyzer_clone.should_slice(&Method::GET, &uri_clone, &headers_clone)
                });
                handles.push(handle);
            }

            // Collect results from all threads
            let results: Vec<bool> = handles.into_iter()
                .map(|h| h.join().unwrap())
                .collect();

            // All results must be identical
            let first_result = results[0];
            for (idx, result) in results.iter().enumerate() {
                prop_assert_eq!(
                    *result,
                    first_result,
                    "Concurrent pattern matching must be deterministic. Thread {} returned {} but first thread returned {}. URI: {}",
                    idx,
                    result,
                    first_result,
                    uri
                );
            }
        }

        /// Property 3 (special characters): Deterministic matching with special URI characters
        /// 
        /// URIs with special characters should be matched deterministically.
        #[test]
        fn prop_special_characters_deterministic(
            base_path in "[a-z]{1,20}",
            special_char_idx in 0usize..5,
            evaluation_count in 2usize..5,
        ) {
            // Create URIs with various special characters
            let special_chars = vec!["-", "_", ".", "~", "%20"];
            let special_char = special_chars[special_char_idx % special_chars.len()];
            let uri = format!("/path{}{}/file.bin", special_char, base_path);

            let patterns = vec![
                "/path*/".to_string(),
                "*.bin".to_string(),
            ];

            let mut config = SliceConfig::default();
            config.slice_patterns = patterns;
            let config = Arc::new(config);
            let analyzer = RequestAnalyzer::new(config);

            let headers = HeaderMap::new();

            let mut results = Vec::new();
            for _ in 0..evaluation_count {
                let result = analyzer.should_slice(&Method::GET, &uri, &headers);
                results.push(result);
            }

            // All results must be identical
            let first_result = results[0];
            for (idx, result) in results.iter().enumerate() {
                prop_assert_eq!(
                    *result,
                    first_result,
                    "Pattern matching with special characters must be deterministic. URI: {}, Evaluation {} returned {} but first returned {}",
                    uri,
                    idx,
                    result,
                    first_result
                );
            }
        }
    }

    #[cfg(test)]
    mod unit_tests {
        use super::*;

        #[test]
        fn test_pattern_matching_is_deterministic() {
            let patterns = vec!["/large-files/".to_string(), "*.bin".to_string()];
            let mut config = SliceConfig::default();
            config.slice_patterns = patterns;
            let config = Arc::new(config);
            let analyzer = RequestAnalyzer::new(config);

            let headers = HeaderMap::new();
            let uri = "/large-files/test.bin";

            // Call multiple times
            let result1 = analyzer.should_slice(&Method::GET, uri, &headers);
            let result2 = analyzer.should_slice(&Method::GET, uri, &headers);
            let result3 = analyzer.should_slice(&Method::GET, uri, &headers);

            assert_eq!(result1, result2);
            assert_eq!(result2, result3);
            assert!(result1, "Should match pattern");
        }

        #[test]
        fn test_empty_patterns_always_matches() {
            let config = Arc::new(SliceConfig::default());
            let analyzer = RequestAnalyzer::new(config);

            let headers = HeaderMap::new();

            // Multiple different URIs should all match (slice everything)
            let uris = vec![
                "/test.bin",
                "/path/to/file.txt",
                "/downloads/archive.zip",
            ];

            for uri in uris {
                let result1 = analyzer.should_slice(&Method::GET, uri, &headers);
                let result2 = analyzer.should_slice(&Method::GET, uri, &headers);
                assert_eq!(result1, result2);
                assert!(result1, "Empty patterns should match all URIs");
            }
        }

        #[test]
        fn test_no_hidden_state() {
            let patterns = vec!["*.bin".to_string()];
            let mut config = SliceConfig::default();
            config.slice_patterns = patterns;
            let config = Arc::new(config);
            let analyzer = RequestAnalyzer::new(config);

            let headers = HeaderMap::new();

            // Evaluate different URIs in sequence
            let uri1 = "/file1.bin";
            let uri2 = "/file2.txt";
            let uri3 = "/file3.bin";

            let result1a = analyzer.should_slice(&Method::GET, uri1, &headers);
            let result2a = analyzer.should_slice(&Method::GET, uri2, &headers);
            let result3a = analyzer.should_slice(&Method::GET, uri3, &headers);

            // Re-evaluate in different order
            let result3b = analyzer.should_slice(&Method::GET, uri3, &headers);
            let result1b = analyzer.should_slice(&Method::GET, uri1, &headers);
            let result2b = analyzer.should_slice(&Method::GET, uri2, &headers);

            // Results should be the same regardless of evaluation order
            assert_eq!(result1a, result1b);
            assert_eq!(result2a, result2b);
            assert_eq!(result3a, result3b);

            assert!(result1a, "*.bin should match /file1.bin");
            assert!(!result2a, "*.bin should not match /file2.txt");
            assert!(result3a, "*.bin should match /file3.bin");
        }
    }
}
