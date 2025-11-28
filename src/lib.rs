//! Pingora Slice Module
//!
//! A high-performance slice module for Pingora proxy server that automatically splits large file
//! requests into multiple small Range requests, similar to Nginx Slice module.
//!
//! # Overview
//!
//! The Pingora Slice Module transparently intercepts large file requests and splits them into
//! smaller, manageable chunks (slices). Each slice is fetched independently using HTTP Range
//! requests, cached separately, and assembled back into the complete response for the client.
//!
//! # Features
//!
//! - **Automatic Request Slicing**: Transparently splits large file requests without client awareness
//! - **Concurrent Fetching**: Fetches multiple slices in parallel with configurable concurrency
//! - **Smart Caching**: Caches individual slices for efficient reuse and partial cache hits
//! - **Range Request Support**: Correctly handles client Range requests (partial content)
//! - **Retry Logic**: Automatic retry with exponential backoff for failed subrequests
//! - **Metrics Collection**: Comprehensive metrics for monitoring and observability
//! - **Property-Based Testing**: Extensive test suite with correctness guarantees
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use pingora_slice::{SliceConfig, SliceProxy};
//! use std::sync::Arc;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Load configuration from file
//! let config = SliceConfig::from_file("pingora_slice.yaml")?;
//!
//! // Create proxy instance
//! let proxy = SliceProxy::new(Arc::new(config));
//!
//! // Access metrics
//! let stats = proxy.metrics().get_stats();
//! println!("Total requests: {}", stats.total_requests);
//! # Ok(())
//! # }
//! ```
//!
//! # Architecture
//!
//! The module consists of several key components:
//!
//! - [`SliceProxy`]: Main proxy implementation that coordinates all components
//! - [`RequestAnalyzer`]: Analyzes requests to determine if slicing should be enabled
//! - [`MetadataFetcher`]: Fetches file metadata from origin using HEAD requests
//! - [`SliceCalculator`]: Calculates slice specifications based on file size
//! - [`SubrequestManager`]: Manages concurrent fetching of slices with retry logic
//! - [`ResponseAssembler`]: Assembles slices and streams response to client
//! - [`SliceCache`]: Manages caching of individual slices
//! - [`SliceMetrics`]: Collects and exposes runtime metrics
//!
//! # Configuration
//!
//! Configuration is loaded from a YAML file:
//!
//! ```yaml
//! slice_size: 1048576              # 1MB slices
//! max_concurrent_subrequests: 4    # 4 concurrent requests
//! max_retries: 3                   # 3 retry attempts
//! slice_patterns:
//!   - "^/large-files/.*"
//! enable_cache: true
//! cache_ttl: 3600                  # 1 hour
//! upstream_address: "origin.example.com:80"
//! ```
//!
//! See [`SliceConfig`] for detailed configuration options.
//!
//! # Examples
//!
//! ## Basic Usage
//!
//! ```rust,no_run
//! use pingora_slice::{SliceConfig, SliceProxy};
//! use std::sync::Arc;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create with default configuration
//! let config = SliceConfig::default();
//! let proxy = SliceProxy::new(Arc::new(config));
//!
//! // Create request context
//! let ctx = proxy.new_ctx();
//! println!("Slice enabled: {}", ctx.is_slice_enabled());
//! # Ok(())
//! # }
//! ```
//!
//! ## Custom Configuration
//!
//! ```rust,no_run
//! use pingora_slice::SliceConfig;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let config = SliceConfig {
//!     slice_size: 512 * 1024,  // 512KB
//!     max_concurrent_subrequests: 8,
//!     max_retries: 5,
//!     slice_patterns: vec!["/downloads/.*".to_string()],
//!     enable_cache: true,
//!     cache_ttl: 7200,
//!     upstream_address: "origin.example.com:80".to_string(),
//!     metrics_endpoint: None,
//! };
//!
//! // Validate configuration
//! config.validate()?;
//! # Ok(())
//! # }
//! ```
//!
//! # Error Handling
//!
//! The module uses a custom error type [`SliceError`] for all error conditions:
//!
//! ```rust,no_run
//! use pingora_slice::{SliceConfig, SliceError};
//!
//! # fn main() {
//! match SliceConfig::from_file("config.yaml") {
//!     Ok(config) => println!("Config loaded successfully"),
//!     Err(SliceError::ConfigError(msg)) => eprintln!("Config error: {}", msg),
//!     Err(SliceError::IoError(e)) => eprintln!("IO error: {}", e),
//!     Err(e) => eprintln!("Other error: {}", e),
//! }
//! # }
//! ```
//!
//! # Performance
//!
//! Performance can be tuned through configuration:
//!
//! - **Slice Size**: Larger slices reduce overhead but decrease cache efficiency
//! - **Concurrency**: Higher concurrency increases throughput but may overwhelm origin
//! - **Cache TTL**: Longer TTL reduces origin load but may serve stale content
//!
//! See the [Configuration Guide](../docs/CONFIGURATION.md) for detailed tuning advice.
//!
//! # Testing
//!
//! The module includes comprehensive tests:
//!
//! - Unit tests for individual components
//! - Property-based tests for correctness guarantees
//! - Integration tests for end-to-end functionality
//!
//! Run tests with:
//!
//! ```bash
//! cargo test
//! ```
//!
//! # Requirements
//!
//! This implementation satisfies all requirements from the specification:
//!
//! - Configuration management (Requirements 1.1-1.4)
//! - Request detection (Requirements 2.1-2.4)
//! - Metadata fetching (Requirements 3.1-3.5)
//! - Slice calculation (Requirements 4.1-4.4)
//! - Concurrent fetching (Requirements 5.1-5.5)
//! - Response assembly (Requirements 6.1-6.5)
//! - Caching (Requirements 7.1-7.5)
//! - Error handling (Requirements 8.1-8.5)
//! - Monitoring (Requirements 9.1-9.5)
//! - Range request support (Requirements 10.1-10.5)
//!
//! # See Also
//!
//! - [README.md](../README.md) - Main documentation
//! - [DEPLOYMENT.md](../docs/DEPLOYMENT.md) - Deployment guide
//! - [CONFIGURATION.md](../docs/CONFIGURATION.md) - Configuration guide
//! - [Design Document](../.kiro/specs/pingora-slice/design.md) - Technical design

pub mod config;
pub mod models;
pub mod error;
pub mod request_analyzer;
pub mod metadata_fetcher;
pub mod slice_calculator;
pub mod cache;
pub mod tiered_cache;  // New two-tier cache implementation
pub mod purge_handler;  // HTTP PURGE method handler
pub mod purge_metrics;  // Prometheus metrics for purge operations
pub mod subrequest_manager;
pub mod response_assembler;
pub mod metrics;
pub mod metrics_endpoint;
pub mod proxy;

// Re-export commonly used types
pub use config::SliceConfig;
pub use models::{ByteRange, SliceSpec, FileMetadata};
pub use error::{SliceError, Result};
pub use request_analyzer::RequestAnalyzer;
pub use metadata_fetcher::MetadataFetcher;
pub use slice_calculator::SliceCalculator;
pub use cache::SliceCache;
pub use tiered_cache::{TieredCache, TieredCacheStats};  // Export new cache
pub use subrequest_manager::{SubrequestManager, SubrequestResult, RetryPolicy};
pub use response_assembler::ResponseAssembler;
pub use metrics::{SliceMetrics, MetricsSnapshot};
pub use metrics_endpoint::MetricsEndpoint;
pub use proxy::{SliceProxy, SliceContext};
