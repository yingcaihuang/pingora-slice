# Pingora Slice Module - API Documentation

This document provides API documentation for the Pingora Slice Module's public interfaces.

## Table of Contents

- [Core Types](#core-types)
- [Configuration](#configuration)
- [Proxy](#proxy)
- [Data Models](#data-models)
- [Error Handling](#error-handling)
- [Metrics](#metrics)
- [Examples](#examples)

## Core Types

### SliceProxy

Main proxy implementation that coordinates all slice module components.

```rust
pub struct SliceProxy {
    config: Arc<SliceConfig>,
    metrics: Arc<SliceMetrics>,
    // ... internal fields
}
```

#### Methods

##### `new(config: Arc<SliceConfig>) -> Self`

Creates a new SliceProxy instance with the given configuration.

**Parameters:**
- `config`: Shared configuration for the proxy

**Returns:** New `SliceProxy` instance

**Example:**
```rust
use pingora_slice::{SliceConfig, SliceProxy};
use std::sync::Arc;

let config = SliceConfig::default();
let proxy = SliceProxy::new(Arc::new(config));
```

##### `new_ctx(&self) -> SliceContext`

Creates a new request context for handling a client request.

**Returns:** New `SliceContext` instance

**Example:**
```rust
let ctx = proxy.new_ctx();
```

##### `config(&self) -> &SliceConfig`

Returns a reference to the proxy's configuration.

**Returns:** Reference to `SliceConfig`

**Example:**
```rust
let slice_size = proxy.config().slice_size;
```

##### `metrics(&self) -> &SliceMetrics`

Returns a reference to the proxy's metrics collector.

**Returns:** Reference to `SliceMetrics`

**Example:**
```rust
let stats = proxy.metrics().get_stats();
println!("Total requests: {}", stats.total_requests);
```

### SliceContext

Request context that stores state for a single client request.

```rust
pub struct SliceContext {
    slice_enabled: bool,
    metadata: Option<FileMetadata>,
    client_range: Option<ByteRange>,
    slices: Vec<SliceSpec>,
}
```

#### Methods

##### `is_slice_enabled(&self) -> bool`

Returns whether slicing is enabled for this request.

**Returns:** `true` if slicing is enabled, `false` otherwise

##### `metadata(&self) -> Option<&FileMetadata>`

Returns the file metadata if available.

**Returns:** Optional reference to `FileMetadata`

##### `has_slices(&self) -> bool`

Returns whether slices have been calculated for this request.

**Returns:** `true` if slices exist, `false` otherwise

## Configuration

### SliceConfig

Configuration for the Slice module.

```rust
pub struct SliceConfig {
    pub slice_size: usize,
    pub max_concurrent_subrequests: usize,
    pub max_retries: usize,
    pub slice_patterns: Vec<String>,
    pub enable_cache: bool,
    pub cache_ttl: u64,
    pub upstream_address: String,
    pub metrics_endpoint: Option<MetricsEndpointConfig>,
}
```

#### Methods

##### `default() -> Self`

Creates a configuration with default values.

**Returns:** `SliceConfig` with defaults

**Example:**
```rust
let config = SliceConfig::default();
```

##### `new(slice_size: usize, max_concurrent: usize, max_retries: usize) -> Result<Self>`

Creates a new configuration with specified core parameters.

**Parameters:**
- `slice_size`: Size of each slice in bytes (64KB - 10MB)
- `max_concurrent`: Maximum concurrent subrequests (> 0)
- `max_retries`: Maximum retry attempts (>= 0)

**Returns:** `Result<SliceConfig>` - Ok with config or Err with validation error

**Example:**
```rust
let config = SliceConfig::new(1024 * 1024, 4, 3)?;
```

##### `from_file(path: &str) -> Result<Self>`

Loads configuration from a YAML file.

**Parameters:**
- `path`: Path to YAML configuration file

**Returns:** `Result<SliceConfig>` - Ok with loaded config or Err on failure

**Example:**
```rust
let config = SliceConfig::from_file("pingora_slice.yaml")?;
```

##### `validate(&self) -> Result<()>`

Validates the configuration parameters.

**Returns:** `Result<()>` - Ok if valid, Err with validation error

**Example:**
```rust
config.validate()?;
```

## Data Models

### ByteRange

Represents a byte range for HTTP Range requests.

```rust
pub struct ByteRange {
    pub start: u64,
    pub end: u64,
}
```

#### Methods

##### `new(start: u64, end: u64) -> Result<Self>`

Creates a new byte range with validation.

**Parameters:**
- `start`: Starting byte position (inclusive)
- `end`: Ending byte position (inclusive)

**Returns:** `Result<ByteRange>` - Ok if valid (start <= end), Err otherwise

**Example:**
```rust
let range = ByteRange::new(0, 1023)?;  // First 1024 bytes
```

##### `size(&self) -> u64`

Returns the size of the range in bytes.

**Returns:** Size in bytes (end - start + 1)

**Example:**
```rust
let size = range.size();  // 1024
```

##### `is_valid(&self) -> bool`

Checks if the range is valid (start <= end).

**Returns:** `true` if valid, `false` otherwise

##### `from_header(header: &str) -> Result<Self>`

Parses a byte range from an HTTP Range header.

**Parameters:**
- `header`: Range header value (e.g., "bytes=0-1023")

**Returns:** `Result<ByteRange>` - Ok with parsed range or Err on invalid format

**Example:**
```rust
let range = ByteRange::from_header("bytes=0-1023")?;
```

##### `to_header(&self) -> String`

Converts the range to HTTP Range header format.

**Returns:** Range header string (e.g., "bytes=0-1023")

**Example:**
```rust
let header = range.to_header();  // "bytes=0-1023"
```

### SliceSpec

Specification for a single slice.

```rust
pub struct SliceSpec {
    pub index: usize,
    pub range: ByteRange,
    pub cached: bool,
}
```

#### Fields

- `index`: Index of this slice in the sequence (0-based)
- `range`: Byte range for this slice
- `cached`: Whether this slice is already cached

### FileMetadata

Metadata about a file from the origin server.

```rust
pub struct FileMetadata {
    pub content_length: u64,
    pub supports_range: bool,
    pub content_type: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}
```

#### Fields

- `content_length`: Total size of the file in bytes
- `supports_range`: Whether the origin server supports Range requests
- `content_type`: Content type of the file (e.g., "video/mp4")
- `etag`: ETag for cache validation
- `last_modified`: Last modified timestamp

## Error Handling

### SliceError

Error type for all slice module operations.

```rust
pub enum SliceError {
    ConfigError(String),
    IoError(std::io::Error),
    ParseError(String),
    ValidationError(String),
    NetworkError(String),
    CacheError(String),
    // ... other variants
}
```

#### Methods

##### `to_string(&self) -> String`

Converts the error to a human-readable string.

**Returns:** Error message string

### Result Type

Type alias for results in the slice module.

```rust
pub type Result<T> = std::result::Result<T, SliceError>;
```

## Metrics

### SliceMetrics

Collects and exposes runtime metrics.

```rust
pub struct SliceMetrics {
    // Atomic counters for thread-safe metrics collection
}
```

#### Methods

##### `new() -> Self`

Creates a new metrics collector.

**Returns:** New `SliceMetrics` instance

##### `record_request(&self, sliced: bool)`

Records a request.

**Parameters:**
- `sliced`: Whether the request was handled with slicing

**Example:**
```rust
metrics.record_request(true);
```

##### `record_cache_hit(&self)`

Records a cache hit.

**Example:**
```rust
metrics.record_cache_hit();
```

##### `record_cache_miss(&self)`

Records a cache miss.

**Example:**
```rust
metrics.record_cache_miss();
```

##### `record_subrequest(&self, success: bool)`

Records a subrequest.

**Parameters:**
- `success`: Whether the subrequest succeeded

**Example:**
```rust
metrics.record_subrequest(true);
```

##### `get_stats(&self) -> MetricsSnapshot`

Returns a snapshot of current metrics.

**Returns:** `MetricsSnapshot` with current values

**Example:**
```rust
let stats = metrics.get_stats();
println!("Cache hit rate: {}%", stats.cache_hit_rate());
```

### MetricsSnapshot

Snapshot of metrics at a point in time.

```rust
pub struct MetricsSnapshot {
    pub total_requests: u64,
    pub sliced_requests: u64,
    pub passthrough_requests: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub total_subrequests: u64,
    pub failed_subrequests: u64,
    // ... other fields
}
```

#### Methods

##### `cache_hit_rate(&self) -> f64`

Calculates the cache hit rate as a percentage.

**Returns:** Cache hit rate (0.0 - 100.0)

##### `subrequest_failure_rate(&self) -> f64`

Calculates the subrequest failure rate as a percentage.

**Returns:** Failure rate (0.0 - 100.0)

## Examples

### Complete Example

```rust
use pingora_slice::{SliceConfig, SliceProxy, ByteRange};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = SliceConfig::from_file("pingora_slice.yaml")?;
    
    // Validate configuration
    config.validate()?;
    
    // Create proxy
    let proxy = SliceProxy::new(Arc::new(config));
    
    // Create request context
    let ctx = proxy.new_ctx();
    
    // Check if slicing is enabled
    if ctx.is_slice_enabled() {
        println!("Slicing is enabled for this request");
    }
    
    // Access metrics
    let stats = proxy.metrics().get_stats();
    println!("Total requests: {}", stats.total_requests);
    println!("Cache hit rate: {:.2}%", stats.cache_hit_rate());
    
    // Work with byte ranges
    let range = ByteRange::new(0, 1048575)?;  // First 1MB
    println!("Range size: {} bytes", range.size());
    println!("Range header: {}", range.to_header());
    
    Ok(())
}
```

### Configuration Example

```rust
use pingora_slice::SliceConfig;

fn create_custom_config() -> Result<SliceConfig, Box<dyn std::error::Error>> {
    let config = SliceConfig {
        slice_size: 512 * 1024,  // 512KB
        max_concurrent_subrequests: 8,
        max_retries: 5,
        slice_patterns: vec![
            "^/downloads/.*".to_string(),
            "^/videos/.*\\.mp4$".to_string(),
        ],
        enable_cache: true,
        cache_ttl: 7200,  // 2 hours
        upstream_address: "origin.example.com:80".to_string(),
        metrics_endpoint: None,
    };
    
    // Validate before use
    config.validate()?;
    
    Ok(config)
}
```

### Metrics Example

```rust
use pingora_slice::{SliceProxy, SliceConfig};
use std::sync::Arc;

fn monitor_metrics(proxy: &SliceProxy) {
    let stats = proxy.metrics().get_stats();
    
    println!("=== Metrics Summary ===");
    println!("Total requests: {}", stats.total_requests);
    println!("Sliced requests: {}", stats.sliced_requests);
    println!("Passthrough requests: {}", stats.passthrough_requests);
    println!();
    println!("Cache hits: {}", stats.cache_hits);
    println!("Cache misses: {}", stats.cache_misses);
    println!("Cache hit rate: {:.2}%", stats.cache_hit_rate());
    println!();
    println!("Total subrequests: {}", stats.total_subrequests);
    println!("Failed subrequests: {}", stats.failed_subrequests);
    println!("Failure rate: {:.2}%", stats.subrequest_failure_rate());
}
```

### Error Handling Example

```rust
use pingora_slice::{SliceConfig, SliceError};

fn load_config_with_error_handling(path: &str) {
    match SliceConfig::from_file(path) {
        Ok(config) => {
            println!("Configuration loaded successfully");
            match config.validate() {
                Ok(_) => println!("Configuration is valid"),
                Err(SliceError::ValidationError(msg)) => {
                    eprintln!("Validation error: {}", msg);
                }
                Err(e) => eprintln!("Unexpected error: {}", e),
            }
        }
        Err(SliceError::IoError(e)) => {
            eprintln!("Failed to read config file: {}", e);
        }
        Err(SliceError::ParseError(msg)) => {
            eprintln!("Failed to parse config: {}", msg);
        }
        Err(e) => {
            eprintln!("Unexpected error: {}", e);
        }
    }
}
```

## See Also

- [README.md](../README.md) - Main documentation
- [CONFIGURATION.md](CONFIGURATION.md) - Configuration guide
- [DEPLOYMENT.md](DEPLOYMENT.md) - Deployment guide
- [Examples](../examples/) - Code examples
