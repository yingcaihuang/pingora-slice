//! Slice Calculator
//!
//! Calculates how to split a file into slices for range requests

use crate::error::{Result, SliceError};
use crate::models::{ByteRange, SliceSpec};
use tracing::debug;

/// Calculator for splitting files into slices
pub struct SliceCalculator {
    /// Size of each slice in bytes
    slice_size: usize,
}

impl SliceCalculator {
    /// Create a new SliceCalculator
    ///
    /// # Arguments
    /// * `slice_size` - Size of each slice in bytes
    pub fn new(slice_size: usize) -> Self {
        SliceCalculator { slice_size }
    }

    /// Calculate the total number of slices needed for a file
    ///
    /// # Arguments
    /// * `file_size` - Total size of the file in bytes
    ///
    /// # Returns
    /// The number of slices needed to cover the entire file
    pub fn calculate_total_slices(&self, file_size: u64) -> usize {
        if file_size == 0 {
            return 0;
        }
        
        let slice_size = self.slice_size as u64;
        ((file_size + slice_size - 1) / slice_size) as usize
    }

    /// Calculate slices for a file or a specific range within a file
    ///
    /// # Arguments
    /// * `file_size` - Total size of the file in bytes
    /// * `client_range` - Optional byte range requested by the client
    ///
    /// # Returns
    /// A vector of SliceSpec representing the slices needed
    ///
    /// # Behavior
    /// - If `client_range` is None, calculates slices for the entire file
    /// - If `client_range` is Some, calculates only the slices needed for that range
    /// - Each slice (except possibly the last) will be `slice_size` bytes
    /// - The last slice will cover remaining bytes to the end of the requested range
    pub fn calculate_slices(
        &self,
        file_size: u64,
        client_range: Option<ByteRange>,
    ) -> Result<Vec<SliceSpec>> {
        if file_size == 0 {
            debug!("File size is 0, returning empty slice list");
            return Ok(Vec::new());
        }

        // Determine the range we need to slice
        let (range_start, range_end) = match client_range {
            Some(range) => {
                debug!(
                    "Calculating slices for client range: {}-{}, file_size={}",
                    range.start, range.end, file_size
                );
                
                // Validate the client range against file size
                if range.start >= file_size {
                    debug!(
                        "Invalid range: start {} is beyond file size {}",
                        range.start, file_size
                    );
                    return Err(SliceError::InvalidRange(format!(
                        "Range start {} is beyond file size {}",
                        range.start, file_size
                    )));
                }
                
                // Clamp the end to file size - 1
                let end = std::cmp::min(range.end, file_size - 1);
                if end != range.end {
                    debug!(
                        "Clamping range end from {} to {} (file_size={})",
                        range.end, end, file_size
                    );
                }
                (range.start, end)
            }
            None => {
                debug!("Calculating slices for entire file: file_size={}", file_size);
                // Slice the entire file
                (0, file_size - 1)
            }
        };

        let mut slices = Vec::new();
        let slice_size = self.slice_size as u64;
        let mut current_pos = range_start;
        let mut index = 0;

        while current_pos <= range_end {
            // Calculate the end of this slice
            let slice_end = std::cmp::min(
                current_pos + slice_size - 1,
                range_end
            );

            // Create the slice specification
            let range = ByteRange::new(current_pos, slice_end)?;
            slices.push(SliceSpec::new(index, range));

            // Move to the next slice
            current_pos = slice_end + 1;
            index += 1;
        }

        debug!(
            "Calculated {} slices for range {}-{} (file_size={}, slice_size={})",
            slices.len(), range_start, range_end, file_size, self.slice_size
        );

        Ok(slices)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_total_slices_exact_multiple() {
        let calculator = SliceCalculator::new(1024);
        assert_eq!(calculator.calculate_total_slices(4096), 4);
    }

    #[test]
    fn test_calculate_total_slices_with_remainder() {
        let calculator = SliceCalculator::new(1024);
        assert_eq!(calculator.calculate_total_slices(4097), 5);
    }

    #[test]
    fn test_calculate_total_slices_smaller_than_slice_size() {
        let calculator = SliceCalculator::new(1024);
        assert_eq!(calculator.calculate_total_slices(512), 1);
    }

    #[test]
    fn test_calculate_total_slices_zero_size() {
        let calculator = SliceCalculator::new(1024);
        assert_eq!(calculator.calculate_total_slices(0), 0);
    }

    #[test]
    fn test_calculate_slices_full_file() {
        let calculator = SliceCalculator::new(1024);
        let slices = calculator.calculate_slices(3000, None).unwrap();
        
        assert_eq!(slices.len(), 3);
        
        // First slice: 0-1023
        assert_eq!(slices[0].index, 0);
        assert_eq!(slices[0].range.start, 0);
        assert_eq!(slices[0].range.end, 1023);
        
        // Second slice: 1024-2047
        assert_eq!(slices[1].index, 1);
        assert_eq!(slices[1].range.start, 1024);
        assert_eq!(slices[1].range.end, 2047);
        
        // Third slice: 2048-2999 (last slice covers remaining bytes)
        assert_eq!(slices[2].index, 2);
        assert_eq!(slices[2].range.start, 2048);
        assert_eq!(slices[2].range.end, 2999);
    }

    #[test]
    fn test_calculate_slices_with_client_range() {
        let calculator = SliceCalculator::new(1024);
        let client_range = ByteRange::new(1000, 3000).unwrap();
        let slices = calculator.calculate_slices(10000, Some(client_range)).unwrap();
        
        assert_eq!(slices.len(), 2);
        
        // First slice: 1000-2023
        assert_eq!(slices[0].index, 0);
        assert_eq!(slices[0].range.start, 1000);
        assert_eq!(slices[0].range.end, 2023);
        
        // Second slice: 2024-3000
        assert_eq!(slices[1].index, 1);
        assert_eq!(slices[1].range.start, 2024);
        assert_eq!(slices[1].range.end, 3000);
    }

    #[test]
    fn test_calculate_slices_client_range_beyond_file_size() {
        let calculator = SliceCalculator::new(1024);
        let client_range = ByteRange::new(5000, 10000).unwrap();
        let result = calculator.calculate_slices(4000, Some(client_range));
        
        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_slices_client_range_end_beyond_file_size() {
        let calculator = SliceCalculator::new(1024);
        let client_range = ByteRange::new(1000, 10000).unwrap();
        let slices = calculator.calculate_slices(3000, Some(client_range)).unwrap();
        
        // Should clamp to file size - 1
        assert_eq!(slices.last().unwrap().range.end, 2999);
    }

    #[test]
    fn test_calculate_slices_empty_file() {
        let calculator = SliceCalculator::new(1024);
        let slices = calculator.calculate_slices(0, None).unwrap();
        assert_eq!(slices.len(), 0);
    }

    #[test]
    fn test_calculate_slices_coverage() {
        // Test that all bytes are covered without gaps
        let calculator = SliceCalculator::new(1000);
        let slices = calculator.calculate_slices(5500, None).unwrap();
        
        // Verify first slice starts at 0
        assert_eq!(slices[0].range.start, 0);
        
        // Verify each slice connects to the next
        for i in 0..slices.len() - 1 {
            assert_eq!(slices[i].range.end + 1, slices[i + 1].range.start);
        }
        
        // Verify last slice ends at file_size - 1
        assert_eq!(slices.last().unwrap().range.end, 5499);
    }

    #[test]
    fn test_calculate_slices_no_overlap() {
        // Test that slices don't overlap
        let calculator = SliceCalculator::new(1000);
        let slices = calculator.calculate_slices(5500, None).unwrap();
        
        for i in 0..slices.len() - 1 {
            for j in i + 1..slices.len() {
                // Verify no overlap: either slice i ends before slice j starts
                // or slice j ends before slice i starts
                assert!(
                    slices[i].range.end < slices[j].range.start ||
                    slices[j].range.end < slices[i].range.start
                );
            }
        }
    }
}
