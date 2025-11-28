//! HTTP PURGE method handler for cache invalidation
//!
//! This module provides HTTP PURGE method support for cache invalidation,
//! following the standard CDN cache purge conventions.
//!
//! Supported PURGE methods:
//! - PURGE /path/to/file - Purge specific URL
//! - PURGE /* - Purge all cache (with X-Purge-All header)

use crate::error::{Result, SliceError};
use crate::purge_metrics::PurgeMetrics;
use crate::tiered_cache::TieredCache;
use bytes::Bytes;
use http::{Method, Request, Response, StatusCode};
use http_body_util::Full;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, warn};

/// PURGE request handler
pub struct PurgeHandler {
    cache: Arc<TieredCache>,
    /// Whether to require authentication for purge requests
    require_auth: bool,
    /// Optional auth token
    auth_token: Option<String>,
    /// Prometheus metrics (optional)
    metrics: Option<Arc<PurgeMetrics>>,
}

/// PURGE response body
#[derive(Debug, Serialize, Deserialize)]
pub struct PurgeResponse {
    pub success: bool,
    pub purged_count: usize,
    pub url: Option<String>,
    pub message: String,
}

impl PurgeHandler {
    /// Create a new PURGE handler
    pub fn new(cache: Arc<TieredCache>) -> Self {
        Self {
            cache,
            require_auth: false,
            auth_token: None,
            metrics: None,
        }
    }

    /// Create a new PURGE handler with authentication
    pub fn with_auth(cache: Arc<TieredCache>, auth_token: String) -> Self {
        Self {
            cache,
            require_auth: true,
            auth_token: Some(auth_token),
            metrics: None,
        }
    }

