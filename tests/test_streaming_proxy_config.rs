//! Tests for StreamingProxy configuration integration

use pingora_slice::{SliceConfig, StreamingProxy};
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_streaming_proxy_from_config_file_backend() {
    // Create a temporary directory for the test
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.yaml");
    let cache_dir = temp_dir.path().join("cache");

    // Create a test configuration file with file backend
    let config_content = format!(
        r#"
slice_size: 1048576
max_concurrent_subrequests: 4
max_retries: 3
slice_patterns:
  - ".*"
enable_cache: true
cache_ttl: 3600
l1_cache_size_bytes: 10485760
enable_l2_cache: true
l2_backend: "file"
l2_cache_dir: "{}"
upstream_address: "example.com:80"
"#,
        cache_dir.display()
    );

    fs::write(&config_path, config_content).unwrap();

    // Create streaming proxy from config
    let proxy = StreamingProxy::from_config(&config_path).await.unwrap();

    // Verify configuration was loaded correctly
    assert_eq!(proxy.config().upstream_address, "example.com:80");
    assert!(proxy.config().enable_cache);
    assert_eq!(proxy.config().cache_ttl, 3600);
    assert_eq!(proxy.config().l1_cache_size_bytes, 10485760);
    assert!(proxy.config().enable_l2_cache);
    assert_eq!(proxy.config().l2_backend, "file");

    // Verify cache was created
    let stats = proxy.cache_stats();
    assert_eq!(stats.l1_entries, 0);
    assert_eq!(stats.l1_bytes, 0);
}

#[tokio::test]
async fn test_streaming_proxy_from_config_raw_disk_backend() {
    // Create a temporary directory for the test
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.yaml");
    let device_path = temp_dir.path().join("raw-cache");

    // Create a test configuration file with raw disk backend
    let config_content = format!(
        r#"
slice_size: 1048576
max_concurrent_subrequests: 4
max_retries: 3
slice_patterns:
  - ".*"
enable_cache: true
cache_ttl: 3600
l1_cache_size_bytes: 10485760
enable_l2_cache: true
l2_backend: "raw_disk"
l2_cache_dir: "{}"
raw_disk_cache:
  device_path: "{}"
  total_size: 10485760
  block_size: 4096
  use_direct_io: false
  enable_compression: true
  enable_prefetch: true
  enable_zero_copy: true
upstream_address: "example.com:80"
"#,
        device_path.display(),
        device_path.display()
    );

    fs::write(&config_path, config_content).unwrap();

    // Create streaming proxy from config
    let proxy = StreamingProxy::from_config(&config_path).await.unwrap();

    // Verify configuration was loaded correctly
    assert_eq!(proxy.config().upstream_address, "example.com:80");
    assert!(proxy.config().enable_cache);
    assert_eq!(proxy.config().l2_backend, "raw_disk");

    // Verify raw disk config
    let raw_disk_config = proxy.config().raw_disk_cache.as_ref().unwrap();
    assert_eq!(raw_disk_config.total_size, 10485760);
    assert_eq!(raw_disk_config.block_size, 4096);
    assert!(!raw_disk_config.use_direct_io);
    assert!(raw_disk_config.enable_compression);

    // Verify cache was created
    let stats = proxy.cache_stats();
    assert_eq!(stats.l1_entries, 0);

    // Verify raw disk stats are available
    let raw_stats = proxy.raw_disk_stats().await;
    assert!(raw_stats.is_some());
}

#[tokio::test]
async fn test_streaming_proxy_from_config_memory_only() {
    // Create a temporary directory for the test
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.yaml");

    // Create a test configuration file with L2 cache disabled
    let config_content = r#"
slice_size: 1048576
max_concurrent_subrequests: 4
max_retries: 3
slice_patterns:
  - ".*"
enable_cache: true
cache_ttl: 3600
l1_cache_size_bytes: 10485760
enable_l2_cache: false
l2_backend: "file"
l2_cache_dir: "/tmp/cache"
upstream_address: "example.com:80"
"#;

    fs::write(&config_path, config_content).unwrap();

    // Create streaming proxy from config
    let proxy = StreamingProxy::from_config(&config_path).await.unwrap();

    // Verify configuration was loaded correctly
    assert_eq!(proxy.config().upstream_address, "example.com:80");
    assert!(proxy.config().enable_cache);
    assert!(!proxy.config().enable_l2_cache);

    // Verify cache was created (memory-only)
    let stats = proxy.cache_stats();
    assert_eq!(stats.l1_entries, 0);

    // Verify raw disk stats are not available
    let raw_stats = proxy.raw_disk_stats().await;
    assert!(raw_stats.is_none());
}

#[tokio::test]
async fn test_streaming_proxy_cache_stats() {
    // Create a temporary directory for the test
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.yaml");
    let cache_dir = temp_dir.path().join("cache");

    // Create a test configuration file
    let config_content = format!(
        r#"
slice_size: 1048576
max_concurrent_subrequests: 4
max_retries: 3
slice_patterns:
  - ".*"
enable_cache: true
cache_ttl: 3600
l1_cache_size_bytes: 10485760
enable_l2_cache: true
l2_backend: "file"
l2_cache_dir: "{}"
upstream_address: "example.com:80"
"#,
        cache_dir.display()
    );

    fs::write(&config_path, config_content).unwrap();

    // Create streaming proxy from config
    let proxy = StreamingProxy::from_config(&config_path).await.unwrap();

    // Get initial stats
    let stats = proxy.cache_stats();
    assert_eq!(stats.l1_entries, 0);
    assert_eq!(stats.l1_bytes, 0);
    assert_eq!(stats.l1_hits, 0);
    assert_eq!(stats.l2_hits, 0);
    assert_eq!(stats.misses, 0);

    // Store some data in cache
    use bytes::Bytes;
    use pingora_slice::ByteRange;

    let range = ByteRange::new(0, 999).unwrap();
    let data = Bytes::from(vec![1u8; 1000]);
    proxy
        .cache()
        .store("http://example.com/test", &range, data)
        .unwrap();

    // Get updated stats
    let stats = proxy.cache_stats();
    assert_eq!(stats.l1_entries, 1);
    assert_eq!(stats.l1_bytes, 1000);
}

#[tokio::test]
async fn test_streaming_proxy_invalid_config() {
    // Create a temporary directory for the test
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.yaml");

    // Create an invalid configuration file (missing required raw_disk_cache)
    let config_content = r#"
slice_size: 1048576
max_concurrent_subrequests: 4
max_retries: 3
slice_patterns:
  - ".*"
enable_cache: true
cache_ttl: 3600
l1_cache_size_bytes: 10485760
enable_l2_cache: true
l2_backend: "raw_disk"
l2_cache_dir: "/tmp/cache"
upstream_address: "example.com:80"
"#;

    fs::write(&config_path, config_content).unwrap();

    // Attempt to create streaming proxy from invalid config
    let result = StreamingProxy::from_config(&config_path).await;
    assert!(result.is_err());
}
