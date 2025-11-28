// Feature: pingora-slice, Property 11: 响应头完整性
// **Validates: Requirements 6.5**
//
// Property: For any response sent to the client, the response headers should 
// include Content-Length (or Transfer-Encoding: chunked) and Content-Type

use http::StatusCode;
use pingora_slice::models::{ByteRange, FileMetadata};
use pingora_slice::response_assembler::ResponseAssembler;
use proptest::prelude::*;

/// Strategy for generating file sizes
fn file_size_strategy() -> impl Strategy<Value = u64> {
    1u64..=100_000_000u64
}

/// Strategy for generating content types
fn content_type_strategy() -> impl Strategy<Value = Option<String>> {
    prop::option::of(prop::sample::select(vec![
        "application/octet-stream".to_string(),
        "text/plain".to_string(),
        "text/html".to_string(),
        "application/json".to_string(),
        "image/jpeg".to_string(),
        "image/png".to_string(),
        "video/mp4".to_string(),
        "application/pdf".to_string(),
    ]))
}

/// Strategy for generating ETags
fn etag_strategy() -> impl Strategy<Value = Option<String>> {
    prop::option::of("[a-zA-Z0-9]{8,16}".prop_map(|s| format!("\"{}\"", s)))
}

/// Strategy for generating Last-Modified dates
fn last_modified_strategy() -> impl Strategy<Value = Option<String>> {
    prop::option::of(Just("Wed, 21 Oct 2015 07:28:00 GMT".to_string()))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property 11: Response header completeness
    /// 
    /// For any file metadata and optional client range, the response headers
    /// should always include Content-Length and Content-Type (if available).
    #[test]
    fn prop_response_header_completeness(
        file_size in file_size_strategy(),
        content_type in content_type_strategy(),
        etag in etag_strategy(),
        last_modified in last_modified_strategy(),
        has_client_range in any::<bool>(),
    ) {
        // Create metadata
        let metadata = FileMetadata::with_headers(
            file_size,
            true,
            content_type.clone(),
            etag,
            last_modified,
        );
        
        // Generate client range if requested
        let client_range = if has_client_range && file_size > 0 {
            let start = file_size / 4;
            let end = (file_size * 3) / 4;
            if start < end && end < file_size {
                ByteRange::new(start, end).ok()
            } else {
                None
            }
        } else {
            None
        };
        
        // Build response headers
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, client_range);
        
        prop_assert!(result.is_ok(), "Header building should succeed");
        
        let (status, headers) = result.unwrap();
        
        // Property 1: Content-Length must be present
        prop_assert!(
            headers.contains_key("content-length"),
            "Response headers must include Content-Length"
        );
        
        // Property 2: Content-Length must be a valid number
        let content_length_str = headers.get("content-length")
            .and_then(|v| v.to_str().ok())
            .expect("Content-Length should be valid string");
        
        let content_length: u64 = content_length_str.parse()
            .expect("Content-Length should be a valid number");
        
        // Property 3: Content-Length should match expected value
        if let Some(range) = client_range {
            let expected_length = range.size();
            prop_assert_eq!(
                content_length,
                expected_length,
                "Content-Length should match range size for partial content"
            );
        } else {
            prop_assert_eq!(
                content_length,
                file_size,
                "Content-Length should match file size for full content"
            );
        }
        
        // Property 4: Content-Type should be present if it was in metadata
        if content_type.is_some() {
            prop_assert!(
                headers.contains_key("content-type"),
                "Response headers must include Content-Type when available in metadata"
            );
            
            let response_content_type = headers.get("content-type")
                .and_then(|v| v.to_str().ok())
                .expect("Content-Type should be valid string");
            
            prop_assert_eq!(
                response_content_type,
                content_type.as_ref().unwrap(),
                "Content-Type should match metadata"
            );
        }
        
        // Property 5: Status code should be correct
        if client_range.is_some() {
            prop_assert_eq!(
                status,
                StatusCode::PARTIAL_CONTENT,
                "Status should be 206 for range requests"
            );
        } else {
            prop_assert_eq!(
                status,
                StatusCode::OK,
                "Status should be 200 for full file requests"
            );
        }
        
        // Property 6: Accept-Ranges should always be present
        prop_assert!(
            headers.contains_key("accept-ranges"),
            "Response headers must include Accept-Ranges"
        );
        
        prop_assert_eq!(
            headers.get("accept-ranges").and_then(|v| v.to_str().ok()),
            Some("bytes"),
            "Accept-Ranges should be 'bytes'"
        );
    }

    /// Property 11 (range requests): Content-Range header for partial content
    /// 
    /// For any range request, the response should include a properly formatted
    /// Content-Range header in addition to Content-Length and Content-Type.
    #[test]
    fn prop_response_header_completeness_with_range(
        file_size in 1000u64..=100_000u64,
        range_start_ratio in 0.0f64..=0.5f64,
        range_length_ratio in 0.1f64..=0.5f64,
        content_type in content_type_strategy(),
    ) {
        // Calculate range
        let range_start = (file_size as f64 * range_start_ratio) as u64;
        let range_length = ((file_size - range_start) as f64 * range_length_ratio).max(1.0) as u64;
        let range_end = std::cmp::min(range_start + range_length - 1, file_size - 1);
        
        if range_start > range_end {
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
        
        prop_assert!(result.is_ok(), "Header building should succeed for range request");
        
        let (status, headers) = result.unwrap();
        
        // Property 1: Status must be 206
        prop_assert_eq!(
            status,
            StatusCode::PARTIAL_CONTENT,
            "Status must be 206 for range requests"
        );
        
        // Property 2: Content-Range must be present
        prop_assert!(
            headers.contains_key("content-range"),
            "Range requests must include Content-Range header"
        );
        
        // Property 3: Content-Range format should be correct
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
            "Content-Range format should be 'bytes start-end/total'"
        );
        
        // Property 4: Content-Length must match range size
        let content_length_str = headers.get("content-length")
            .and_then(|v| v.to_str().ok())
            .expect("Content-Length should be present");
        
        let content_length: u64 = content_length_str.parse()
            .expect("Content-Length should be a valid number");
        
        let expected_length = range_end - range_start + 1;
        prop_assert_eq!(
            content_length,
            expected_length,
            "Content-Length should match range size"
        );
        
        // Property 5: Content-Type should be present if available
        if content_type.is_some() {
            prop_assert!(
                headers.contains_key("content-type"),
                "Content-Type should be present when available"
            );
        }
    }

    /// Property 11 (metadata preservation): All metadata headers preserved
    /// 
    /// For any file metadata with ETag and Last-Modified, these headers
    /// should be preserved in the response along with required headers.
    #[test]
    fn prop_response_header_metadata_preservation(
        file_size in file_size_strategy(),
        content_type in content_type_strategy(),
        etag in etag_strategy(),
        last_modified in last_modified_strategy(),
    ) {
        // Create metadata with all optional fields
        let metadata = FileMetadata::with_headers(
            file_size,
            true,
            content_type.clone(),
            etag.clone(),
            last_modified.clone(),
        );
        
        // Build response headers (no range)
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, None);
        
        prop_assert!(result.is_ok(), "Header building should succeed");
        
        let (_status, headers) = result.unwrap();
        
        // Property 1: ETag should be preserved if present
        if let Some(expected_etag) = &etag {
            prop_assert!(
                headers.contains_key("etag"),
                "ETag should be present when in metadata"
            );
            
            let response_etag = headers.get("etag")
                .and_then(|v| v.to_str().ok())
                .expect("ETag should be valid string");
            
            prop_assert_eq!(
                response_etag,
                expected_etag,
                "ETag should match metadata"
            );
        }
        
        // Property 2: Last-Modified should be preserved if present
        if let Some(expected_last_modified) = &last_modified {
            prop_assert!(
                headers.contains_key("last-modified"),
                "Last-Modified should be present when in metadata"
            );
            
            let response_last_modified = headers.get("last-modified")
                .and_then(|v| v.to_str().ok())
                .expect("Last-Modified should be valid string");
            
            prop_assert_eq!(
                response_last_modified,
                expected_last_modified,
                "Last-Modified should match metadata"
            );
        }
        
        // Property 3: Required headers still present
        prop_assert!(
            headers.contains_key("content-length"),
            "Content-Length must always be present"
        );
        
        prop_assert!(
            headers.contains_key("accept-ranges"),
            "Accept-Ranges must always be present"
        );
    }

    /// Property 11 (edge case): Zero-byte range
    /// 
    /// Even for a single-byte range, all required headers should be present.
    #[test]
    fn prop_response_header_single_byte_range(
        file_size in 100u64..=10_000u64,
        byte_position in 0u64..100u64,
    ) {
        if byte_position >= file_size {
            return Ok(());
        }
        
        // Single byte range
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
        
        // Property: All required headers present even for single byte
        prop_assert_eq!(status, StatusCode::PARTIAL_CONTENT);
        prop_assert!(headers.contains_key("content-length"));
        prop_assert!(headers.contains_key("content-range"));
        prop_assert!(headers.contains_key("content-type"));
        prop_assert!(headers.contains_key("accept-ranges"));
        
        // Content-Length should be 1
        let content_length: u64 = headers.get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse().ok())
            .expect("Content-Length should be valid");
        
        prop_assert_eq!(content_length, 1, "Single byte range should have Content-Length of 1");
    }

    /// Property 11 (consistency): Headers consistent across multiple calls
    /// 
    /// For the same metadata and range, building headers multiple times
    /// should produce identical results.
    #[test]
    fn prop_response_header_consistency(
        file_size in 1000u64..=100_000u64,
        has_range in any::<bool>(),
    ) {
        let metadata = FileMetadata::with_headers(
            file_size,
            true,
            Some("text/plain".to_string()),
            Some("\"test123\"".to_string()),
            Some("Wed, 21 Oct 2015 07:28:00 GMT".to_string()),
        );
        
        let client_range = if has_range {
            Some(ByteRange::new(0, file_size / 2).unwrap())
        } else {
            None
        };
        
        let assembler = ResponseAssembler::new();
        
        // Build headers twice
        let result1 = assembler.build_response_header(&metadata, client_range);
        let result2 = assembler.build_response_header(&metadata, client_range);
        
        prop_assert!(result1.is_ok() && result2.is_ok());
        
        let (status1, headers1) = result1.unwrap();
        let (status2, headers2) = result2.unwrap();
        
        // Property: Results should be identical
        prop_assert_eq!(status1, status2, "Status codes should be consistent");
        
        // Check all header keys and values match
        prop_assert_eq!(
            headers1.len(),
            headers2.len(),
            "Header count should be consistent"
        );
        
        for (key, value1) in headers1.iter() {
            let value2 = headers2.get(key)
                .expect("Same headers should be present in both calls");
            
            prop_assert_eq!(
                value1,
                value2,
                "Header value for {:?} should be consistent",
                key
            );
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_full_file_response_has_required_headers() {
        let metadata = FileMetadata::with_headers(
            10240,
            true,
            Some("application/octet-stream".to_string()),
            Some("\"abc123\"".to_string()),
            Some("Wed, 21 Oct 2015 07:28:00 GMT".to_string()),
        );

        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, None);
        assert!(result.is_ok());

        let (status, headers) = result.unwrap();
        
        // Check status
        assert_eq!(status, StatusCode::OK);
        
        // Check required headers
        assert!(headers.contains_key("content-length"));
        assert!(headers.contains_key("content-type"));
        assert!(headers.contains_key("accept-ranges"));
        
        // Check values
        assert_eq!(headers.get("content-length").unwrap(), "10240");
        assert_eq!(headers.get("content-type").unwrap(), "application/octet-stream");
        assert_eq!(headers.get("accept-ranges").unwrap(), "bytes");
    }

    #[test]
    fn test_range_response_has_required_headers() {
        let metadata = FileMetadata::with_headers(
            10240,
            true,
            Some("text/plain".to_string()),
            None,
            None,
        );

        let range = ByteRange::new(0, 1023).unwrap();
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(range));
        assert!(result.is_ok());

        let (status, headers) = result.unwrap();
        
        // Check status
        assert_eq!(status, StatusCode::PARTIAL_CONTENT);
        
        // Check required headers for range request
        assert!(headers.contains_key("content-length"));
        assert!(headers.contains_key("content-range"));
        assert!(headers.contains_key("content-type"));
        assert!(headers.contains_key("accept-ranges"));
        
        // Check values
        assert_eq!(headers.get("content-length").unwrap(), "1024");
        assert_eq!(headers.get("content-range").unwrap(), "bytes 0-1023/10240");
        assert_eq!(headers.get("content-type").unwrap(), "text/plain");
    }

    #[test]
    fn test_response_without_content_type() {
        let metadata = FileMetadata::with_headers(
            5000,
            true,
            None, // No content type
            None,
            None,
        );

        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, None);
        assert!(result.is_ok());

        let (_status, headers) = result.unwrap();
        
        // Content-Length and Accept-Ranges should still be present
        assert!(headers.contains_key("content-length"));
        assert!(headers.contains_key("accept-ranges"));
        
        // Content-Type should not be present
        assert!(!headers.contains_key("content-type"));
    }

    #[test]
    fn test_metadata_headers_preserved() {
        let metadata = FileMetadata::with_headers(
            8192,
            true,
            Some("image/jpeg".to_string()),
            Some("\"xyz789\"".to_string()),
            Some("Thu, 22 Oct 2015 08:30:00 GMT".to_string()),
        );

        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, None);
        assert!(result.is_ok());

        let (_status, headers) = result.unwrap();
        
        // Check all metadata headers are preserved
        assert_eq!(headers.get("etag").unwrap(), "\"xyz789\"");
        assert_eq!(headers.get("last-modified").unwrap(), "Thu, 22 Oct 2015 08:30:00 GMT");
        assert_eq!(headers.get("content-type").unwrap(), "image/jpeg");
    }

    #[test]
    fn test_single_byte_range_headers() {
        let metadata = FileMetadata::with_headers(
            1000,
            true,
            Some("application/octet-stream".to_string()),
            None,
            None,
        );

        // Request a single byte
        let range = ByteRange::new(500, 500).unwrap();
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(range));
        assert!(result.is_ok());

        let (status, headers) = result.unwrap();
        
        assert_eq!(status, StatusCode::PARTIAL_CONTENT);
        assert_eq!(headers.get("content-length").unwrap(), "1");
        assert_eq!(headers.get("content-range").unwrap(), "bytes 500-500/1000");
        assert!(headers.contains_key("content-type"));
        assert!(headers.contains_key("accept-ranges"));
    }

    #[test]
    fn test_last_byte_range_headers() {
        let metadata = FileMetadata::with_headers(
            1000,
            true,
            Some("text/html".to_string()),
            None,
            None,
        );

        // Request the last byte
        let range = ByteRange::new(999, 999).unwrap();
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(range));
        assert!(result.is_ok());

        let (status, headers) = result.unwrap();
        
        assert_eq!(status, StatusCode::PARTIAL_CONTENT);
        assert_eq!(headers.get("content-length").unwrap(), "1");
        assert_eq!(headers.get("content-range").unwrap(), "bytes 999-999/1000");
    }

    #[test]
    fn test_full_range_headers() {
        let metadata = FileMetadata::with_headers(
            5000,
            true,
            Some("video/mp4".to_string()),
            None,
            None,
        );

        // Request the entire file as a range
        let range = ByteRange::new(0, 4999).unwrap();
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(range));
        assert!(result.is_ok());

        let (status, headers) = result.unwrap();
        
        assert_eq!(status, StatusCode::PARTIAL_CONTENT);
        assert_eq!(headers.get("content-length").unwrap(), "5000");
        assert_eq!(headers.get("content-range").unwrap(), "bytes 0-4999/5000");
    }
}
