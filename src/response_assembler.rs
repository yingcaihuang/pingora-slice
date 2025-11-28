//! Response assembler for streaming slices to the client

use crate::error::{Result, SliceError};
use crate::models::{ByteRange, FileMetadata};
use crate::subrequest_manager::SubrequestResult;
use bytes::Bytes;
use http::{HeaderMap, HeaderValue, StatusCode};
use std::collections::BTreeMap;
use tracing::debug;

/// Response assembler that handles streaming slices to the client
pub struct ResponseAssembler;

impl ResponseAssembler {
    /// Create a new ResponseAssembler
    pub fn new() -> Self {
        ResponseAssembler
    }

    /// Create a new ResponseAssembler (alias for new)
    ///
    /// # Arguments
    /// * `_total_size` - Total size of the file in bytes (kept for API compatibility)
    #[allow(dead_code)]
    pub fn with_size(_total_size: u64) -> Self {
        ResponseAssembler
    }

    /// Build response headers for the client
    ///
    /// # Arguments
    /// * `metadata` - File metadata from the origin server
    /// * `client_range` - Optional byte range requested by the client
    ///
    /// # Returns
    /// A tuple of (StatusCode, HeaderMap) representing the response headers
    pub fn build_response_header(
        &self,
        metadata: &FileMetadata,
        client_range: Option<ByteRange>,
    ) -> Result<(StatusCode, HeaderMap)> {
        debug!(
            "Building response headers: file_size={}, client_range={:?}",
            metadata.content_length, client_range
        );
        
        let mut headers = HeaderMap::new();

        // Determine status code based on whether this is a range request
        let status = if client_range.is_some() {
            StatusCode::PARTIAL_CONTENT // 206
        } else {
            StatusCode::OK // 200
        };

        // Set Content-Length and Content-Range headers
        if let Some(range) = client_range {
            // Validate that the range is within the file bounds
            if range.end >= metadata.content_length {
                return Err(SliceError::InvalidRange(format!(
                    "Range end {} exceeds file size {}",
                    range.end, metadata.content_length
                )));
            }

            let content_length = range.size();
            headers.insert(
                "content-length",
                HeaderValue::from_str(&content_length.to_string())
                    .map_err(|e| SliceError::AssemblyError(format!("Invalid header value: {}", e)))?,
            );

            let content_range = format!(
                "bytes {}-{}/{}",
                range.start, range.end, metadata.content_length
            );
            headers.insert(
                "content-range",
                HeaderValue::from_str(&content_range)
                    .map_err(|e| SliceError::AssemblyError(format!("Invalid header value: {}", e)))?,
            );
        } else {
            // Full file response
            headers.insert(
                "content-length",
                HeaderValue::from_str(&metadata.content_length.to_string())
                    .map_err(|e| SliceError::AssemblyError(format!("Invalid header value: {}", e)))?,
            );
        }

        // Set Content-Type if available
        if let Some(content_type) = &metadata.content_type {
            headers.insert(
                "content-type",
                HeaderValue::from_str(content_type)
                    .map_err(|e| SliceError::AssemblyError(format!("Invalid header value: {}", e)))?,
            );
        }

        // Set Accept-Ranges to indicate we support range requests
        headers.insert(
            "accept-ranges",
            HeaderValue::from_static("bytes"),
        );

        // Set ETag if available
        if let Some(etag) = &metadata.etag {
            headers.insert(
                "etag",
                HeaderValue::from_str(etag)
                    .map_err(|e| SliceError::AssemblyError(format!("Invalid header value: {}", e)))?,
            );
        }

        // Set Last-Modified if available
        if let Some(last_modified) = &metadata.last_modified {
            headers.insert(
                "last-modified",
                HeaderValue::from_str(last_modified)
                    .map_err(|e| SliceError::AssemblyError(format!("Invalid header value: {}", e)))?,
            );
        }

        debug!(
            "Built response headers: status={}, content_length={:?}",
            status,
            headers.get("content-length").and_then(|v| v.to_str().ok())
        );

        Ok((status, headers))
    }

