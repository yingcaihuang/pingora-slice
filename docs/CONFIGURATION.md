# Pingora Slice Module - Configuration Guide

This guide provides detailed information about configuring the Pingora Slice Module for various use cases and environments.

## Table of Contents

- [Configuration File Format](#configuration-file-format)
- [Configuration Parameters](#configuration-parameters)
- [URL Pattern Matching](#url-pattern-matching)
- [Performance Tuning](#performance-tuning)
- [Use Case Examples](#use-case-examples)
- [Validation and Testing](#validation-and-testing)
- [Best Practices](#best-practices)
- [Troubleshooting](#troubleshooting)

## Configuration File Format

The Pingora Slice Module uses YAML format for configuration. The configuration file should be named `pingora_slice.yaml` and placed in:
- The current working directory, or
- A custom path specified as a command-line argument

### Basic Structure

```yaml
# Core slicing parameters
slice_size: 1048576
max_concurrent_subrequests: 4
max_retries: 3

# URL pattern matching
slice_patterns:
  - "^/downloads/.*"

# Cache configuration
enable_cache: true
cache_ttl: 3600

# Upstream server
upstream_address: "origin.example.com:80"

# Optional metrics endpoint
metrics_endpoint:
  enabled: true
  address: "127.0.0.1:9090"
```

## Configuration Parameters

### slice_size

**Type:** Integer (bytes)  
**Default:** 1048576 (1 MB)  
**Valid Range:** 65536 (64 KB) to 10485760 (10 MB)  
**Required:** No

Size of each slice in bytes. This determines how large each Range request will be when fetching from the origin server.

**Examples:**
```yaml
slice_size: 262144    # 256 KB
slice_size: 524288    # 512 KB
slice_size: 1048576   # 1 MB (default)
slice_size: 2097152   # 2 MB
slice_size: 4194304   # 4 MB
```

**Tuning Guidelines:**
- **Small files (< 10 MB):** 256 KB - 512 KB
- **Medium files (10-100 MB):** 1 MB - 2 MB
- **Large files (> 100 MB):** 2 MB - 4 MB
- **Very large files (> 1 GB):** 4 MB - 10 MB

**Trade-offs:**
- Smaller slices: Better cache granularity, more overhead
- Larger slices: Less overhead, less cache efficiency

### max_concurrent_subrequests

**Type:** Integer  
**Default:** 4  
**Valid Range:** > 0  
**Required:** No

Maximum number of concurrent subrequests to the origin server.

**Examples:**
```yaml
max_concurrent_subrequests: 2   # Conservative
max_concurrent_subrequests: 4   # Default
max_concurrent_subrequests: 8   # Aggressive
max_concurrent_subrequests: 16  # Very aggressive
```

**Tuning Guidelines:**
- **Slow origin:** 2-4
- **Standard origin:** 4-8
- **Fast origin:** 8-16
- **CDN origin:** 16+

**Considerations:**
- Higher values increase throughput but may overwhelm origin
- Monitor origin server load when tuning
- Consider origin rate limiting

### max_retries

**Type:** Integer  
**Default:** 3  
**Valid Range:** >= 0  
**Required:** No

Maximum number of retry attempts for failed subrequests.

**Examples:**
```yaml
max_retries: 0   # No retries (not recommended)
max_retries: 1   # Minimal retries
max_retries: 3   # Default
max_retries: 5   # Aggressive retries
```

**Retry Backoff Schedule:**
- Attempt 1: 100 ms
- Attempt 2: 200 ms
- Attempt 3: 400 ms
- Attempt 4: 800 ms
- Attempt 5+: 800 ms

**Tuning Guidelines:**
- **Reliable network:** 1-2 retries
- **Standard network:** 3 retries (default)
- **Unreliable network:** 5+ retries

### slice_patterns

**Type:** Array of strings (regex patterns)  
**Default:** [] (empty, matches all requests)  
**Required:** No

URL patterns that should enable slicing. Uses Rust regex syntax.

**Examples:**
```yaml
slice_patterns:
  # Match all files in /large-files/ directory
  - "^/large-files/.*"
  
  # Match binary files in /downloads/
  - "^/downloads/.*\\.bin$"
  
  # Match video files
  - "^/videos/.*\\.(mp4|mkv|avi|mov)$"
  
  # Match ISO images
  - "^/isos/.*\\.iso$"
  
  # Match compressed archives
  - "^/archives/.*\\.(zip|tar|gz|bz2|7z)$"
  
  # Match files larger than certain size (requires custom logic)
  - "^/files/large/.*"
```

**Pattern Syntax:**
- `^` - Start of path
- `$` - End of path
- `.*` - Any characters
- `\\.` - Literal dot
- `(a|b)` - Alternative (a or b)
- `[0-9]` - Character class
- `+` - One or more
- `*` - Zero or more

**Empty List Behavior:**
If `slice_patterns` is empty or omitted, all GET requests without Range headers will be considered for slicing.

### enable_cache

**Type:** Boolean  
**Default:** true  
**Required:** No

Whether to enable caching of slices.

**Examples:**
```yaml
enable_cache: true   # Enable caching (default)
enable_cache: false  # Disable caching
```

**When to Disable:**
- Content changes very frequently
- Limited storage space
- Content already cached upstream
- Testing/debugging

### cache_ttl

**Type:** Integer (seconds)  
**Default:** 3600 (1 hour)  
**Valid Range:** > 0 (when caching enabled)  
**Required:** No (when caching enabled)

Cache Time-To-Live in seconds.

**Examples:**
```yaml
cache_ttl: 300      # 5 minutes
cache_ttl: 1800     # 30 minutes
cache_ttl: 3600     # 1 hour (default)
cache_ttl: 7200     # 2 hours
cache_ttl: 86400    # 24 hours
cache_ttl: 604800   # 7 days
```

**Tuning Guidelines:**
- **Frequently changing:** 5-30 minutes
- **Moderately stable:** 1-2 hours
- **Static content:** 24 hours - 7 days

### upstream_address

**Type:** String  
**Default:** "127.0.0.1:8080"  
**Required:** Yes

Upstream origin server address.

**Format:** `"host:port"` or `"ip:port"`

**Examples:**
```yaml
upstream_address: "origin.example.com:80"
upstream_address: "192.168.1.100:8080"
upstream_address: "backend.internal:3000"
upstream_address: "cdn.cloudflare.com:443"
```

### metrics_endpoint

**Type:** Object (optional)  
**Default:** null (disabled)  
**Required:** No

Configuration for the HTTP metrics endpoint.

**Structure:**
```yaml
metrics_endpoint:
  enabled: true
  address: "127.0.0.1:9090"
```

**Parameters:**
- `enabled` (boolean): Whether to enable the endpoint
- `address` (string): Bind address in format `"host:port"`

**Examples:**
```yaml
# Enable on localhost only (recommended)
metrics_endpoint:
  enabled: true
  address: "127.0.0.1:9090"

# Enable on all interfaces (use with caution)
metrics_endpoint:
  enabled: true
  address: "0.0.0.0:9090"

# Enable on specific internal IP
metrics_endpoint:
  enabled: true
  address: "10.0.0.5:9090"

# Disable metrics endpoint
metrics_endpoint: null
# or omit the section entirely
```

**Security Considerations:**
- Bind to `127.0.0.1` for local access only
- Use reverse proxy with authentication for external access
- Configure firewall rules to restrict access
- Monitor for unauthorized access attempts

## URL Pattern Matching

### Pattern Examples

#### Match by Directory

```yaml
slice_patterns:
  # All files in /downloads/
  - "^/downloads/.*"
  
  # All files in /media/ and subdirectories
  - "^/media/.*"
  
  # Specific subdirectory
  - "^/files/large/.*"
```

#### Match by File Extension

```yaml
slice_patterns:
  # Binary files
  - ".*\\.bin$"
  
  # Video files
  - ".*\\.(mp4|mkv|avi|mov|wmv|flv)$"
  
  # Archive files
  - ".*\\.(zip|tar|gz|bz2|7z|rar)$"
  
  # ISO images
  - ".*\\.iso$"
  
  # Large documents
  - ".*\\.(pdf|doc|docx|ppt|pptx)$"
```

#### Match by Path and Extension

```yaml
slice_patterns:
  # Videos in /media/ directory
  - "^/media/.*\\.(mp4|mkv)$"
  
  # Archives in /downloads/
  - "^/downloads/.*\\.(zip|tar\\.gz)$"
  
  # ISOs in /isos/
  - "^/isos/.*\\.iso$"
```

#### Complex Patterns

```yaml
slice_patterns:
  # Files with "large" in the name
  - ".*large.*"
  
  # Files with version numbers
  - ".*-v[0-9]+\\.[0-9]+.*"
  
  # Files in dated directories (YYYY/MM/DD)
  - "^/archive/[0-9]{4}/[0-9]{2}/[0-9]{2}/.*"
  
  # Exclude certain patterns (requires negative lookahead)
  - "^/files/(?!small).*"
```

### Testing Patterns

Test your patterns before deployment:

```rust
use regex::Regex;

fn test_pattern(pattern: &str, path: &str) -> bool {
    let re = Regex::new(pattern).unwrap();
    re.is_match(path)
}

// Test
assert!(test_pattern("^/downloads/.*\\.bin$", "/downloads/file.bin"));
assert!(!test_pattern("^/downloads/.*\\.bin$", "/uploads/file.bin"));
```

## Performance Tuning

### Scenario-Based Configurations

#### High-Performance Setup (Fast Networks, Powerful Origin)

```yaml
slice_size: 4194304              # 4 MB
max_concurrent_subrequests: 8    # High concurrency
max_retries: 2                   # Fewer retries
cache_ttl: 86400                 # 24 hours
```

**Best for:**
- High-bandwidth networks (1 Gbps+)
- Powerful origin servers
- Static content
- Low latency requirements

#### Conservative Setup (Slow Networks, Limited Origin)

```yaml
slice_size: 262144               # 256 KB
max_concurrent_subrequests: 2    # Low concurrency
max_retries: 5                   # More retries
cache_ttl: 3600                  # 1 hour
```

**Best for:**
- Slow or unreliable networks
- Limited origin capacity
- Shared hosting environments
- High packet loss scenarios

#### Balanced Setup (General Purpose)

```yaml
slice_size: 1048576              # 1 MB
max_concurrent_subrequests: 4    # Moderate concurrency
max_retries: 3                   # Standard retries
cache_ttl: 7200                  # 2 hours
```

**Best for:**
- Most production environments
- Mixed content types
- Standard network conditions
- General-purpose CDN

#### Minimal Caching (Frequently Changing Content)

```yaml
slice_size: 1048576              # 1 MB
max_concurrent_subrequests: 4    # Standard
max_retries: 3                   # Standard
cache_ttl: 300                   # 5 minutes
```

**Best for:**
- Frequently updated content
- Live streaming archives
- Dynamic content
- Short-lived files

## Use Case Examples

### Video Streaming Platform

```yaml
slice_size: 2097152              # 2 MB (good for video chunks)
max_concurrent_subrequests: 6
max_retries: 3
slice_patterns:
  - "^/videos/.*\\.(mp4|mkv|avi|mov)$"
  - "^/streams/.*\\.m3u8$"
enable_cache: true
cache_ttl: 86400                 # 24 hours (videos rarely change)
upstream_address: "video-origin.example.com:80"
metrics_endpoint:
  enabled: true
  address: "127.0.0.1:9090"
```

### Software Distribution

```yaml
slice_size: 4194304              # 4 MB (large installers)
max_concurrent_subrequests: 8
max_retries: 3
slice_patterns:
  - "^/downloads/.*\\.(exe|dmg|deb|rpm|msi)$"
  - "^/releases/.*\\.(zip|tar\\.gz)$"
enable_cache: true
cache_ttl: 604800                # 7 days (releases are stable)
upstream_address: "releases.example.com:443"
metrics_endpoint:
  enabled: true
  address: "127.0.0.1:9090"
```

### ISO/Disk Image Distribution

```yaml
slice_size: 10485760             # 10 MB (very large files)
max_concurrent_subrequests: 4
max_retries: 3
slice_patterns:
  - "^/isos/.*\\.iso$"
  - "^/images/.*\\.(img|vdi|vmdk)$"
enable_cache: true
cache_ttl: 2592000               # 30 days (ISOs rarely change)
upstream_address: "iso-mirror.example.com:80"
metrics_endpoint:
  enabled: true
  address: "127.0.0.1:9090"
```

### Document Archive

```yaml
slice_size: 524288               # 512 KB (smaller documents)
max_concurrent_subrequests: 4
max_retries: 3
slice_patterns:
  - "^/documents/.*\\.(pdf|doc|docx|ppt|pptx)$"
  - "^/archive/.*"
enable_cache: true
cache_ttl: 3600                  # 1 hour (documents may update)
upstream_address: "docs.example.com:80"
metrics_endpoint:
  enabled: true
  address: "127.0.0.1:9090"
```

### Game Asset Distribution

```yaml
slice_size: 2097152              # 2 MB
max_concurrent_subrequests: 8
max_retries: 2
slice_patterns:
  - "^/assets/.*\\.(pak|bundle|asset)$"
  - "^/updates/.*"
enable_cache: true
cache_ttl: 43200                 # 12 hours
upstream_address: "game-cdn.example.com:80"
metrics_endpoint:
  enabled: true
  address: "127.0.0.1:9090"
```

## Validation and Testing

### Configuration Validation

The module validates configuration on startup:

```bash
# Test configuration without starting server
./pingora-slice --check-config

# View validation errors
./pingora-slice 2>&1 | grep -i error
```

### Validation Rules

1. **slice_size:**
   - Must be between 65536 and 10485760
   - Error: "slice_size must be between 64KB and 10MB"

2. **max_concurrent_subrequests:**
   - Must be greater than 0
   - Error: "max_concurrent_subrequests must be greater than 0"

3. **max_retries:**
   - Must be >= 0
   - Error: "max_retries must be non-negative"

4. **cache_ttl:**
   - Must be > 0 when caching enabled
   - Error: "cache_ttl must be greater than 0 when caching is enabled"

5. **slice_patterns:**
   - Must be valid regex patterns
   - Error: "Invalid regex pattern: <pattern>"

6. **upstream_address:**
   - Must be valid address format
   - Error: "Invalid upstream address format"

### Testing Configuration

```bash
# 1. Validate syntax
./pingora-slice --check-config

# 2. Test with dry-run (if supported)
./pingora-slice --dry-run

# 3. Start in foreground with debug logging
RUST_LOG=debug ./pingora-slice

# 4. Test with curl
curl -v http://localhost:8080/test-file

# 5. Check metrics
curl http://localhost:9090/metrics
```

## Best Practices

### General Guidelines

1. **Start Conservative:**
   - Begin with default values
   - Monitor performance
   - Adjust gradually

2. **Monitor Metrics:**
   - Track cache hit rate
   - Monitor subrequest failures
   - Watch latency trends

3. **Test Before Production:**
   - Test configuration in staging
   - Verify pattern matching
   - Load test with realistic traffic

4. **Document Changes:**
   - Comment configuration changes
   - Track performance impact
   - Maintain change log

### Configuration Management

1. **Version Control:**
   ```bash
   git add pingora_slice.yaml
   git commit -m "Update slice size to 2MB"
   ```

2. **Environment-Specific Configs:**
   ```
   config/
   ├── dev.yaml
   ├── staging.yaml
   └── production.yaml
   ```

3. **Configuration Templates:**
   ```yaml
   # Template with placeholders
   upstream_address: "${ORIGIN_HOST}:${ORIGIN_PORT}"
   ```

### Security Best Practices

1. **Restrict Metrics Access:**
   ```yaml
   metrics_endpoint:
     address: "127.0.0.1:9090"  # Localhost only
   ```

2. **Validate Patterns:**
   - Test regex patterns thoroughly
   - Avoid overly broad patterns
   - Consider security implications

3. **Limit Resource Usage:**
   ```yaml
   max_concurrent_subrequests: 4  # Prevent origin overload
   max_retries: 3                 # Limit retry storms
   ```

## Troubleshooting

### Configuration Won't Load

**Problem:** Server fails to start with configuration error

**Solutions:**
```bash
# Check YAML syntax
yamllint pingora_slice.yaml

# Validate configuration
./pingora-slice --check-config

# Check file permissions
ls -la pingora_slice.yaml

# View detailed error
RUST_LOG=debug ./pingora-slice
```

### Patterns Not Matching

**Problem:** Files not being sliced despite matching patterns

**Solutions:**
```bash
# Test pattern matching
echo "/downloads/file.bin" | grep -E "^/downloads/.*\\.bin$"

# Enable debug logging
RUST_LOG=debug ./pingora-slice

# Check request logs
journalctl -u pingora-slice | grep "pattern"

# Verify pattern syntax
# Use online regex tester: regex101.com
```

### Poor Performance

**Problem:** Slow response times or high latency

**Solutions:**
```yaml
# Increase concurrency
max_concurrent_subrequests: 8

# Increase slice size
slice_size: 2097152

# Reduce retries
max_retries: 2

# Increase cache TTL
cache_ttl: 7200
```

### High Cache Miss Rate

**Problem:** Cache hit rate < 50%

**Solutions:**
```yaml
# Reduce slice size for better granularity
slice_size: 524288

# Increase cache TTL
cache_ttl: 7200

# Review URL patterns
# Ensure consistent URLs (no varying query params)
```

## Additional Resources

- [README.md](../README.md) - Main documentation
- [DEPLOYMENT.md](DEPLOYMENT.md) - Deployment guide
- [examples/pingora_slice.yaml](../examples/pingora_slice.yaml) - Annotated example
- [Design Document](.kiro/specs/pingora-slice/design.md) - Technical design

## Support

For configuration assistance:
- Review this guide and examples
- Check logs: `journalctl -u pingora-slice -f`
- Test patterns: Use regex testers
- Report issues: GitHub with configuration details
