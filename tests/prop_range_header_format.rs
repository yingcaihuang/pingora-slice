// Feature: pingora-slice, Property 6: Range 头格式正确性
// **Validates: Requirements 4.2**
//
// Property: For any generated slice specification, the corresponding Range header 
// should be in the format "bytes=start-end" where start <= end

use pingora_slice::slice_calculator::SliceCalculator;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property 6: Range header format correctness
    /// 
    /// For any file size and slice size, all generated slice specifications
    /// should produce Range headers in the correct format "bytes=start-end"
    /// where start <= end.
    #[test]
    fn prop_range_header_format_correctness(
        file_size in 1u64..=100_000_000u64,
        slice_size in 1usize..=10_000_000usize,
    ) {
        let calculator = SliceCalculator::new(slice_size);
        let slices = calculator.calculate_slices(file_size, None)
            .expect("Slice calculation should succeed for valid inputs");
        
        // For each generated slice specification
        for slice in slices.iter() {
            let range_header = slice.range.to_header();
            
            // 1. Header should start with "bytes="
            prop_assert!(
                range_header.starts_with("bytes="),
                "Range header should start with 'bytes=', got: {}",
                range_header
            );
            
            // 2. Header should contain exactly one hyphen after "bytes="
            let after_prefix = &range_header[6..]; // Skip "bytes="
            prop_assert!(
                after_prefix.contains('-'),
                "Range header should contain '-' separator, got: {}",
                range_header
            );
            
            // 3. Verify the format is exactly "bytes=start-end"
            let expected = format!("bytes={}-{}", slice.range.start, slice.range.end);
            prop_assert_eq!(
                &range_header,
                &expected,
                "Range header format should be 'bytes=start-end'"
            );
            
            // 4. Verify start <= end (this should always be true for valid slices)
            prop_assert!(
                slice.range.start <= slice.range.end,
                "Range start ({}) should be <= end ({})",
                slice.range.start,
                slice.range.end
            );
            
            // 5. Verify the header can be parsed back correctly
            let parsed = pingora_slice::models::ByteRange::from_header(&range_header)
                .expect("Generated header should be parseable");
            
            prop_assert_eq!(
                parsed.start,
                slice.range.start,
                "Parsed start should match original"
            );
            prop_assert_eq!(
                parsed.end,
                slice.range.end,
                "Parsed end should match original"
            );
        }
    }

    /// Property 6 (with client range): Range header format for partial requests
    /// 
    /// When calculating slices for a client range request, all generated
    /// Range headers should still be in the correct format.
    #[test]
    fn prop_range_header_format_with_client_range(
        file_size in 1000u64..=100_000_000u64,
        slice_size in 1usize..=10_000_000usize,
        range_start in 0u64..=50_000_000u64,
        range_length in 1u64..=50_000_000u64,
    ) {
        // Ensure range is within file bounds
        let range_start = range_start % file_size;
        let range_end = std::cmp::min(range_start + range_length - 1, file_size - 1);
        
        // Skip if range is invalid
        if range_start > range_end {
            return Ok(());
        }
        
        let client_range = pingora_slice::models::ByteRange::new(range_start, range_end)
            .expect("Range should be valid");
        
        let calculator = SliceCalculator::new(slice_size);
        let slices = calculator.calculate_slices(file_size, Some(client_range))
            .expect("Slice calculation should succeed");
        
        // Verify all generated slices have correct Range header format
        for slice in slices.iter() {
            let range_header = slice.range.to_header();
            
            // Verify format
            prop_assert!(
                range_header.starts_with("bytes="),
                "Range header should start with 'bytes='"
            );
            
            let expected = format!("bytes={}-{}", slice.range.start, slice.range.end);
            prop_assert_eq!(
                range_header,
                expected,
                "Range header format should be 'bytes=start-end'"
            );
            
            // Verify start <= end
            prop_assert!(
                slice.range.start <= slice.range.end,
                "Range start should be <= end"
            );
        }
    }

    /// Property 6 (edge case): Single slice file
    /// 
    /// Even for files that result in a single slice, the Range header
    /// should be in the correct format.
    #[test]
    fn prop_range_header_format_single_slice(
        file_size in 1u64..=1000u64,
        slice_size in 1001usize..=10000usize,
    ) {
        let calculator = SliceCalculator::new(slice_size);
        let slices = calculator.calculate_slices(file_size, None)
            .expect("Slice calculation should succeed");
        
        prop_assert_eq!(
            slices.len(),
            1,
            "File smaller than slice size should have exactly one slice"
        );
        
        let range_header = slices[0].range.to_header();
        
        // Verify format
        prop_assert!(
            range_header.starts_with("bytes="),
            "Range header should start with 'bytes='"
        );
        
        let expected = format!("bytes={}-{}", slices[0].range.start, slices[0].range.end);
        prop_assert_eq!(
            range_header,
            expected,
            "Range header format should be 'bytes=start-end'"
        );
        
        // For a single slice of the entire file
        prop_assert_eq!(slices[0].range.start, 0, "Should start at 0");
        prop_assert_eq!(slices[0].range.end, file_size - 1, "Should end at file_size - 1");
    }

    /// Property 6 (verification): No invalid characters in Range header
    /// 
    /// The generated Range header should only contain valid characters:
    /// "bytes=", digits, and a single hyphen separator.
    #[test]
    fn prop_range_header_no_invalid_characters(
        file_size in 1u64..=10_000_000u64,
        slice_size in 1usize..=5_000_000usize,
    ) {
        let calculator = SliceCalculator::new(slice_size);
        let slices = calculator.calculate_slices(file_size, None)
            .expect("Slice calculation should succeed");
        
        for slice in slices.iter() {
            let range_header = slice.range.to_header();
            
            // After "bytes=", should only have digits and one hyphen
            let after_prefix = &range_header[6..];
            
            // Count hyphens - should be exactly 1
            let hyphen_count = after_prefix.chars().filter(|&c| c == '-').count();
            prop_assert_eq!(
                hyphen_count,
                1,
                "Range header should contain exactly one hyphen, got {} in '{}'",
                hyphen_count,
                range_header
            );
            
            // All other characters should be digits
            for (i, ch) in after_prefix.chars().enumerate() {
                prop_assert!(
                    ch.is_ascii_digit() || ch == '-',
                    "Character at position {} should be digit or hyphen, got '{}' in '{}'",
                    i,
                    ch,
                    range_header
                );
            }
            
            // Should not contain whitespace
            prop_assert!(
                !range_header.contains(' '),
                "Range header should not contain whitespace: '{}'",
                range_header
            );
        }
    }

    /// Property 6 (consistency): Multiple calculations produce consistent headers
    /// 
    /// Calculating slices multiple times for the same file should produce
    /// identical Range headers.
    #[test]
    fn prop_range_header_consistency(
        file_size in 1u64..=10_000_000u64,
        slice_size in 1usize..=5_000_000usize,
    ) {
        let calculator = SliceCalculator::new(slice_size);
        
        // Calculate slices twice
        let slices1 = calculator.calculate_slices(file_size, None)
            .expect("First calculation should succeed");
        let slices2 = calculator.calculate_slices(file_size, None)
            .expect("Second calculation should succeed");
        
        prop_assert_eq!(
            slices1.len(),
            slices2.len(),
            "Both calculations should produce same number of slices"
        );
        
        // Verify each slice produces the same Range header
        for (slice1, slice2) in slices1.iter().zip(slices2.iter()) {
            let header1 = slice1.range.to_header();
            let header2 = slice2.range.to_header();
            
            prop_assert_eq!(
                header1,
                header2,
                "Same slice should produce identical Range headers"
            );
        }
    }

    /// Property 6 (boundary values): Range headers for boundary cases
    /// 
    /// Range headers should be correct even for extreme values.
    #[test]
    fn prop_range_header_boundary_values(
        slice_size in 1usize..=1000usize,
    ) {
        let calculator = SliceCalculator::new(slice_size);
        
        // Test with file size = 1 (single byte)
        let slices = calculator.calculate_slices(1, None)
            .expect("Should handle single byte file");
        
        prop_assert_eq!(slices.len(), 1);
        let header = slices[0].range.to_header();
        prop_assert_eq!(header, "bytes=0-0", "Single byte should be 'bytes=0-0'");
        
        // Test with file size = slice_size (exact match)
        let slices = calculator.calculate_slices(slice_size as u64, None)
            .expect("Should handle exact slice size");
        
        prop_assert_eq!(slices.len(), 1);
        let header = slices[0].range.to_header();
        let expected = format!("bytes=0-{}", slice_size - 1);
        prop_assert_eq!(header, expected, "Exact slice size should produce correct header");
    }
}

