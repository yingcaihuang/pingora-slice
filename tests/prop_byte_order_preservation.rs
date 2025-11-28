// Feature: pingora-slice, Property 10: 字节顺序保持（关键属性）
// **Validates: Requirements 6.2**
//
// Property: For any set of slices assembled into a response, the bytes in the 
// final response should be in the exact same order as they appear in the original file

use bytes::Bytes;
use http::HeaderMap;
use pingora_slice::models::ByteRange;
use pingora_slice::response_assembler::ResponseAssembler;
use pingora_slice::slice_calculator::SliceCalculator;
use pingora_slice::subrequest_manager::SubrequestResult;
use proptest::prelude::*;
use proptest::collection::vec as prop_vec;
use proptest::strategy::ValueTree;

/// Generate a random file content as a vector of bytes
fn file_content_strategy(max_size: usize) -> impl Strategy<Value = Vec<u8>> {
    prop_vec(any::<u8>(), 1..=max_size)
}

/// Generate a permutation of indices to simulate out-of-order arrival
fn permutation_strategy(size: usize) -> impl Strategy<Value = Vec<usize>> {
    Just((0..size).collect::<Vec<_>>()).prop_shuffle()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property 10: Byte order preservation
    /// 
    /// For any file content and slice size, when slices are assembled
    /// (potentially arriving out of order), the final byte sequence should
    /// match the original file's byte order exactly.
    #[test]
    fn prop_byte_order_preservation(
        file_content in file_content_strategy(100_000),
        slice_size in 1usize..=10_000usize,
    ) {
        let file_size = file_content.len() as u64;
        
        // Calculate slices for the file
        let calculator = SliceCalculator::new(slice_size);
        let slices = calculator.calculate_slices(file_size, None)
            .expect("Slice calculation should succeed");
        
        // Create SubrequestResults from the file content
        let mut subrequest_results = Vec::new();
        for slice_spec in &slices {
            let start = slice_spec.range.start as usize;
            let end = (slice_spec.range.end + 1) as usize; // end is inclusive, so +1 for slice
            let slice_data = &file_content[start..end];
            
            subrequest_results.push(SubrequestResult {
                slice_index: slice_spec.index,
                data: Bytes::copy_from_slice(slice_data),
                status: 206,
                headers: HeaderMap::new(),
            });
        }
        
        // Shuffle the results to simulate out-of-order arrival
        let permutation = permutation_strategy(subrequest_results.len())
            .new_tree(&mut Default::default())
            .unwrap()
            .current();
        
        let mut shuffled_results = Vec::new();
        for &idx in &permutation {
            shuffled_results.push(subrequest_results[idx].clone());
        }
        
        // Assemble the slices
        let assembler = ResponseAssembler::new();
        let assembled = assembler.assemble_slices(shuffled_results);
        let streamed = assembler.stream_slices(assembled);
        
        // Concatenate all streamed bytes
        let mut final_bytes = Vec::new();
        for chunk in streamed {
            final_bytes.extend_from_slice(&chunk);
        }
        
        // Property: The final bytes should match the original file content exactly
        prop_assert_eq!(
            final_bytes,
            file_content,
            "Assembled bytes do not match original file content"
        );
    }

    /// Property 10 (with client range): Byte order preservation for partial requests
    /// 
    /// When a client requests a specific range, the assembled bytes should match
    /// the corresponding portion of the original file in the correct order.
    #[test]
    fn prop_byte_order_preservation_with_client_range(
        file_content in file_content_strategy(100_000),
        slice_size in 1usize..=10_000usize,
        range_start_ratio in 0.0f64..=0.7f64,
        range_length_ratio in 0.1f64..=0.5f64,
    ) {
        let file_size = file_content.len() as u64;
        
        if file_size == 0 {
            return Ok(());
        }
        
        // Calculate client range based on ratios
        let range_start = (file_size as f64 * range_start_ratio) as u64;
        let range_length = ((file_size - range_start) as f64 * range_length_ratio).max(1.0) as u64;
        let range_end = std::cmp::min(range_start + range_length - 1, file_size - 1);
        
        if range_start > range_end {
            return Ok(());
        }
        
        let client_range = ByteRange::new(range_start, range_end)
            .expect("Range should be valid");
        
        // Calculate slices for the client range
        let calculator = SliceCalculator::new(slice_size);
        let slices = calculator.calculate_slices(file_size, Some(client_range))
            .expect("Slice calculation should succeed");
        
        // Create SubrequestResults from the file content
        let mut subrequest_results = Vec::new();
        for slice_spec in &slices {
            let start = slice_spec.range.start as usize;
            let end = (slice_spec.range.end + 1) as usize;
            let slice_data = &file_content[start..end];
            
            subrequest_results.push(SubrequestResult {
                slice_index: slice_spec.index,
                data: Bytes::copy_from_slice(slice_data),
                status: 206,
                headers: HeaderMap::new(),
            });
        }
        
        // Shuffle the results to simulate out-of-order arrival
        let permutation = permutation_strategy(subrequest_results.len())
            .new_tree(&mut Default::default())
            .unwrap()
            .current();
        
        let mut shuffled_results = Vec::new();
        for &idx in &permutation {
            shuffled_results.push(subrequest_results[idx].clone());
        }
        
        // Assemble the slices
        let assembler = ResponseAssembler::new();
        let assembled = assembler.assemble_slices(shuffled_results);
        let streamed = assembler.stream_slices(assembled);
        
        // Concatenate all streamed bytes
        let mut final_bytes = Vec::new();
        for chunk in streamed {
            final_bytes.extend_from_slice(&chunk);
        }
        
        // Extract the expected bytes from the original file
        let expected_bytes = &file_content[range_start as usize..=range_end as usize];
        
        // Property: The final bytes should match the requested range exactly
        prop_assert_eq!(
            final_bytes,
            expected_bytes,
            "Assembled bytes for range request do not match original file content"
        );
    }

    /// Property 10 (verification): Byte-by-byte order check
    /// 
    /// For smaller files, verify that each byte position in the assembled
    /// output matches the corresponding byte in the original file.
    #[test]
    fn prop_byte_by_byte_order_verification(
        file_content in file_content_strategy(10_000),
        slice_size in 1usize..=5_000usize,
    ) {
        let file_size = file_content.len() as u64;
        
        // Calculate slices
        let calculator = SliceCalculator::new(slice_size);
        let slices = calculator.calculate_slices(file_size, None)
            .expect("Slice calculation should succeed");
        
        // Create SubrequestResults
        let mut subrequest_results = Vec::new();
        for slice_spec in &slices {
            let start = slice_spec.range.start as usize;
            let end = (slice_spec.range.end + 1) as usize;
            let slice_data = &file_content[start..end];
            
            subrequest_results.push(SubrequestResult {
                slice_index: slice_spec.index,
                data: Bytes::copy_from_slice(slice_data),
                status: 206,
                headers: HeaderMap::new(),
            });
        }
        
        // Shuffle to simulate out-of-order arrival
        let permutation = permutation_strategy(subrequest_results.len())
            .new_tree(&mut Default::default())
            .unwrap()
            .current();
        
        let mut shuffled_results = Vec::new();
        for &idx in &permutation {
            shuffled_results.push(subrequest_results[idx].clone());
        }
        
        // Assemble and stream
        let assembler = ResponseAssembler::new();
        let assembled = assembler.assemble_slices(shuffled_results);
        let streamed = assembler.stream_slices(assembled);
        
        // Concatenate
        let mut final_bytes = Vec::new();
        for chunk in streamed {
            final_bytes.extend_from_slice(&chunk);
        }
        
        // Verify byte-by-byte
        prop_assert_eq!(
            final_bytes.len(),
            file_content.len(),
            "Final byte count does not match original file size"
        );
        
        for (i, (&actual, &expected)) in final_bytes.iter().zip(file_content.iter()).enumerate() {
            prop_assert_eq!(
                actual,
                expected,
                "Byte mismatch at position {}: expected {}, got {}",
                i,
                expected,
                actual
            );
        }
    }

    /// Property 10 (edge case): Single slice preserves order
    #[test]
    fn prop_single_slice_order_preservation(
        file_content in file_content_strategy(1_000),
        slice_size in 1_001usize..=10_000usize,
    ) {
        let file_size = file_content.len() as u64;
        
        // File smaller than slice size should produce one slice
        let calculator = SliceCalculator::new(slice_size);
        let slices = calculator.calculate_slices(file_size, None)
            .expect("Slice calculation should succeed");
        
        prop_assert_eq!(
            slices.len(),
            1,
            "File smaller than slice size should have exactly one slice"
        );
        
        // Create SubrequestResult
        let subrequest_result = SubrequestResult {
            slice_index: 0,
            data: Bytes::copy_from_slice(&file_content),
            status: 206,
            headers: HeaderMap::new(),
        };
        
        // Assemble
        let assembler = ResponseAssembler::new();
        let assembled = assembler.assemble_slices(vec![subrequest_result]);
        let streamed = assembler.stream_slices(assembled);
        
        // Concatenate
        let mut final_bytes = Vec::new();
        for chunk in streamed {
            final_bytes.extend_from_slice(&chunk);
        }
        
        // Property: Single slice should preserve exact byte order
        prop_assert_eq!(
            final_bytes,
            file_content,
            "Single slice does not preserve byte order"
        );
    }

    /// Property 10 (stress test): Many small slices preserve order
    #[test]
    fn prop_many_small_slices_order_preservation(
        file_content in file_content_strategy(10_000),
        slice_size in 1usize..=100usize,
    ) {
        let file_size = file_content.len() as u64;
        
        // Small slice size will create many slices
        let calculator = SliceCalculator::new(slice_size);
        let slices = calculator.calculate_slices(file_size, None)
            .expect("Slice calculation should succeed");
        
        // Create SubrequestResults
        let mut subrequest_results = Vec::new();
        for slice_spec in &slices {
            let start = slice_spec.range.start as usize;
            let end = (slice_spec.range.end + 1) as usize;
            let slice_data = &file_content[start..end];
            
            subrequest_results.push(SubrequestResult {
                slice_index: slice_spec.index,
                data: Bytes::copy_from_slice(slice_data),
                status: 206,
                headers: HeaderMap::new(),
            });
        }
        
        // Shuffle heavily to test ordering logic
        let permutation = permutation_strategy(subrequest_results.len())
            .new_tree(&mut Default::default())
            .unwrap()
            .current();
        
        let mut shuffled_results = Vec::new();
        for &idx in &permutation {
            shuffled_results.push(subrequest_results[idx].clone());
        }
        
        // Assemble
        let assembler = ResponseAssembler::new();
        let assembled = assembler.assemble_slices(shuffled_results);
        let streamed = assembler.stream_slices(assembled);
        
        // Concatenate
        let mut final_bytes = Vec::new();
        for chunk in streamed {
            final_bytes.extend_from_slice(&chunk);
        }
        
        // Property: Even with many small slices arriving out of order,
        // the final byte sequence should match the original
        prop_assert_eq!(
            final_bytes,
            file_content,
            "Many small slices do not preserve byte order"
        );
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_byte_order_simple_case() {
        // Create a simple file: "ABCDEFGHIJ"
        let file_content = b"ABCDEFGHIJ".to_vec();
        let file_size = file_content.len() as u64;
        
        // Use slice size of 3 bytes
        let calculator = SliceCalculator::new(3);
        let slices = calculator.calculate_slices(file_size, None).unwrap();
        
        // Should create 4 slices: [0-2], [3-5], [6-8], [9-9]
        assert_eq!(slices.len(), 4);
        
        // Create SubrequestResults in order
        let mut results = Vec::new();
        for slice_spec in &slices {
            let start = slice_spec.range.start as usize;
            let end = (slice_spec.range.end + 1) as usize;
            let slice_data = &file_content[start..end];
            
            results.push(SubrequestResult {
                slice_index: slice_spec.index,
                data: Bytes::copy_from_slice(slice_data),
                status: 206,
                headers: HeaderMap::new(),
            });
        }
        
        // Assemble
        let assembler = ResponseAssembler::new();
        let assembled = assembler.assemble_slices(results);
        let streamed = assembler.stream_slices(assembled);
        
        // Concatenate
        let mut final_bytes = Vec::new();
        for chunk in streamed {
            final_bytes.extend_from_slice(&chunk);
        }
        
        assert_eq!(final_bytes, file_content);
        assert_eq!(std::str::from_utf8(&final_bytes).unwrap(), "ABCDEFGHIJ");
    }

    #[test]
    fn test_byte_order_out_of_order_arrival() {
        // Create a simple file: "0123456789"
        let file_content = b"0123456789".to_vec();
        let file_size = file_content.len() as u64;
        
        // Use slice size of 2 bytes
        let calculator = SliceCalculator::new(2);
        let slices = calculator.calculate_slices(file_size, None).unwrap();
        
        // Should create 5 slices
        assert_eq!(slices.len(), 5);
        
        // Create SubrequestResults
        let mut results = Vec::new();
        for slice_spec in &slices {
            let start = slice_spec.range.start as usize;
            let end = (slice_spec.range.end + 1) as usize;
            let slice_data = &file_content[start..end];
            
            results.push(SubrequestResult {
                slice_index: slice_spec.index,
                data: Bytes::copy_from_slice(slice_data),
                status: 206,
                headers: HeaderMap::new(),
            });
        }
        
        // Shuffle: arrive in order [4, 1, 3, 0, 2]
        let shuffled = vec![
            results[4].clone(),
            results[1].clone(),
            results[3].clone(),
            results[0].clone(),
            results[2].clone(),
        ];
        
        // Assemble
        let assembler = ResponseAssembler::new();
        let assembled = assembler.assemble_slices(shuffled);
        let streamed = assembler.stream_slices(assembled);
        
        // Concatenate
        let mut final_bytes = Vec::new();
        for chunk in streamed {
            final_bytes.extend_from_slice(&chunk);
        }
        
        // Should still be in correct order
        assert_eq!(final_bytes, file_content);
        assert_eq!(std::str::from_utf8(&final_bytes).unwrap(), "0123456789");
    }

    #[test]
    fn test_byte_order_with_range_request() {
        // Create a file: "ABCDEFGHIJKLMNOP"
        let file_content = b"ABCDEFGHIJKLMNOP".to_vec();
        let file_size = file_content.len() as u64;
        
        // Request range [4-11] which is "EFGHIJKL"
        let client_range = ByteRange::new(4, 11).unwrap();
        
        // Use slice size of 3 bytes
        let calculator = SliceCalculator::new(3);
        let slices = calculator.calculate_slices(file_size, Some(client_range)).unwrap();
        
        // Create SubrequestResults
        let mut results = Vec::new();
        for slice_spec in &slices {
            let start = slice_spec.range.start as usize;
            let end = (slice_spec.range.end + 1) as usize;
            let slice_data = &file_content[start..end];
            
            results.push(SubrequestResult {
                slice_index: slice_spec.index,
                data: Bytes::copy_from_slice(slice_data),
                status: 206,
                headers: HeaderMap::new(),
            });
        }
        
        // Reverse order to test ordering
        results.reverse();
        
        // Assemble
        let assembler = ResponseAssembler::new();
        let assembled = assembler.assemble_slices(results);
        let streamed = assembler.stream_slices(assembled);
        
        // Concatenate
        let mut final_bytes = Vec::new();
        for chunk in streamed {
            final_bytes.extend_from_slice(&chunk);
        }
        
        // Should match the requested range
        let expected = &file_content[4..=11];
        assert_eq!(final_bytes, expected);
        assert_eq!(std::str::from_utf8(&final_bytes).unwrap(), "EFGHIJKL");
    }

    #[test]
    fn test_byte_order_single_byte_slices() {
        // Create a file: "ABC"
        let file_content = b"ABC".to_vec();
        let file_size = file_content.len() as u64;
        
        // Use slice size of 1 byte
        let calculator = SliceCalculator::new(1);
        let slices = calculator.calculate_slices(file_size, None).unwrap();
        
        assert_eq!(slices.len(), 3);
        
        // Create SubrequestResults in reverse order
        let mut results = Vec::new();
        for slice_spec in slices.iter().rev() {
            let start = slice_spec.range.start as usize;
            let end = (slice_spec.range.end + 1) as usize;
            let slice_data = &file_content[start..end];
            
            results.push(SubrequestResult {
                slice_index: slice_spec.index,
                data: Bytes::copy_from_slice(slice_data),
                status: 206,
                headers: HeaderMap::new(),
            });
        }
        
        // Assemble
        let assembler = ResponseAssembler::new();
        let assembled = assembler.assemble_slices(results);
        let streamed = assembler.stream_slices(assembled);
        
        // Concatenate
        let mut final_bytes = Vec::new();
        for chunk in streamed {
            final_bytes.extend_from_slice(&chunk);
        }
        
        // Should still be "ABC" not "CBA"
        assert_eq!(final_bytes, file_content);
        assert_eq!(std::str::from_utf8(&final_bytes).unwrap(), "ABC");
    }
}
