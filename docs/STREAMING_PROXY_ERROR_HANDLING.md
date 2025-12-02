# Streaming Proxy Error Handling

## Overview

This document describes the comprehensive error handling implementation for the Pingora-based streaming proxy. The error handling system ensures high reliability and implements a degradation strategy where cache failures do not prevent the proxy from functioning.

## Error Handling Strategy

The streaming proxy implements a **graceful degradation** strategy:

1. **Cache errors never stop proxying** - If cache lookup or write fails, the proxy continues to serve content from upstream
2. **Upstream failures are logged and tracked** - Connection failures, timeouts, and other upstream errors are properly logged
3. **Partial data is cleaned up** - If an error occurs during streaming, partial cached data is discarded
4. **Stale cache can be served** - When upstream fails, Pingora's built-in stale cache serving can be used

## Implemented Error Handlers

### 1. `fail_to_connect()`

**Purpose**: Handle upstream connection failures

**Behavior**:
- Logs the connection failure with context
- Marks the request as failed in the context
- Returns the error to Pingora for handling
- Pingora's built-in stale cache serving can kick in if configured

**Example**:
```rust
fn fail_to_connect(
    &self,
    _session: &mut Session,
    _peer: &HttpPeer,
    ctx: &mut Self::CTX,
    e: Box<Error>,
) -> Box<Error> {
    error!("Failed to connect to upstream for {}: {}", ctx.url(), e);
    ctx.set_upstream_failed(true);
    e
}
```

### 2. `error_while_proxy()`

**Purpose**: Handle errors during proxying (timeouts, connection resets, etc.)

**Behavior**:
- Logs the error with detailed context
- Identifies the error type (timeout, connection closed, etc.)
- Marks the request as failed
- Discards partial cached data if caching was in progress
- Returns the error to Pingora

**Error Types Handled**:
- `ConnectTimedout` - Connection timeout
- `ReadTimedout` - Read timeout
- `WriteTimedout` - Write timeout
- `ConnectionClosed` - Connection closed unexpectedly
- `ConnectError` - General connection error

**Example**:
```rust
fn error_while_proxy(
    &self,
    _peer: &HttpPeer,
    _session: &mut Session,
    e: Box<Error>,
    ctx: &mut Self::CTX,
    _client_reused: bool,
) -> Box<Error> {
    error!("Error while proxying {}: {}", ctx.url(), e);
    ctx.set_upstream_failed(true);
    
    // Discard partial cached data
    if ctx.is_cache_enabled() && !ctx.buffer().is_empty() {
        warn!("Discarding {} bytes of partial cached data", ctx.buffer_size());
        ctx.clear_buffer();
        ctx.disable_cache();
    }
    
    e
}
```

### 3. `logging()`

**Purpose**: Log request completion (success or failure)

**Behavior**:
- Logs request completion status
- Logs cache hit/miss status
- Logs bytes transferred
- Logs any errors that occurred
- Logs cache errors separately for monitoring

**Example**:
```rust
async fn logging(
    &self,
    _session: &mut Session,
    e: Option<&Error>,
    ctx: &mut Self::CTX,
) {
    if let Some(error) = e {
        error!("Request completed with error for {}: {}", ctx.url(), error);
        error!("  Upstream failed: {}", ctx.is_upstream_failed());
        error!("  Cache error: {}", ctx.has_cache_error());
        error!("  Bytes received: {}", ctx.bytes_received());
    } else {
        info!("Request completed successfully for: {}", ctx.url());
        info!("  Cache hit: {}", ctx.is_cache_hit());
        info!("  Bytes received: {}", ctx.bytes_received());
    }
}
```

## Cache Error Handling

### Cache Lookup Errors

When cache lookup fails:
1. Error is logged with context
2. `cache_error` flag is set in context
3. Request continues to upstream (degradation)
4. Caching is still enabled to try to cache the response

**Code**:
```rust
match self.cache.lookup(cache_key, &range).await {
    Ok(Some(data)) => {
        // Cache hit - serve from cache
    }
    Ok(None) => {
        // Cache miss - continue to upstream
    }
    Err(e) => {
        // Cache error - log and continue (degradation)
        error!("Cache lookup error for {}: {}", url, e);
        ctx.set_cache_error(true);
        ctx.enable_cache();  // Still try to cache the response
        warn!("Continuing to upstream despite cache error (degradation)");
    }
}
```

### Cache Write Errors

When cache write fails:
1. Error is logged with context
2. `cache_error` flag is set in context
3. Response to client is NOT affected (degradation)
4. Request completes successfully

**Code**:
```rust
if let Err(e) = self.cache.store(cache_key, &range, data) {
    error!("Failed to cache data for {}: {}", ctx.url(), e);
    ctx.set_cache_error(true);
    warn!("Cache write failed but response was successfully served (degradation)");
} else {
    info!("Successfully cached {} bytes for: {}", data_len, ctx.url());
}
```

## Context Tracking

The `ProxyContext` tracks error state throughout the request lifecycle:

```rust
pub struct ProxyContext {
    // ... other fields ...
    
    /// Whether upstream connection/request failed
    upstream_failed: bool,
    
    /// Whether a cache error occurred (for logging/metrics)
    cache_error: bool,
}
```

### Helper Methods

```rust
// Check if upstream failed
pub fn is_upstream_failed(&self) -> bool

// Set upstream failed status
pub fn set_upstream_failed(&mut self, failed: bool)

// Check if a cache error occurred
pub fn has_cache_error(&self) -> bool

// Set cache error status
pub fn set_cache_error(&mut self, error: bool)
```

