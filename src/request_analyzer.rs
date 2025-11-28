//! Request analysis for determining if slicing should be enabled

use crate::config::SliceConfig;
use crate::models::ByteRange;
use http::{Method, HeaderMap, HeaderValue};
use std::sync::Arc;
use tracing::debug;

/// Analyzes incoming requests to determine if slicing should be applied
pub struct RequestAnalyzer {
    config: Arc<SliceConfig>,
}

impl RequestAnalyzer {
    /// Create a new RequestAnalyzer with the given configuration
    pub fn new(config: Arc<SliceConfig>) -> Self {
        RequestAnalyzer { config }
    }

    /// Determine if slicing should be enabled for this request
    ///
    /// # Arguments
    /// * `method` - HTTP method of the request
    /// * `uri` - Request URI
    /// * `headers` - Request headers
    ///
    /// # Returns
    /// `true` if slicing should be enabled, `false` otherwise
    ///
    /// # Logic
    /// Slicing is enabled when:
    /// 1. Request method is GET
    /// 2. Request does NOT already contain a Range header
    /// 3. URL matches one of the configured slice patterns (or patterns list is empty)
    pub fn should_slice(&self, method: &Method, uri: &str, headers: &HeaderMap<HeaderValue>) -> bool {
        // Check 1: Must be GET request
        if method != Method::GET {
            debug!(
                "Slicing not applicable: non-GET method={} for uri={}",
                method, uri
            );
            return false;
        }

        // Check 2: Must NOT have Range header (pass through Range requests)
        if headers.contains_key("range") || headers.contains_key("Range") {
            debug!(
                "Slicing not applicable: Range header present for uri={}",
                uri
            );
            return false;
        }

        // Check 3: URL must match configured patterns
        // If no patterns configured, slice all requests
        if self.config.slice_patterns.is_empty() {
            debug!("Slicing enabled: no patterns configured, slicing all GET requests for uri={}", uri);
            return true;
        }

        // Check if URI matches any of the configured patterns
        let matches = self.matches_pattern(uri);
        if matches {
            debug!("Slicing enabled: uri={} matches configured patterns", uri);
        } else {
            debug!("Slicing not applicable: uri={} does not match any configured patterns", uri);
        }
        matches
    }

    /// Extract the client's Range header if present
    ///
    /// # Arguments
    /// * `headers` - Request headers
    ///
    /// # Returns
    /// * `Some(ByteRange)` if a valid Range header is present
    /// * `None` if no Range header or parsing fails
    pub fn extract_client_range(&self, headers: &HeaderMap<HeaderValue>) -> Option<ByteRange> {
        // Try both lowercase and capitalized versions
        let range_value = headers
            .get("range")
            .or_else(|| headers.get("Range"))?;

        // Convert HeaderValue to string
        let range_str = range_value.to_str().ok()?;

        // Parse the Range header
        match ByteRange::from_header(range_str) {
            Ok(range) => {
                debug!(
                    "Extracted client range: {}-{} from header: {}",
                    range.start, range.end, range_str
                );
                Some(range)
            }
            Err(e) => {
                debug!(
                    "Failed to parse Range header '{}': {:?}",
                    range_str, e
                );
                None
            }
        }
    }

    /// Check if the URI matches any of the configured patterns
    ///
    /// # Arguments
    /// * `uri` - Request URI to check
    ///
    /// # Returns
    /// `true` if URI matches any pattern, `false` otherwise
    fn matches_pattern(&self, uri: &str) -> bool {
        // For now, use simple string matching
        // In a production implementation, you might want to use regex
        for pattern in &self.config.slice_patterns {
            if self.pattern_matches(pattern, uri) {
                return true;
            }
        }
        false
    }