#[cfg(test)]
mod unit_tests {
    use pingora_slice::slice_calculator::SliceCalculator;

    #[test]
    fn test_range_header_format_simple() {
        let calculator = SliceCalculator::new(1000);
        let slices = calculator.calculate_slices(3000, None).unwrap();
        
        assert_eq!(slices[0].range.to_header(), "bytes=0-999");
        assert_eq!(slices[1].range.to_header(), "bytes=1000-1999");
        assert_eq!(slices[2].range.to_header(), "bytes=2000-2999");
    }

    #[test]
    fn test_range_header_format_with_remainder() {
        let calculator = SliceCalculator::new(1000);
        let slices = calculator.calculate_slices(2500, None).unwrap();
        
        assert_eq!(slices[0].range.to_header(), "bytes=0-999");
        assert_eq!(slices[1].range.to_header(), "bytes=1000-1999");
        assert_eq!(slices[2].range.to_header(), "bytes=2000-2499");
    }

    #[test]
    fn test_range_header_format_single_byte() {
        let calculator = SliceCalculator::new(1000);
        let slices = calculator.calculate_slices(1, None).unwrap();
        
        assert_eq!(slices.len(), 1);
        assert_eq!(slices[0].range.to_header(), "bytes=0-0");
    }

    #[test]
    fn test_range_header_format_large_values() {
        let calculator = SliceCalculator::new(1_000_000);
        let slices = calculator.calculate_slices(10_000_000, None).unwrap();
        
        assert_eq!(slices[0].range.to_header(), "bytes=0-999999");
        assert_eq!(slices[9].range.to_header(), "bytes=9000000-9999999");
    }

    #[test]
    fn test_range_header_start_less_than_or_equal_end() {
        let calculator = SliceCalculator::new(1024);
        let slices = calculator.calculate_slices(5000, None).unwrap();
        
        for slice in slices.iter() {
            assert!(
                slice.range.start <= slice.range.end,
                "Range start should be <= end for slice {}: start={}, end={}",
                slice.index,
                slice.range.start,
                slice.range.end
            );
        }
    }

    #[test]
    fn test_range_header_parseable() {
        use pingora_slice::models::ByteRange;
        
        let calculator = SliceCalculator::new(1000);
        let slices = calculator.calculate_slices(3000, None).unwrap();
        
        for slice in slices.iter() {
            let header = slice.range.to_header();
            let parsed = ByteRange::from_header(&header).unwrap();
            
            assert_eq!(parsed.start, slice.range.start);
            assert_eq!(parsed.end, slice.range.end);
        }
    }
}
