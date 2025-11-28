# SliceProxy Implementation Documentation

## Overview

The `SliceProxy` module provides the main proxy structure and per-request context for the Pingora Slice module. It integrates all components of the slice functionality and manages the state for each request.

## Components

### SliceProxy

The main proxy structure that coordinates all slice module operations.

#### Structure

```rust
pub struct SliceProxy {
    config: Arc<SliceConfig>,
    metrics: Arc<SliceMetrics>,
}
```

#### Fields

- **config**: Shared configuration for the slice module, wrapped in `Arc` for thread-safe sharing
- **metrics**: Thread-safe metrics collector for monitoring slice operations

#### Methods

##### `new(config: Arc<SliceConfig>) -> Self`

Creates a new `SliceProxy` instance with the provided configuration.

**Example:**
```rust
use pingora_slice::{SliceConfig, SliceProxy};
use std::sync::Arc;

let config = SliceConfig::default();
let proxy = SliceProxy::new(Arc::new(config));
```

##### `new_ctx(&self) -> SliceContext`

Creates a new request context for each incoming request. The context starts with default values (slicing disabled, no metadata).

**Example:**
```rust
let ctx = proxy.new_ctx();
assert!(!ctx.slice_enabled);
```

##### `config(&self) -> &SliceConfig`

Returns a reference to the configuration.

##### `metrics(&self) -> &SliceMetrics`

Returns a reference to the metrics collector.

##### `config_arc(&self) -> Arc<SliceConfig>`

Returns a cloned `Arc` to the configuration, useful for passing to other components.

##### `metrics_arc(&self) -> Arc<SliceMetrics>`

Returns a cloned `Arc` to the metrics collector, useful for passing to other components.

##### `upstream_peer(&self, ctx: &SliceContext) -> Result<String>`

Returns the upstream server address for normal proxy mode. This method is called when slicing is not enabled and the request should be proxied normally to the origin server.

**Requirements:** 9.3, 9.4

