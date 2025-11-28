# Pingora Slice Module

[English](README.md) | [中文](README_zh.md)

A high-performance slice module for Pingora proxy server that automatically splits large file requests into multiple small Range requests, similar to Nginx Slice module. Built with Rust for safety, performance, and reliability.

## Overview

The Pingora Slice Module transparently intercepts large file requests and splits them into smaller, manageable chunks (slices). Each slice is fetched independently using HTTP Range requests, cached separately, and assembled back into the complete response for the client. This approach provides several benefits:

- **Improved Cache Efficiency**: Small slices are easier to cache and reuse across different requests
- **Reduced Origin Load**: Partial cache hits mean fewer bytes need to be fetched from origin
- **Better Reliability**: Failed slices can be retried independently without re-fetching the entire file
- **Concurrent Fetching**: Multiple slices can be fetched in parallel for faster response times
- **Bandwidth Optimization**: Only fetch the slices that aren't already cached

## Features

### Core Features
- **Automatic Request Slicing**: Transparently splits large file requests into smaller chunks without client awareness
- **Concurrent Fetching**: Fetches multiple slices in parallel with configurable concurrency limits
- **Range Request Support**: Correctly handles client Range requests (partial content, byte ranges)
- **Retry Logic**: Automatic retry with exponential backoff for failed subrequests
- **Flexible Configuration**: YAML-based configuration for slice size, concurrency, caching, and URL patterns
- **Property-Based Testing**: Comprehensive test suite with property-based tests for correctness guarantees
- **Error Handling**: Robust error handling with fallback to normal proxy mode when needed

### Caching Features
- **Two-Tier Cache System**: L1 (memory) + L2 (disk) for optimal performance and persistence
  - **L1 Memory Cache**: Microsecond-level access for hot data with LRU eviction
  - **L2 Disk Cache**: Persistent storage that survives restarts
  - **Automatic Promotion**: L2 hits are automatically promoted to L1
  - **Async Disk Operations**: Non-blocking disk writes for minimal latency impact
- **Smart Caching**: Caches individual slices for efficient reuse and partial cache hits
- **Cache Persistence**: Cached data survives service restarts (L2 cache)

### Cache Management
- **HTTP PURGE Support**: Industry-standard cache invalidation via HTTP PURGE method
  - Purge specific URLs or all cache
  - Token-based authentication
  - Prometheus metrics for purge operations
- **Flexible Purge Options**: Single URL, URL prefix, or全部缓存清除

### Monitoring & Observability
- **Metrics Endpoint**: Exposes detailed metrics in Prometheus format
  - Cache hit/miss rates (L1 and L2)
  - Slice processing statistics
  - Purge operation metrics
  - Performance metrics (latency, throughput)
- **Structured Logging**: Comprehensive logging with tracing support

## Table of Contents

