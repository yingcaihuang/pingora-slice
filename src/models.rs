//! Core data models for the Pingora Slice module

use crate::error::{Result, SliceError};
use serde::{Deserialize, Serialize};

/// Represents a byte range for HTTP Range requests
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ByteRange {
    /// Starting byte position (inclusive)
    pub start: u64,
    /// Ending byte position (inclusive)
    pub end: u64,
}

impl ByteRange {
    /// Create a new ByteRange
    ///
    /// # Arguments
    /// * `start` - Starting byte position (inclusive)
    /// * `end` - Ending byte position (inclusive)
    ///
    /// # Returns
    /// * `Ok(ByteRange)` if the range is valid
    /// * `Err(SliceError)` if start > end
    pub fn new(start: u64, end: u64) -> Result<Self> {
        if start > end {
            return Err(SliceError::InvalidRange(format!(
                "start ({}) must be <= end ({})",
                start, end
            )));
        }
        Ok(ByteRange { start, end })
    }

    /// Get the size of this byte range in bytes
    pub fn size(&self) -> u64 {
        self.end - self.start + 1
    }

    /// Check if this byte range is valid
    pub fn is_valid(&self) -> bool {
        self.start <= self.end
    }

    /// Parse a ByteRange from an HTTP Range header value
    ///
    /// # Arguments
    /// * `header` - The Range header value (e.g., "bytes=0-1023")
    ///
    /// # Returns
    /// * `Ok(ByteRange)` if parsing succeeds
    /// * `Err(SliceError)` if the header format is invalid
    pub fn from_header(header: &str) -> Result<Self> {
        // Expected format: "bytes=start-end"
        let header = header.trim();
        
        if !header.starts_with("bytes=") {
            return Err(SliceError::ParseError(format!(
                "Range header must start with 'bytes=', got: {}",
                header
            )));
        }

        let range_part = &header[6..]; // Skip "bytes="
        let parts: Vec<&str> = range_part.split('-').collect();

        if parts.len() != 2 {
            return Err(SliceError::ParseError(format!(
                "Invalid range format, expected 'start-end', got: {}",
                range_part
            )));
        }

        let start = parts[0].trim().parse::<u64>().map_err(|e| {
            SliceError::ParseError(format!("Invalid start value: {}", e))
        })?;

        let end = parts[1].trim().parse::<u64>().map_err(|e| {
            SliceError::ParseError(format!("Invalid end value: {}", e))
        })?;

        ByteRange::new(start, end)
    }

    /// Convert this ByteRange to an HTTP Range header value
    ///
    /// # Returns
    /// A string in the format "bytes=start-end"
    pub fn to_header(&self) -> String {
        format!("bytes={}-{}", self.start, self.end)
    }
}

/// Specification for a single slice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SliceSpec {
    /// Index of this slice in the sequence
    pub index: usize,
    /// Byte range for this slice
    pub range: ByteRange,
    /// Whether this slice is already cached
    #[serde(default)]
    pub cached: bool,
}

impl SliceSpec {
    /// Create a new SliceSpec
    pub fn new(index: usize, range: ByteRange) -> Self {
        SliceSpec {
            index,
            range,
            cached: false,
        }
    }
}

/// Metadata about a file from the origin server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    /// Total size of the file in bytes
    pub content_length: u64,
    /// Whether the origin server supports Range requests
    pub supports_range: bool,
    /// Content type of the file
    pub content_type: Option<String>,
    /// ETag for cache validation
    pub etag: Option<String>,
    /// Last modified timestamp
    pub last_modified: Option<String>,
}

impl FileMetadata {
    /// Create a new FileMetadata
    pub fn new(content_length: u64, supports_range: bool) -> Self {
        FileMetadata {
            content_length,
            supports_range,
            content_type: None,
            etag: None,
            last_modified: None,
        }
    }

    /// Create a FileMetadata with all fields
    pub fn with_headers(
        content_length: u64,
        supports_range: bool,
        content_type: Option<String>,
        etag: Option<String>,
        last_modified: Option<String>,
    ) -> Self {
        FileMetadata {
            content_length,
            supports_range,
            content_type,
            etag,
            last_modified,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_byte_range_new() {
        let range = ByteRange::new(0, 1023).unwrap();
        assert_eq!(range.start, 0);
        assert_eq!(range.end, 1023);
        assert_eq!(range.size(), 1024);
    }

    #[test]
    fn test_byte_range_invalid() {
        let result = ByteRange::new(100, 50);
        assert!(result.is_err());
    }

    #[test]
    fn test_byte_range_from_header() {
        let range = ByteRange::from_header("bytes=0-1023").unwrap();
        assert_eq!(range.start, 0);
        assert_eq!(range.end, 1023);
    }

    #[test]
    fn test_byte_range_to_header() {
        let range = ByteRange::new(0, 1023).unwrap();
        assert_eq!(range.to_header(), "bytes=0-1023");
    }

    #[test]
    fn test_slice_spec_new() {
        let range = ByteRange::new(0, 1023).unwrap();
        let spec = SliceSpec::new(0, range);
        assert_eq!(spec.index, 0);
        assert_eq!(spec.range.start, 0);
        assert_eq!(spec.range.end, 1023);
        assert!(!spec.cached);
    }

    #[test]
    fn test_file_metadata_new() {
        let metadata = FileMetadata::new(1024000, true);
        assert_eq!(metadata.content_length, 1024000);
        assert!(metadata.supports_range);
        assert!(metadata.content_type.is_none());
    }
}