## Error Scenarios

### Scenario 1: Upstream Connection Failure

```
1. Client requests /file.dat
2. Proxy attempts to connect to upstream
3. Connection fails (network error, upstream down, etc.)
4. fail_to_connect() is called
5. Error is logged: "Failed to connect to upstream for /file.dat"
6. upstream_failed flag is set
7. Pingora can serve stale cache if available and configured
8. If no stale cache, error response is sent to client
```

### Scenario 2: Upstream Timeout During Transfer

```
1. Client requests /file.dat
2. Proxy connects to upstream successfully
3. Starts receiving data (e.g., 50% of file)
4. Upstream times out (read timeout)
5. error_while_proxy() is called
6. Error is logged: "Read timeout for /file.dat"
7. Partial cached data (50%) is discarded
8. upstream_failed flag is set
9. Error response is sent to client
```

### Scenario 3: Cache Lookup Error

```
1. Client requests /file.dat
2. Proxy attempts cache lookup
3. Cache lookup fails (disk error, corruption, etc.)
4. Error is logged: "Cache lookup error for /file.dat"
5. cache_error flag is set
6. Request continues to upstream (degradation)
7. Response is served from upstream
8. Attempt to cache the response (may succeed or fail)
```

### Scenario 4: Cache Write Error

```
1. Client requests /file.dat
2. Cache miss - fetch from upstream
3. Receive data from upstream
4. Forward data to client (success)
5. Attempt to cache data
6. Cache write fails (disk full, I/O error, etc.)
7. Error is logged: "Failed to cache data for /file.dat"
8. cache_error flag is set
9. Response to client is NOT affected (degradation)
10. Request completes successfully
```

### Scenario 5: Multiple Errors

```
1. Client requests /file.dat
2. Cache lookup fails (cache_error = true)
3. Continue to upstream despite cache error
4. Upstream connection fails (upstream_failed = true)
5. Both errors are tracked in context
6. logging() logs both errors
7. Error response is sent to client
```

## Testing

### Unit Tests

The implementation includes comprehensive unit tests:

- `test_upstream_failed_flag` - Test upstream failure tracking
- `test_cache_error_flag` - Test cache error tracking
- `test_error_handling_disables_caching` - Test partial data cleanup
- `test_degradation_strategy_cache_lookup_error` - Test cache lookup error handling
- `test_degradation_strategy_cache_write_error` - Test cache write error handling
- `test_partial_data_discarded_on_error` - Test partial data cleanup
- `test_error_context_tracking` - Test multiple error tracking

### Integration Tests

Integration tests verify end-to-end error handling:

- `test_cache_error_does_not_prevent_proxying` - Verify degradation strategy
- `test_upstream_failure_tracking` - Verify upstream failure tracking
- `test_partial_data_cleanup_on_error` - Verify partial data cleanup
- `test_error_context_accumulation` - Verify multiple error tracking
- `test_cache_write_failure_does_not_affect_response` - Verify degradation
- `test_stale_cache_availability_on_upstream_failure` - Verify stale cache serving

## Monitoring and Observability

### Log Levels

- **ERROR**: Critical errors (upstream failures, cache errors)
- **WARN**: Non-critical issues (partial data discarded, cache write failures)
- **INFO**: Normal operations (request completion, cache hits/misses)
- **DEBUG**: Detailed debugging information

### Metrics

The error handling system provides data for metrics:

- Upstream failure count (via `upstream_failed` flag)
- Cache error count (via `cache_error` flag)
- Bytes received before failure
- Request completion status

### Example Log Output

**Successful Request**:
```
INFO Request completed successfully for: /file.dat
INFO   Cache hit: false
INFO   Bytes received: 1048576
```

**Request with Cache Error**:
```
ERROR Cache lookup error for /file.dat: I/O error
WARN Continuing to upstream despite cache error (degradation)
INFO Request completed successfully for: /file.dat
WARN   Cache error occurred (but request succeeded)
```

**Request with Upstream Failure**:
```
ERROR Failed to connect to upstream for /file.dat: Connection refused
ERROR Request completed with error for /file.dat: Connection refused
ERROR   Upstream failed: true
ERROR   Cache error: false
ERROR   Bytes received: 0
```

## Best Practices

1. **Always log errors with context** - Include URL, error type, and relevant state
2. **Track errors in context** - Use flags to track error state throughout request
3. **Implement degradation** - Cache failures should not prevent proxying
4. **Clean up partial data** - Discard partial cached data on errors
5. **Monitor error rates** - Track upstream failures and cache errors for alerting

## Future Enhancements

1. **Retry logic** - Implement automatic retries for transient errors
2. **Circuit breaker** - Implement circuit breaker pattern for upstream failures
3. **Metrics integration** - Export error metrics to Prometheus
4. **Custom error responses** - Customize error responses based on error type
5. **Stale cache configuration** - Add configuration for stale cache serving

## References

- [Pingora ProxyHttp Trait](https://docs.rs/pingora-proxy/latest/pingora_proxy/trait.ProxyHttp.html)
- [Pingora Error Handling](https://github.com/cloudflare/pingora/blob/main/docs/user_guide/error.md)
- [HTTP Caching RFC 9111](https://datatracker.ietf.org/doc/html/rfc9111)
- [Stale-While-Revalidate RFC 5861](https://www.rfc-editor.org/rfc/rfc5861)
