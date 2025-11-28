//! Subrequest manager for fetching slices from origin server

use crate::error::{Result, SliceError};
use crate::models::{ByteRange, SliceSpec};
use bytes::Bytes;
use http::HeaderMap;
use reqwest::Client;
use std::time::Duration;
use tokio::time::sleep;

/// Result of a subrequest for a single slice
#[derive(Debug, Clone)]
pub struct SubrequestResult {
    /// Index of the slice in the sequence
    pub slice_index: usize,
    /// The actual data bytes
    pub data: Bytes,
    /// HTTP status code
    pub status: u16,
    /// Response headers
    pub headers: HeaderMap,
}

/// Retry policy for failed subrequests
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retries
    pub max_retries: usize,
    /// Backoff durations in milliseconds for each retry attempt
    pub backoff_ms: Vec<u64>,
}

impl RetryPolicy {
    /// Create a new retry policy with exponential backoff
    pub fn new(max_retries: usize) -> Self {
        // Generate exponential backoff: 100ms, 200ms, 400ms, 800ms, ...
        let backoff_ms = (0..max_retries)
            .map(|i| 100 * 2u64.pow(i as u32))
            .collect();

        RetryPolicy {
            max_retries,
            backoff_ms,
        }
    }

    /// Check if we should retry based on the attempt number and error
    pub fn should_retry(&self, attempt: usize, error: &SliceError) -> bool {
        attempt < self.max_retries && error.should_retry()
    }

    /// Get the backoff duration for a given attempt
    pub fn backoff_duration(&self, attempt: usize) -> Duration {
        let ms = self
            .backoff_ms
            .get(attempt)
            .copied()
            .unwrap_or_else(|| *self.backoff_ms.last().unwrap_or(&1000));
        Duration::from_millis(ms)
    }
}

/// Manager for handling subrequests to fetch slices
pub struct SubrequestManager {
    /// HTTP client for making requests
    http_client: Client,
    /// Maximum number of concurrent subrequests
    max_concurrent: usize,
    /// Retry policy
    retry_policy: RetryPolicy,
}

impl SubrequestManager {
    /// Create a new SubrequestManager
    ///
    /// # Arguments
    /// * `max_concurrent` - Maximum number of concurrent subrequests
    /// * `max_retries` - Maximum number of retry attempts for failed requests
    pub fn new(max_concurrent: usize, max_retries: usize) -> Self {
        // Optimized HTTP client configuration for better performance
        let http_client = Client::builder()
            .timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(10)  // Connection pooling for reuse
            .pool_idle_timeout(Duration::from_secs(90))  // Keep connections alive
            .tcp_nodelay(true)  // Disable Nagle's algorithm for lower latency
            .http2_adaptive_window(true)  // Adaptive flow control for HTTP/2
            .build()
            .expect("Failed to create HTTP client");

        SubrequestManager {
            http_client,
            max_concurrent,
            retry_policy: RetryPolicy::new(max_retries),
        }
    }

    /// Build a Range request for a specific byte range
    ///
    /// # Arguments
    /// * `url` - The URL to request
    /// * `range` - The byte range to request
    ///
    /// # Returns
    /// A reqwest::RequestBuilder configured with the Range header
    fn build_range_request(&self, url: &str, range: &ByteRange) -> reqwest::RequestBuilder {
        let range_header = range.to_header();
        
        self.http_client
            .get(url)
            .header("Range", range_header)
    }

