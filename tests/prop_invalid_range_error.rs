// Feature: pingora-slice, Property 20: 无效 Range 错误处理
// **Validates: Requirements 10.5**
//
// Property: For any Range request where the requested range is invalid or unsatisfiable
// (e.g., start > file_size), the response should be 416 Range Not Satisfiable

use pingora_slice::error::SliceError;
use pingora_slice::models::{ByteRange, FileMetadata};
use pingora_slice::response_assembler::ResponseAssembler;
use proptest::prelude::*;

/// Strategy for generating file sizes
fn file_size_strategy() -> impl Strategy<Value = u64> {
    1000u64..=100_000_000u64
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property 20: Invalid Range error handling - Range end exceeds file size
    /// 
    /// For any Range request where the end position is >= file size,
    /// the system should return an error that maps to 416 Range Not Satisfiable.
    #[test]
    fn prop_invalid_range_end_exceeds_file_size(
        file_size in file_size_strategy(),
        excess_amount in 1u64..=1_000_000u64,
    ) {
        // Create a range that exceeds the file size
        let range_start = 0;
        let range_end = file_size + excess_amount - 1;
        
        let invalid_range = ByteRange::new(range_start, range_end)
            .expect("Range construction should succeed even if logically invalid");
        
        let metadata = FileMetadata::with_headers(
            file_size,
            true,
            Some("application/octet-stream".to_string()),
            None,
            None,
        );
        
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(invalid_range));
        
        // Property: The result MUST be an error
        prop_assert!(
            result.is_err(),
            "Range with end >= file_size should return an error"
        );
        
        let error = result.unwrap_err();
        
        // Property: The error MUST map to HTTP 416
        prop_assert_eq!(
            error.to_http_status(),
            416,
            "Invalid range error must map to HTTP 416 Range Not Satisfiable"
        );
        
        // Property: The error should be an InvalidRange error
        prop_assert!(
            matches!(error, SliceError::InvalidRange(_)),
            "Error should be InvalidRange variant, got: {:?}",
            error
        );
    }

    /// Property 20: Invalid Range error handling - Range start at file size
    /// 
    /// For any Range request where the start position equals the file size,
    /// the range is unsatisfiable and should return 416.
    #[test]
    fn prop_invalid_range_start_at_file_size(
        file_size in file_size_strategy(),
    ) {
        // Create a range starting at the file size (invalid)
        let range_start = file_size;
        let range_end = file_size + 100;
        
        let invalid_range = ByteRange::new(range_start, range_end)
            .expect("Range construction should succeed");
        
        let metadata = FileMetadata::with_headers(
            file_size,
            true,
            Some("text/plain".to_string()),
            None,
            None,
        );
        
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(invalid_range));
        
        // Property: Must return an error
        prop_assert!(
            result.is_err(),
            "Range starting at file_size should be unsatisfiable"
        );
        
        let error = result.unwrap_err();
        
        // Property: Must map to 416
        prop_assert_eq!(
            error.to_http_status(),
            416,
            "Unsatisfiable range must return 416"
        );
    }

    /// Property 20: Invalid Range error handling - Range start beyond file size
    /// 
    /// For any Range request where the start position is beyond the file size,
    /// the range is unsatisfiable and should return 416.
    #[test]
    fn prop_invalid_range_start_beyond_file_size(
        file_size in file_size_strategy(),
        offset in 1u64..=1_000_000u64,
    ) {
        // Create a range starting beyond the file size
        let range_start = file_size + offset;
        let range_end = range_start + 1000;
        
        let invalid_range = ByteRange::new(range_start, range_end)
            .expect("Range construction should succeed");
        
        let metadata = FileMetadata::with_headers(
            file_size,
            true,
            Some("video/mp4".to_string()),
            None,
            None,
        );
        
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(invalid_range));
        
        // Property: Must return an error
        prop_assert!(
            result.is_err(),
            "Range starting beyond file_size should be unsatisfiable"
        );
        
        let error = result.unwrap_err();
        
        // Property: Must map to 416
        prop_assert_eq!(
            error.to_http_status(),
            416,
            "Range beyond file bounds must return 416"
        );
    }

    /// Property 20: Invalid Range error handling - Range end exactly at file size
    /// 
    /// For any Range request where the end position equals the file size,
    /// the range is invalid (since end is inclusive and valid positions are 0..file_size-1).
    #[test]
    fn prop_invalid_range_end_at_file_size(
        file_size in file_size_strategy(),
        range_start_ratio in 0.0f64..=0.9f64,
    ) {
        let range_start = (file_size as f64 * range_start_ratio) as u64;
        let range_end = file_size; // Exactly at file size (invalid)
        
        // Skip if range_start >= file_size
        if range_start >= file_size {
            return Ok(());
        }
        
        let invalid_range = ByteRange::new(range_start, range_end)
            .expect("Range construction should succeed");
        
        let metadata = FileMetadata::with_headers(
            file_size,
            true,
            Some("image/jpeg".to_string()),
            None,
            None,
        );
        
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(invalid_range));
        
        // Property: Must return an error
        prop_assert!(
            result.is_err(),
            "Range with end == file_size should be invalid (valid range is 0..file_size-1)"
        );
        
        let error = result.unwrap_err();
        
        // Property: Must map to 416
        prop_assert_eq!(
            error.to_http_status(),
            416,
            "Invalid range must return 416"
        );
    }

    /// Property 20: Valid Range should NOT return 416
    /// 
    /// For any valid Range request (within file bounds), the system should NOT
    /// return a 416 error. This is a negative test to ensure we don't over-reject.
    #[test]
    fn prop_valid_range_does_not_return_416(
        file_size in file_size_strategy(),
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
        
        let valid_range = ByteRange::new(range_start, range_end)
            .expect("Range should be valid");
        
        let metadata = FileMetadata::with_headers(
            file_size,
            true,
            Some("application/json".to_string()),
            None,
            None,
        );
        
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(valid_range));
        
        // Property: Valid range MUST succeed
        prop_assert!(
            result.is_ok(),
            "Valid range (start={}, end={}, file_size={}) should not return an error",
            range_start,
            range_end,
            file_size
        );
        
        let (status, _headers) = result.unwrap();
        
        // Property: Status MUST NOT be 416
        prop_assert_ne!(
            status.as_u16(),
            416,
            "Valid range must not return 416"
        );
        
        // Property: Status MUST be 206 for range requests
        prop_assert_eq!(
            status.as_u16(),
            206,
            "Valid range request must return 206 Partial Content"
        );
    }

    /// Property 20: Error consistency - Same invalid range produces same error
    /// 
    /// For any invalid range, calling the validation multiple times should
    /// produce consistent error results.
    #[test]
    fn prop_invalid_range_error_consistency(
        file_size in file_size_strategy(),
        excess in 1u64..=10_000u64,
    ) {
        let invalid_range = ByteRange::new(0, file_size + excess - 1)
            .expect("Range construction should succeed");
        
        let metadata = FileMetadata::with_headers(
            file_size,
            true,
            Some("application/octet-stream".to_string()),
            None,
            None,
        );
        
        let assembler = ResponseAssembler::new();
        
        // Call twice with the same invalid range
        let result1 = assembler.build_response_header(&metadata, Some(invalid_range));
        let result2 = assembler.build_response_header(&metadata, Some(invalid_range));
        
        // Both must be errors
        prop_assert!(result1.is_err() && result2.is_err());
        
        let error1 = result1.unwrap_err();
        let error2 = result2.unwrap_err();
        
        // Both must map to 416
        prop_assert_eq!(error1.to_http_status(), 416);
        prop_assert_eq!(error2.to_http_status(), 416);
        
        // Both must be the same error type
        prop_assert!(
            matches!(error1, SliceError::InvalidRange(_)) && 
            matches!(error2, SliceError::InvalidRange(_)),
            "Both errors should be InvalidRange"
        );
    }

    /// Property 20: Boundary case - Last valid byte
    /// 
    /// For any file, requesting the last valid byte (file_size - 1) should succeed,
    /// but requesting file_size should fail with 416.
    #[test]
    fn prop_invalid_range_boundary_at_last_byte(
        file_size in file_size_strategy(),
    ) {
        let metadata = FileMetadata::with_headers(
            file_size,
            true,
            Some("text/html".to_string()),
            None,
            None,
        );
        
        let assembler = ResponseAssembler::new();
        
        // Test 1: Last valid byte (file_size - 1) should succeed
        let valid_range = ByteRange::new(file_size - 1, file_size - 1)
            .expect("Last byte range should be valid");
        
        let result_valid = assembler.build_response_header(&metadata, Some(valid_range));
        
        prop_assert!(
            result_valid.is_ok(),
            "Requesting last valid byte (file_size - 1) should succeed"
        );
        
        let (status_valid, _) = result_valid.unwrap();
        prop_assert_eq!(status_valid.as_u16(), 206);
        
        // Test 2: Byte at file_size should fail with 416
        let invalid_range = ByteRange::new(file_size, file_size)
            .expect("Range construction should succeed");
        
        let result_invalid = assembler.build_response_header(&metadata, Some(invalid_range));
        
        prop_assert!(
            result_invalid.is_err(),
            "Requesting byte at file_size should fail"
        );
        
        let error = result_invalid.unwrap_err();
        prop_assert_eq!(
            error.to_http_status(),
            416,
            "Byte at file_size must return 416"
        );
    }

    /// Property 20: Large file handling
    /// 
    /// For very large files, invalid ranges should still be properly detected
    /// and return 416.
    #[test]
    fn prop_invalid_range_large_files(
        file_size in 1_000_000_000u64..=10_000_000_000u64,
        excess in 1u64..=1_000_000u64,
    ) {
        let invalid_range = ByteRange::new(file_size, file_size + excess)
            .expect("Range construction should succeed");
        
        let metadata = FileMetadata::with_headers(
            file_size,
            true,
            Some("video/mp4".to_string()),
            None,
            None,
        );
        
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(invalid_range));
        
        // Property: Must return an error even for large files
        prop_assert!(
            result.is_err(),
            "Invalid range on large file should return error"
        );
        
        let error = result.unwrap_err();
        
        // Property: Must map to 416
        prop_assert_eq!(
            error.to_http_status(),
            416,
            "Invalid range on large file must return 416"
        );
    }

    /// Property 20: Zero-length file edge case
    /// 
    /// For a zero-length file, any range request should be unsatisfiable.
    #[test]
    fn prop_invalid_range_zero_length_file(
        range_start in 0u64..=100u64,
        range_end in 0u64..=100u64,
    ) {
        let (range_start, range_end) = if range_start <= range_end {
            (range_start, range_end)
        } else {
            (range_end, range_start)
        };
        
        let range = ByteRange::new(range_start, range_end)
            .expect("Range construction should succeed");
        
        let metadata = FileMetadata::with_headers(
            0, // Zero-length file
            true,
            Some("application/octet-stream".to_string()),
            None,
            None,
        );
        
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(range));
        
        // Property: Any range on zero-length file should fail
        prop_assert!(
            result.is_err(),
            "Any range request on zero-length file should be unsatisfiable"
        );
        
        let error = result.unwrap_err();
        
        // Property: Must map to 416
        prop_assert_eq!(
            error.to_http_status(),
            416,
            "Range on zero-length file must return 416"
        );
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_invalid_range_end_exceeds_file_size() {
        let metadata = FileMetadata::with_headers(
            10000,
            true,
            Some("text/plain".to_string()),
            None,
            None,
        );

        let invalid_range = ByteRange::new(0, 10000).unwrap();
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(invalid_range));

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.to_http_status(), 416);
        assert!(matches!(error, SliceError::InvalidRange(_)));
    }

    #[test]
    fn test_invalid_range_start_beyond_file_size() {
        let metadata = FileMetadata::with_headers(
            10000,
            true,
            Some("text/plain".to_string()),
            None,
            None,
        );

        let invalid_range = ByteRange::new(10001, 20000).unwrap();
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(invalid_range));

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.to_http_status(), 416);
    }

    #[test]
    fn test_valid_range_last_byte() {
        let metadata = FileMetadata::with_headers(
            10000,
            true,
            Some("text/plain".to_string()),
            None,
            None,
        );

        let valid_range = ByteRange::new(9999, 9999).unwrap();
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(valid_range));

        assert!(result.is_ok());
        let (status, _) = result.unwrap();
        assert_eq!(status.as_u16(), 206);
    }

    #[test]
    fn test_invalid_range_at_file_size() {
        let metadata = FileMetadata::with_headers(
            10000,
            true,
            Some("text/plain".to_string()),
            None,
            None,
        );

        let invalid_range = ByteRange::new(10000, 10000).unwrap();
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(invalid_range));

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.to_http_status(), 416);
    }

    #[test]
    fn test_valid_range_does_not_return_416() {
        let metadata = FileMetadata::with_headers(
            10000,
            true,
            Some("application/octet-stream".to_string()),
            None,
            None,
        );

        let valid_range = ByteRange::new(1000, 5000).unwrap();
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(valid_range));

        assert!(result.is_ok());
        let (status, _) = result.unwrap();
        assert_eq!(status.as_u16(), 206);
        assert_ne!(status.as_u16(), 416);
    }

    #[test]
    fn test_zero_length_file() {
        let metadata = FileMetadata::with_headers(
            0,
            true,
            Some("text/plain".to_string()),
            None,
            None,
        );

        let range = ByteRange::new(0, 0).unwrap();
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(range));

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.to_http_status(), 416);
    }

    #[test]
    fn test_large_file_invalid_range() {
        let metadata = FileMetadata::with_headers(
            5_000_000_000, // 5GB
            true,
            Some("video/mp4".to_string()),
            None,
            None,
        );

        let invalid_range = ByteRange::new(5_000_000_000, 5_000_001_000).unwrap();
        let assembler = ResponseAssembler::new();
        let result = assembler.build_response_header(&metadata, Some(invalid_range));

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.to_http_status(), 416);
    }

    #[test]
    fn test_error_consistency() {
        let metadata = FileMetadata::with_headers(
            10000,
            true,
            Some("text/plain".to_string()),
            None,
            None,
        );

        let invalid_range = ByteRange::new(0, 10000).unwrap();
        let assembler = ResponseAssembler::new();

        let result1 = assembler.build_response_header(&metadata, Some(invalid_range));
        let result2 = assembler.build_response_header(&metadata, Some(invalid_range));

        assert!(result1.is_err() && result2.is_err());
        assert_eq!(result1.unwrap_err().to_http_status(), 416);
        assert_eq!(result2.unwrap_err().to_http_status(), 416);
    }
}
