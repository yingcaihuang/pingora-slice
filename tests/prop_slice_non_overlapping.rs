// Feature: pingora-slice, Property 5: 分片无重叠性
// **Validates: Requirements 4.2**
//
// Property: For any set of calculated slices, no two slices should have 
// overlapping byte ranges

use pingora_slice::slice_calculator::SliceCalculator;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property 5: Slice non-overlapping
    /// 
    /// For any file size and slice size, the calculated slices should:
    /// 1. Have no overlapping byte ranges between any two slices
    /// 2. Each byte position should be covered by at most one slice
    #[test]
    fn prop_slice_non_overlapping(
        file_size in 1u64..=100_000_000u64,
        slice_size in 1usize..=10_000_000usize,
    ) {
        let calculator = SliceCalculator::new(slice_size);
        let slices = calculator.calculate_slices(file_size, None)
            .expect("Slice calculation should succeed for valid inputs");
        
        // For empty files, no slices means no overlaps
        if slices.is_empty() {
            return Ok(());
        }
        
        // Check all pairs of slices for overlaps
        for i in 0..slices.len() {
            for j in (i + 1)..slices.len() {
                let slice_i = &slices[i];
                let slice_j = &slices[j];
                
                // Two ranges overlap if:
                // - slice_i.start <= slice_j.end AND slice_j.start <= slice_i.end
                // 
                // They don't overlap if:
                // - slice_i.end < slice_j.start OR slice_j.end < slice_i.start
                let overlaps = !(slice_i.range.end < slice_j.range.start || 
                                 slice_j.range.end < slice_i.range.start);
                
                prop_assert!(
                    !overlaps,
                    "Slices {} and {} overlap: [{}-{}] and [{}-{}]",
                    i,
                    j,
                    slice_i.range.start,
                    slice_i.range.end,
                    slice_j.range.start,
                    slice_j.range.end
                );
            }
        }
    }

    /// Property 5 (with client range): Non-overlapping for partial requests
    /// 
    /// When a client requests a specific range, the slices calculated for
    /// that range should not overlap with each other
    #[test]
    fn prop_slice_non_overlapping_with_client_range(
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
        
        // Check all pairs of slices for overlaps
        for i in 0..slices.len() {
            for j in (i + 1)..slices.len() {
                let slice_i = &slices[i];
                let slice_j = &slices[j];
                
                let overlaps = !(slice_i.range.end < slice_j.range.start || 
                                 slice_j.range.end < slice_i.range.start);
                
                prop_assert!(
                    !overlaps,
                    "Slices {} and {} overlap in client range request: [{}-{}] and [{}-{}]",
                    i,
                    j,
                    slice_i.range.start,
                    slice_i.range.end,
                    slice_j.range.start,
                    slice_j.range.end
                );
            }
        }
    }

    /// Property 5 (verification): Consecutive slices should be adjacent, not overlapping
    /// 
    /// For consecutive slices i and i+1:
    /// - slice[i].end + 1 should equal slice[i+1].start
    /// - This ensures they are adjacent but not overlapping
    #[test]
    fn prop_consecutive_slices_adjacent_not_overlapping(
        file_size in 1u64..=100_000_000u64,
        slice_size in 1usize..=10_000_000usize,
    ) {
        let calculator = SliceCalculator::new(slice_size);
        let slices = calculator.calculate_slices(file_size, None)
            .expect("Slice calculation should succeed");
        
        if slices.len() < 2 {
            return Ok(());
        }
        
        // Check each consecutive pair
        for i in 0..slices.len() - 1 {
            let current = &slices[i];
            let next = &slices[i + 1];
            
            // Consecutive slices should be adjacent: current.end + 1 == next.start
            prop_assert_eq!(
                current.range.end + 1,
                next.range.start,
                "Consecutive slices {} and {} are not adjacent: slice {} ends at {}, slice {} starts at {}",
                i,
                i + 1,
                i,
                current.range.end,
                i + 1,
                next.range.start
            );
            
            // Verify they don't overlap (current.end < next.start)
            prop_assert!(
                current.range.end < next.range.start,
                "Consecutive slices {} and {} overlap: slice {} ends at {}, slice {} starts at {}",
                i,
                i + 1,
                i,
                current.range.end,
                i + 1,
                next.range.start
            );
        }
    }

    /// Property 5 (edge case): Single slice should not overlap with itself
    #[test]
    fn prop_single_slice_no_self_overlap(
        file_size in 1u64..=1000u64,
        slice_size in 1001usize..=10000usize,
    ) {
        let calculator = SliceCalculator::new(slice_size);
        let slices = calculator.calculate_slices(file_size, None)
            .expect("Slice calculation should succeed");
        
        // File smaller than slice size should produce exactly one slice
        prop_assert_eq!(
            slices.len(),
            1,
            "File smaller than slice size should have exactly one slice"
        );
        
        // A single slice trivially has no overlaps with other slices
        // Just verify it's well-formed
        prop_assert!(
            slices[0].range.start <= slices[0].range.end,
            "Single slice should have valid range"
        );
    }

    /// Property 5 (verification): Byte-level non-overlapping check
    /// 
    /// For smaller files, verify that each byte position is covered by
    /// at most one slice (no byte appears in multiple slices)
    #[test]
    fn prop_byte_level_non_overlapping(
        file_size in 1u64..=10000u64,  // Smaller range for performance
        slice_size in 1usize..=5000usize,
    ) {
        let calculator = SliceCalculator::new(slice_size);
        let slices = calculator.calculate_slices(file_size, None)
            .expect("Slice calculation should succeed");
        
        // For each byte position, count how many slices cover it
        for byte_pos in 0..file_size {
            let covering_slices: Vec<_> = slices.iter()
                .enumerate()
                .filter(|(_, slice)| slice.range.start <= byte_pos && byte_pos <= slice.range.end)
                .collect();
            
            prop_assert!(
                covering_slices.len() <= 1,
                "Byte position {} is covered by {} slices (should be at most 1): {:?}",
                byte_pos,
                covering_slices.len(),
                covering_slices.iter().map(|(i, s)| (i, s.range.start, s.range.end)).collect::<Vec<_>>()
            );
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use pingora_slice::slice_calculator::SliceCalculator;

    #[test]
    fn test_non_overlapping_simple_case() {
        let calculator = SliceCalculator::new(1000);
        let slices = calculator.calculate_slices(3000, None).unwrap();
        
        assert_eq!(slices.len(), 3);
        
        // Verify no overlaps between any pair
        for i in 0..slices.len() {
            for j in (i + 1)..slices.len() {
                let slice_i = &slices[i];
                let slice_j = &slices[j];
                
                // Slices should not overlap
                assert!(
                    slice_i.range.end < slice_j.range.start || 
                    slice_j.range.end < slice_i.range.start,
                    "Slices {} and {} overlap",
                    i,
                    j
                );
            }
        }
    }

    #[test]
    fn test_non_overlapping_with_remainder() {
        let calculator = SliceCalculator::new(1000);
        let slices = calculator.calculate_slices(2500, None).unwrap();
        
        assert_eq!(slices.len(), 3);
        
        // Check specific ranges
        assert_eq!(slices[0].range.start, 0);
        assert_eq!(slices[0].range.end, 999);
        assert_eq!(slices[1].range.start, 1000);
        assert_eq!(slices[1].range.end, 1999);
        assert_eq!(slices[2].range.start, 2000);
        assert_eq!(slices[2].range.end, 2499);
        
        // Verify no overlaps
        assert!(slices[0].range.end < slices[1].range.start);
        assert!(slices[1].range.end < slices[2].range.start);
    }

    #[test]
    fn test_non_overlapping_consecutive_adjacent() {
        let calculator = SliceCalculator::new(1024);
        let slices = calculator.calculate_slices(5000, None).unwrap();
        
        // Verify consecutive slices are adjacent (not overlapping, no gaps)
        for i in 0..slices.len() - 1 {
            assert_eq!(
                slices[i].range.end + 1,
                slices[i + 1].range.start,
                "Slices {} and {} should be adjacent",
                i,
                i + 1
            );
        }
    }

    #[test]
    fn test_non_overlapping_single_slice() {
        let calculator = SliceCalculator::new(10000);
        let slices = calculator.calculate_slices(5000, None).unwrap();
        
        assert_eq!(slices.len(), 1);
        // Single slice trivially has no overlaps
    }
}
