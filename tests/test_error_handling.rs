// Unit tests for error handling logic
// Requirements: 8.1, 8.2, 8.5, 10.5

use pingora_slice::error::SliceError;

#[test]
fn test_4xx_errors_not_retryable() {
    // Requirement 8.1: 4xx errors should be passed through without retry
    let error = SliceError::origin_client_error(404, "Not Found");
    assert!(!error.should_retry(), "4xx errors should not be retried");
    
    let error = SliceError::origin_client_error(400, "Bad Request");
    assert!(!error.should_retry(), "400 errors should not be retried");
    
    let error = SliceError::origin_client_error(403, "Forbidden");
    assert!(!error.should_retry(), "403 errors should not be retried");
}

#[test]
fn test_5xx_errors_retryable() {
    // Requirement 8.2: 5xx errors should be retried
    let error = SliceError::origin_server_error(500, "Internal Server Error");
    assert!(error.should_retry(), "5xx errors should be retried");
    
    let error = SliceError::origin_server_error(502, "Bad Gateway");
    assert!(error.should_retry(), "502 errors should be retried");
    
    let error = SliceError::origin_server_error(503, "Service Unavailable");
    assert!(error.should_retry(), "503 errors should be retried");
}

#[test]
fn test_invalid_range_not_retryable() {
    // Requirement 10.5: Invalid range errors should not be retried
    let error = SliceError::InvalidRange("start > end".to_string());
    assert!(!error.should_retry(), "Invalid range should not be retried");
    
    let error = SliceError::UnsatisfiableRange("range exceeds file size".to_string());
    assert!(!error.should_retry(), "Unsatisfiable range should not be retried");
}

#[test]
fn test_network_errors_retryable() {
    // Network errors should be retried
    let error = SliceError::Timeout("Connection timeout".to_string());
    assert!(error.should_retry(), "Timeout errors should be retried");
    
    let error = SliceError::IoError("Connection reset".to_string());
    assert!(error.should_retry(), "IO errors should be retried");
}

#[test]
fn test_config_errors_not_retryable() {
    // Configuration errors should not be retried
    let error = SliceError::ConfigError("Invalid slice size".to_string());
    assert!(!error.should_retry(), "Config errors should not be retried");
}

#[test]
fn test_4xx_status_code_passthrough() {
    // Requirement 8.1: 4xx errors should return the same status code
    let error = SliceError::origin_client_error(404, "Not Found");
    assert_eq!(error.to_http_status(), 404, "404 should be passed through");
    
    let error = SliceError::origin_client_error(400, "Bad Request");
    assert_eq!(error.to_http_status(), 400, "400 should be passed through");
    
    let error = SliceError::origin_client_error(403, "Forbidden");
    assert_eq!(error.to_http_status(), 403, "403 should be passed through");
}

#[test]
fn test_5xx_status_code_conversion() {
    // Requirement 8.2: 5xx errors should return 502 Bad Gateway
    let error = SliceError::origin_server_error(500, "Internal Server Error");
    assert_eq!(error.to_http_status(), 502, "5xx errors should return 502");
    
    let error = SliceError::origin_server_error(503, "Service Unavailable");
    assert_eq!(error.to_http_status(), 502, "503 should return 502");
}

#[test]
fn test_invalid_range_status_code() {
    // Requirement 10.5: Invalid range should return 416
    let error = SliceError::InvalidRange("start > end".to_string());
    assert_eq!(error.to_http_status(), 416, "Invalid range should return 416");
    
    let error = SliceError::UnsatisfiableRange("range exceeds file size".to_string());
    assert_eq!(error.to_http_status(), 416, "Unsatisfiable range should return 416");
}

#[test]
fn test_subrequest_failed_status_code() {
    // Subrequest failures should return 502 Bad Gateway
    let error = SliceError::SubrequestFailed {
        slice_index: 0,
        attempts: 3,
    };
    assert_eq!(error.to_http_status(), 502, "Subrequest failure should return 502");
}

