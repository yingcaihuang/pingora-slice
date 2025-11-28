# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2024-11-28

### Added
- **Two-Tier Cache System**: L1 (memory) + L2 (disk) cache architecture
  - Automatic promotion from L2 to L1 on cache hits
  - LRU eviction policy for both cache tiers
  - Async disk operations for non-blocking performance
  - Configurable size limits for both tiers
- **HTTP PURGE Support**: Cache invalidation via HTTP PURGE method
  - Purge specific URLs or all cached content
  - Token-based authentication for security
  - Comprehensive purge metrics
- **Enhanced Metrics**: New Prometheus metrics for cache and purge operations
  - L1/L2 cache hit/miss counters
  - Cache promotion and eviction metrics
  - Purge operation success/failure tracking
- **Complete Documentation**: 
  - Tiered cache architecture guide
  - HTTP PURGE integration guide
  - Chinese documentation for cache purge
  - Quick start and reference guides
- **Examples and Tools**:
  - HTTP PURGE server example
  - Tiered cache example with purge support
  - Test script for purge operations

### Changed
- Updated cache implementation to support two-tier architecture
- Enhanced configuration with L2 cache and purge settings
- Improved README with new features documentation

### Performance
- Non-blocking disk I/O for L2 cache operations
- Efficient memory management with automatic promotion
- Optimized cache lookup across both tiers

## [0.1.0] - 2024-01-XX

### Added
- Initial release of Pingora Slice module
- Automatic file slicing for large files
- Concurrent subrequest management with configurable limits
- Intelligent cache management with LRU eviction
- Support for client Range requests (HTTP 206 Partial Content)
- Comprehensive error handling and retry mechanism
- Prometheus metrics endpoint for monitoring
- Property-based testing for correctness validation
- Complete documentation in English and Chinese
- RPM packages for CentOS 8/9, Rocky Linux 8/9, AlmaLinux 8/9
- GitHub Actions CI/CD pipeline
- Systemd service integration

### Features
- **Request Analysis**: Automatic detection of requests suitable for slicing
- **Metadata Fetching**: HEAD requests to determine file size and Range support
- **Slice Calculation**: Intelligent splitting of files into optimal chunks
- **Concurrent Fetching**: Parallel subrequests with semaphore-based limiting
- **Response Assembly**: Ordered streaming of slices to clients
- **Cache Management**: Per-slice caching with unique key generation
- **Metrics Collection**: Comprehensive statistics for monitoring
- **Error Recovery**: Exponential backoff retry with configurable limits

### Configuration
- Configurable slice size (64KB - 10MB)
- Adjustable concurrent subrequest limit
- Customizable retry policy
- URL pattern matching for selective slicing
- Cache TTL and size limits
- Metrics endpoint configuration

### Testing
- 115 unit tests covering all modules
- 69 integration tests for end-to-end scenarios
- 200+ property-based tests validating 20 correctness properties
- All tests passing with comprehensive coverage

### Documentation
- README with quick start guide
- Chinese README (README_zh.md)
- API documentation
- Configuration guide
- Deployment guide
- Performance tuning guide
- Optimization summary

### Performance
- Efficient memory usage with bounded buffers
- Zero-copy data transfer where possible
- Async I/O with Tokio runtime
- Optimized cache lookup with batch operations

### Security
- Systemd service hardening
- User isolation (dedicated pingora-slice user)
- File system access restrictions
- Resource limits (file descriptors, processes)

## [0.0.1] - 2024-01-XX (Pre-release)

### Added
- Initial project structure
- Core data models
- Basic proxy implementation
- Configuration management

---

## Release Notes

### v0.1.0 - Initial Release

This is the first stable release of Pingora Slice, a high-performance proxy module for Pingora that automatically splits large file requests into multiple Range requests.

**Key Features:**
- ✅ Automatic file slicing with configurable chunk size
- ✅ Concurrent subrequests for improved performance
- ✅ Intelligent per-slice caching
- ✅ Full support for HTTP Range requests
- ✅ Comprehensive error handling and retry logic
- ✅ Prometheus metrics for monitoring
- ✅ Production-ready with extensive testing

**Installation:**
```bash
# CentOS 8 / Rocky Linux 8 / AlmaLinux 8
sudo dnf install -y ./pingora-slice-0.1.0-1.el8.x86_64.rpm

# CentOS 9 / Rocky Linux 9 / AlmaLinux 9
sudo dnf install -y ./pingora-slice-0.1.0-1.el9.x86_64.rpm
```

**Quick Start:**
```bash
# Edit configuration
sudo vi /etc/pingora-slice/pingora_slice.yaml

# Start service
sudo systemctl start pingora-slice
sudo systemctl enable pingora-slice

# Check status
sudo systemctl status pingora-slice
```

**Documentation:**
- [README](README.md)
- [中文文档](README_zh.md)
- [Quick Start](QUICKSTART.md)
- [Configuration Guide](docs/CONFIGURATION.md)
- [Deployment Guide](docs/DEPLOYMENT.md)

**Known Issues:**
- None

**Breaking Changes:**
- N/A (initial release)

**Contributors:**
- Initial development team

---

## Version History

| Version | Release Date | Notes |
|---------|-------------|-------|
| 0.1.0   | 2024-01-XX  | Initial stable release |
| 0.0.1   | 2024-01-XX  | Pre-release / Development |

## Upgrade Guide

### From Source to RPM

If you previously installed from source:

1. Stop the running service
2. Remove old binary: `sudo rm /usr/local/bin/pingora-slice`
3. Install RPM package
4. Migrate configuration to `/etc/pingora-slice/pingora_slice.yaml`
5. Start systemd service

## Support

For issues, questions, or contributions:
- GitHub Issues: https://github.com/your-username/pingora-slice/issues
- Documentation: https://github.com/your-username/pingora-slice/tree/main/docs

## License

MIT License - see [LICENSE](LICENSE) file for details