    /// Assemble slices into ordered data ready for streaming
    ///
    /// This method takes a vector of SubrequestResults (which may be out of order)
    /// and organizes them by slice index using a BTreeMap to ensure correct ordering.
    ///
    /// # Arguments
    /// * `slice_results` - Vector of subrequest results (may be out of order)
    ///
    /// # Returns
    /// A BTreeMap with slice indices as keys and data as values, ensuring ordered iteration
    pub fn assemble_slices(&self, slice_results: Vec<SubrequestResult>) -> BTreeMap<usize, Bytes> {
        debug!("Assembling {} slices into ordered map", slice_results.len());
        
        let mut slices = BTreeMap::new();

        for result in slice_results {
            debug!(
                "Adding slice {} to assembly (size={} bytes)",
                result.slice_index,
                result.data.len()
            );
            slices.insert(result.slice_index, result.data);
        }

        debug!("Assembly complete: {} slices in order", slices.len());
        slices
    }

    /// Stream assembled slices in order
    ///
    /// This method takes the assembled slices and returns them as a vector in order.
    /// The BTreeMap ensures that iteration happens in ascending key order.
    ///
    /// # Arguments
    /// * `assembled_slices` - BTreeMap of slice index to data
    ///
    /// # Returns
    /// A vector of Bytes in the correct order for streaming
    pub fn stream_slices(&self, assembled_slices: BTreeMap<usize, Bytes>) -> Vec<Bytes> {
        let slice_count = assembled_slices.len();
        let total_bytes: usize = assembled_slices.values().map(|b| b.len()).sum();
        
        debug!(
            "Streaming {} slices in order (total {} bytes)",
            slice_count, total_bytes
        );
        
        assembled_slices.into_values().collect()
    }

    /// Validate that all expected slices are present
    ///
    /// # Arguments
    /// * `assembled_slices` - BTreeMap of slice index to data
    /// * `expected_count` - Expected number of slices
    ///
    /// # Returns
    /// * `Ok(())` if all slices are present
    /// * `Err(SliceError)` if any slices are missing
    pub fn validate_completeness(
        &self,
        assembled_slices: &BTreeMap<usize, Bytes>,
        expected_count: usize,
    ) -> Result<()> {
        debug!(
            "Validating slice completeness: expected={}, actual={}",
            expected_count,
            assembled_slices.len()
        );
        
        if assembled_slices.len() != expected_count {
            debug!(
                "Slice count mismatch: expected {}, got {}",
                expected_count,
                assembled_slices.len()
            );
            return Err(SliceError::AssemblyError(format!(
                "Missing slices: expected {}, got {}",
                expected_count,
                assembled_slices.len()
            )));
        }

        // Verify that indices are contiguous from 0 to expected_count-1
        for i in 0..expected_count {
            if !assembled_slices.contains_key(&i) {
                debug!("Missing slice at index {}", i);
                return Err(SliceError::AssemblyError(format!(
                    "Missing slice at index {}",
                    i
                )));
            }
        }

        debug!("Slice completeness validation passed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_metadata() -> FileMetadata {
        FileMetadata::with_headers(
            10240,
            true,
            Some("application/octet-stream".to_string()),
            Some("\"abc123\"".to_string()),
            Some("Wed, 21 Oct 2015 07:28:00 GMT".to_string()),
        )
    }

    #[test]
    fn test_new() {
        let _assembler = ResponseAssembler::new();
        // Just verify it can be created
    }

    #[test]
    fn test_build_response_header_full_file() {
        let assembler = ResponseAssembler::new();
        let metadata = create_test_metadata();

        let result = assembler.build_response_header(&metadata, None);
        assert!(result.is_ok());

        let (status, headers) = result.unwrap();
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers.get("content-length").unwrap(), "10240");
        assert_eq!(headers.get("content-type").unwrap(), "application/octet-stream");
        assert_eq!(headers.get("accept-ranges").unwrap(), "bytes");
        assert_eq!(headers.get("etag").unwrap(), "\"abc123\"");
        assert!(headers.get("content-range").is_none());
    }

    #[test]
    fn test_build_response_header_range_request() {
        let assembler = ResponseAssembler::new();
        let metadata = create_test_metadata();
        let range = ByteRange::new(0, 1023).unwrap();

        let result = assembler.build_response_header(&metadata, Some(range));
        assert!(result.is_ok());

        let (status, headers) = result.unwrap();
        assert_eq!(status, StatusCode::PARTIAL_CONTENT);
        assert_eq!(headers.get("content-length").unwrap(), "1024");
        assert_eq!(headers.get("content-range").unwrap(), "bytes 0-1023/10240");
        assert_eq!(headers.get("accept-ranges").unwrap(), "bytes");
    }

