# Pingora Slice Server Startup Guide

This document explains how to start and configure the Pingora Slice proxy server.

## Overview

The Pingora Slice server is the main entry point for running the slice module as a standalone proxy service. It loads configuration from a YAML file, initializes the SliceProxy, and prepares the server for handling requests.

## Building the Server

```bash
# Build in debug mode
cargo build

# Build in release mode (optimized)
cargo build --release
```

## Running the Server

### Using Default Configuration

By default, the server looks for `pingora_slice.yaml` in the current directory:

```bash
# Debug build
cargo run

# Release build
./target/release/pingora-slice
```

### Using Custom Configuration

You can specify a custom configuration file path:

```bash
# Debug build
cargo run -- /path/to/config.yaml

# Release build
./target/release/pingora-slice /path/to/config.yaml
```

### Example with the provided configuration

```bash
cargo run -- examples/pingora_slice.yaml
```

## Configuration File Format

The configuration file is in YAML format. Here's a complete example:

```yaml
# Size of each slice in bytes
# Valid range: 64KB (65536) to 10MB (10485760)
# Default: 1MB (1048576)
slice_size: 1048576

# Maximum number of concurrent subrequests to the origin server
# Default: 4
max_concurrent_subrequests: 4

# Maximum number of retry attempts for failed subrequests
# Default: 3
max_retries: 3

# URL patterns that should enable slicing (regex patterns)
# Empty list means all requests will be considered for slicing
slice_patterns:
  - "^/large-files/.*"
  - "^/downloads/.*\\.bin$"
  - "^/videos/.*\\.(mp4|mkv|avi)$"

# Whether to enable caching of slices
# Default: true
enable_cache: true

# Cache TTL (Time To Live) in seconds
# Default: 3600 (1 hour)
cache_ttl: 3600

# Upstream origin server address
# Format: "host:port"
# Default: "127.0.0.1:8080"
upstream_address: "origin.example.com:80"
```

## Configuration Validation

The server validates the configuration on startup. If validation fails, the server will exit with an error message.

### Validation Rules

1. **slice_size**: Must be between 64KB and 10MB
2. **max_concurrent_subrequests**: Must be greater than 0
3. **max_retries**: Must be >= 0
4. **cache_ttl**: Must be greater than 0 when caching is enabled

### Example Validation Errors

```bash
# Invalid slice size (too small)
ERROR Failed to load configuration: Configuration error: slice_size must be between 64KB and 10MB, got 1024 bytes

# Invalid concurrent limit
ERROR Failed to load configuration: Configuration error: max_concurrent_subrequests must be greater than 0

# Missing configuration file
ERROR Failed to load configuration: Configuration error: Failed to read config file: No such file or directory
```

## Logging

The server uses structured logging with the `tracing` crate. Log output includes:

- Timestamp
- Thread ID
- Line number
- Log level (INFO, WARN, ERROR)
- Message

### Log Levels

By default, the server logs at INFO level. You can adjust this by modifying the `tracing_subscriber` initialization in `src/main.rs`:

```rust
tracing_subscriber::fmt()
    .with_max_level(tracing::Level::DEBUG)  // Change to DEBUG for more verbose output
    .init();
```

### Example Log Output

```
2025-11-28T04:04:08.550013Z  INFO ThreadId(01) 34: Starting Pingora Slice Module Server
2025-11-28T04:04:08.550143Z  INFO ThreadId(01) 41: Loading configuration from: pingora_slice.yaml
2025-11-28T04:04:08.550783Z  INFO ThreadId(01) 46: Configuration loaded successfully
2025-11-28T04:04:08.550803Z  INFO ThreadId(01) 47:   - Slice size: 1048576 bytes (1024 KB)
2025-11-28T04:04:08.550809Z  INFO ThreadId(01) 48:   - Max concurrent subrequests: 4
2025-11-28T04:04:08.550814Z  INFO ThreadId(01) 49:   - Max retries: 3
2025-11-28T04:04:08.550825Z  INFO ThreadId(01) 50:   - Cache enabled: true
2025-11-28T04:04:08.550830Z  INFO ThreadId(01) 51:   - Cache TTL: 3600 seconds
2025-11-28T04:04:08.550835Z  INFO ThreadId(01) 52:   - Upstream address: 127.0.0.1:8080
```

