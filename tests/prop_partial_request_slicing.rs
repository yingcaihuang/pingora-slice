// Feature: pingora-slice, Property 18: 部分请求分片计算
// **Validates: Requirements 10.2, 10.3**
//
// Property: For any client Range request, the calculated slices should only 
// cover the requested byte range, not the entire file

use pingora_slice::models::ByteRange;
use pingora_slice::slice_calculator::SliceCalculator;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property 18: Partial request slice calculation
    /// 
    /// When a client requests a specific byte range, the calculated slices should:
    /// 1. Only cover the requested range, not the entire file
    /// 2. Start at or before the requested range start
    /// 3. End at or after the requested range end
    /// 4. Not include slices outside the requested range
    #[test]
    fn prop_partial_request_slicing(
        file_size in 10000u64..=100_000_000u64,
        slice_size in 1000usize..=10_000_000usize,
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
        
        let client_range = ByteRange::new(range_start, range_end)
            .expect("Range should be valid");
        
        let calculator = SliceCalculator::new(slice_size);
        
        // Calculate slices for the partial request
        let slices = calculator.calculate_slices(file_size, Some(client_range))
            .expect("Slice calculation should succeed");
        
        // Property 1: Slices should not be empty for valid range
        prop_assert!(
            !slices.is_empty(),
            "Non-empty range should produce at least one slice"
        );
        
        // Property 2: First slice should start at the requested range start
        prop_assert_eq!(
            slices[0].range.start,
            range_start,
            "First slice should start at requested range start ({})",
            range_start
        );
        
        // Property 3: Last slice should end at the requested range end
        prop_assert_eq!(
            slices.last().unwrap().range.end,
            range_end,
            "Last slice should end at requested range end ({})",
            range_end
        );
        
        // Property 4: All slices should be within the requested range
        for (i, slice) in slices.iter().enumerate() {
            prop_assert!(
                slice.range.start >= range_start,
                "Slice {} starts at {} which is before requested range start {}",
                i,
                slice.range.start,
                range_start
            );
            
            prop_assert!(
                slice.range.end <= range_end,
                "Slice {} ends at {} which is after requested range end {}",
                i,
                slice.range.end,
                range_end
            );
        }
        
        // Property 5: Total bytes covered should equal requested range size
        let total_bytes_covered: u64 = slices.iter()
            .map(|slice| slice.range.size())
            .sum();
        
        let expected_size = range_end - range_start + 1;
        prop_assert_eq!(
            total_bytes_covered,
            expected_size,
            "Total bytes covered ({}) should equal requested range size ({})",
            total_bytes_covered,
            expected_size
        );
        
        // Property 6: Slices should not extend beyond the requested range
        // (This is a stronger check than property 4)
        let min_slice_start = slices.iter().map(|s| s.range.start).min().unwrap();
        let max_slice_end = slices.iter().map(|s| s.range.end).max().unwrap();
        
        prop_assert_eq!(
            min_slice_start,
            range_start,
            "Minimum slice start should be exactly the requested range start"
        );
        
        prop_assert_eq!(
            max_slice_end,
            range_end,
            "Maximum slice end should be exactly the requested range end"
        );
    }

    /// Property 18 (verification): Slices should not cover bytes outside requested range
    /// 
    /// For any byte position outside the requested range, it should not be covered by any slice
    #[test]
    fn prop_partial_request_no_extra_coverage(
        file_size in 10000u64..=100_000u64,  // Smaller for performance
        slice_size in 1000usize..=50_000usize,
        range_start in 100u64..=50_000u64,
        range_length in 100u64..=40_000u64,
    ) {
        // Ensure range is within file bounds and has space before/after
        let range_start = range_start % (file_size / 2);
        let range_end = std::cmp::min(range_start + range_length - 1, file_size - 100);
        
        // Skip if range is invalid or too close to boundaries
        if range_start > range_end || range_start < 10 || range_end >= file_size - 10 {
            return Ok(());
        }
        
        let client_range = ByteRange::new(range_start, range_end)
            .expect("Range should be valid");
        
        let calculator = SliceCalculator::new(slice_size);
        let slices = calculator.calculate_slices(file_size, Some(client_range))
            .expect("Slice calculation should succeed");
        
        // Check some bytes before the requested range
        for byte_pos in (range_start.saturating_sub(10))..range_start {
            let covering_slices: Vec<_> = slices.iter()
                .filter(|slice| slice.range.start <= byte_pos && byte_pos <= slice.range.end)
                .collect();
            
            prop_assert_eq!(
                covering_slices.len(),
                0,
                "Byte position {} (before requested range) should not be covered by any slice",
                byte_pos
            );
        }
        
        // Check some bytes after the requested range
        for byte_pos in (range_end + 1)..std::cmp::min(range_end + 11, file_size) {
            let covering_slices: Vec<_> = slices.iter()
                .filter(|slice| slice.range.start <= byte_pos && byte_pos <= slice.range.end)
                .collect();
            
            prop_assert_eq!(
                covering_slices.len(),
                0,
                "Byte position {} (after requested range) should not be covered by any slice",
                byte_pos
            );
        }
    }

    /// Property 18 (edge case): Single byte range request
    #[test]
    fn prop_partial_request_single_byte(
        file_size in 100u64..=100_000u64,
        slice_size in 1usize..=10_000usize,
        byte_pos in 0u64..=99_999u64,
    ) {
        let byte_pos = byte_pos % file_size;
        let client_range = ByteRange::new(byte_pos, byte_pos)
            .expect("Single byte range should be valid");
        
        let calculator = SliceCalculator::new(slice_size);
        let slices = calculator.calculate_slices(file_size, Some(client_range))
            .expect("Slice calculation should succeed");
        
        prop_assert_eq!(
            slices.len(),
            1,
            "Single byte range should produce exactly one slice"
        );
        
        prop_assert_eq!(
            slices[0].range.start,
            byte_pos,
            "Slice should start at requested byte"
        );
        
        prop_assert_eq!(
            slices[0].range.end,
            byte_pos,
            "Slice should end at requested byte"
        );
        
        prop_assert_eq!(
            slices[0].range.size(),
            1,
            "Slice should cover exactly 1 byte"
        );
    }

    /// Property 18 (edge case): Range smaller than slice size
    #[test]
    fn prop_partial_request_smaller_than_slice(
        file_size in 10000u64..=100_000u64,
        slice_size in 5000usize..=50_000usize,
        range_start in 0u64..=50_000u64,
        range_length in 1u64..=4999u64,
    ) {
        let range_start = range_start % (file_size - 5000);
        let range_end = std::cmp::min(range_start + range_length - 1, file_size - 1);
        
        // Ensure range is smaller than slice size
        if range_end - range_start + 1 >= slice_size as u64 {
            return Ok(());
        }
        
        let client_range = ByteRange::new(range_start, range_end)
            .expect("Range should be valid");
        
        let calculator = SliceCalculator::new(slice_size);
        let slices = calculator.calculate_slices(file_size, Some(client_range))
            .expect("Slice calculation should succeed");
        
        prop_assert_eq!(
            slices.len(),
            1,
            "Range smaller than slice size should produce exactly one slice"
        );
        
        prop_assert_eq!(
            slices[0].range.start,
            range_start,
            "Slice should start at requested range start"
        );
        
        prop_assert_eq!(
            slices[0].range.end,
            range_end,
            "Slice should end at requested range end"
        );
        
        let expected_size = range_end - range_start + 1;
        prop_assert_eq!(
            slices[0].range.size(),
            expected_size,
            "Slice should cover exactly the requested range size"
        );
    }

    /// Property 18 (comparison): Partial request should produce fewer slices than full file
    #[test]
    fn prop_partial_request_fewer_slices_than_full(
        file_size in 10000u64..=100_000u64,
        slice_size in 1000usize..=10_000usize,
        range_start in 0u64..=40_000u64,
        range_length in 1u64..=30_000u64,
    ) {
        let range_start = range_start % (file_size / 2);
        let range_end = std::cmp::min(range_start + range_length - 1, file_size - 1);
        
        // Ensure range is significantly smaller than file
        if range_end - range_start + 1 >= file_size / 2 {
            return Ok(());
        }
        
        let client_range = ByteRange::new(range_start, range_end)
            .expect("Range should be valid");
        
        let calculator = SliceCalculator::new(slice_size);
        
        // Calculate slices for partial request
        let partial_slices = calculator.calculate_slices(file_size, Some(client_range))
            .expect("Partial slice calculation should succeed");
        
        // Calculate slices for full file
        let full_slices = calculator.calculate_slices(file_size, None)
            .expect("Full slice calculation should succeed");
        
        prop_assert!(
            partial_slices.len() <= full_slices.len(),
            "Partial request should produce at most as many slices as full file request. \
             Partial: {}, Full: {}",
            partial_slices.len(),
            full_slices.len()
        );
        
        // If range is significantly smaller, should have fewer slices
        let range_size = range_end - range_start + 1;
        if range_size < file_size / 2 {
            prop_assert!(
                partial_slices.len() < full_slices.len(),
                "Partial request for half the file should produce fewer slices. \
                 Partial: {}, Full: {}",
                partial_slices.len(),
                full_slices.len()
            );
        }
    }

    /// Property 18 (alignment): Partial request slices should align with slice boundaries
    #[test]
    fn prop_partial_request_slice_alignment(
        file_size in 10000u64..=100_000u64,
        slice_size in 1000usize..=10_000usize,
        range_start in 0u64..=50_000u64,
        range_length in 1000u64..=40_000u64,
    ) {
        let range_start = range_start % (file_size - 1000);
        let range_end = std::cmp::min(range_start + range_length - 1, file_size - 1);
        
        let client_range = ByteRange::new(range_start, range_end)
            .expect("Range should be valid");
        
        let calculator = SliceCalculator::new(slice_size);
        let slices = calculator.calculate_slices(file_size, Some(client_range))
            .expect("Slice calculation should succeed");
        
        // Verify that internal slices (not first or last) are exactly slice_size
        for (i, slice) in slices.iter().enumerate() {
            if i == 0 {
                // First slice: should start at range_start
                prop_assert_eq!(
                    slice.range.start,
                    range_start,
                    "First slice should start at requested range start"
                );
            } else {
                // Non-first slices: should connect to previous slice
                prop_assert_eq!(
                    slice.range.start,
                    slices[i - 1].range.end + 1,
                    "Slice {} should start immediately after previous slice",
                    i
                );
            }
            
            if i == slices.len() - 1 {
                // Last slice: should end at range_end
                prop_assert_eq!(
                    slice.range.end,
                    range_end,
                    "Last slice should end at requested range end"
                );
            }
            
            // Middle slices should be exactly slice_size (if possible)
            if i > 0 && i < slices.len() - 1 {
                let expected_size = std::cmp::min(slice_size as u64, range_end - slice.range.start + 1);
                prop_assert!(
                    slice.range.size() <= expected_size,
                    "Middle slice {} should not exceed slice_size",
                    i
                );
            }
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use pingora_slice::models::ByteRange;
    use pingora_slice::slice_calculator::SliceCalculator;

    #[test]
    fn test_partial_request_simple() {
        let calculator = SliceCalculator::new(1000);
        let file_size = 10000;
        let client_range = ByteRange::new(2000, 4999).unwrap();
        
        let slices = calculator.calculate_slices(file_size, Some(client_range)).unwrap();
        
        // Should produce 3 slices: 2000-2999, 3000-3999, 4000-4999
        assert_eq!(slices.len(), 3);
        assert_eq!(slices[0].range.start, 2000);
        assert_eq!(slices[0].range.end, 2999);
        assert_eq!(slices[1].range.start, 3000);
        assert_eq!(slices[1].range.end, 3999);
        assert_eq!(slices[2].range.start, 4000);
        assert_eq!(slices[2].range.end, 4999);
    }

    #[test]
    fn test_partial_request_single_byte() {
        let calculator = SliceCalculator::new(1000);
        let file_size = 10000;
        let client_range = ByteRange::new(5000, 5000).unwrap();
        
        let slices = calculator.calculate_slices(file_size, Some(client_range)).unwrap();
        
        assert_eq!(slices.len(), 1);
        assert_eq!(slices[0].range.start, 5000);
        assert_eq!(slices[0].range.end, 5000);
        assert_eq!(slices[0].range.size(), 1);
    }

    #[test]
    fn test_partial_request_smaller_than_slice() {
        let calculator = SliceCalculator::new(10000);
        let file_size = 100000;
        let client_range = ByteRange::new(1000, 5000).unwrap();
        
        let slices = calculator.calculate_slices(file_size, Some(client_range)).unwrap();
        
        assert_eq!(slices.len(), 1);
        assert_eq!(slices[0].range.start, 1000);
        assert_eq!(slices[0].range.end, 5000);
    }

    #[test]
    fn test_partial_request_at_file_start() {
        let calculator = SliceCalculator::new(1000);
        let file_size = 10000;
        let client_range = ByteRange::new(0, 2499).unwrap();
        
        let slices = calculator.calculate_slices(file_size, Some(client_range)).unwrap();
        
        assert_eq!(slices.len(), 3);
        assert_eq!(slices[0].range.start, 0);
        assert_eq!(slices.last().unwrap().range.end, 2499);
    }

    #[test]
    fn test_partial_request_at_file_end() {
        let calculator = SliceCalculator::new(1000);
        let file_size = 10000;
        let client_range = ByteRange::new(8000, 9999).unwrap();
        
        let slices = calculator.calculate_slices(file_size, Some(client_range)).unwrap();
        
        assert_eq!(slices.len(), 2);
        assert_eq!(slices[0].range.start, 8000);
        assert_eq!(slices.last().unwrap().range.end, 9999);
    }

    #[test]
    fn test_partial_request_no_extra_bytes() {
        let calculator = SliceCalculator::new(1000);
        let file_size = 10000;
        let client_range = ByteRange::new(3000, 5999).unwrap();
        
        let slices = calculator.calculate_slices(file_size, Some(client_range)).unwrap();
        
        // Verify no slice covers bytes before 3000
        for slice in &slices {
            assert!(slice.range.start >= 3000);
        }
        
        // Verify no slice covers bytes after 5999
        for slice in &slices {
            assert!(slice.range.end <= 5999);
        }
        
        // Verify total coverage
        let total: u64 = slices.iter().map(|s| s.range.size()).sum();
        assert_eq!(total, 3000); // 5999 - 3000 + 1
    }
}
