# SubrequestManager Implementation

## Overview

The SubrequestManager is responsible for fetching slices from the origin server using HTTP Range requests. It implements retry logic with exponential backoff and supports concurrent fetching of multiple slices.

## Components

### SubrequestManager

The main struct that manages subrequests:

```rust
pub struct SubrequestManager {
    http_client: Client,
    max_concurrent: usize,
    retry_policy: RetryPolicy,
}
```

**Key Methods:**

- `new(max_concurrent, max_retries)` - Creates a new manager
- `build_range_request(url, range)` - Builds an HTTP Range request
- `try_fetch_slice(slice, url)` - Attempts to fetch a single slice (no retry)
- `fetch_single_slice(slice, url)` - Fetches a slice with retry logic
- `fetch_slices(slices, url)` - Fetches multiple slices concurrently
- `validate_content_range(content_range, expected_range)` - Validates Content-Range header

### SubrequestResult

Contains the result of a successful subrequest:

```rust
pub struct SubrequestResult {
    pub slice_index: usize,
    pub data: Bytes,
    pub status: u16,
    pub headers: HeaderMap,
}
```

### RetryPolicy

Manages retry logic with exponential backoff:

```rust
pub struct RetryPolicy {
    pub max_retries: usize,
    pub backoff_ms: Vec<u64>,
}
```

**Backoff Strategy:**
- Attempt 0: 100ms
- Attempt 1: 200ms
- Attempt 2: 400ms
- Attempt 3: 800ms
- And so on...

## Implementation Details

### Range Request Construction

The manager constructs HTTP Range requests using the standard format:
```
Range: bytes=start-end
```

### Response Validation

For each subrequest, the manager validates:

1. **Status Code**: Must be 206 (Partial Content)
2. **Content-Range Header**: Must match the requested range

Example Content-Range validation:
```
Content-Range: bytes 0-1023/10240
```

The manager parses this header and verifies that the start and end positions match the requested range.

### Retry Logic

The retry logic follows these rules:

1. Only retryable errors trigger retries (network errors, HTTP errors)
2. Maximum retry attempts are configurable
3. Exponential backoff between retries
4. After all retries are exhausted, returns `SubrequestFailed` error

### Concurrent Fetching

The `fetch_slices` method uses Tokio's `Semaphore` to limit concurrency:

```rust
let semaphore = Arc::new(Semaphore::new(self.max_concurrent));
```

This ensures that at most `max_concurrent` requests are active at any time.

## Requirements Satisfied

### Requirement 5.4
✅ **WHEN a subrequest fails THEN the Slice Module SHALL retry the subrequest up to a configured maximum retry count**

Implemented in `fetch_single_slice()` with configurable `max_retries`.

### Requirement 8.3
✅ **WHEN a subrequest receives a 206 Partial Content response THEN the Slice Module SHALL validate the Content-Range header matches the request**

Implemented in `try_fetch_slice()` - validates status code is 206.

### Requirement 8.4
✅ **IF Content-Range does not match the requested range THEN the Slice Module SHALL treat it as an error and retry**

Implemented in `validate_content_range()` - parses and validates the Content-Range header.

## Usage Example

```rust
use pingora_slice::{SubrequestManager, SliceSpec, ByteRange};

// Create manager with 4 concurrent requests and 3 retries
let manager = SubrequestManager::new(4, 3);

// Create slices to fetch
let slices = vec![
    SliceSpec::new(0, ByteRange::new(0, 1023)?),
    SliceSpec::new(1, ByteRange::new(1024, 2047)?),
];

// Fetch all slices concurrently
let results = manager.fetch_slices(slices, "https://example.com/file.bin").await?;

for result in results {
    println!("Slice {}: {} bytes", result.slice_index, result.data.len());
}
```

## Testing

The implementation includes:

- **Unit tests** for retry policy, Content-Range validation, and manager creation
- **Integration tests** (marked as ignored) that require a server supporting Range requests
- All unit tests pass successfully

## Error Handling

The manager handles various error scenarios:

- **Network errors**: Retried with exponential backoff
- **Invalid status codes**: Treated as errors and retried
- **Content-Range mismatch**: Treated as errors and retried
- **Exhausted retries**: Returns `SubrequestFailed` error

## Performance Considerations

- Uses `reqwest` for efficient HTTP client implementation
- Concurrent fetching with configurable limits
- Exponential backoff prevents overwhelming the origin server
- 30-second timeout per request to prevent hanging

## Future Enhancements

Potential improvements:

1. Support for custom headers in subrequests
2. Configurable timeout per request
3. Circuit breaker pattern for failing origins
4. Metrics collection for subrequest performance
5. Support for HTTP/2 multiplexing