#[test]
fn test_timeout_status_code() {
    // Timeout should return 504 Gateway Timeout
    let error = SliceError::Timeout("Connection timeout".to_string());
    assert_eq!(error.to_http_status(), 504, "Timeout should return 504");
}

#[test]
fn test_fallback_to_normal_proxy() {
    // Range not supported should fallback
    let error = SliceError::RangeNotSupported;
    assert!(error.fallback_to_normal_proxy(), "RangeNotSupported should fallback");
    
    // Metadata fetch error should fallback
    let error = SliceError::MetadataFetchError("Failed to fetch".to_string());
    assert!(error.fallback_to_normal_proxy(), "MetadataFetchError should fallback");
}

#[test]
fn test_no_fallback_for_client_errors() {
    // 4xx errors should not fallback (Requirement 8.1)
    let error = SliceError::origin_client_error(404, "Not Found");
    assert!(!error.fallback_to_normal_proxy(), "4xx errors should not fallback");
    
    // Invalid range should not fallback
    let error = SliceError::InvalidRange("Invalid".to_string());
    assert!(!error.fallback_to_normal_proxy(), "Invalid range should not fallback");
}

#[test]
fn test_from_http_status_4xx() {
    // Test automatic categorization of 4xx errors
    let error = SliceError::from_http_status(404, "Not Found");
    match &error {
        SliceError::OriginClientError { status, message } => {
            assert_eq!(*status, 404);
            assert_eq!(message, "Not Found");
        }
        _ => panic!("Expected OriginClientError"),
    }
    
    assert!(!error.should_retry());
    assert_eq!(error.to_http_status(), 404);
}

#[test]
fn test_from_http_status_5xx() {
    // Test automatic categorization of 5xx errors
    let error = SliceError::from_http_status(500, "Internal Server Error");
    match &error {
        SliceError::OriginServerError { status, message } => {
            assert_eq!(*status, 500);
            assert_eq!(message, "Internal Server Error");
        }
        _ => panic!("Expected OriginServerError"),
    }
    
    assert!(error.should_retry());
    assert_eq!(error.to_http_status(), 502);
}

#[test]
fn test_content_range_mismatch_retryable() {
    // Content-Range mismatch might be transient, so should be retried
    let error = SliceError::ContentRangeMismatch {
        expected: "bytes 0-999/1000".to_string(),
        actual: "bytes 0-499/1000".to_string(),
    };
    assert!(error.should_retry(), "Content-Range mismatch should be retried");
    assert_eq!(error.to_http_status(), 502);
}

#[test]
fn test_cache_error_not_retryable() {
    // Cache errors should not block the request or trigger retries
    let error = SliceError::CacheError("Cache write failed".to_string());
    assert!(!error.should_retry(), "Cache errors should not be retried");
    assert_eq!(error.to_http_status(), 500);
}

#[test]
fn test_assembly_error_not_retryable() {
    // Assembly errors are internal and should not be retried
    let error = SliceError::AssemblyError("Failed to assemble".to_string());
    assert!(!error.should_retry(), "Assembly errors should not be retried");
    assert_eq!(error.to_http_status(), 500);
}

#[test]
fn test_parse_error_not_retryable() {
    // Parse errors are client errors and should not be retried
    let error = SliceError::ParseError("Invalid format".to_string());
    assert!(!error.should_retry(), "Parse errors should not be retried");
    assert_eq!(error.to_http_status(), 400);
}

#[test]
fn test_error_display() {
    // Test that error messages are formatted correctly
    let error = SliceError::origin_client_error(404, "Not Found");
    let display = format!("{}", error);
    assert!(display.contains("404"));
    assert!(display.contains("Not Found"));
    
    let error = SliceError::InvalidRange("start > end".to_string());
    let display = format!("{}", error);
    assert!(display.contains("Invalid byte range"));
    assert!(display.contains("start > end"));
}

#[test]
fn test_subrequest_failed_not_retryable() {
    // SubrequestFailed means retries were already exhausted
    let error = SliceError::SubrequestFailed {
        slice_index: 5,
        attempts: 3,
    };
    assert!(!error.should_retry(), "SubrequestFailed should not be retried (already exhausted)");
}
