# Cache Manager Implementation

## Overview

The `SliceCache` module provides caching functionality for individual file slices in the Pingora Slice system. It implements an in-memory cache with TTL-based expiration.

## Features

### 1. Unique Cache Key Generation
- Format: `{url}:slice:{start}:{end}`
- Ensures uniqueness across different URLs and byte ranges
- Example: `http://example.com/file.bin:slice:0:1048575`

### 2. Single Slice Operations
- `lookup_slice()`: Retrieve a single cached slice
- `store_slice()`: Store a slice in the cache
- Automatic expiration checking on lookup

### 3. Batch Operations
- `lookup_multiple()`: Efficiently look up multiple slices at once
- Returns a HashMap of slice indices to cached data
- Only returns slices that are found and not expired

### 4. Error Handling
- Cache errors are logged as warnings but don't fail requests
- Graceful degradation: continues without cache on errors
- Thread-safe using RwLock for concurrent access

### 5. TTL-Based Expiration
- Configurable time-to-live for cached entries
- Automatic cleanup of expired entries (periodic)
- Expiration checked on every lookup

## Implementation Details

### Data Structures

```rust
struct CacheEntry {
    data: Bytes,
    expires_at: SystemTime,
}

pub struct SliceCache {
    storage: Arc<RwLock<HashMap<String, CacheEntry>>>,
    ttl: Duration,
}
```

### Thread Safety
- Uses `Arc<RwLock<HashMap>>` for concurrent access
- Multiple readers can access simultaneously
- Writers get exclusive access

### Memory Management
- Periodic cleanup of expired entries (every 100 insertions)
- Entries are removed on expiration check
- No maximum size limit (can be added if needed)

## Usage Example

```rust
use pingora_slice::{SliceCache, ByteRange};
use std::time::Duration;
use bytes::Bytes;

// Create cache with 1 hour TTL
let cache = SliceCache::new(Duration::from_secs(3600));

// Store a slice
let range = ByteRange::new(0, 1048575)?;
let data = Bytes::from(vec![1, 2, 3, 4, 5]);
cache.store_slice("http://example.com/file.bin", &range, data).await?;

// Look up a slice
if let Some(cached_data) = cache.lookup_slice("http://example.com/file.bin", &range).await? {
    println!("Cache hit! Size: {}", cached_data.len());
}

// Batch lookup
let ranges = vec![
    ByteRange::new(0, 1048575)?,
    ByteRange::new(1048576, 2097151)?,
];
let cached = cache.lookup_multiple("http://example.com/file.bin", &ranges).await;
println!("Found {} slices in cache", cached.len());
```

## Testing

The implementation includes comprehensive tests:

1. **Cache Key Uniqueness**: Verifies different URLs/ranges produce different keys
2. **Store and Lookup**: Tests basic cache operations
3. **Cache Miss**: Verifies behavior when slice not in cache
4. **Batch Lookup**: Tests multiple slice retrieval
5. **Expiration**: Verifies TTL-based expiration works correctly

All tests pass successfully.

## Requirements Satisfied

This implementation satisfies the following requirements from the design document:

- **Requirement 7.1**: Stores slices with unique cache keys
- **Requirement 7.2**: Generates unique keys including URL and byte range
- **Requirement 7.3**: Checks cache before creating subrequests
- **Requirement 7.4**: Uses cached data when available, only requests missing slices
- **Requirement 7.5**: Logs warnings on cache errors but continues processing

## Future Enhancements

Potential improvements for production use:

1. **Size Limits**: Add maximum cache size with LRU eviction
2. **Persistence**: Option to persist cache to disk
3. **Metrics**: Track cache hit/miss rates
4. **Compression**: Compress cached data to save memory
5. **Distributed Cache**: Support for Redis or other distributed caches
6. **Cache Warming**: Pre-populate cache with frequently accessed slices