## Server Initialization Steps

The server performs the following initialization steps:

1. **Initialize Logging**: Sets up structured logging with tracing
2. **Load Configuration**: Reads and parses the YAML configuration file
3. **Validate Configuration**: Ensures all configuration values are valid
4. **Create SliceProxy**: Initializes the main proxy instance with the configuration
5. **Initialize Metrics**: Sets up metrics collection
6. **Display Status**: Logs the current configuration and status

## Integration with Pingora

The current implementation provides the startup code structure. To fully integrate with Pingora, you would need to:

1. **Implement ProxyHttp trait**: Make SliceProxy implement Pingora's ProxyHttp trait
2. **Create Pingora Server**: Initialize a Pingora Server instance
3. **Create HTTP Proxy Service**: Create an HTTP proxy service with the SliceProxy
4. **Configure Listening**: Set up the listening address and port (e.g., 0.0.0.0:8080)
5. **Start Server**: Call `server.run_forever()` to start accepting requests

### Example Pingora Integration (Pseudo-code)

```rust
use pingora::prelude::*;

fn main() {
    // ... load configuration and create proxy ...
    
    let mut server = Server::new(None).unwrap();
    server.bootstrap();
    
    let mut proxy_service = http_proxy_service(
        &server.configuration,
        proxy
    );
    proxy_service.add_tcp("0.0.0.0:8080");
    
    server.add_service(proxy_service);
    server.run_forever();
}
```

## Troubleshooting

### Configuration File Not Found

**Error**: `Failed to load configuration: Configuration error: Failed to read config file: No such file or directory`

**Solution**: Ensure the configuration file exists at the specified path. If no path is provided, the server looks for `pingora_slice.yaml` in the current directory.

### Invalid Configuration Values

**Error**: `Failed to load configuration: Configuration error: slice_size must be between 64KB and 10MB`

**Solution**: Check your configuration file and ensure all values meet the validation requirements listed above.

### YAML Parsing Error

**Error**: `Failed to load configuration: Configuration error: Failed to parse config file`

**Solution**: Verify that your YAML file is properly formatted. Common issues include:
- Incorrect indentation
- Missing colons after keys
- Invalid YAML syntax

## Environment Variables

Currently, the server does not use environment variables for configuration. All configuration is done through the YAML file.

## Signal Handling

In a production deployment with full Pingora integration, the server would handle signals like SIGTERM and SIGINT for graceful shutdown. The current implementation is a demonstration of the startup code structure.

## Performance Considerations

### Slice Size

- **Smaller slices** (64KB - 256KB): Better for caching granularity, but more overhead
- **Larger slices** (1MB - 2MB): Less overhead, but less efficient caching
- **Recommended**: 512KB - 1MB for most use cases

### Concurrent Subrequests

- **Lower values** (2-4): Less load on origin, but slower for large files
- **Higher values** (8-16): Faster downloads, but more load on origin
- **Recommended**: 4-8 for most use cases

### Cache TTL

- **Shorter TTL** (300-1800s): More frequent updates, higher origin load
- **Longer TTL** (3600-86400s): Better cache efficiency, less fresh content
- **Recommended**: 3600s (1 hour) for most use cases

## Requirements Validation

This implementation validates the following requirements:

- **Requirement 1.1**: Load configured slice size from configuration file
- **Requirement 1.2**: Validate slice size is between 64KB and 10MB
- **Requirement 1.3**: Use default value of 1MB if not configured
- **Requirement 1.4**: Log error and refuse to start on invalid configuration

## See Also

- [Configuration Guide](../examples/pingora_slice.yaml)
- [Proxy Implementation](proxy_implementation.md)
- [Metrics Guide](metrics_implementation.md)
