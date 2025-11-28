# handle_slice_request Implementation

## Overview

This document describes the implementation of the `handle_slice_request` method in the SliceProxy structure. This is the core method that orchestrates the entire slice request handling flow.

## Implementation Location

- **File**: `src/proxy.rs`
- **Method**: `SliceProxy::handle_slice_request`

## Method Signature

```rust
pub async fn handle_slice_request(
    &self,
    url: &str,
    ctx: &SliceContext,
) -> Result<(http::StatusCode, HeaderMap, Vec<Bytes>)>
```

## Functionality

The `handle_slice_request` method implements the complete slice request handling flow as specified in the requirements. It performs the following steps:

### 1. Build Response Headers (Requirement 6.5)

- Creates appropriate HTTP response headers based on the file metadata
- Sets status code to 200 (OK) for full file requests or 206 (Partial Content) for range requests
- Includes Content-Length, Content-Type, Content-Range (for range requests), and Accept-Ranges headers
- Preserves metadata like ETag and Last-Modified from the origin server

### 2. Identify Slices to Fetch (Requirements 5.1, 7.4)

- Filters the slice list to identify which slices need to be fetched from origin
- Slices marked as `cached` are skipped for fetching
- Only uncached slices are sent to the SubrequestManager

### 3. Concurrent Slice Fetching (Requirements 5.1, 5.2, 5.3, 5.4, 5.5)

- Uses SubrequestManager to fetch uncached slices concurrently
- Respects the configured maximum concurrent subrequest limit
- Implements retry logic with exponential backoff for failed requests
- Records metrics for successful and failed subrequests
- Aborts the entire request if any slice fails after all retries

### 4. Merge Cached and Fetched Slices (Requirement 6.2)

- Retrieves cached slices from the SliceCache
- Combines cached slices with newly fetched slices
- Uses a BTreeMap to maintain correct ordering by slice index
- Records cache hit/miss metrics

### 5. Store New Slices in Cache (Requirements 7.1, 7.5)

- Stores newly fetched slices in the cache for future requests
- Continues processing even if cache storage fails (logs warning)
- Records cache errors in metrics

### 6. Validate Completeness (Requirement 6.2)

- Verifies that all expected slices are present
- Checks that slice indices are contiguous from 0 to N-1
- Returns an error if any slices are missing

### 7. Stream Slices in Order (Requirements 6.1, 6.2, 6.3)

- Converts the BTreeMap to an ordered vector of Bytes
- Ensures slices are in the correct order for streaming
- Calculates total bytes sent to client

### 8. Record Metrics (Requirements 9.1, 9.2)

- Records request duration
- Records assembly duration
- Records bytes from origin, cache, and to client
- Records subrequest statistics

## Return Value

The method returns a tuple containing:
- `StatusCode`: HTTP status code (200 or 206)
- `HeaderMap`: Response headers
- `Vec<Bytes>`: Ordered slice data ready for streaming

## Error Handling

The method handles various error conditions:

- **Missing Metadata**: Returns AssemblyError if file metadata is not available
- **Subrequest Failures**: Propagates errors from SubrequestManager after retries
- **Cache Errors**: Logs warnings but continues processing
- **Missing Slices**: Returns AssemblyError if not all slices are present

## Metrics Recorded

The method records the following metrics:

- `total_subrequests`: Number of subrequests made
- `failed_subrequests`: Number of failed subrequests
- `bytes_from_origin`: Bytes fetched from origin server
- `bytes_from_cache`: Bytes retrieved from cache
- `bytes_to_client`: Total bytes sent to client
- `request_duration`: Total request processing time
- `subrequest_duration`: Time spent fetching slices
- `assembly_duration`: Time spent assembling slices

## Requirements Validated

This implementation validates the following requirements:

- **5.1**: Concurrent subrequest sending
- **5.2**: Concurrency limit enforcement
- **5.3**: Sequential subrequest initiation
- **5.4**: Retry logic for failed subrequests
- **5.5**: Request abortion on exhausted retries
- **6.1**: Immediate streaming start
- **6.2**: Correct byte order maintenance
- **6.3**: Buffering of out-of-order slices
- **6.4**: Response completion
- **6.5**: Appropriate response headers
- **7.1**: Slice caching with unique keys
- **7.4**: Cache usage before origin requests
- **7.5**: Cache storage error handling

## Testing

The implementation is thoroughly tested with:

### Unit Tests (in `src/proxy.rs`)

- `test_handle_slice_request_no_slices_to_fetch`: Tests fetching all slices from origin
- `test_handle_slice_request_fetch_from_origin`: Tests basic slice fetching
- `test_handle_slice_request_multiple_slices`: Tests handling multiple slices
- `test_handle_slice_request_with_client_range`: Tests range request handling
- `test_handle_slice_request_missing_metadata`: Tests error handling for missing metadata
- `test_handle_slice_request_subrequest_failure`: Tests error handling for failed subrequests

### Integration Tests (in `tests/test_handle_slice_request.rs`)

- `test_handle_slice_request_complete_flow`: Tests the complete end-to-end flow
- `test_handle_slice_request_with_range`: Tests range request handling
- `test_handle_slice_request_concurrent_fetching`: Tests concurrent slice fetching
- `test_handle_slice_request_error_handling`: Tests error handling

## Usage Example

```rust
use pingora_slice::{SliceProxy, SliceConfig, SliceContext};
use std::sync::Arc;

// Create proxy
let config = Arc::new(SliceConfig::default());
let proxy = SliceProxy::new(config);

// Create context (populated by request_filter)
let mut ctx = SliceContext::new();
// ... populate ctx with metadata and slices ...

// Handle the request
let result = proxy.handle_slice_request("http://example.com/file.bin", &ctx).await;

match result {
    Ok((status, headers, slices)) => {
        // Send response to client
        println!("Status: {}", status);
        println!("Headers: {:?}", headers);
        println!("Total slices: {}", slices.len());
    }
    Err(e) => {
        eprintln!("Error: {:?}", e);
    }
}
```

## Performance Considerations

1. **Concurrent Fetching**: The method uses tokio's Semaphore to limit concurrent subrequests, preventing resource exhaustion
2. **Memory Efficiency**: Slices are stored in a BTreeMap which provides efficient ordered iteration
3. **Cache Efficiency**: Cached slices are retrieved first to minimize origin requests
4. **Metrics Overhead**: Atomic operations are used for metrics to minimize performance impact

## Future Enhancements

Potential improvements for future versions:

1. **Shared Cache Storage**: Add a shared cache storage field to SliceProxy to enable true cache persistence across requests
2. **Streaming Assembly**: Stream slices to client as they arrive instead of waiting for all slices
3. **Partial Failure Handling**: Allow partial success if some slices are available
4. **Adaptive Concurrency**: Dynamically adjust concurrency based on network conditions
5. **Compression Support**: Add support for compressed slice transfer