    /// Try to fetch a single slice (single attempt, no retry)
    ///
    /// # Arguments
    /// * `slice` - The slice specification
    /// * `url` - The URL to fetch from
    ///
    /// # Returns
    /// * `Ok(SubrequestResult)` if the request succeeds
    /// * `Err(SliceError)` if the request fails
    async fn try_fetch_slice(&self, slice: &SliceSpec, url: &str) -> Result<SubrequestResult> {
        let request = self.build_range_request(url, &slice.range);
        
        let response = request
            .send()
            .await
            .map_err(|e| SliceError::HttpError(format!("Request failed: {}", e)))?;

        let status = response.status().as_u16();
        let headers = response.headers().clone();

        // Validate status code - we expect 206 Partial Content
        if status != 206 {
            return Err(SliceError::HttpError(format!(
                "Expected status 206, got {}",
                status
            )));
        }

        // Validate Content-Range header
        if let Some(content_range) = headers.get("content-range") {
            let content_range_str = content_range
                .to_str()
                .map_err(|e| SliceError::ParseError(format!("Invalid Content-Range header: {}", e)))?;
            
            // Parse Content-Range header (format: "bytes start-end/total")
            if !Self::validate_content_range(content_range_str, &slice.range)? {
                return Err(SliceError::HttpError(format!(
                    "Content-Range mismatch: expected {}-{}, got {}",
                    slice.range.start, slice.range.end, content_range_str
                )));
            }
        } else {
            return Err(SliceError::HttpError(
                "Missing Content-Range header in 206 response".to_string()
            ));
        }

        // Read the response body
        let data = response
            .bytes()
            .await
            .map_err(|e| SliceError::HttpError(format!("Failed to read response body: {}", e)))?;

        Ok(SubrequestResult {
            slice_index: slice.index,
            data,
            status,
            headers,
        })
    }

    /// Validate that the Content-Range header matches the requested range
    ///
    /// # Arguments
    /// * `content_range` - The Content-Range header value (e.g., "bytes 0-1023/10240")
    /// * `expected_range` - The expected byte range
    ///
    /// # Returns
    /// * `Ok(true)` if the range matches
    /// * `Ok(false)` if the range doesn't match
    /// * `Err(SliceError)` if parsing fails
    fn validate_content_range(content_range: &str, expected_range: &ByteRange) -> Result<bool> {
        // Expected format: "bytes start-end/total"
        let content_range = content_range.trim();
        
        if !content_range.starts_with("bytes ") {
            return Err(SliceError::ParseError(format!(
                "Content-Range must start with 'bytes ', got: {}",
                content_range
            )));
        }

        let range_part = &content_range[6..]; // Skip "bytes "
        let parts: Vec<&str> = range_part.split('/').collect();

        if parts.len() != 2 {
            return Err(SliceError::ParseError(format!(
                "Invalid Content-Range format, expected 'start-end/total', got: {}",
                range_part
            )));
        }

        let range_str = parts[0];
        let range_parts: Vec<&str> = range_str.split('-').collect();

        if range_parts.len() != 2 {
            return Err(SliceError::ParseError(format!(
                "Invalid range format in Content-Range: {}",
                range_str
            )));
        }

        let start = range_parts[0]
            .trim()
            .parse::<u64>()
            .map_err(|e| SliceError::ParseError(format!("Invalid start value: {}", e)))?;

        let end = range_parts[1]
            .trim()
            .parse::<u64>()
            .map_err(|e| SliceError::ParseError(format!("Invalid end value: {}", e)))?;

        Ok(start == expected_range.start && end == expected_range.end)
    }

