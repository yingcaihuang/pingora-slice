// Feature: pingora-slice, Property 4: 分片覆盖完整性
// **Validates: Requirements 4.1, 4.2**
//
// Property: For any file size and slice size, the calculated slices should 
// cover all bytes from 0 to file_size-1 without gaps

use pingora_slice::slice_calculator::SliceCalculator;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property 4: Slice coverage completeness
    /// 
    /// For any file size and slice size, the calculated slices should:
    /// 1. Start at byte 0
    /// 2. End at byte file_size - 1
    /// 3. Have no gaps between consecutive slices
    /// 4. Cover every byte exactly once
    #[test]
    fn prop_slice_coverage_completeness(
        file_size in 1u64..=100_000_000u64,
        slice_size in 1usize..=10_000_000usize,
    ) {
        let calculator = SliceCalculator::new(slice_size);
        let slices = calculator.calculate_slices(file_size, None)
            .expect("Slice calculation should succeed for valid inputs");
        
        // Property should not apply to empty files
        if file_size == 0 {
            prop_assert_eq!(slices.len(), 0, "Empty file should have no slices");
            return Ok(());
        }
        
        // Verify we have at least one slice for non-empty files
        prop_assert!(
            !slices.is_empty(),
            "Non-empty file should have at least one slice"
        );
        
        // 1. First slice should start at byte 0
        prop_assert_eq!(
            slices[0].range.start,
            0,
            "First slice should start at byte 0"
        );
        
        // 2. Last slice should end at byte file_size - 1
        prop_assert_eq!(
            slices.last().unwrap().range.end,
            file_size - 1,
            "Last slice should end at byte file_size - 1 ({})",
            file_size - 1
        );
        
        // 3. Verify no gaps between consecutive slices
        for i in 0..slices.len() - 1 {
            let current_end = slices[i].range.end;
            let next_start = slices[i + 1].range.start;
            
            prop_assert_eq!(
                current_end + 1,
                next_start,
                "Gap detected between slice {} (ends at {}) and slice {} (starts at {})",
                i,
                current_end,
                i + 1,
                next_start
            );
        }
        
        // 4. Verify every byte is covered exactly once by computing total coverage
        let total_bytes_covered: u64 = slices.iter()
            .map(|slice| slice.range.size())
            .sum();
        
        prop_assert_eq!(
            total_bytes_covered,
            file_size,
            "Total bytes covered ({}) should equal file size ({})",
            total_bytes_covered,
            file_size
        );
    }

    /// Property 4 (with client range): Coverage completeness for partial requests
    /// 
    /// When a client requests a specific range, the slices should cover
    /// exactly that range without gaps
    #[test]
    fn prop_slice_coverage_with_client_range(
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
        
        // Verify we have at least one slice
        prop_assert!(
            !slices.is_empty(),
            "Non-empty range should have at least one slice"
        );
        
        // First slice should start at the requested range start
        prop_assert_eq!(
            slices[0].range.start,
            range_start,
            "First slice should start at requested range start"
        );
        
        // Last slice should end at the requested range end
        prop_assert_eq!(
            slices.last().unwrap().range.end,
            range_end,
            "Last slice should end at requested range end"
        );
        
        // Verify no gaps between consecutive slices
        for i in 0..slices.len() - 1 {
            let current_end = slices[i].range.end;
            let next_start = slices[i + 1].range.start;
            
            prop_assert_eq!(
                current_end + 1,
                next_start,
                "Gap detected between slice {} and slice {}",
                i,
                i + 1
            );
        }
        
        // Verify total coverage equals requested range size
        let total_bytes_covered: u64 = slices.iter()
            .map(|slice| slice.range.size())
            .sum();
        
        let expected_size = range_end - range_start + 1;
        prop_assert_eq!(
            total_bytes_covered,
            expected_size,
            "Total bytes covered should equal requested range size"
        );
    }

    /// Property 4 (edge case): Single byte file
    #[test]
    fn prop_slice_coverage_single_byte(
        slice_size in 1usize..=1_000_000usize,
    ) {
        let file_size = 1u64;
        let calculator = SliceCalculator::new(slice_size);
        let slices = calculator.calculate_slices(file_size, None)
            .expect("Slice calculation should succeed");
        
        prop_assert_eq!(slices.len(), 1, "Single byte file should have exactly one slice");
        prop_assert_eq!(slices[0].range.start, 0, "Slice should start at 0");
        prop_assert_eq!(slices[0].range.end, 0, "Slice should end at 0");
        prop_assert_eq!(slices[0].range.size(), 1, "Slice should cover 1 byte");
    }

    /// Property 4 (edge case): File smaller than slice size
    #[test]
    fn prop_slice_coverage_file_smaller_than_slice(
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
        prop_assert_eq!(slices[0].range.start, 0, "Slice should start at 0");
        prop_assert_eq!(
            slices[0].range.end,
            file_size - 1,
            "Slice should end at file_size - 1"
        );
        prop_assert_eq!(
            slices[0].range.size(),
            file_size,
            "Slice should cover entire file"
        );
    }

    /// Property 4 (edge case): File size exactly divisible by slice size
    #[test]
    fn prop_slice_coverage_exact_multiple(
        num_slices in 1usize..=1000usize,
        slice_size in 1usize..=100000usize,
    ) {
        let file_size = (num_slices as u64) * (slice_size as u64);
        let calculator = SliceCalculator::new(slice_size);
        let slices = calculator.calculate_slices(file_size, None)
            .expect("Slice calculation should succeed");
        
        prop_assert_eq!(
            slices.len(),
            num_slices,
            "File size exactly divisible by slice size should have expected number of slices"
        );
        
        // All slices should be exactly slice_size bytes
        for (i, slice) in slices.iter().enumerate() {
            prop_assert_eq!(
                slice.range.size(),
                slice_size as u64,
                "Slice {} should be exactly slice_size bytes",
                i
            );
        }
        
        // Verify coverage
        prop_assert_eq!(slices[0].range.start, 0);
        prop_assert_eq!(slices.last().unwrap().range.end, file_size - 1);
    }

    /// Property 4 (verification): Set-based coverage check
    /// 
    /// Every byte position from 0 to file_size-1 should be covered by exactly one slice
    #[test]
    fn prop_slice_coverage_every_byte_covered(
        file_size in 1u64..=10000u64,  // Smaller range for performance
        slice_size in 1usize..=5000usize,
    ) {
        let calculator = SliceCalculator::new(slice_size);
        let slices = calculator.calculate_slices(file_size, None)
            .expect("Slice calculation should succeed");
        
        // For each byte position, verify it's covered by exactly one slice
        for byte_pos in 0..file_size {
            let covering_slices: Vec<_> = slices.iter()
                .filter(|slice| slice.range.start <= byte_pos && byte_pos <= slice.range.end)
                .collect();
            
            prop_assert_eq!(
                covering_slices.len(),
                1,
                "Byte position {} should be covered by exactly one slice, found {}",
                byte_pos,
                covering_slices.len()
            );
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use pingora_slice::slice_calculator::SliceCalculator;

    #[test]
    fn test_coverage_simple_case() {
        let calculator = SliceCalculator::new(1000);
        let slices = calculator.calculate_slices(3000, None).unwrap();
        
        assert_eq!(slices.len(), 3);
        assert_eq!(slices[0].range.start, 0);
        assert_eq!(slices[0].range.end, 999);
        assert_eq!(slices[1].range.start, 1000);
        assert_eq!(slices[1].range.end, 1999);
        assert_eq!(slices[2].range.start, 2000);
        assert_eq!(slices[2].range.end, 2999);
    }

    #[test]
    fn test_coverage_with_remainder() {
        let calculator = SliceCalculator::new(1000);
        let slices = calculator.calculate_slices(2500, None).unwrap();
        
        assert_eq!(slices.len(), 3);
        assert_eq!(slices[0].range.start, 0);
        assert_eq!(slices[0].range.end, 999);
        assert_eq!(slices[1].range.start, 1000);
        assert_eq!(slices[1].range.end, 1999);
        assert_eq!(slices[2].range.start, 2000);
        assert_eq!(slices[2].range.end, 2499);
        
        // Verify no gaps
        assert_eq!(slices[0].range.end + 1, slices[1].range.start);
        assert_eq!(slices[1].range.end + 1, slices[2].range.start);
    }

    #[test]
    fn test_coverage_single_slice() {
        let calculator = SliceCalculator::new(10000);
        let slices = calculator.calculate_slices(5000, None).unwrap();
        
        assert_eq!(slices.len(), 1);
        assert_eq!(slices[0].range.start, 0);
        assert_eq!(slices[0].range.end, 4999);
    }

    #[test]
    fn test_total_bytes_covered() {
        let calculator = SliceCalculator::new(1024);
        let file_size = 10000u64;
        let slices = calculator.calculate_slices(file_size, None).unwrap();
        
        let total: u64 = slices.iter().map(|s| s.range.size()).sum();
        assert_eq!(total, file_size);
    }
}