- [Quick Start](#quick-start)
- [How It Works](#how-it-works)
- [Project Structure](#project-structure)
- [Core Components](#core-components)
- [Configuration](#configuration)
- [Building and Running](#building-and-running)
- [Metrics and Monitoring](#metrics-and-monitoring)
- [Testing](#testing)
- [Deployment Guide](#deployment-guide)
- [Performance Tuning](#performance-tuning)
- [Troubleshooting](#troubleshooting)
- [Requirements Coverage](#requirements-coverage)
- [Contributing](#contributing)
- [License](#license)

## Quick Start

```bash
# 1. Clone the repository
git clone <repository-url>
cd pingora-slice

# 2. Build the project
cargo build --release

# 3. Create or edit configuration file
cp examples/pingora_slice.yaml pingora_slice.yaml
# Edit pingora_slice.yaml to set your upstream_address

# 4. Run the server
./target/release/pingora-slice

# 5. Test with a request
curl -v http://localhost:8080/large-file.bin

# 6. Check metrics (if enabled)
curl http://localhost:9090/metrics

# 7. Purge cache (if enabled)
curl -X PURGE http://localhost:8080/large-file.bin \
  -H "Authorization: Bearer your-secret-token"
```

## How It Works

### Request Flow

1. **Client Request**: Client sends a normal GET request for a file
2. **Request Analysis**: Module checks if the request should be sliced based on URL patterns
3. **Metadata Fetch**: Sends HEAD request to origin to get file size and check Range support
4. **Slice Calculation**: Divides the file into slices based on configured slice size
5. **Cache Lookup**: Checks which slices are already cached
6. **Concurrent Fetching**: Fetches missing slices in parallel with Range requests
7. **Response Assembly**: Assembles slices in correct order and streams to client
8. **Cache Storage**: Stores newly fetched slices for future requests

### Example Scenario

```
Client requests: GET /video.mp4 (100MB file)
Slice size: 1MB
Slices needed: 100 slices

Cache status:
- Slices 0-49: Cached (from previous request)
- Slices 50-99: Not cached

Action:
1. Return cached slices 0-49 immediately
2. Fetch slices 50-99 from origin (4 concurrent requests)
3. Stream all slices to client in order
4. Cache slices 50-99 for future use

Result:
- Only 50MB fetched from origin (50% cache hit)
- Client receives complete 100MB file
- Future requests can use all 100 cached slices
```

## Project Structure

```
pingora-slice/
├── src/
│   ├── lib.rs                  # Main library entry point and module exports
│   ├── main.rs                 # Server binary entry point
│   ├── config.rs               # Configuration management and validation
│   ├── models.rs               # Core data structures (ByteRange, SliceSpec, FileMetadata)
│   ├── error.rs                # Error types and handling
│   ├── proxy.rs                # Main SliceProxy implementation (ProxyHttp trait)
│   ├── request_analyzer.rs     # Request analysis and pattern matching
│   ├── metadata_fetcher.rs     # Origin metadata fetching (HEAD requests)
│   ├── slice_calculator.rs     # Slice calculation logic
│   ├── subrequest_manager.rs   # Concurrent subrequest management
│   ├── response_assembler.rs   # Response assembly and streaming
│   ├── cache.rs                # Cache management (SliceCache)
│   ├── metrics.rs              # Metrics collection (SliceMetrics)
│   └── metrics_endpoint.rs     # HTTP metrics endpoint server
├── tests/
│   ├── prop_*.rs               # Property-based tests (20 properties)
│   └── test_*.rs               # Unit and integration tests
├── examples/
│   ├── pingora_slice.yaml      # Example configuration file (heavily commented)
│   ├── server_example.rs       # Server startup example
│   ├── cache_example.rs        # Cache usage example
│   ├── metrics_example.rs      # Metrics collection example
│   └── *.rs                    # Other component examples
├── docs/
│   ├── *.md                    # Detailed implementation documentation
│   └── DEPLOYMENT.md           # Deployment guide (see below)
├── Cargo.toml                  # Project dependencies and metadata
├── pingora_slice.yaml          # Default configuration file
└── README.md                   # This file
```

## Core Components

### 1. SliceProxy
Main proxy implementation that integrates with Pingora's `ProxyHttp` trait. Coordinates all other components and manages the request lifecycle.

**Key Methods:**
- `new()`: Creates a new proxy instance with configuration
- `new_ctx()`: Creates a new request context
- `request_filter()`: Decides whether to enable slicing for a request
- `handle_slice_request()`: Handles the complete slice request flow

### 2. RequestAnalyzer
Analyzes incoming requests to determine if slicing should be enabled.

**Checks:**
- Request method is GET
- URL matches configured patterns
- Request doesn't already contain Range header
- Returns decision and extracts client range if present

### 3. MetadataFetcher
Fetches file metadata from origin server using HEAD requests.

**Retrieves:**
- Content-Length (file size)
- Accept-Ranges header (Range support)
- Content-Type, ETag, Last-Modified
- Validates origin supports Range requests

### 4. SliceCalculator
Calculates slice specifications based on file size and configuration.

**Functions:**
- Divides file into equal-sized slices
- Handles last slice (may be smaller)
- Supports partial requests (client Range)
- Ensures no gaps or overlaps

### 5. SubrequestManager
Manages concurrent fetching of slices from origin.

**Features:**
- Concurrent request limiting (semaphore-based)
- Retry logic with exponential backoff
- Content-Range validation
- Error handling and propagation

### 6. ResponseAssembler
Assembles slices and streams response to client.

**Capabilities:**
- Ordered streaming (maintains byte order)
- Buffering for out-of-order slices
- Response header generation
- Supports both 200 and 206 responses

### 7. SliceCache
Manages caching of individual slices.

**Operations:**
- Generate unique cache keys (URL + byte range)
- Store slices with TTL
- Lookup single or multiple slices
- Handle cache errors gracefully

### 8. SliceMetrics
Collects and exposes runtime metrics.

**Tracks:**
- Request counts (total, sliced, passthrough)
- Cache statistics (hits, misses, hit rate)
- Subrequest statistics (total, failed, failure rate)
- Byte transfers (origin, cache, client)
- Latencies (request, subrequest, assembly)

### 9. MetricsEndpoint
HTTP server that exposes metrics in Prometheus format.

**Endpoints:**
- `/` - Index page with links
- `/metrics` - Prometheus format metrics
- `/health` - Health check endpoint

## Core Data Structures

### ByteRange
Represents a byte range for HTTP Range requests.

```rust
pub struct ByteRange {
    pub start: u64,  // Starting byte position (inclusive)
    pub end: u64,    // Ending byte position (inclusive)
}
```

**Methods:**
- `new(start, end)`: Creates a new range with validation
- `size()`: Returns the size of the range in bytes
- `is_valid()`: Checks if the range is valid (start <= end)
- `from_header(header)`: Parses from HTTP Range header
- `to_header()`: Converts to HTTP Range header format

### SliceSpec
Specification for a single slice.

```rust
pub struct SliceSpec {
    pub index: usize,      // Index of this slice in the sequence
    pub range: ByteRange,  // Byte range for this slice
    pub cached: bool,      // Whether this slice is already cached
}
```

### FileMetadata
Metadata about a file from the origin server.

```rust
pub struct FileMetadata {
    pub content_length: u64,           // Total size of the file in bytes
    pub supports_range: bool,          // Whether origin supports Range requests
    pub content_type: Option<String>,  // Content type of the file
    pub etag: Option<String>,          // ETag for cache validation
    pub last_modified: Option<String>, // Last modified timestamp
}
```

### SliceConfig
Configuration for the Slice module.

```rust
pub struct SliceConfig {
    pub slice_size: usize,                    // Size of each slice (64KB - 10MB)
    pub max_concurrent_subrequests: usize,    // Max concurrent subrequests
    pub max_retries: usize,                   // Max retry attempts
    pub slice_patterns: Vec<String>,          // URL patterns for slicing
    pub enable_cache: bool,                   // Enable caching
    pub cache_ttl: u64,                       // Cache TTL in seconds
    pub upstream_address: String,             // Upstream origin server
    pub metrics_endpoint: Option<MetricsEndpointConfig>,  // Metrics config
}
```

**Validation Rules:**
- `slice_size`: Must be between 64KB (65536) and 10MB (10485760)
- `max_concurrent_subrequests`: Must be greater than 0
- `max_retries`: Must be greater than or equal to 0
- `cache_ttl`: Must be greater than 0 when caching is enabled

## Configuration

The module is configured using a YAML file. See `examples/pingora_slice.yaml` for a complete, heavily commented configuration example.

### Configuration File Location

The server looks for configuration in the following order:
1. Path specified as command-line argument: `./pingora-slice /path/to/config.yaml`
2. `pingora_slice.yaml` in the current directory
3. Falls back to default configuration if no file is found

### Basic Configuration Example

```yaml
# Slice size in bytes (64KB to 10MB)
slice_size: 1048576  # 1MB

# Maximum concurrent subrequests to origin
max_concurrent_subrequests: 4

# Maximum retry attempts for failed subrequests
max_retries: 3

# URL patterns that enable slicing (regex)
slice_patterns:
  - "^/large-files/.*"
  - "^/downloads/.*\\.bin$"

# Cache configuration
enable_cache: true
cache_ttl: 3600  # 1 hour in seconds

# Two-tier cache configuration
l1_cache_size_bytes: 104857600  # 100MB memory cache
l2_cache_dir: "/var/cache/pingora-slice"
enable_l2_cache: true

# Upstream origin server
upstream_address: "origin.example.com:80"

# Optional metrics endpoint
metrics_endpoint:
  enabled: true
  address: "127.0.0.1:9090"

# Optional cache purge configuration
purge:
  enabled: true
  auth_token: "your-secret-token-here"
  enable_metrics: true
```

### Configuration Parameters

| Parameter | Type | Default | Valid Range | Description |
|-----------|------|---------|-------------|-------------|
| `slice_size` | integer | 1048576 | 65536 - 10485760 | Size of each slice in bytes |
| `max_concurrent_subrequests` | integer | 4 | > 0 | Maximum concurrent subrequests |
| `max_retries` | integer | 3 | >= 0 | Maximum retry attempts |
| `slice_patterns` | array | [] | - | URL regex patterns for slicing |
| `enable_cache` | boolean | true | - | Enable slice caching |
| `cache_ttl` | integer | 3600 | > 0 | Cache TTL in seconds |
| `l1_cache_size_bytes` | integer | 104857600 | > 0 | L1 (memory) cache size in bytes |
| `l2_cache_dir` | string | "/var/cache/pingora-slice" | - | L2 (disk) cache directory |
| `enable_l2_cache` | boolean | true | - | Enable L2 disk cache |
| `upstream_address` | string | "127.0.0.1:8080" | - | Origin server address |
| `metrics_endpoint.enabled` | boolean | false | - | Enable metrics endpoint |
| `metrics_endpoint.address` | string | "127.0.0.1:9090" | - | Metrics server bind address |
| `purge.enabled` | boolean | false | - | Enable HTTP PURGE method |
| `purge.auth_token` | string | null | - | Authentication token for PURGE requests |
| `purge.enable_metrics` | boolean | true | - | Enable Prometheus metrics for purge operations |

### Validation Rules

Configuration is validated on load. The server will refuse to start if validation fails:

- ✓ `slice_size` must be between 64KB (65536) and 10MB (10485760)
- ✓ `max_concurrent_subrequests` must be greater than 0
- ✓ `max_retries` must be greater than or equal to 0
- ✓ `cache_ttl` must be greater than 0 when caching is enabled
- ✓ `upstream_address` must be a valid address format
- ✓ `slice_patterns` must be valid regex patterns

### URL Pattern Matching

URL patterns use standard Rust regex syntax:

```yaml
slice_patterns:
  # Match all files in /large-files/ directory
  - "^/large-files/.*"
  
  # Match binary files in /downloads/
  - "^/downloads/.*\\.bin$"
  
  # Match video files
  - "^/videos/.*\\.(mp4|mkv|avi)$"
  
  # Match ISO images
  - "^/isos/.*\\.iso$"
```

**Empty list behavior:** If `slice_patterns` is empty (`[]`), all GET requests without Range headers will be considered for slicing.

### Configuration Presets

#### High-Performance Setup (Fast Networks)
```yaml
slice_size: 4194304              # 4MB
max_concurrent_subrequests: 8    # Higher concurrency
max_retries: 2                   # Fewer retries
cache_ttl: 86400                 # 24 hours
```

#### Conservative Setup (Slow/Unreliable Networks)
```yaml
slice_size: 262144               # 256KB
max_concurrent_subrequests: 2    # Lower concurrency
max_retries: 5                   # More retries
cache_ttl: 3600                  # 1 hour
```

#### Minimal Caching (Frequently Changing Content)
```yaml
slice_size: 1048576              # 1MB
max_concurrent_subrequests: 4    # Standard
max_retries: 3                   # Standard
cache_ttl: 300                   # 5 minutes
```

## Building and Running

### Prerequisites

- **Rust**: Version 1.70 or later (install from [rustup.rs](https://rustup.rs))
- **Cargo**: Comes with Rust installation
- **Operating System**: Linux, macOS, or Windows (Linux recommended for production)

### Building

```bash
# Clone the repository
git clone <repository-url>
cd pingora-slice

# Build in debug mode (faster compilation, slower runtime)
cargo build

# Build in release mode (optimized for performance)
cargo build --release

# Build with all features
cargo build --release --all-features

# Check for compilation errors without building
cargo check
```

**Build artifacts:**
- Debug: `target/debug/pingora-slice`
- Release: `target/release/pingora-slice`

### Running the Server

#### Using Default Configuration

The server looks for `pingora_slice.yaml` in the current directory:

```bash
# Run with cargo (debug mode)
cargo run

# Run release binary directly
./target/release/pingora-slice
```

#### Using Custom Configuration

Specify a custom configuration file path:

```bash
# With cargo
cargo run -- /path/to/config.yaml

# With release binary
./target/release/pingora-slice /path/to/config.yaml

# Using example configuration
cargo run -- examples/pingora_slice.yaml
```

#### Running in Background

```bash
# Using nohup
nohup ./target/release/pingora-slice &

# Using systemd (see Deployment Guide below)
systemctl start pingora-slice

# Using screen
screen -dmS pingora-slice ./target/release/pingora-slice

# Using tmux
tmux new-session -d -s pingora-slice './target/release/pingora-slice'
```

### Running Examples

The project includes several examples demonstrating different components:

```bash
# Server startup example
cargo run --example server_example

# Cache usage example
cargo run --example cache_example

# Metrics collection example
cargo run --example metrics_example

# Metrics endpoint example
cargo run --example metrics_endpoint_example

# List all examples
cargo run --example
```

### Development Mode

For development with automatic recompilation:

```bash
# Install cargo-watch
cargo install cargo-watch

# Run with auto-reload on file changes
cargo watch -x run

# Run tests on file changes
cargo watch -x test
```

See [Server Startup Guide](docs/server_startup.md) for detailed information.

## Metrics and Monitoring

The slice module provides comprehensive metrics for monitoring performance and behavior.

### Enabling Metrics Endpoint

Configure the metrics endpoint in your YAML file:

```yaml
metrics_endpoint:
  enabled: true
  address: "127.0.0.1:9090"  # Bind to localhost only (recommended)
```

**Security Note:** Binding to `127.0.0.1` restricts access to the local machine. For external access, use a reverse proxy with authentication.

### Available Endpoints

| Endpoint | Description |
|----------|-------------|
| `http://127.0.0.1:9090/` | Index page with links to all endpoints |
| `http://127.0.0.1:9090/metrics` | Prometheus format metrics |
| `http://127.0.0.1:9090/health` | Health check endpoint (returns 200 OK) |

### Exposed Metrics

#### Request Metrics
```
pingora_slice_requests_total              # Total requests processed
pingora_slice_sliced_requests_total       # Requests handled with slicing
pingora_slice_passthrough_requests_total  # Requests passed through without slicing
```

#### Cache Metrics
```
pingora_slice_cache_hits_total            # Cache hits
pingora_slice_cache_misses_total          # Cache misses
pingora_slice_cache_hit_rate              # Cache hit rate (0-100%)
```

#### Subrequest Metrics
```
pingora_slice_subrequests_total           # Total subrequests sent
pingora_slice_failed_subrequests_total    # Failed subrequests
pingora_slice_subrequest_failure_rate     # Failure rate (0-100%)
```

#### Byte Transfer Metrics
```
pingora_slice_bytes_from_origin_total     # Bytes fetched from origin
pingora_slice_bytes_from_cache_total      # Bytes served from cache
pingora_slice_bytes_to_client_total       # Bytes sent to clients
```

#### Latency Metrics
```
pingora_slice_request_duration_ms_avg     # Average request duration (ms)
pingora_slice_subrequest_duration_ms_avg  # Average subrequest duration (ms)
pingora_slice_assembly_duration_ms_avg    # Average assembly duration (ms)
```

### Using Metrics

#### Manual Inspection

```bash
# View all metrics
curl http://127.0.0.1:9090/metrics

# Filter specific metrics
curl http://127.0.0.1:9090/metrics | grep cache_hit

# Check health
curl http://127.0.0.1:9090/health
```

#### Prometheus Integration

Add to your `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'pingora-slice'
    static_configs:
      - targets: ['localhost:9090']
    scrape_interval: 15s
```

#### Grafana Dashboard

Create a dashboard with panels for:
- Request rate (requests/sec)
- Cache hit rate (%)
- Subrequest failure rate (%)
- Bandwidth usage (bytes/sec)
- Latency percentiles (p50, p95, p99)

### Example Metrics Output

```
# HELP pingora_slice_requests_total Total number of requests
# TYPE pingora_slice_requests_total counter
pingora_slice_requests_total 1523

# HELP pingora_slice_cache_hit_rate Cache hit rate percentage
# TYPE pingora_slice_cache_hit_rate gauge
pingora_slice_cache_hit_rate 78.5

# HELP pingora_slice_bytes_from_cache_total Total bytes from cache
# TYPE pingora_slice_bytes_from_cache_total counter
pingora_slice_bytes_from_cache_total 15728640000
```

### Running Metrics Example

```bash
# Start the metrics endpoint example
cargo run --example metrics_endpoint_example

# In another terminal, access metrics
curl http://127.0.0.1:9090/metrics
```

See [Metrics Endpoint Documentation](docs/metrics_endpoint_implementation.md) for implementation details.

## Testing

The project includes comprehensive test coverage with both unit tests and property-based tests.

### Running Tests

```bash
# Run all tests
cargo test

# Run all tests with output
cargo test -- --nocapture

# Run only library unit tests
cargo test --lib

# Run specific test file
cargo test --test test_config_loading

# Run property-based tests only
cargo test --test 'prop_*'

# Run with specific test name pattern
cargo test cache

# Run tests in parallel (default)
cargo test

# Run tests sequentially
cargo test -- --test-threads=1
```

### Test Categories

#### Unit Tests
Located in `tests/test_*.rs` files:
- `test_config_loading.rs` - Configuration parsing and validation
- `test_metadata_fetcher.rs` - Metadata fetching logic
- `test_subrequest_manager.rs` - Subrequest management
- `test_cache_integration.rs` - Cache operations
- `test_error_handling.rs` - Error handling scenarios
- `test_handle_slice_request.rs` - Request handling flow
- `test_metrics_endpoint.rs` - Metrics endpoint functionality
- `test_integration.rs` - End-to-end integration tests

#### Property-Based Tests
Located in `tests/prop_*.rs` files (20 properties):

**Configuration Properties:**
- `prop_config_validation.rs` - Property 1: Configuration value range validation

**Request Analysis Properties:**
- `prop_request_analysis.rs` - Property 2: Range request passthrough
- Property 3: URL pattern matching consistency

**Slice Calculation Properties:**
- `prop_slice_coverage.rs` - Property 4: Slice coverage completeness
- `prop_slice_non_overlapping.rs` - Property 5: Slice non-overlapping
- `prop_range_header_format.rs` - Property 6: Range header format correctness
- `prop_partial_request_slicing.rs` - Property 18: Partial request slicing

**Concurrent Fetching Properties:**
- `prop_concurrent_limit.rs` - Property 7: Concurrent limit enforcement
- `prop_retry_limit.rs` - Property 8: Retry count limit
- `prop_failure_propagation.rs` - Property 9: Failure propagation

**Response Assembly Properties:**
- `prop_byte_order_preservation.rs` - Property 10: Byte order preservation (critical)
- `prop_response_header_completeness.rs` - Property 11: Response header completeness
- `prop_206_response_format.rs` - Property 19: 206 response format

**Cache Properties:**
- `prop_cache_key_uniqueness.rs` - Property 12: Cache key uniqueness
- `prop_cache_hit_correctness.rs` - Property 13: Cache hit correctness
- `prop_partial_cache_hit.rs` - Property 14: Partial cache hit optimization

**Error Handling Properties:**
- `prop_4xx_error_passthrough.rs` - Property 15: 4xx error passthrough
- `prop_content_range_validation.rs` - Property 16: Content-Range validation
- `prop_invalid_range_error.rs` - Property 20: Invalid Range error handling

**Range Request Properties:**
- `prop_range_parsing.rs` - Property 17: Range parsing correctness

### Property-Based Testing

The project uses [proptest](https://github.com/proptest-rs/proptest) for property-based testing. Each property test:
- Runs 100+ iterations with random inputs
- Validates correctness properties across all inputs
- Is tagged with the corresponding design property number
- References specific requirements from the specification

Example property test structure:
```rust
// Feature: pingora-slice, Property 4: 分片覆盖完整性
// Validates: Requirements 4.1, 4.2
#[test]
fn prop_slice_coverage_completeness() {
    // Test implementation with proptest
}
```

### Test Coverage

```bash
# Install tarpaulin for coverage
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --out Html --output-dir coverage

# View coverage report
open coverage/index.html
```

### Continuous Integration

Tests are automatically run on:
- Every commit (pre-commit hook)
- Pull requests
- Main branch merges

### Writing New Tests

When adding new functionality:
1. Write unit tests for specific behaviors
2. Write property tests for universal properties
3. Update integration tests if needed
4. Ensure all tests pass before committing

See individual test files for examples and patterns.

## Deployment Guide

### Production Deployment

See [DEPLOYMENT.md](docs/DEPLOYMENT.md) for comprehensive deployment instructions including:
- System requirements and dependencies
- Installation steps
- Systemd service configuration
- Nginx reverse proxy setup
- Security hardening
- Monitoring and logging
- Backup and recovery
- Performance tuning

### Quick Production Setup

```bash
# 1. Build release binary
cargo build --release

# 2. Create deployment directory
sudo mkdir -p /opt/pingora-slice
sudo cp target/release/pingora-slice /opt/pingora-slice/
sudo cp pingora_slice.yaml /opt/pingora-slice/

# 3. Create systemd service
sudo cp deployment/pingora-slice.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable pingora-slice
sudo systemctl start pingora-slice

# 4. Verify service is running
sudo systemctl status pingora-slice
curl http://localhost:9090/health
```

## Performance Tuning

### Slice Size Selection

Choose slice size based on your use case:

| Use Case | Recommended Size | Reasoning |
|----------|------------------|-----------|
| Small files (< 10MB) | 256KB - 512KB | Better granularity, more cache hits |
| Medium files (10-100MB) | 1MB - 2MB | Good balance |
| Large files (> 100MB) | 2MB - 4MB | Fewer requests, less overhead |
| Very large files (> 1GB) | 4MB - 10MB | Minimize request overhead |
| Slow networks | 256KB - 512KB | Smaller chunks, better reliability |
| Fast networks | 2MB - 4MB | Maximize throughput |

### Concurrency Tuning

Adjust concurrent subrequests based on:

```yaml
# Low-end origin (shared hosting, limited bandwidth)
max_concurrent_subrequests: 2

# Standard origin (dedicated server, moderate bandwidth)
max_concurrent_subrequests: 4

# High-performance origin (CDN, high bandwidth)
max_concurrent_subrequests: 8

# Very high-performance origin (multiple servers, load balanced)
max_concurrent_subrequests: 16
```

### Cache Tuning

Optimize cache settings:

```yaml
# Frequently changing content
cache_ttl: 300  # 5 minutes

# Moderately stable content
cache_ttl: 3600  # 1 hour

# Static content
cache_ttl: 86400  # 24 hours

# Rarely changing content
cache_ttl: 604800  # 7 days
```

### System-Level Tuning

#### Linux Kernel Parameters

```bash
# Increase file descriptor limits
ulimit -n 65536

# TCP tuning for high throughput
sudo sysctl -w net.core.rmem_max=134217728
sudo sysctl -w net.core.wmem_max=134217728
sudo sysctl -w net.ipv4.tcp_rmem="4096 87380 134217728"
sudo sysctl -w net.ipv4.tcp_wmem="4096 65536 134217728"
```

#### Resource Limits

```bash
# Set in /etc/security/limits.conf
pingora-slice soft nofile 65536
pingora-slice hard nofile 65536
```

### Monitoring Performance

Key metrics to monitor:
- **Cache hit rate**: Should be > 70% for optimal performance
- **Subrequest failure rate**: Should be < 1%
- **Average request duration**: Baseline and monitor for increases
- **Bytes from cache vs origin**: Higher cache ratio is better

## Troubleshooting

### Common Issues

#### 1. Server Won't Start

**Symptom:** Server exits immediately after starting

**Possible causes:**
- Invalid configuration file
- Port already in use
- Insufficient permissions

**Solutions:**
```bash
# Check configuration validity
cargo run -- --check-config

# Check if port is in use
sudo lsof -i :8080
sudo lsof -i :9090

# Check logs
journalctl -u pingora-slice -n 50

# Run with verbose logging
RUST_LOG=debug ./target/release/pingora-slice
```

#### 2. High Cache Miss Rate

**Symptom:** Cache hit rate < 50%

**Possible causes:**
- Slice size too large
- Cache TTL too short
- Insufficient cache storage
- Varying query parameters in URLs

**Solutions:**
```yaml
# Reduce slice size for better granularity
slice_size: 524288  # 512KB

# Increase cache TTL
cache_ttl: 7200  # 2 hours

# Normalize URLs (strip query parameters)
# Increase cache storage capacity
```

#### 3. Origin Server Overload

**Symptom:** Many failed subrequests, slow responses

**Possible causes:**
- Too many concurrent subrequests
- Insufficient retry backoff
- Origin rate limiting

**Solutions:**
```yaml
# Reduce concurrency
max_concurrent_subrequests: 2

# Increase retry backoff
max_retries: 5

# Increase slice size to reduce request count
slice_size: 2097152  # 2MB
```

#### 4. Memory Usage Growing

**Symptom:** Increasing memory consumption over time

**Possible causes:**
- Large files with many slices
- Buffering too many out-of-order slices
- Memory leak (report as bug)

**Solutions:**
```bash
# Monitor memory usage
ps aux | grep pingora-slice

# Restart service periodically (temporary)
sudo systemctl restart pingora-slice

# Reduce slice size
# Limit concurrent requests
# Report issue with reproduction steps
```

#### 5. Metrics Endpoint Not Accessible

**Symptom:** Cannot access http://localhost:9090/metrics

**Possible causes:**
- Metrics endpoint not enabled
- Wrong bind address
- Firewall blocking port

**Solutions:**
```yaml
# Ensure metrics are enabled
metrics_endpoint:
  enabled: true
  address: "127.0.0.1:9090"
```

```bash
# Check if port is listening
sudo netstat -tlnp | grep 9090

# Check firewall
sudo ufw status
sudo firewall-cmd --list-ports

# Test locally
curl http://127.0.0.1:9090/health
```

### Debug Logging

Enable debug logging for troubleshooting:

```bash
# Set log level via environment variable
RUST_LOG=debug ./target/release/pingora-slice

# Different log levels
RUST_LOG=trace  # Very verbose
RUST_LOG=debug  # Detailed information
RUST_LOG=info   # General information (default)
RUST_LOG=warn   # Warnings only
RUST_LOG=error  # Errors only

# Module-specific logging
RUST_LOG=pingora_slice::proxy=debug,pingora_slice::cache=trace
```

### Getting Help

If you encounter issues:
1. Check this troubleshooting section
2. Review logs with debug logging enabled
3. Check GitHub issues for similar problems
4. Open a new issue with:
   - Configuration file (sanitized)
   - Error messages and logs
   - Steps to reproduce
   - Environment details (OS, Rust version)

## Requirements Coverage

This implementation satisfies all requirements from the specification:

### Requirement 1: Configuration Management
- ✓ 1.1: Load slice size from configuration file
- ✓ 1.2: Validate slice size is between 64KB and 10MB
- ✓ 1.3: Use default value of 1MB if not configured
- ✓ 1.4: Log error and refuse to start on invalid configuration

### Requirement 2: Request Detection
- ✓ 2.1: Check if request method is GET
- ✓ 2.2: Determine if slicing should be enabled
- ✓ 2.3: Pass through requests with Range header
- ✓ 2.4: Match request URL against configured patterns

### Requirement 3: Metadata Fetching
- ✓ 3.1: Send HEAD request to origin server
- ✓ 3.2: Extract Content-Length from response
- ✓ 3.3: Check Accept-Ranges header for Range support
- ✓ 3.4: Fall back to normal proxy if Range not supported
- ✓ 3.5: Fall back if Content-Length missing or invalid

### Requirement 4: Slice Calculation
- ✓ 4.1: Calculate number of slices needed
- ✓ 4.2: Generate Range headers with correct byte ranges
- ✓ 4.3: Ensure last slice covers remaining bytes
- ✓ 4.4: Create list of subrequest specifications

### Requirement 5: Concurrent Fetching
- ✓ 5.1: Send multiple subrequests concurrently
- ✓ 5.2: Limit number of concurrent subrequests
- ✓ 5.3: Initiate next subrequest when one completes
- ✓ 5.4: Retry failed subrequests up to max retry count
- ✓ 5.5: Abort entire request if all retries fail

### Requirement 6: Response Assembly
- ✓ 6.1: Start streaming data immediately
- ✓ 6.2: Maintain correct byte order
- ✓ 6.3: Buffer out-of-order slices
- ✓ 6.4: Complete response when all slices received
- ✓ 6.5: Set appropriate response headers

### Requirement 7: Caching
- ✓ 7.1: Store slices in cache with unique keys
- ✓ 7.2: Include URL and byte range in cache key
- ✓ 7.3: Check cache before creating subrequests
- ✓ 7.4: Use cached data and request missing slices only
- ✓ 7.5: Log warning and continue on cache failure

### Requirement 8: Error Handling
- ✓ 8.1: Return 4xx errors to client
- ✓ 8.2: Retry 5xx errors up to configured limit
- ✓ 8.3: Validate Content-Range matches request
- ✓ 8.4: Treat Content-Range mismatch as error
- ✓ 8.5: Return 502 Bad Gateway on unexpected status

### Requirement 9: Monitoring
- ✓ 9.1: Record metrics for requests and cache hits
- ✓ 9.2: Record subrequest counts and latencies
- ✓ 9.3: Log detailed error information
- ✓ 9.4: Log summary information on completion
- ✓ 9.5: Expose metrics via HTTP endpoint

### Requirement 10: Range Request Support
- ✓ 10.1: Parse client Range header
- ✓ 10.2: Calculate slices for requested range
- ✓ 10.3: Request and return necessary slices only
- ✓ 10.4: Return 206 with correct Content-Range
- ✓ 10.5: Return 416 for invalid ranges

## Contributing

Contributions are welcome! Please follow these guidelines:

### Development Setup

```bash
# Fork and clone the repository
git clone https://github.com/yourusername/pingora-slice.git
cd pingora-slice

# Create a feature branch
git checkout -b feature/your-feature-name

# Make your changes and test
cargo test
cargo clippy
cargo fmt

# Commit and push
git commit -m "Add your feature"
git push origin feature/your-feature-name

# Open a pull request
```

### Code Style

- Follow Rust standard style (use `cargo fmt`)
- Run `cargo clippy` and fix warnings
- Add documentation comments for public APIs
- Write tests for new functionality
- Update documentation as needed

### Pull Request Process

1. Ensure all tests pass
2. Update README.md if needed
3. Add entry to CHANGELOG.md
4. Request review from maintainers
5. Address review feedback
6. Squash commits if requested

### Reporting Issues

When reporting bugs, include:
- Rust version (`rustc --version`)
- Operating system and version
- Configuration file (sanitized)
- Steps to reproduce
- Expected vs actual behavior
- Relevant logs

## License

[Add your license here]

## Acknowledgments

- Built on [Cloudflare Pingora](https://github.com/cloudflare/pingora)
- Inspired by [Nginx Slice Module](http://nginx.org/en/docs/http/ngx_http_slice_module.html)
- Property-based testing with [proptest](https://github.com/proptest-rs/proptest)

## Documentation

### General Documentation
- [Configuration Guide](docs/CONFIGURATION.md) - Detailed configuration options
- [Deployment Guide](docs/DEPLOYMENT.md) - Production deployment instructions
- [API Documentation](docs/API.md) - API reference and usage examples
- [Performance Tuning Guide](docs/PERFORMANCE_TUNING.md) - Optimization and tuning
- [Performance Optimization Report](docs/performance_optimization.md) - Detailed analysis and benchmarks

### Cache Documentation
- [Two-Tier Cache Architecture](docs/TIERED_CACHE.md) - L1 + L2 cache system design
- [Cache Implementation](docs/cache_implementation.md) - Technical implementation details

### Cache Purge Documentation
- [Purge Quick Start](docs/PURGE_QUICK_START.md) - Get started with cache purge in 3 steps
- [Purge Integration Guide](docs/PURGE_INTEGRATION_GUIDE.md) - How purge integrates with Pingora Slice
- [HTTP PURGE Reference](docs/HTTP_PURGE_REFERENCE.md) - Complete HTTP PURGE method reference
- [Purge Configuration and Metrics](docs/PURGE_CONFIG_AND_METRICS.md) - Configuration and Prometheus metrics
- [Cache Purge Guide (中文)](docs/CACHE_PURGE_zh.md) - 缓存清除详细指南（中文）

## Related Projects

- [Pingora](https://github.com/cloudflare/pingora) - Rust-based HTTP proxy framework
- [Nginx Slice Module](http://nginx.org/en/docs/http/ngx_http_slice_module.html) - Original inspiration
- [Varnish](https://varnish-cache.org/) - HTTP accelerator with similar capabilities
