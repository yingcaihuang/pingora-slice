//! Error types for the Pingora Slice module

use thiserror::Error;

/// Result type alias for Slice operations
pub type Result<T> = std::result::Result<T, SliceError>;

/// Error types that can occur in the Slice module
#[derive(Error, Debug, Clone)]
pub enum SliceError {
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Metadata fetch error: {0}")]
    MetadataFetchError(String),

    #[error("Range requests not supported by origin server")]
    RangeNotSupported,

    #[error("Subrequest failed for slice {slice_index} after {attempts} attempts")]
    SubrequestFailed { slice_index: usize, attempts: usize },

    #[error("Cache error: {0}")]
    CacheError(String),

    #[error("Response assembly error: {0}")]
    AssemblyError(String),

    #[error("Invalid byte range: {0}")]
    InvalidRange(String),

    #[error("IO error: {0}")]
    IoError(String),

    #[error("HTTP error: {0}")]
    HttpError(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Origin server returned 4xx error: {status} - {message}")]
    OriginClientError { status: u16, message: String },

    #[error("Origin server returned 5xx error: {status} - {message}")]
    OriginServerError { status: u16, message: String },

    #[error("Content-Range mismatch: expected {expected}, got {actual}")]
    ContentRangeMismatch { expected: String, actual: String },

    #[error("Unsatisfiable range: {0}")]
    UnsatisfiableRange(String),

    #[error("Network timeout: {0}")]
    Timeout(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}

impl From<std::io::Error> for SliceError {
    fn from(err: std::io::Error) -> Self {
        SliceError::IoError(err.to_string())
    }
}

impl SliceError {
    /// Determine if this error should trigger a retry
    /// 
    /// Returns true for errors that are potentially transient and may succeed on retry:
    /// - 5xx errors from origin server (server errors)
    /// - Network timeouts
    /// - IO errors
    /// - Content-Range mismatches (may be transient)
    /// 
    /// Returns false for errors that are permanent and won't benefit from retry:
    /// - 4xx errors from origin server (client errors)
    /// - Configuration errors
    /// - Invalid ranges
    /// - Range not supported
    /// - Parse errors
    /// 
    /// Requirements: 8.1, 8.2
    pub fn should_retry(&self) -> bool {
        match self {
            // 5xx errors should be retried (Requirement 8.2)
            SliceError::OriginServerError { .. } => true,
            
            // Network and IO errors should be retried
            SliceError::Timeout(_) => true,
            SliceError::IoError(_) => true,
            
            // Content-Range mismatch might be transient
            SliceError::ContentRangeMismatch { .. } => true,
            
            // Generic HTTP errors might be retryable
            SliceError::HttpError(_) => true,
            
            // 4xx errors should NOT be retried (Requirement 8.1)
            SliceError::OriginClientError { .. } => false,
            
            // Configuration and validation errors should not be retried
            SliceError::ConfigError(_) => false,
            SliceError::InvalidRange(_) => false,
            SliceError::UnsatisfiableRange(_) => false,
            SliceError::ParseError(_) => false,
            SliceError::RangeNotSupported => false,
            
            // Other errors
            SliceError::MetadataFetchError(_) => true,
            SliceError::SubrequestFailed { .. } => false, // Already exhausted retries
            SliceError::CacheError(_) => false, // Cache errors shouldn't block request
            SliceError::AssemblyError(_) => false,
            SliceError::InternalError(_) => false,
        }
    }

    /// Convert error to HTTP status code
    /// 
    /// Maps internal errors to appropriate HTTP status codes:
    /// - 4xx errors: Pass through from origin (Requirement 8.1)
    /// - 5xx errors: Return 502 Bad Gateway
    /// - Invalid Range: Return 416 Range Not Satisfiable (Requirement 10.5)
    /// - Other errors: Return 500 Internal Server Error
    /// 
    /// Requirements: 8.1, 8.5, 10.5
    pub fn to_http_status(&self) -> u16 {
        match self {
            // Pass through 4xx errors from origin (Requirement 8.1)
            SliceError::OriginClientError { status, .. } => *status,
            
            // 5xx errors from origin become 502 Bad Gateway
            SliceError::OriginServerError { .. } => 502,
            
            // Invalid range errors return 416 (Requirement 10.5)
            SliceError::InvalidRange(_) => 416,
            SliceError::UnsatisfiableRange(_) => 416,
            
            // Parse errors are client errors
            SliceError::ParseError(_) => 400,
            
            // Network and subrequest errors become 502 Bad Gateway
            SliceError::MetadataFetchError(_) => 502,
            SliceError::SubrequestFailed { .. } => 502,
            SliceError::HttpError(_) => 502,
            SliceError::Timeout(_) => 504, // Gateway Timeout
            SliceError::ContentRangeMismatch { .. } => 502,
            
            // Internal errors return 500
            SliceError::ConfigError(_) => 500,
            SliceError::RangeNotSupported => 500,
            SliceError::CacheError(_) => 500,
            SliceError::AssemblyError(_) => 500,
            SliceError::IoError(_) => 500,
            SliceError::InternalError(_) => 500,
        }
    }

    /// Determine if we should fallback to normal proxy mode
    /// 
    /// Returns true for errors that indicate slicing is not possible or appropriate,
    /// but the request could still be served via normal proxy mode:
    /// - Origin doesn't support Range requests
    /// - Metadata fetch failures (can't determine file size)
    /// 
    /// Returns false for errors that should be returned to the client:
    /// - 4xx errors (client's fault)
    /// - Invalid ranges
    /// - Configuration errors
    /// 
    /// Requirements: 8.1, 8.2
    pub fn fallback_to_normal_proxy(&self) -> bool {
        match self {
            // These errors mean slicing won't work, but normal proxy might
            SliceError::RangeNotSupported => true,
            SliceError::MetadataFetchError(_) => true,
            
            // All other errors should be handled or returned to client
            _ => false,
        }
    }

    /// Create an OriginClientError from a status code and message
    pub fn origin_client_error(status: u16, message: impl Into<String>) -> Self {
        SliceError::OriginClientError {
            status,
            message: message.into(),
        }
    }

    /// Create an OriginServerError from a status code and message
    pub fn origin_server_error(status: u16, message: impl Into<String>) -> Self {
        SliceError::OriginServerError {
            status,
            message: message.into(),
        }
    }

    /// Create an error from an HTTP status code
    /// 
    /// Automatically categorizes as 4xx or 5xx error
    pub fn from_http_status(status: u16, message: impl Into<String>) -> Self {
        let message = message.into();
        if (400..500).contains(&status) {
            SliceError::origin_client_error(status, message)
        } else if (500..600).contains(&status) {
            SliceError::origin_server_error(status, message)
        } else {
            SliceError::HttpError(format!("HTTP {}: {}", status, message))
        }
    }
}
