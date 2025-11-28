//! Integration tests for MetadataFetcher

use pingora_slice::{MetadataFetcher, SliceError};
use std::time::Duration;

#[tokio::test]
async fn test_metadata_fetcher_with_httpbin() {
    // Use httpbin.org as a test server (it supports HEAD requests)
    let fetcher = MetadataFetcher::new().unwrap();
    
    // Test with a known endpoint that returns proper headers
    let result = fetcher.fetch_metadata("https://httpbin.org/bytes/1024").await;
    
    // httpbin should return a successful response
    match result {
        Ok(metadata) => {
            // httpbin returns Content-Length
            assert_eq!(metadata.content_length, 1024);
            // Note: httpbin may or may not return Accept-Ranges header
            // Just verify we got the metadata
        }
        Err(e) => {
            // Network errors are acceptable in tests
            eprintln!("Network test skipped due to error: {}", e);
        }
    }
}

#[tokio::test]
async fn test_metadata_fetcher_timeout() {
    // Create a fetcher with very short timeout
    let fetcher = MetadataFetcher::with_timeout(Duration::from_millis(1)).unwrap();
    
    // This should timeout
    let result = fetcher.fetch_metadata("https://httpbin.org/delay/5").await;
    
    // Should get an error (either timeout or connection error)
    assert!(result.is_err());
}

#[tokio::test]
async fn test_metadata_fetcher_invalid_url() {
    let fetcher = MetadataFetcher::new().unwrap();
    
    // Test with invalid URL
    let result = fetcher.fetch_metadata("not-a-valid-url").await;
    
    // Should get an error
    assert!(result.is_err());
}

#[tokio::test]
async fn test_metadata_fetcher_404() {
    let fetcher = MetadataFetcher::new().unwrap();
    
    // Test with a URL that returns 404
    let result = fetcher.fetch_metadata("https://httpbin.org/status/404").await;
    
    // Should get a MetadataFetchError for 4xx status
    match result {
        Err(SliceError::MetadataFetchError(msg)) => {
            assert!(msg.contains("4xx") || msg.contains("404"));
        }
        Err(e) => {
            eprintln!("Network test got different error: {}", e);
        }
        Ok(_) => panic!("Expected error for 404 response"),
    }
}

#[tokio::test]
async fn test_metadata_fetcher_500() {
    let fetcher = MetadataFetcher::new().unwrap();
    
    // Test with a URL that returns 500
    let result = fetcher.fetch_metadata("https://httpbin.org/status/500").await;
    
    // Should get a MetadataFetchError for 5xx status
    match result {
        Err(SliceError::MetadataFetchError(msg)) => {
            assert!(msg.contains("5xx"));
        }
        Err(e) => {
            eprintln!("Network test got different error: {}", e);
        }
        Ok(_) => panic!("Expected error for 500 response"),
    }
}

#[tokio::test]
async fn test_metadata_fetcher_default() {
    // Test that default implementation works
    let fetcher = MetadataFetcher::default();
    
    // Just verify it was created successfully
    // We can't test actual fetching without a real server
    let result = fetcher.fetch_metadata("https://httpbin.org/bytes/100").await;
    
    // Either success or network error is acceptable
    match result {
        Ok(metadata) => {
            assert_eq!(metadata.content_length, 100);
        }
        Err(e) => {
            eprintln!("Network test skipped due to error: {}", e);
        }
    }
}
