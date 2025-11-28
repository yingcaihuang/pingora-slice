// Feature: pingora-slice, Property 19: 206 响应格式
// **Validates: Requirements 10.4**
//
// Property: For any successful Range request, the response should have status code 206 
// and include a Content-Range header matching the requested range

use http::StatusCode;
use pingora_slice::models::{ByteRange, FileMetadata};
use pingora_slice::response_assembler::ResponseAssembler;
use proptest::prelude::*;

/// Strategy for generating file sizes
fn file_size_strategy() -> impl Strategy<Value = u64> {
    1000u64..=100_000_000u64
}

/// Strategy for generating content types
fn content_type_strategy() -> impl Strategy<Value = Option<String>> {
    prop::option::of(prop::sample::select(vec![
        "application/octet-stream".to_string(),
        "text/plain".to_string(),
        "text/html".to_string(),
        "application/json".to_string(),
        "image/jpeg".to_string(),
        "video/mp4".to_string(),
    ]))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property 19: 206 response format correctness
    /// 
    /// For any Range request, the response should:
    /// 1. Have status code 206 (Partial Content)
    /// 2. Include a Content-Range header in the format "bytes start-end/total"
    /// 3. Have Content-Length matching the range size
    /// 4. Include all other required headers (Accept-Ranges, Content-Type if available)
    #[test]
    fn prop_206_response_format(
        file_size in file_size_strategy(),
        content_type in content_type_strategy(),
        range_start_ratio in 0.0f64..=0.7f64,
        range_length_ratio in 0.1f64..=0.5f64,
    ) {
        // Calculate a valid range within the file
        let range_start = (file_size as f64 * range_start_ratio) as u64;
        let range_length = ((file_size - range_start) as f64 * range_length_ratio).max(1.0) as u64;
        let range_end = std::cmp::min(range_start + range_length - 1, file_size - 1);
        
        // Skip if range is invalid
        if range_start > range_end || range_end >= file_size {
            return Ok(());
        }
        
        let client_range = ByteRange::new(range_start, range_end)
            .expect("Range should be valid");
        
        // Create metadata
        let metadata = FileMetadata::with_headers(
            file_size,
            true,
            content_type.clone(),
            None,
            None,
        );
        
        // Build response headers
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(client_range));
        
        prop_assert!(
            result.is_ok(),
            "Building response headers for range request should succeed"
        );
        
        let (status, headers) = result.unwrap();
        
        // Property 1: Status code MUST be 206 Partial Content
        prop_assert_eq!(
            status,
            StatusCode::PARTIAL_CONTENT,
            "Range request response must have status code 206 Partial Content"
        );
        
        // Property 2: Content-Range header MUST be present
        prop_assert!(
            headers.contains_key("content-range"),
            "206 response must include Content-Range header"
        );
        
        // Property 3: Content-Range format MUST be "bytes start-end/total"
        let content_range = headers.get("content-range")
            .and_then(|v| v.to_str().ok())
            .expect("Content-Range should be valid string");
        
        let expected_content_range = format!(
            "bytes {}-{}/{}",
            range_start,
            range_end,
            file_size
        );
        
        prop_assert_eq!(
            content_range,
            expected_content_range,
            "Content-Range must be in format 'bytes start-end/total'"
        );
        
        // Property 4: Content-Length MUST match the range size
        prop_assert!(
            headers.contains_key("content-length"),
            "206 response must include Content-Length header"
        );
        
        let content_length_str = headers.get("content-length")
            .and_then(|v| v.to_str().ok())
            .expect("Content-Length should be valid string");
        
        let content_length: u64 = content_length_str.parse()
            .expect("Content-Length should be a valid number");
        
        let expected_length = range_end - range_start + 1;
        prop_assert_eq!(
            content_length,
            expected_length,
            "Content-Length must match the size of the requested range"
        );
        
        // Property 5: Accept-Ranges header MUST be present
        prop_assert!(
            headers.contains_key("accept-ranges"),
            "206 response must include Accept-Ranges header"
        );
        
        prop_assert_eq!(
            headers.get("accept-ranges").and_then(|v| v.to_str().ok()),
            Some("bytes"),
            "Accept-Ranges must be 'bytes'"
        );
        
        // Property 6: Content-Type MUST be present if available in metadata
        if content_type.is_some() {
            prop_assert!(
                headers.contains_key("content-type"),
                "206 response must include Content-Type when available in metadata"
            );
            
            let response_content_type = headers.get("content-type")
                .and_then(|v| v.to_str().ok())
                .expect("Content-Type should be valid string");
            
            prop_assert_eq!(
                response_content_type,
                content_type.as_ref().unwrap(),
                "Content-Type must match metadata"
            );
        }
    }

    /// Property 19 (edge case): Single byte range
    /// 
    /// Even for a single-byte range request, the 206 response format must be correct.
    #[test]
    fn prop_206_response_format_single_byte(
        file_size in 100u64..=100_000u64,
        byte_position in 0u64..=99_999u64,
    ) {
        let byte_position = byte_position % file_size;
        
        let client_range = ByteRange::new(byte_position, byte_position)
            .expect("Single byte range should be valid");
        
        let metadata = FileMetadata::with_headers(
            file_size,
            true,
            Some("application/octet-stream".to_string()),
            None,
            None,
        );
        
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(client_range));
        
        prop_assert!(result.is_ok(), "Single byte range should succeed");
        
        let (status, headers) = result.unwrap();
        
        // Status must be 206
        prop_assert_eq!(
            status,
            StatusCode::PARTIAL_CONTENT,
            "Single byte range must return 206"
        );
        
        // Content-Range must be correct
        let content_range = headers.get("content-range")
            .and_then(|v| v.to_str().ok())
            .expect("Content-Range should be present");
        
        let expected = format!("bytes {}-{}/{}", byte_position, byte_position, file_size);
        prop_assert_eq!(
            content_range,
            expected,
            "Content-Range must be correct for single byte"
        );
        
        // Content-Length must be 1
        let content_length: u64 = headers.get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse().ok())
            .expect("Content-Length should be valid");
        
        prop_assert_eq!(
            content_length,
            1,
            "Content-Length must be 1 for single byte range"
        );
    }

    /// Property 19 (edge case): Range at file boundaries
    /// 
    /// Range requests at the start or end of the file must have correct 206 format.
    #[test]
    fn prop_206_response_format_boundaries(
        file_size in 1000u64..=100_000u64,
        at_start in proptest::bool::ANY,
        range_length in 1u64..=1000u64,
    ) {
        let (range_start, range_end) = if at_start {
            // Range at the start of the file
            let end = std::cmp::min(range_length - 1, file_size - 1);
            (0, end)
        } else {
            // Range at the end of the file
            let start = file_size.saturating_sub(range_length);
            (start, file_size - 1)
        };
        
        let client_range = ByteRange::new(range_start, range_end)
            .expect("Boundary range should be valid");
        
        let metadata = FileMetadata::with_headers(
            file_size,
            true,
            Some("text/plain".to_string()),
            None,
            None,
        );
        
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(client_range));
        
        prop_assert!(result.is_ok(), "Boundary range should succeed");
        
        let (status, headers) = result.unwrap();
        
        // Must be 206
        prop_assert_eq!(status, StatusCode::PARTIAL_CONTENT);
        
        // Content-Range must be correct
        let content_range = headers.get("content-range")
            .and_then(|v| v.to_str().ok())
            .expect("Content-Range should be present");
        
        let expected = format!("bytes {}-{}/{}", range_start, range_end, file_size);
        prop_assert_eq!(content_range, expected);
        
        // Content-Length must match range size
        let content_length: u64 = headers.get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse().ok())
            .expect("Content-Length should be valid");
        
        let expected_length = range_end - range_start + 1;
        prop_assert_eq!(content_length, expected_length);
    }

    /// Property 19 (comparison): Full file request should NOT be 206
    /// 
    /// When no range is requested, the response should be 200, not 206.
    /// This verifies that 206 is only used for actual range requests.
    #[test]
    fn prop_206_only_for_range_requests(
        file_size in file_size_strategy(),
        has_range in proptest::bool::ANY,
    ) {
        let metadata = FileMetadata::with_headers(
            file_size,
            true,
            Some("application/octet-stream".to_string()),
            None,
            None,
        );
        
        let client_range = if has_range {
            Some(ByteRange::new(0, file_size / 2).unwrap())
        } else {
            None
        };
        
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, client_range);
        
        prop_assert!(result.is_ok());
        
        let (status, headers) = result.unwrap();
        
        if has_range {
            // With range: must be 206 with Content-Range
            prop_assert_eq!(
                status,
                StatusCode::PARTIAL_CONTENT,
                "Range request must return 206"
            );
            prop_assert!(
                headers.contains_key("content-range"),
                "Range request must include Content-Range"
            );
        } else {
            // Without range: must be 200 without Content-Range
            prop_assert_eq!(
                status,
                StatusCode::OK,
                "Full file request must return 200, not 206"
            );
            prop_assert!(
                !headers.contains_key("content-range"),
                "Full file request must NOT include Content-Range"
            );
        }
    }

    /// Property 19 (consistency): Multiple calls with same range produce identical 206 response
    /// 
    /// For the same metadata and range, building headers multiple times
    /// should produce identical 206 responses.
    #[test]
    fn prop_206_response_consistency(
        file_size in 1000u64..=100_000u64,
        range_start in 0u64..=50_000u64,
        range_length in 100u64..=10_000u64,
    ) {
        let range_start = range_start % (file_size / 2);
        let range_end = std::cmp::min(range_start + range_length - 1, file_size - 1);
        
        if range_start > range_end {
            return Ok(());
        }
        
        let client_range = ByteRange::new(range_start, range_end).unwrap();
        
        let metadata = FileMetadata::with_headers(
            file_size,
            true,
            Some("video/mp4".to_string()),
            Some("\"etag123\"".to_string()),
            Some("Wed, 21 Oct 2015 07:28:00 GMT".to_string()),
        );
        
        let assembler = ResponseAssembler::new();
        
        // Build headers twice
        let result1 = assembler.build_response_header(&metadata, Some(client_range));
        let result2 = assembler.build_response_header(&metadata, Some(client_range));
        
        prop_assert!(result1.is_ok() && result2.is_ok());
        
        let (status1, headers1) = result1.unwrap();
        let (status2, headers2) = result2.unwrap();
        
        // Status codes must be identical
        prop_assert_eq!(status1, status2);
        prop_assert_eq!(status1, StatusCode::PARTIAL_CONTENT);
        
        // Content-Range must be identical
        prop_assert_eq!(
            headers1.get("content-range"),
            headers2.get("content-range"),
            "Content-Range must be consistent"
        );
        
        // Content-Length must be identical
        prop_assert_eq!(
            headers1.get("content-length"),
            headers2.get("content-length"),
            "Content-Length must be consistent"
        );
        
        // All headers must be identical
        prop_assert_eq!(
            headers1.len(),
            headers2.len(),
            "Header count must be consistent"
        );
    }

    /// Property 19 (metadata preservation): 206 response preserves metadata headers
    /// 
    /// For any range request, metadata headers (ETag, Last-Modified) should be
    /// preserved in the 206 response along with required range headers.
    #[test]
    fn prop_206_response_metadata_preservation(
        file_size in file_size_strategy(),
        range_start_ratio in 0.0f64..=0.5f64,
        range_length_ratio in 0.1f64..=0.5f64,
        has_etag in proptest::bool::ANY,
        has_last_modified in proptest::bool::ANY,
    ) {
        let range_start = (file_size as f64 * range_start_ratio) as u64;
        let range_length = ((file_size - range_start) as f64 * range_length_ratio).max(1.0) as u64;
        let range_end = std::cmp::min(range_start + range_length - 1, file_size - 1);
        
        if range_start > range_end {
            return Ok(());
        }
        
        let client_range = ByteRange::new(range_start, range_end).unwrap();
        
        let etag = if has_etag {
            Some("\"abc123xyz\"".to_string())
        } else {
            None
        };
        
        let last_modified = if has_last_modified {
            Some("Thu, 22 Oct 2015 08:30:00 GMT".to_string())
        } else {
            None
        };
        
        let metadata = FileMetadata::with_headers(
            file_size,
            true,
            Some("image/jpeg".to_string()),
            etag.clone(),
            last_modified.clone(),
        );
        
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(client_range));
        
        prop_assert!(result.is_ok());
        
        let (status, headers) = result.unwrap();
        
        // Must be 206
        prop_assert_eq!(status, StatusCode::PARTIAL_CONTENT);
        
        // Required range headers must be present
        prop_assert!(headers.contains_key("content-range"));
        prop_assert!(headers.contains_key("content-length"));
        
        // Metadata headers must be preserved if present
        if has_etag {
            prop_assert!(
                headers.contains_key("etag"),
                "ETag must be preserved in 206 response"
            );
            prop_assert_eq!(
                headers.get("etag").and_then(|v| v.to_str().ok()),
                etag.as_deref(),
                "ETag value must match metadata"
            );
        }
        
        if has_last_modified {
            prop_assert!(
                headers.contains_key("last-modified"),
                "Last-Modified must be preserved in 206 response"
            );
            prop_assert_eq!(
                headers.get("last-modified").and_then(|v| v.to_str().ok()),
                last_modified.as_deref(),
                "Last-Modified value must match metadata"
            );
        }
    }

    /// Property 19 (large values): 206 response handles large byte values correctly
    /// 
    /// For very large files and byte positions, the 206 response format must
    /// still be correct with proper numeric formatting.
    #[test]
    fn prop_206_response_large_values(
        file_size in 1_000_000_000u64..=10_000_000_000u64,
        range_start_ratio in 0.0f64..=0.7f64,
        range_length in 1_000_000u64..=100_000_000u64,
    ) {
        let range_start = (file_size as f64 * range_start_ratio) as u64;
        let range_end = std::cmp::min(range_start + range_length - 1, file_size - 1);
        
        if range_start > range_end {
            return Ok(());
        }
        
        let client_range = ByteRange::new(range_start, range_end).unwrap();
        
        let metadata = FileMetadata::with_headers(
            file_size,
            true,
            Some("application/octet-stream".to_string()),
            None,
            None,
        );
        
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(client_range));
        
        prop_assert!(result.is_ok(), "Large values should be handled correctly");
        
        let (status, headers) = result.unwrap();
        
        prop_assert_eq!(status, StatusCode::PARTIAL_CONTENT);
        
        // Verify Content-Range format with large numbers
        let content_range = headers.get("content-range")
            .and_then(|v| v.to_str().ok())
            .expect("Content-Range should be present");
        
        let expected = format!("bytes {}-{}/{}", range_start, range_end, file_size);
        prop_assert_eq!(
            content_range,
            expected,
            "Content-Range must be correct for large values"
        );
        
        // Verify Content-Length is correct
        let content_length: u64 = headers.get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse().ok())
            .expect("Content-Length should be valid");
        
        let expected_length = range_end - range_start + 1;
        prop_assert_eq!(
            content_length,
            expected_length,
            "Content-Length must be correct for large values"
        );
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_206_response_basic() {
        let metadata = FileMetadata::with_headers(
            10000,
            true,
            Some("text/plain".to_string()),
            None,
            None,
        );

        let range = ByteRange::new(1000, 2999).unwrap();
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(range));

        assert!(result.is_ok());
        let (status, headers) = result.unwrap();

        assert_eq!(status, StatusCode::PARTIAL_CONTENT);
        assert_eq!(headers.get("content-range").unwrap(), "bytes 1000-2999/10000");
        assert_eq!(headers.get("content-length").unwrap(), "2000");
        assert_eq!(headers.get("accept-ranges").unwrap(), "bytes");
        assert_eq!(headers.get("content-type").unwrap(), "text/plain");
    }

    #[test]
    fn test_206_response_single_byte() {
        let metadata = FileMetadata::with_headers(
            5000,
            true,
            Some("application/octet-stream".to_string()),
            None,
            None,
        );

        let range = ByteRange::new(2500, 2500).unwrap();
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(range));

        assert!(result.is_ok());
        let (status, headers) = result.unwrap();

        assert_eq!(status, StatusCode::PARTIAL_CONTENT);
        assert_eq!(headers.get("content-range").unwrap(), "bytes 2500-2500/5000");
        assert_eq!(headers.get("content-length").unwrap(), "1");
    }

    #[test]
    fn test_206_response_first_byte() {
        let metadata = FileMetadata::with_headers(
            10000,
            true,
            Some("text/html".to_string()),
            None,
            None,
        );

        let range = ByteRange::new(0, 0).unwrap();
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(range));

        assert!(result.is_ok());
        let (status, headers) = result.unwrap();

        assert_eq!(status, StatusCode::PARTIAL_CONTENT);
        assert_eq!(headers.get("content-range").unwrap(), "bytes 0-0/10000");
        assert_eq!(headers.get("content-length").unwrap(), "1");
    }

    #[test]
    fn test_206_response_last_byte() {
        let metadata = FileMetadata::with_headers(
            10000,
            true,
            Some("application/json".to_string()),
            None,
            None,
        );

        let range = ByteRange::new(9999, 9999).unwrap();
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(range));

        assert!(result.is_ok());
        let (status, headers) = result.unwrap();

        assert_eq!(status, StatusCode::PARTIAL_CONTENT);
        assert_eq!(headers.get("content-range").unwrap(), "bytes 9999-9999/10000");
        assert_eq!(headers.get("content-length").unwrap(), "1");
    }

    #[test]
    fn test_206_response_with_metadata() {
        let metadata = FileMetadata::with_headers(
            20000,
            true,
            Some("video/mp4".to_string()),
            Some("\"etag-abc123\"".to_string()),
            Some("Fri, 23 Oct 2015 09:00:00 GMT".to_string()),
        );

        let range = ByteRange::new(5000, 14999).unwrap();
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(range));

        assert!(result.is_ok());
        let (status, headers) = result.unwrap();

        assert_eq!(status, StatusCode::PARTIAL_CONTENT);
        assert_eq!(headers.get("content-range").unwrap(), "bytes 5000-14999/20000");
        assert_eq!(headers.get("content-length").unwrap(), "10000");
        assert_eq!(headers.get("etag").unwrap(), "\"etag-abc123\"");
        assert_eq!(headers.get("last-modified").unwrap(), "Fri, 23 Oct 2015 09:00:00 GMT");
    }

    #[test]
    fn test_206_response_large_file() {
        let metadata = FileMetadata::with_headers(
            5_000_000_000, // 5GB
            true,
            Some("application/octet-stream".to_string()),
            None,
            None,
        );

        let range = ByteRange::new(1_000_000_000, 1_999_999_999).unwrap();
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(range));

        assert!(result.is_ok());
        let (status, headers) = result.unwrap();

        assert_eq!(status, StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            headers.get("content-range").unwrap(),
            "bytes 1000000000-1999999999/5000000000"
        );
        assert_eq!(headers.get("content-length").unwrap(), "1000000000");
    }

    #[test]
    fn test_200_response_not_206() {
        let metadata = FileMetadata::with_headers(
            10000,
            true,
            Some("text/plain".to_string()),
            None,
            None,
        );

        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, None);

        assert!(result.is_ok());
        let (status, headers) = result.unwrap();

        // Full file request should be 200, not 206
        assert_eq!(status, StatusCode::OK);
        assert!(!headers.contains_key("content-range"));
        assert_eq!(headers.get("content-length").unwrap(), "10000");
    }

    #[test]
    fn test_206_response_entire_file_as_range() {
        let metadata = FileMetadata::with_headers(
            10000,
            true,
            Some("text/plain".to_string()),
            None,
            None,
        );

        // Request entire file as a range
        let range = ByteRange::new(0, 9999).unwrap();
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(range));

        assert!(result.is_ok());
        let (status, headers) = result.unwrap();

        // Even when requesting entire file as range, should be 206
        assert_eq!(status, StatusCode::PARTIAL_CONTENT);
        assert_eq!(headers.get("content-range").unwrap(), "bytes 0-9999/10000");
        assert_eq!(headers.get("content-length").unwrap(), "10000");
    }
}
