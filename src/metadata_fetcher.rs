//! Metadata fetcher for retrieving file information from origin servers

use crate::error::{Result, SliceError};
use crate::models::FileMetadata;
use reqwest::Client;
use std::time::Duration;
use tracing::{debug, info, warn};

/// MetadataFetcher is responsible for fetching file metadata from origin servers
/// using HEAD requests
pub struct MetadataFetcher {
    client: Client,
}

impl MetadataFetcher {
    /// Create a new MetadataFetcher with default settings
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| SliceError::HttpError(format!("Failed to create HTTP client: {}", e)))?;
        
        Ok(MetadataFetcher { client })
    }

    /// Create a new MetadataFetcher with a custom timeout
    pub fn with_timeout(timeout: Duration) -> Result<Self> {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| SliceError::HttpError(format!("Failed to create HTTP client: {}", e)))?;
        
        Ok(MetadataFetcher { client })
    }

    /// Fetch metadata for a file from the origin server
    ///
    /// This method sends a HEAD request to the origin server and extracts:
    /// - Content-Length: Total file size
    /// - Accept-Ranges: Whether the server supports Range requests
    /// - Content-Type: MIME type of the file
    /// - ETag: Entity tag for cache validation
    /// - Last-Modified: Last modification timestamp
    ///
    /// # Arguments
    /// * `url` - The URL of the file to fetch metadata for
    ///
    /// # Returns
    /// * `Ok(FileMetadata)` if the request succeeds and required headers are present
    /// * `Err(SliceError)` if the request fails or required headers are missing
    ///
    /// # Requirements
    /// Validates: Requirements 3.1, 3.2, 3.3, 3.4, 3.5
    pub async fn fetch_metadata(&self, url: &str) -> Result<FileMetadata> {
        debug!("Fetching metadata for url={}", url);
        
        // Send HEAD request to origin server (Requirement 3.1)
        let response = self
            .client
            .head(url)
            .send()
            .await
            .map_err(|e| {
                warn!("HEAD request failed for url={}: {}", url, e);
                SliceError::MetadataFetchError(format!("HEAD request failed: {}", e))
            })?;

        let status = response.status();
        debug!("Received HEAD response for url={}, status={}", url, status);

        // Handle 4xx errors - return error to client (Requirement 8.1)
        if status.is_client_error() {
            warn!(
                "Origin returned 4xx error for url={}: status={}",
                url, status
            );
            return Err(SliceError::origin_client_error(
                status.as_u16(),
                format!("Origin server returned client error: {}", status),
            ));
        }

        // Handle 5xx errors - should be retried by caller (Requirement 8.2)
        if status.is_server_error() {
            warn!(
                "Origin returned 5xx error for url={}: status={}",
                url, status
            );
            return Err(SliceError::origin_server_error(
                status.as_u16(),
                format!("Origin server returned server error: {}", status),
            ));
        }

        // Check if response is successful
        if !status.is_success() {
            warn!(
                "Unexpected status code for url={}: status={}",
                url, status
            );
            return Err(SliceError::HttpError(format!(
                "Unexpected status code: {}",
                status
            )));
        }

        let headers = response.headers();

        // Extract Content-Length (Requirement 3.2)
        let content_length = headers
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .ok_or_else(|| {
                warn!(
                    "Content-Length header missing or invalid for url={}",
                    url
                );
                SliceError::MetadataFetchError(
                    "Content-Length header missing or invalid".to_string(),
                )
            })?;

        // Check Accept-Ranges header (Requirement 3.3)
        let supports_range = headers
            .get("accept-ranges")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.eq_ignore_ascii_case("bytes"))
            .unwrap_or(false);
        
        debug!(
            "Parsed metadata for url={}: content_length={}, supports_range={}",
            url, content_length, supports_range
        );

        // Extract optional headers
        let content_type = headers
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_string());

        let etag = headers
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_string());

        let last_modified = headers
            .get("last-modified")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_string());

        info!(
            "Successfully fetched metadata for url={}: size={}, supports_range={}, content_type={:?}",
            url, content_length, supports_range, content_type
        );

        Ok(FileMetadata::with_headers(
            content_length,
            supports_range,
            content_type,
            etag,
            last_modified,
        ))
    }
}

impl Default for MetadataFetcher {
    fn default() -> Self {
        Self::new().expect("Failed to create default MetadataFetcher")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metadata_fetcher_creation() {
        let fetcher = MetadataFetcher::new();
        assert!(fetcher.is_ok());
    }

    #[tokio::test]
    async fn test_metadata_fetcher_with_timeout() {
        let fetcher = MetadataFetcher::with_timeout(Duration::from_secs(5));
        assert!(fetcher.is_ok());
    }
}
