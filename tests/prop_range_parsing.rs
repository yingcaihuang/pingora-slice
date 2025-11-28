// Feature: pingora-slice, Property 17: Range 解析正确性
// **Validates: Requirements 10.1**
//
// Property: For any valid HTTP Range header, the parsed byte range should 
// correctly represent the start and end positions specified in the header

use pingora_slice::models::ByteRange;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property 17: Range parsing correctness
    /// 
    /// For any valid start and end positions where start <= end,
    /// converting to a Range header and parsing it back should yield
    /// the same start and end values (round-trip property).
    #[test]
    fn prop_range_parsing_round_trip(
        start in 0u64..=u64::MAX / 2,
        end in 0u64..=u64::MAX / 2,
    ) {
        // Ensure start <= end
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        
        // Create a ByteRange
        let original_range = ByteRange::new(start, end)
            .expect("Range should be valid since start <= end");
        
        // Convert to header format
        let header = original_range.to_header();
        
        // Parse back from header
        let parsed_range = ByteRange::from_header(&header)
            .expect("Parsing should succeed for valid header");
        
        // Verify round-trip correctness
        prop_assert_eq!(
            parsed_range.start,
            original_range.start,
            "Parsed start should match original start"
        );
        prop_assert_eq!(
            parsed_range.end,
            original_range.end,
            "Parsed end should match original end"
        );
        prop_assert_eq!(
            parsed_range,
            original_range,
            "Parsed range should equal original range"
        );
    }

    /// Property 17 (format validation): Header format correctness
    /// 
    /// For any valid ByteRange, the generated header should follow
    /// the format "bytes=start-end"
    #[test]
    fn prop_range_header_format(
        start in 0u64..=u64::MAX / 2,
        end in 0u64..=u64::MAX / 2,
    ) {
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        
        let range = ByteRange::new(start, end)
            .expect("Range should be valid");
        
        let header = range.to_header();
        
        // Verify format
        prop_assert!(
            header.starts_with("bytes="),
            "Header should start with 'bytes=', got: {}",
            header
        );
        
        // Verify it contains a hyphen
        prop_assert!(
            header.contains('-'),
            "Header should contain '-', got: {}",
            header
        );
        
        // Verify the expected format
        let expected = format!("bytes={}-{}", start, end);
        prop_assert_eq!(
            header,
            expected,
            "Header format should be 'bytes=start-end'"
        );
    }

    /// Property 17 (parsing validation): Valid header parsing
    /// 
    /// For any valid header string in the format "bytes=start-end",
    /// parsing should succeed and extract correct values
    #[test]
    fn prop_range_parsing_valid_headers(
        start in 0u64..=1_000_000_000u64,
        end in 0u64..=1_000_000_000u64,
    ) {
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        
        // Create header string manually
        let header = format!("bytes={}-{}", start, end);
        
        // Parse it
        let result = ByteRange::from_header(&header);
        
        prop_assert!(
            result.is_ok(),
            "Parsing should succeed for valid header: {}",
            header
        );
        
        let range = result.unwrap();
        prop_assert_eq!(range.start, start, "Start should match");
        prop_assert_eq!(range.end, end, "End should match");
    }

    /// Property 17 (whitespace handling): Whitespace tolerance
    /// 
    /// Parsing should handle whitespace around the header value
    #[test]
    fn prop_range_parsing_with_whitespace(
        start in 0u64..=1_000_000u64,
        end in 0u64..=1_000_000u64,
        leading_spaces in 0usize..5,
        trailing_spaces in 0usize..5,
    ) {
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        
        // Create header with whitespace
        let leading = " ".repeat(leading_spaces);
        let trailing = " ".repeat(trailing_spaces);
        let header = format!("{}bytes={}-{}{}", leading, start, end, trailing);
        
        // Parse it
        let result = ByteRange::from_header(&header);
        
        prop_assert!(
            result.is_ok(),
            "Parsing should succeed with whitespace: '{}'",
            header
        );
        
        let range = result.unwrap();
        prop_assert_eq!(range.start, start, "Start should match");
        prop_assert_eq!(range.end, end, "End should match");
    }

    /// Property 17 (error handling): Invalid headers should fail
    /// 
    /// Headers that don't follow the correct format should fail to parse
    #[test]
    fn prop_range_parsing_invalid_prefix(
        prefix in "[a-z]{1,10}",
        start in 0u64..=1000u64,
        end in 0u64..=1000u64,
    ) {
        // Skip if prefix happens to be "bytes"
        prop_assume!(prefix != "bytes");
        
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        
        // Create header with wrong prefix
        let header = format!("{}={}-{}", prefix, start, end);
        
        // Parse it - should fail
        let result = ByteRange::from_header(&header);
        
        prop_assert!(
            result.is_err(),
            "Parsing should fail for invalid prefix: {}",
            header
        );
    }

    /// Property 17 (boundary values): Extreme values
    /// 
    /// Parsing should work correctly for boundary values
    #[test]
    fn prop_range_parsing_boundaries(
        use_zero_start in proptest::bool::ANY,
        use_max_end in proptest::bool::ANY,
    ) {
        let start = if use_zero_start { 0 } else { 1 };
        let end = if use_max_end { u64::MAX } else { u64::MAX - 1 };
        
        // Only test if start <= end
        if start <= end {
            let header = format!("bytes={}-{}", start, end);
            let result = ByteRange::from_header(&header);
            
            prop_assert!(
                result.is_ok(),
                "Parsing should succeed for boundary values: {}",
                header
            );
            
            let range = result.unwrap();
            prop_assert_eq!(range.start, start);
            prop_assert_eq!(range.end, end);
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_simple_range() {
        let header = "bytes=0-1023";
        let range = ByteRange::from_header(header).unwrap();
        assert_eq!(range.start, 0);
        assert_eq!(range.end, 1023);
    }

    #[test]
    fn test_round_trip() {
        let original = ByteRange::new(100, 200).unwrap();
        let header = original.to_header();
        let parsed = ByteRange::from_header(&header).unwrap();
        assert_eq!(parsed, original);
    }

    #[test]
    fn test_invalid_prefix() {
        let header = "range=0-100";
        let result = ByteRange::from_header(header);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_format() {
        let header = "bytes=0:100";
        let result = ByteRange::from_header(header);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_start_greater_than_end() {
        let header = "bytes=100-50";
        let result = ByteRange::from_header(header);
        assert!(result.is_err());
    }

    #[test]
    fn test_whitespace_handling() {
        let header = "  bytes=0-1023  ";
        let range = ByteRange::from_header(header).unwrap();
        assert_eq!(range.start, 0);
        assert_eq!(range.end, 1023);
    }

    #[test]
    fn test_large_values() {
        let header = "bytes=0-9999999999";
        let range = ByteRange::from_header(header).unwrap();
        assert_eq!(range.start, 0);
        assert_eq!(range.end, 9999999999);
    }
}