    /// Enable Prometheus metrics
    pub fn with_metrics(mut self, metrics: Arc<PurgeMetrics>) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Handle HTTP PURGE request
    ///
    /// Supports:
    /// - `PURGE /path/to/file` - Purge specific URL
    /// - `PURGE /*` with `X-Purge-All: true` - Purge all cache
    /// - `PURGE /path/*` with `X-Purge-Pattern: prefix` - Purge by prefix
    pub async fn handle_purge<B>(
        &self,
        req: Request<B>,
    ) -> Result<Response<Full<Bytes>>> {
        let start_time = Instant::now();

        // Check if method is PURGE
        if req.method() != Method::from_bytes(b"PURGE").unwrap() {
            return self.error_response(
                StatusCode::METHOD_NOT_ALLOWED,
                "Only PURGE method is allowed",
            );
        }

        // Check authentication if required
        if self.require_auth {
            if let Err(e) = self.check_auth(&req) {
                // Record auth failure
                if let Some(metrics) = &self.metrics {
                    metrics.record_auth_failure("invalid_token");
                }
                return self.error_response(StatusCode::UNAUTHORIZED, &e.to_string());
            }
        }

        // Get the URL path
        let path = req.uri().path();
        let host = req
            .headers()
            .get("host")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("localhost");

        // Construct full URL
        let scheme = if req
            .headers()
            .get("x-forwarded-proto")
            .and_then(|h| h.to_str().ok())
            == Some("https")
        {
            "https"
        } else {
            "http"
        };
        let url = format!("{}://{}{}", scheme, host, path);

        // Check for special purge modes
        let purge_all = req
            .headers()
            .get("x-purge-all")
            .and_then(|h| h.to_str().ok())
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let purge_pattern = req
            .headers()
            .get("x-purge-pattern")
            .and_then(|h| h.to_str().ok());

        // Determine purge method for metrics
        let purge_method = if purge_all {
            "all"
        } else if purge_pattern.is_some() {
            "pattern"
        } else {
            "url"
        };

        // Record request metric
        if let Some(metrics) = &self.metrics {
            metrics.record_request(purge_method);
        }

        // Execute purge operation
        let (purged_count, message, success) = if purge_all {
            // Purge all cache
            info!("Purging all cache entries");
            match self.cache.purge_all().await {
                Ok(count) => {
                    info!("Purged all {} cache entries", count);
                    (count, format!("Successfully purged all {} cache entries", count), true)
                }
                Err(e) => {
                    warn!("Failed to purge all cache: {}", e);
                    if let Some(metrics) = &self.metrics {
                        metrics.record_result(purge_method, false);
                        metrics.record_duration(purge_method, start_time.elapsed().as_secs_f64());
                    }
                    return self.error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        &format!("Failed to purge cache: {}", e),
                    );
                }
            }
        } else if let Some(pattern) = purge_pattern {
            // Purge by pattern (currently only supports prefix)
            if pattern == "prefix" {
                info!("Purging cache entries with prefix: {}", url);
                match self.cache.purge_url(&url).await {
                    Ok(count) => {
                        info!("Purged {} cache entries for URL: {}", count, url);
                        (
                            count,
                            format!("Successfully purged {} cache entries for {}", count, url),
                            true,
                        )
                    }
                    Err(e) => {
                        warn!("Failed to purge URL {}: {}", url, e);
                        if let Some(metrics) = &self.metrics {
                            metrics.record_result(purge_method, false);
                            metrics.record_duration(purge_method, start_time.elapsed().as_secs_f64());
                        }
                        return self.error_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            &format!("Failed to purge cache: {}", e),
                        );
                    }
                }
            } else {
                return self.error_response(
                    StatusCode::BAD_REQUEST,
                    &format!("Unsupported purge pattern: {}", pattern),
                );
            }
        } else {
            // Purge specific URL
            info!("Purging cache for URL: {}", url);
            match self.cache.purge_url(&url).await {
                Ok(count) => {
                    info!("Purged {} cache entries for URL: {}", count, url);
                    if count > 0 {
                        (
                            count,
                            format!("Successfully purged {} cache entries for {}", count, url),
                            true,
                        )
                    } else {
                        (0, format!("No cache entries found for {}", url), true)
                    }
                }
                Err(e) => {
                    warn!("Failed to purge URL {}: {}", url, e);
                    if let Some(metrics) = &self.metrics {
                        metrics.record_result(purge_method, false);
                        metrics.record_duration(purge_method, start_time.elapsed().as_secs_f64());
                    }
                    return self.error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        &format!("Failed to purge cache: {}", e),
                    );
                }
            }
        };

        // Record metrics
        if let Some(metrics) = &self.metrics {
            metrics.record_result(purge_method, success);
            metrics.record_purged_items(purge_method, purged_count);
            metrics.record_duration(purge_method, start_time.elapsed().as_secs_f64());
        }

        // Build success response
        let response = PurgeResponse {
            success: true,
            purged_count,
            url: if purge_all { None } else { Some(url) },
            message,
        };

        self.json_response(StatusCode::OK, &response)
    }

    /// Check authentication
    fn check_auth<B>(&self, req: &Request<B>) -> Result<()> {
        if let Some(expected_token) = &self.auth_token {
            // Check Authorization header
            if let Some(auth_header) = req.headers().get("authorization") {
                if let Ok(auth_str) = auth_header.to_str() {
                    // Support both "Bearer <token>" and direct token
                    let token = if auth_str.starts_with("Bearer ") {
                        &auth_str[7..]
                    } else {
                        auth_str
                    };

                    if token == expected_token {
                        return Ok(());
                    }
                }
            }

            // Check X-Purge-Token header (alternative)
            if let Some(token_header) = req.headers().get("x-purge-token") {
                if let Ok(token) = token_header.to_str() {
                    if token == expected_token {
                        return Ok(());
                    }
                }
            }

            Err(SliceError::ConfigError(
                "Invalid or missing authentication token".to_string(),
            ))
        } else {
            Ok(())
        }
    }

    /// Build JSON response
    fn json_response(
        &self,
        status: StatusCode,
        body: &PurgeResponse,
    ) -> Result<Response<Full<Bytes>>> {
        let json = serde_json::to_string(body)
            .map_err(|e| SliceError::CacheError(format!("Failed to serialize response: {}", e)))?;

        Response::builder()
            .status(status)
            .header("content-type", "application/json")
            .header("cache-control", "no-cache, no-store, must-revalidate")
            .body(Full::new(Bytes::from(json)))
            .map_err(|e| SliceError::CacheError(format!("Failed to build response: {}", e)))
    }

    /// Build error response
    fn error_response(&self, status: StatusCode, message: &str) -> Result<Response<Full<Bytes>>> {
        let response = PurgeResponse {
            success: false,
            purged_count: 0,
            url: None,
            message: message.to_string(),
        };

        self.json_response(status, &response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ByteRange;
    use http::Request;
    use std::time::Duration;

    async fn create_test_handler() -> (PurgeHandler, tempfile::TempDir) {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let cache = Arc::new(
            TieredCache::new(Duration::from_secs(60), 1024 * 1024, temp_dir.path())
                .await
                .unwrap(),
        );
        (PurgeHandler::new(cache), temp_dir)
    }

    #[tokio::test]
    async fn test_purge_specific_url() {
        let (handler, _temp_dir) = create_test_handler().await;

        // Store some data first
        let url = "http://example.com/test.dat";
        let range = ByteRange::new(0, 1023).unwrap();
        let data = Bytes::from(vec![1u8; 1024]);
        handler.cache.store(url, &range, data).unwrap();

        // Create PURGE request
        let req = Request::builder()
            .method(Method::from_bytes(b"PURGE").unwrap())
            .uri("/test.dat")
            .header("host", "example.com")
            .body(())
            .unwrap();

        // Handle purge
        let response = handler.handle_purge(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Verify cache is purged
        let result = handler.cache.lookup(url, &range).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_purge_all() {
        let (handler, _temp_dir) = create_test_handler().await;

        // Store multiple entries
        let range = ByteRange::new(0, 1023).unwrap();
        let data = Bytes::from(vec![1u8; 1024]);
        handler
            .cache
            .store("http://example.com/file1.dat", &range, data.clone())
            .unwrap();
        handler
            .cache
            .store("http://example.com/file2.dat", &range, data)
            .unwrap();

        // Create PURGE request with X-Purge-All header
        let req = Request::builder()
            .method(Method::from_bytes(b"PURGE").unwrap())
            .uri("/*")
            .header("host", "example.com")
            .header("x-purge-all", "true")
            .body(())
            .unwrap();

        // Handle purge
        let response = handler.handle_purge(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Verify all cache is purged
        let stats = handler.cache.get_stats();
        assert_eq!(stats.l1_entries, 0);
    }

    #[tokio::test]
    async fn test_purge_with_auth() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let cache = Arc::new(
            TieredCache::new(Duration::from_secs(60), 1024 * 1024, temp_dir.path())
                .await
                .unwrap(),
        );
        let handler = PurgeHandler::with_auth(cache, "secret-token".to_string());

        // Request without auth should fail
        let req = Request::builder()
            .method(Method::from_bytes(b"PURGE").unwrap())
            .uri("/test.dat")
            .header("host", "example.com")
            .body(())
            .unwrap();

        let response = handler.handle_purge(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // Request with correct auth should succeed
        let req = Request::builder()
            .method(Method::from_bytes(b"PURGE").unwrap())
            .uri("/test.dat")
            .header("host", "example.com")
            .header("authorization", "Bearer secret-token")
            .body(())
            .unwrap();

        let response = handler.handle_purge(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_non_purge_method() {
        let (handler, _temp_dir) = create_test_handler().await;

        // Try with GET method
        let req = Request::builder()
            .method(Method::GET)
            .uri("/test.dat")
            .header("host", "example.com")
            .body(())
            .unwrap();

        let response = handler.handle_purge(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }
}