    /// Fetch a single slice with retry logic
    ///
    /// # Arguments
    /// * `slice` - The slice specification
    /// * `url` - The URL to fetch from
    ///
    /// # Returns
    /// * `Ok(SubrequestResult)` if the request succeeds (possibly after retries)
    /// * `Err(SliceError)` if all retry attempts fail
    pub async fn fetch_single_slice(&self, slice: &SliceSpec, url: &str) -> Result<SubrequestResult> {
        let mut attempt = 0;

        loop {
            match self.try_fetch_slice(slice, url).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    if !self.retry_policy.should_retry(attempt, &e) {
                        // All retries exhausted, return the final error
                        return Err(SliceError::SubrequestFailed {
                            slice_index: slice.index,
                            attempts: attempt + 1,
                        });
                    }

                    // Wait before retrying
                    let backoff = self.retry_policy.backoff_duration(attempt);
                    tracing::warn!(
                        "Subrequest failed for slice {} (attempt {}), retrying after {:?}: {}",
                        slice.index,
                        attempt + 1,
                        backoff,
                        e
                    );
                    sleep(backoff).await;
                    
                    attempt += 1;
                }
            }
        }
    }

    /// Fetch multiple slices concurrently
    ///
    /// # Arguments
    /// * `slices` - Vector of slice specifications to fetch
    /// * `url` - The URL to fetch from
    ///
    /// # Returns
    /// * `Ok(Vec<SubrequestResult>)` if all slices are fetched successfully
    /// * `Err(SliceError)` if any slice fails after all retries
    pub async fn fetch_slices(&self, slices: Vec<SliceSpec>, url: &str) -> Result<Vec<SubrequestResult>> {
        use tokio::sync::Semaphore;
        use std::sync::Arc;

        let semaphore = Arc::new(Semaphore::new(self.max_concurrent));
        let mut tasks = Vec::new();

        for slice in slices {
            let sem = semaphore.clone();
            let url = url.to_string();
            let manager = self.clone_for_task();

            let task = tokio::spawn(async move {
                // Acquire semaphore permit to limit concurrency
                let _permit = sem.acquire().await.expect("Semaphore closed");
                
                manager.fetch_single_slice(&slice, &url).await
            });

            tasks.push(task);
        }

        // Wait for all tasks to complete
        let mut results = Vec::new();
        for task in tasks {
            let result = task
                .await
                .map_err(|e| SliceError::HttpError(format!("Task join error: {}", e)))??;
            results.push(result);
        }

        // Sort results by slice index to maintain order
        results.sort_by_key(|r| r.slice_index);

        Ok(results)
    }

    /// Clone the necessary fields for use in async tasks
    fn clone_for_task(&self) -> Self {
        SubrequestManager {
            http_client: self.http_client.clone(),
            max_concurrent: self.max_concurrent,
            retry_policy: self.retry_policy.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_policy_new() {
        let policy = RetryPolicy::new(3);
        assert_eq!(policy.max_retries, 3);
        assert_eq!(policy.backoff_ms.len(), 3);
        assert_eq!(policy.backoff_ms[0], 100);
        assert_eq!(policy.backoff_ms[1], 200);
        assert_eq!(policy.backoff_ms[2], 400);
    }

    #[test]
    fn test_retry_policy_should_retry() {
        let policy = RetryPolicy::new(3);
        let error = SliceError::HttpError("test".to_string());
        
        assert!(policy.should_retry(0, &error));
        assert!(policy.should_retry(1, &error));
        assert!(policy.should_retry(2, &error));
        assert!(!policy.should_retry(3, &error));
    }

    #[test]
    fn test_retry_policy_backoff_duration() {
        let policy = RetryPolicy::new(3);
        
        assert_eq!(policy.backoff_duration(0), Duration::from_millis(100));
        assert_eq!(policy.backoff_duration(1), Duration::from_millis(200));
        assert_eq!(policy.backoff_duration(2), Duration::from_millis(400));
        assert_eq!(policy.backoff_duration(10), Duration::from_millis(400)); // Uses last value
    }

    #[test]
    fn test_validate_content_range_valid() {
        let range = ByteRange::new(0, 1023).unwrap();
        let result = SubrequestManager::validate_content_range("bytes 0-1023/10240", &range);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_validate_content_range_mismatch() {
        let range = ByteRange::new(0, 1023).unwrap();
        let result = SubrequestManager::validate_content_range("bytes 0-2047/10240", &range);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_validate_content_range_invalid_format() {
        let range = ByteRange::new(0, 1023).unwrap();
        let result = SubrequestManager::validate_content_range("invalid", &range);
        assert!(result.is_err());
    }

    #[test]
    fn test_subrequest_manager_new() {
        let manager = SubrequestManager::new(4, 3);
        assert_eq!(manager.max_concurrent, 4);
        assert_eq!(manager.retry_policy.max_retries, 3);
    }
}