**Returns:**
- `Ok(String)` - The upstream server address from configuration
- `Err(SliceError)` - If slicing is enabled (this method shouldn't be called in that case)

**Example:**
```rust
let proxy = SliceProxy::new(Arc::new(config));
let ctx = SliceContext::new(); // Slicing disabled by default

let upstream = proxy.upstream_peer(&ctx)?;
println!("Upstream server: {}", upstream);
```

**Note:** This method should only be called when `ctx.is_slice_enabled()` returns `false`. If slicing is enabled, the request will be handled by the slice module and this method will return an error.

##### `logging(&self, method: &Method, uri: &str, ctx: &SliceContext, error: Option<&SliceError>, duration_ms: u64)`

Logs detailed information about request processing. This method implements requirements 9.3 and 9.4 for comprehensive logging.

**Requirements:** 9.3, 9.4

**Parameters:**
- `method` - HTTP method of the request
- `uri` - Request URI
- `ctx` - The request context containing state information
- `error` - Optional error that occurred during processing
- `duration_ms` - Request duration in milliseconds

**Logging Behavior:**

1. **Error Logging (Requirement 9.3)**
   - Logs detailed error information including request URL and error type
   - Uses `warn!` level for visibility
   - Includes method, URI, error type, error message, and duration

2. **Slice Request Completion (Requirement 9.4)**
   - Logs summary information for successful slice requests
   - Includes total time, number of slices, cache statistics
   - Uses `info!` level
   - Reports bytes transferred from origin, cache, and to client

3. **Normal Proxy Completion**
   - Logs basic information for non-sliced requests
   - Includes method, URI, and duration

**Example:**
```rust
use http::Method;
use std::time::Instant;

let start = Instant::now();

// Process request...

let duration_ms = start.elapsed().as_millis() as u64;

// Log successful request
proxy.logging(
    &Method::GET,
    "http://example.com/file.bin",
    &ctx,
    None,
    duration_ms,
);

// Log failed request
let error = SliceError::MetadataFetchError("Connection refused".to_string());
proxy.logging(
    &Method::GET,
    "http://example.com/unreachable.bin",
    &ctx,
    Some(&error),
    duration_ms,
);
```

**Log Output Examples:**

Success (slice mode):
```
INFO Slice request completed: method=GET, uri=http://example.com/file.bin, 
     slices=10, cached=3, uncached=7, duration_ms=250, 
     bytes_from_origin=7340032, bytes_from_cache=3145728, bytes_to_client=10485760
```

Success (normal mode):
```
INFO Normal proxy request completed: method=GET, uri=http://example.com/small.txt, 
     duration_ms=50
```

Error:
```
WARN Request failed: method=GET, uri=http://example.com/unreachable.bin, 
     error_type=MetadataFetchError, error=Metadata fetch error: Connection refused, 
     duration_ms=100
```

##### `request_filter(&self, method: &Method, uri: &str, headers: &HeaderMap<HeaderValue>, ctx: &mut SliceContext) -> Result<bool>`

The core request filter that determines if slicing should be enabled for a request. This method orchestrates all the components to make the slicing decision.

**Process Flow:**

1. **Request Analysis** (Requirements 2.1-2.4)
   - Checks if request method is GET
   - Verifies no Range header is present
   - Validates URL matches configured patterns

2. **Client Range Extraction** (Requirement 10.1)
   - Extracts client's Range header if present
   - Stores in context for later use

3. **Metadata Fetching** (Requirements 3.1-3.5)
   - Sends HEAD request to origin server
   - Extracts Content-Length, Accept-Ranges, and other headers
   - Falls back to normal proxy if metadata fetch fails

4. **Range Support Check** (Requirements 3.3-3.4)
   - Verifies origin supports Range requests
   - Falls back to normal proxy if not supported

5. **Slice Calculation** (Requirements 4.1-4.4)
   - Calculates slices based on file size and slice size
   - Handles client Range requests appropriately
   - Falls back to normal proxy for empty files

6. **Cache Lookup** (Requirement 7.3)
   - Checks cache for existing slices
   - Marks which slices are cached
   - Records cache hits and misses in metrics

7. **Context Update**
   - Updates context with metadata, slices, and cache info
   - Enables slicing mode
   - Records metrics

**Returns:**
- `Ok(true)` - Continue with normal proxy mode (slicing not enabled)
- `Ok(false)` - Slicing enabled, will handle response ourselves
- `Err(SliceError)` - An error occurred during processing

**Example:**
```rust
use http::{Method, HeaderMap};

let proxy = SliceProxy::new(Arc::new(config));
let mut ctx = SliceContext::new();
let headers = HeaderMap::new();

let result = proxy.request_filter(
    &Method::GET,
    "http://example.com/large-file.bin",
    &headers,
    &mut ctx,
).await?;

if result {
    // Continue with normal proxy
} else {
    // Slicing enabled, handle slice request
    assert!(ctx.is_slice_enabled());
    assert!(ctx.has_slices());
}
```

### SliceContext

Per-request context that stores all state information for a single request being processed.

#### Structure

```rust
pub struct SliceContext {
    pub slice_enabled: bool,
    pub metadata: Option<FileMetadata>,
    pub client_range: Option<ByteRange>,
    pub slices: Vec<SliceSpec>,
}
```

#### Fields

- **slice_enabled**: Whether slicing is enabled for this request
- **metadata**: File metadata from the origin server (if fetched)
- **client_range**: Client's requested byte range (if present in request)
- **slices**: Calculated slice specifications for this request

#### Methods

##### State Management

- `new() -> Self`: Create a new context with default values
- `is_slice_enabled(&self) -> bool`: Check if slicing is enabled
- `enable_slicing(&mut self)`: Enable slicing for this request
- `disable_slicing(&mut self)`: Disable slicing for this request

##### Metadata Management

- `set_metadata(&mut self, metadata: FileMetadata)`: Set file metadata
- `metadata(&self) -> Option<&FileMetadata>`: Get reference to metadata

##### Range Management

- `set_client_range(&mut self, range: ByteRange)`: Set client's requested range
- `client_range(&self) -> Option<ByteRange>`: Get client's requested range

##### Slice Management

- `set_slices(&mut self, slices: Vec<SliceSpec>)`: Set calculated slices
- `slices(&self) -> &[SliceSpec]`: Get reference to slices
- `slices_mut(&mut self) -> &mut Vec<SliceSpec>`: Get mutable reference to slices
- `slice_count(&self) -> usize`: Get number of slices
- `has_slices(&self) -> bool`: Check if there are any slices
- `cached_slice_count(&self) -> usize`: Get number of cached slices
- `uncached_slice_count(&self) -> usize`: Get number of uncached slices

## Usage Example

### Basic Setup

```rust
use pingora_slice::{SliceConfig, SliceProxy};
use std::sync::Arc;

// Create configuration
let config = SliceConfig::default();
let proxy = SliceProxy::new(Arc::new(config));
```

### Request Processing

```rust
// Create context for new request
let mut ctx = proxy.new_ctx();

// Enable slicing
ctx.enable_slicing();

// Set file metadata
let metadata = FileMetadata::new(10_000_000, true);
ctx.set_metadata(metadata);

// Set client range (if present)
let range = ByteRange::new(0, 1_000_000).unwrap();
ctx.set_client_range(range);

// Calculate and set slices
let slices = calculate_slices(...);
ctx.set_slices(slices);

// Check cache status
println!("Cached: {}, Uncached: {}", 
         ctx.cached_slice_count(), 
         ctx.uncached_slice_count());
```

### Metrics Recording

```rust
// Record request
proxy.metrics().record_request(true);

// Record cache operations
proxy.metrics().record_cache_hit();
proxy.metrics().record_cache_miss();

// Record subrequests
proxy.metrics().record_subrequest(true);

// Get metrics snapshot
let stats = proxy.metrics().get_stats();
println!("Cache hit rate: {:.1}%", stats.cache_hit_rate());
```

### Upstream Peer Selection

```rust
use http::Method;

// For normal proxy mode (slicing disabled)
let ctx = proxy.new_ctx();
let upstream = proxy.upstream_peer(&ctx)?;
println!("Proxying to: {}", upstream);

// For slice mode (slicing enabled)
let mut slice_ctx = proxy.new_ctx();
slice_ctx.enable_slicing();
// upstream_peer will return an error if called with slice mode enabled
```

### Request Logging

```rust
use http::Method;
use std::time::Instant;

let start = Instant::now();

// Process request...

let duration_ms = start.elapsed().as_millis() as u64;

// Log successful slice request
proxy.logging(
    &Method::GET,
    "http://example.com/large-file.bin",
    &ctx,
    None,
    duration_ms,
);

// Log failed request
let error = SliceError::MetadataFetchError("Connection refused".to_string());
proxy.logging(
    &Method::GET,
    "http://example.com/unreachable.bin",
    &ctx,
    Some(&error),
    duration_ms,
);
```

## Thread Safety

Both `SliceProxy` and its components are designed to be thread-safe:

- `SliceProxy` implements `Clone` and can be shared across threads
- Configuration is wrapped in `Arc` for shared ownership
- Metrics use atomic operations for thread-safe updates
- Each request gets its own `SliceContext` instance

## Request Filter Workflow

The `request_filter` method implements a comprehensive decision-making process:

```
┌─────────────────────────────────────────────────────────────┐
│                    Request Filter Flow                       │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
                    ┌──────────────────┐
                    │ Request Analysis │
                    └────────┬─────────┘
                             │
                    ┌────────▼─────────┐
                    │ Should Slice?    │
                    └────────┬─────────┘
                             │
                    ┌────────▼─────────┐
                    │ GET + No Range + │
                    │ Pattern Match?   │
                    └────────┬─────────┘
                             │
                    No ◄─────┴─────► Yes
                    │                 │
                    ▼                 ▼
            ┌──────────────┐  ┌──────────────────┐
            │ Normal Proxy │  │ Extract Client   │
            │ Return true  │  │ Range (if any)   │
            └──────────────┘  └────────┬─────────┘
                                       │
                              ┌────────▼─────────┐
                              │ Fetch Metadata   │
                              │ (HEAD request)   │
                              └────────┬─────────┘
                                       │
                              ┌────────▼─────────┐
                              │ Metadata OK?     │
                              └────────┬─────────┘
                                       │
                              No ◄─────┴─────► Yes
                              │                 │
                              ▼                 ▼
                      ┌──────────────┐  ┌──────────────────┐
                      │ Normal Proxy │  │ Supports Range?  │
                      │ Return true  │  └────────┬─────────┘
                      └──────────────┘           │
                                        No ◄─────┴─────► Yes
                                        │                 │
                                        ▼                 ▼
                                ┌──────────────┐  ┌──────────────────┐
                                │ Normal Proxy │  │ Calculate Slices │
                                │ Return true  │  └────────┬─────────┘
                                └──────────────┘           │
                                                  ┌────────▼─────────┐
                                                  │ Check Cache      │
                                                  │ for Slices       │
                                                  └────────┬─────────┘
                                                           │
                                                  ┌────────▼─────────┐
                                                  │ Mark Cached      │
                                                  │ Slices           │
                                                  └────────┬─────────┘
                                                           │
                                                  ┌────────▼─────────┐
                                                  │ Update Context   │
                                                  │ Enable Slicing   │
                                                  └────────┬─────────┘
                                                           │
                                                  ┌────────▼─────────┐
                                                  │ Record Metrics   │
                                                  └────────┬─────────┘
                                                           │
                                                  ┌────────▼─────────┐
                                                  │ Return false     │
                                                  │ (Handle slicing) │
                                                  └──────────────────┘
```

### Decision Points

1. **Should Slice?**
   - Must be GET request
   - Must NOT have Range header
   - Must match URL pattern (or no patterns configured)

2. **Metadata OK?**
   - HEAD request succeeds
   - Content-Length header present
   - No 4xx/5xx errors

3. **Supports Range?**
   - Accept-Ranges: bytes header present
   - Origin server supports partial content

4. **Valid Slices?**
   - File size > 0
   - Slices calculated successfully
   - No invalid range errors

### Fallback Scenarios

The request filter falls back to normal proxy mode in these cases:

1. **Request not eligible**: Non-GET, has Range header, pattern mismatch
2. **Metadata fetch fails**: Network error, timeout, origin error
3. **No Range support**: Origin doesn't support partial content
4. **Empty file**: File size is 0
5. **Invalid range**: Client requested invalid byte range

## Integration with Pingora

The `SliceProxy` structure is designed to integrate with Pingora's `ProxyHttp` trait:

```rust
#[async_trait]
impl ProxyHttp for SliceProxy {
    type CTX = SliceContext;
    
    fn new_ctx(&self) -> Self::CTX {
        self.new_ctx()
    }
    
    async fn request_filter(
        &self, 
        session: &mut Session, 
        ctx: &mut Self::CTX
    ) -> Result<bool> {
        // Extract request information from session
        let method = session.req_header().method.clone();
        let uri = session.req_header().uri.to_string();
        let headers = session.req_header().headers.clone();
        
        // Call our request filter implementation
        self.request_filter(&method, &uri, &headers, ctx).await
    }
    
    // Other trait methods...
}
```

## Requirements Validation

This implementation validates the following requirements:

- **Requirements 1.1-1.4**: Configuration management through `SliceConfig`
- **Requirements 9.1-9.2**: Metrics collection through `SliceMetrics`
- **All requirements**: Context management for request processing

## Testing

The module includes comprehensive unit tests covering:

### Unit Tests

- Proxy creation and initialization
- Context creation and state management
- Configuration access
- Metrics recording and retrieval
- Thread safety
- Clone behavior

### Request Filter Tests

The `request_filter` method has extensive test coverage:

1. **Request Analysis Tests**
   - Non-GET requests (POST, PUT, etc.)
   - Requests with Range headers
   - URL pattern matching and mismatches

2. **Metadata Fetching Tests**
   - Successful metadata fetch
   - Origin supports Range requests
   - Origin doesn't support Range requests
   - 4xx errors from origin
   - Network failures

3. **Edge Cases**
   - Empty files (size = 0)
   - Invalid URLs
   - Missing headers

4. **Integration Tests**
   - Complete flow with mock server
   - Cache lookup integration
   - Metrics recording

**Example Test:**
```rust
#[tokio::test]
async fn test_request_filter_origin_supports_range() {
    let mock_server = MockServer::start().await;
    
    Mock::given(method("HEAD"))
        .and(path("/file.bin"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Length", "10240")
                .insert_header("Accept-Ranges", "bytes")
        )
        .mount(&mock_server)
        .await;
    
    let proxy = create_test_proxy(vec![]);
    let mut ctx = SliceContext::new();
    let headers = HeaderMap::new();
    
    let url = format!("{}/file.bin", mock_server.uri());
    let result = proxy.request_filter(
        &Method::GET,
        &url,
        &headers,
        &mut ctx,
    ).await;
    
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), false); // Slicing enabled
    assert!(ctx.is_slice_enabled());
    assert_eq!(ctx.slice_count(), 10); // 10240 / 1024
}
```

Run tests with:
```bash
cargo test proxy
```

## Performance Considerations

1. **Memory Efficiency**: Each context is lightweight and only allocates memory for slices when needed
2. **Thread Safety**: Uses `Arc` for shared data and atomic operations for metrics
3. **Clone Cost**: Cloning `SliceProxy` is cheap (only clones `Arc` pointers)
4. **Context Lifecycle**: Contexts are created per-request and dropped when request completes

## Future Enhancements

Potential improvements for future versions:

1. **Connection Pooling**: Add HTTP connection pool management
2. **Cache Integration**: Direct integration with Pingora's cache storage
3. **Advanced Metrics**: Add histogram support for latency tracking
4. **Request Prioritization**: Support for prioritizing certain requests
5. **Dynamic Configuration**: Support for runtime configuration updates