    /// Check if a single pattern matches the URI
    ///
    /// Supports simple glob-style patterns:
    /// - `*` matches any sequence of characters
    /// - Exact string matching otherwise
    ///
    /// # Arguments
    /// * `pattern` - Pattern to match against
    /// * `uri` - URI to check
    ///
    /// # Returns
    /// `true` if pattern matches, `false` otherwise
    fn pattern_matches(&self, pattern: &str, uri: &str) -> bool {
        // Simple wildcard matching
        if pattern.contains('*') {
            let parts: Vec<&str> = pattern.split('*').collect();
            
            if parts.is_empty() {
                return true;
            }

            // Check if URI starts with first part
            if !parts[0].is_empty() && !uri.starts_with(parts[0]) {
                return false;
            }

            // Check if URI ends with last part
            if parts.len() > 1 && !parts[parts.len() - 1].is_empty() && !uri.ends_with(parts[parts.len() - 1]) {
                return false;
            }

            // For middle parts, check if they appear in order
            let mut current_pos = parts[0].len();
            for i in 1..parts.len() - 1 {
                if parts[i].is_empty() {
                    continue;
                }
                if let Some(pos) = uri[current_pos..].find(parts[i]) {
                    current_pos += pos + parts[i].len();
                } else {
                    return false;
                }
            }

            true
        } else {
            // Exact match or prefix match
            uri == pattern || uri.starts_with(pattern)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SliceConfig;

    fn create_test_config(patterns: Vec<String>) -> Arc<SliceConfig> {
        Arc::new(SliceConfig {
            slice_patterns: patterns,
            ..Default::default()
        })
    }

    fn create_headers_with_range(range: &str) -> HeaderMap<HeaderValue> {
        let mut headers = HeaderMap::new();
        headers.insert("range", HeaderValue::from_str(range).unwrap());
        headers
    }

    #[test]
    fn test_should_slice_get_request() {
        let config = create_test_config(vec![]);
        let analyzer = RequestAnalyzer::new(config);
        let headers = HeaderMap::new();

        assert!(analyzer.should_slice(&Method::GET, "/test.bin", &headers));
    }

    #[test]
    fn test_should_not_slice_post_request() {
        let config = create_test_config(vec![]);
        let analyzer = RequestAnalyzer::new(config);
        let headers = HeaderMap::new();

        assert!(!analyzer.should_slice(&Method::POST, "/test.bin", &headers));
    }

    #[test]
    fn test_should_not_slice_with_range_header() {
        let config = create_test_config(vec![]);
        let analyzer = RequestAnalyzer::new(config);
        let headers = create_headers_with_range("bytes=0-1023");

        assert!(!analyzer.should_slice(&Method::GET, "/test.bin", &headers));
    }

    #[test]
    fn test_should_slice_with_matching_pattern() {
        let config = create_test_config(vec!["/large-files/".to_string()]);
        let analyzer = RequestAnalyzer::new(config);
        let headers = HeaderMap::new();

        assert!(analyzer.should_slice(&Method::GET, "/large-files/test.bin", &headers));
    }

    #[test]
    fn test_should_not_slice_with_non_matching_pattern() {
        let config = create_test_config(vec!["/large-files/".to_string()]);
        let analyzer = RequestAnalyzer::new(config);
        let headers = HeaderMap::new();

        assert!(!analyzer.should_slice(&Method::GET, "/small-files/test.bin", &headers));
    }

    #[test]
    fn test_should_slice_with_wildcard_pattern() {
        let config = create_test_config(vec!["*.bin".to_string()]);
        let analyzer = RequestAnalyzer::new(config);
        let headers = HeaderMap::new();

        assert!(analyzer.should_slice(&Method::GET, "/path/to/file.bin", &headers));
        assert!(!analyzer.should_slice(&Method::GET, "/path/to/file.txt", &headers));
    }

    #[test]
    fn test_extract_client_range() {
        let config = create_test_config(vec![]);
        let analyzer = RequestAnalyzer::new(config);
        let headers = create_headers_with_range("bytes=100-200");

        let range = analyzer.extract_client_range(&headers);
        assert!(range.is_some());
        let range = range.unwrap();
        assert_eq!(range.start, 100);
        assert_eq!(range.end, 200);
    }

    #[test]
    fn test_extract_client_range_none() {
        let config = create_test_config(vec![]);
        let analyzer = RequestAnalyzer::new(config);
        let headers = HeaderMap::new();

        let range = analyzer.extract_client_range(&headers);
        assert!(range.is_none());
    }

    #[test]
    fn test_pattern_matches_exact() {
        let config = create_test_config(vec![]);
        let analyzer = RequestAnalyzer::new(config);

        assert!(analyzer.pattern_matches("/test", "/test"));
        assert!(analyzer.pattern_matches("/test", "/test/file.bin"));
        assert!(!analyzer.pattern_matches("/test", "/other"));
    }

    #[test]
    fn test_pattern_matches_wildcard_suffix() {
        let config = create_test_config(vec![]);
        let analyzer = RequestAnalyzer::new(config);

        assert!(analyzer.pattern_matches("*.bin", "/file.bin"));
        assert!(analyzer.pattern_matches("*.bin", "/path/to/file.bin"));
        assert!(!analyzer.pattern_matches("*.bin", "/file.txt"));
    }

    #[test]
    fn test_pattern_matches_wildcard_prefix() {
        let config = create_test_config(vec![]);
        let analyzer = RequestAnalyzer::new(config);

        assert!(analyzer.pattern_matches("/downloads/*", "/downloads/file.bin"));
        assert!(analyzer.pattern_matches("/downloads/*", "/downloads/"));
        assert!(!analyzer.pattern_matches("/downloads/*", "/uploads/file.bin"));
    }

    #[test]
    fn test_pattern_matches_multiple_wildcards() {
        let config = create_test_config(vec![]);
        let analyzer = RequestAnalyzer::new(config);

        assert!(analyzer.pattern_matches("/*/files/*.bin", "/user/files/test.bin"));
        assert!(analyzer.pattern_matches("/*/files/*.bin", "/admin/files/data.bin"));
        assert!(!analyzer.pattern_matches("/*/files/*.bin", "/user/docs/test.txt"));
    }
}