    #[test]
    fn test_build_response_header_invalid_range() {
        let assembler = ResponseAssembler::new();
        let metadata = create_test_metadata();
        let range = ByteRange::new(10240, 20000).unwrap();

        let result = assembler.build_response_header(&metadata, Some(range));
        assert!(result.is_err());
    }

    #[test]
    fn test_assemble_slices_ordered() {
        let assembler = ResponseAssembler::new();

        let results = vec![
            SubrequestResult {
                slice_index: 0,
                data: Bytes::from("slice0"),
                status: 206,
                headers: HeaderMap::new(),
            },
            SubrequestResult {
                slice_index: 1,
                data: Bytes::from("slice1"),
                status: 206,
                headers: HeaderMap::new(),
            },
            SubrequestResult {
                slice_index: 2,
                data: Bytes::from("slice2"),
                status: 206,
                headers: HeaderMap::new(),
            },
        ];

        let assembled = assembler.assemble_slices(results);
        assert_eq!(assembled.len(), 3);
        assert_eq!(assembled.get(&0).unwrap(), &Bytes::from("slice0"));
        assert_eq!(assembled.get(&1).unwrap(), &Bytes::from("slice1"));
        assert_eq!(assembled.get(&2).unwrap(), &Bytes::from("slice2"));
    }

    #[test]
    fn test_assemble_slices_out_of_order() {
        let assembler = ResponseAssembler::new();

        // Slices arrive out of order
        let results = vec![
            SubrequestResult {
                slice_index: 2,
                data: Bytes::from("slice2"),
                status: 206,
                headers: HeaderMap::new(),
            },
            SubrequestResult {
                slice_index: 0,
                data: Bytes::from("slice0"),
                status: 206,
                headers: HeaderMap::new(),
            },
            SubrequestResult {
                slice_index: 1,
                data: Bytes::from("slice1"),
                status: 206,
                headers: HeaderMap::new(),
            },
        ];

        let assembled = assembler.assemble_slices(results);
        
        // BTreeMap ensures correct ordering
        let ordered: Vec<_> = assembled.values().collect();
        assert_eq!(ordered[0], &Bytes::from("slice0"));
        assert_eq!(ordered[1], &Bytes::from("slice1"));
        assert_eq!(ordered[2], &Bytes::from("slice2"));
    }

    #[test]
    fn test_stream_slices() {
        let assembler = ResponseAssembler::new();

        let mut slices = BTreeMap::new();
        slices.insert(0, Bytes::from("slice0"));
        slices.insert(1, Bytes::from("slice1"));
        slices.insert(2, Bytes::from("slice2"));

        let streamed = assembler.stream_slices(slices);
        assert_eq!(streamed.len(), 3);
        assert_eq!(streamed[0], Bytes::from("slice0"));
        assert_eq!(streamed[1], Bytes::from("slice1"));
        assert_eq!(streamed[2], Bytes::from("slice2"));
    }

    #[test]
    fn test_validate_completeness_success() {
        let assembler = ResponseAssembler::new();

        let mut slices = BTreeMap::new();
        slices.insert(0, Bytes::from("slice0"));
        slices.insert(1, Bytes::from("slice1"));
        slices.insert(2, Bytes::from("slice2"));

        let result = assembler.validate_completeness(&slices, 3);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_completeness_missing_slice() {
        let assembler = ResponseAssembler::new();

        let mut slices = BTreeMap::new();
        slices.insert(0, Bytes::from("slice0"));
        slices.insert(2, Bytes::from("slice2"));

        let result = assembler.validate_completeness(&slices, 3);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_completeness_wrong_count() {
        let assembler = ResponseAssembler::new();

        let mut slices = BTreeMap::new();
        slices.insert(0, Bytes::from("slice0"));
        slices.insert(1, Bytes::from("slice1"));

        let result = assembler.validate_completeness(&slices, 3);
        assert!(result.is_err());
    }
}
