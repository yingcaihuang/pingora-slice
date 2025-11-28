# Error Handling Implementation

## Overview

This document describes the error handling implementation for the Pingora Slice module, covering Requirements 8.1, 8.2, 8.5, and 10.5.

## Implementation Details

### SliceError Enum

The `SliceError` enum has been enhanced with the following error types:

1. **Configuration Errors**
   - `ConfigError(String)` - Invalid configuration parameters
   - Returns HTTP 500, not retryable, no fallback

2. **Origin Server Errors**
   - `OriginClientError { status: u16, message: String }` - 4xx errors from origin
   - `OriginServerError { status: u16, message: String }` - 5xx errors from origin
   - 4xx: Pass through status code, not retryable (Requirement 8.1)
   - 5xx: Return 502, retryable (Requirement 8.2)

3. **Range Errors**
   - `InvalidRange(String)` - Invalid byte range format
   - `UnsatisfiableRange(String)` - Range exceeds file size
   - Both return HTTP 416, not retryable (Requirement 10.5)

4. **Network Errors**
   - `Timeout(String)` - Connection timeout
   - `IoError(String)` - IO errors
   - Timeout returns 504, IO returns 500, both retryable

5. **Validation Errors**
   - `ContentRangeMismatch { expected: String, actual: String }` - Response doesn't match request
   - Returns 502, retryable (Requirement 8.4)

6. **Other Errors**
   - `MetadataFetchError(String)` - Failed to fetch file metadata
   - `RangeNotSupported` - Origin doesn't support Range requests
   - `SubrequestFailed { slice_index: usize, attempts: usize }` - Exhausted retries
   - `CacheError(String)` - Cache operation failed
   - `AssemblyError(String)` - Response assembly failed
   - `HttpError(String)` - Generic HTTP error
   - `ParseError(String)` - Parse error

### Key Methods

#### `should_retry() -> bool`

Determines if an error should trigger a retry:

**Retryable errors:**
- 5xx errors from origin (Requirement 8.2)
- Network timeouts
- IO errors
- Content-Range mismatches
- Metadata fetch errors

**Non-retryable errors:**
- 4xx errors from origin (Requirement 8.1)
- Configuration errors
- Invalid ranges
- Parse errors
- Range not supported
- Already exhausted retries (SubrequestFailed)

#### `to_http_status() -> u16`

Converts errors to HTTP status codes:

- **4xx errors**: Pass through original status (Requirement 8.1)
- **5xx errors**: Return 502 Bad Gateway
- **Invalid/Unsatisfiable Range**: Return 416 (Requirement 10.5)
- **Timeout**: Return 504 Gateway Timeout
- **Parse errors**: Return 400 Bad Request
- **Other errors**: Return 500 or 502 as appropriate

#### `fallback_to_normal_proxy() -> bool`

Determines if we should fallback to normal proxy mode:

**Fallback cases:**
- `RangeNotSupported` - Origin doesn't support Range requests
- `MetadataFetchError` - Can't determine file size

**No fallback cases:**
- 4xx errors (client's fault, should be returned)
- Invalid ranges (should return 416)
- Configuration errors (should fail)

### Helper Methods

#### `origin_client_error(status: u16, message: impl Into<String>) -> Self`

Creates an `OriginClientError` for 4xx responses.

#### `origin_server_error(status: u16, message: impl Into<String>) -> Self`

Creates an `OriginServerError` for 5xx responses.

#### `from_http_status(status: u16, message: impl Into<String>) -> Self`

Automatically categorizes HTTP status codes:
- 400-499: Creates `OriginClientError`
- 500-599: Creates `OriginServerError`
- Other: Creates generic `HttpError`

## Error Handling Flow

```
┌─────────────────┐
│  Error Occurs   │
└────────┬────────┘
         │
         ▼
┌─────────────────────────┐
│  should_retry()?        │
└────────┬────────────────┘
         │
    ┌────┴────┐
    │         │
   Yes       No
    │         │
    ▼         ▼
┌────────┐  ┌──────────────────┐
│ Retry  │  │ to_http_status() │
└────────┘  └────────┬─────────┘
                     │
                     ▼
            ┌─────────────────┐
            │ Return to Client│
            └─────────────────┘
```

## Requirements Coverage

### Requirement 8.1: 4xx Error Passthrough
✅ 4xx errors from origin are passed through to the client with the same status code
✅ 4xx errors are not retried
✅ 4xx errors do not fallback to normal proxy

### Requirement 8.2: 5xx Error Retry
✅ 5xx errors from origin trigger retry logic
✅ 5xx errors return 502 Bad Gateway to client after exhausting retries

### Requirement 8.5: Unexpected Status Codes
✅ Unexpected status codes are handled appropriately
✅ Non-206 responses for subrequests are treated as errors

### Requirement 10.5: Invalid Range Handling
✅ Invalid ranges return 416 Range Not Satisfiable
✅ Unsatisfiable ranges return 416
✅ Invalid ranges are not retried

## Testing

Comprehensive unit tests cover:
- 4xx error handling (not retryable, pass through status)
- 5xx error handling (retryable, return 502)
- Invalid range handling (return 416, not retryable)
- Network error handling (retryable)
- Configuration error handling (not retryable)
- Fallback logic
- Status code conversion
- Error display formatting

All tests pass successfully.

## Usage Example

```rust
use pingora_slice::error::SliceError;

// Create a 4xx error
let error = SliceError::origin_client_error(404, "Not Found");
assert!(!error.should_retry());
assert_eq!(error.to_http_status(), 404);

// Create a 5xx error
let error = SliceError::origin_server_error(500, "Internal Server Error");
assert!(error.should_retry());
assert_eq!(error.to_http_status(), 502);

// Create an invalid range error
let error = SliceError::InvalidRange("start > end".to_string());
assert!(!error.should_retry());
assert_eq!(error.to_http_status(), 416);

// Automatic categorization
let error = SliceError::from_http_status(404, "Not Found");
// Automatically creates OriginClientError
```

## Integration with Other Components

The error handling logic integrates with:

1. **SubrequestManager**: Uses `should_retry()` to determine retry behavior
2. **MetadataFetcher**: Uses error types to signal fetch failures
3. **ResponseAssembler**: Uses error types for assembly failures
4. **Cache**: Uses `CacheError` for non-blocking cache failures
5. **Main Proxy Logic**: Uses `fallback_to_normal_proxy()` to decide fallback behavior

## Future Enhancements

Potential improvements:
- Add more specific error types for different failure scenarios
- Implement error rate limiting to prevent retry storms
- Add structured logging for better observability
- Implement circuit breaker pattern for failing origins
