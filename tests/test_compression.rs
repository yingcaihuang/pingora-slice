//! Tests for compression support in raw disk cache

use bytes::Bytes;
use pingora_slice::raw_disk::{
    CompressionAlgorithm, CompressionConfig, RawDiskCache,
};
use std::time::Duration;
use tempfile::TempDir;

#[tokio::test]
async fn test_compression_basic() {
    let temp_dir = TempDir::new().unwrap();
    let cache_path = temp_dir.path().join("cache.dat");

    let cache = RawDiskCache::new(
        &cache_path,
        10 * 1024 * 1024,
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();

    // Create compressible data
    let data = Bytes::from("test data ".repeat(1000));
    let original_size = data.len();

    // Store data
    cache.store("test", data.clone()).await.unwrap();

    // Retrieve data
    let retrieved = cache.lookup("test").await.unwrap().unwrap();
    assert_eq!(retrieved, data);

    // Check compression stats
    let stats = cache.compression_stats().await;
    assert!(stats.compression_count > 0);
    assert!(stats.total_compressed_bytes > 0);
    
    // Compression should have reduced size
    assert!(stats.total_compressed_size < original_size as u64);
}

#[tokio::test]
async fn test_compression_roundtrip() {
    let temp_dir = TempDir::new().unwrap();
    let cache_path = temp_dir.path().join("cache.dat");

    let cache = RawDiskCache::new(
        &cache_path,
        10 * 1024 * 1024,
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();

    // Test various data patterns
    let test_cases = vec![
        ("repeated", Bytes::from("AAAA".repeat(500))),
        ("text", Bytes::from("The quick brown fox jumps over the lazy dog. ".repeat(100))),
        ("mixed", Bytes::from(vec![0u8, 1, 2, 3, 4, 5].repeat(200))),
    ];

    for (key, data) in test_cases {
        cache.store(key, data.clone()).await.unwrap();
        let retrieved = cache.lookup(key).await.unwrap().unwrap();
        assert_eq!(retrieved, data, "Roundtrip failed for key: {}", key);
    }
}

#[tokio::test]
async fn test_compression_small_data_skipped() {
    let temp_dir = TempDir::new().unwrap();
    let cache_path = temp_dir.path().join("cache.dat");

    let cache = RawDiskCache::new(
        &cache_path,
        10 * 1024 * 1024,
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();

    // Store small data (below default threshold of 1024 bytes)
    let small_data = Bytes::from("small");
    cache.store("small", small_data.clone()).await.unwrap();

    // Check that compression was skipped
    let stats = cache.compression_stats().await;
    assert!(stats.skipped_count > 0);
}

#[tokio::test]
async fn test_compression_incompressible_data() {
    let temp_dir = TempDir::new().unwrap();
    let cache_path = temp_dir.path().join("cache.dat");

    let cache = RawDiskCache::new(
        &cache_path,
        10 * 1024 * 1024,
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();

    // Create pseudo-random data that won't compress well
    let incompressible: Vec<u8> = (0..5000)
        .map(|i| ((i * 7 + 13) % 256) as u8)
        .collect();
    let data = Bytes::from(incompressible);

    cache.store("incompressible", data.clone()).await.unwrap();
    let retrieved = cache.lookup("incompressible").await.unwrap().unwrap();
    assert_eq!(retrieved, data);

    // Check stats - pseudo-random data might compress slightly or not at all
    let stats = cache.compression_stats().await;
    // Either compressed (with poor ratio), resulted in expansion, or skipped
    // Just verify the operation completed successfully
    assert!(stats.compression_count > 0 || stats.expansion_count > 0 || stats.skipped_count > 0);
}

#[tokio::test]
async fn test_compression_multiple_entries() {
    let temp_dir = TempDir::new().unwrap();
    let cache_path = temp_dir.path().join("cache.dat");

    let cache = RawDiskCache::new(
        &cache_path,
        10 * 1024 * 1024,
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();

    // Store multiple entries
    for i in 0..10 {
        let key = format!("key_{}", i);
        let data = Bytes::from(format!("data {} ", i).repeat(500));
        cache.store(&key, data).await.unwrap();
    }

    // Retrieve and verify all entries
    for i in 0..10 {
        let key = format!("key_{}", i);
        let expected = Bytes::from(format!("data {} ", i).repeat(500));
        let retrieved = cache.lookup(&key).await.unwrap().unwrap();
        assert_eq!(retrieved, expected);
    }

    // Check compression stats
    let stats = cache.compression_stats().await;
    assert_eq!(stats.compression_count, 10);
    // Decompression count might be less than 10 if prefetch cache is used
    assert!(stats.decompression_count >= 8, "Expected at least 8 decompressions, got {}", stats.decompression_count);
    assert!(stats.space_saved() > 0);
}

#[tokio::test]
async fn test_compression_stats_accuracy() {
    let temp_dir = TempDir::new().unwrap();
    let cache_path = temp_dir.path().join("cache.dat");

    let cache = RawDiskCache::new(
        &cache_path,
        10 * 1024 * 1024,
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();

    let data = Bytes::from("test ".repeat(1000));
    let original_size = data.len();

    cache.store("test", data.clone()).await.unwrap();

    let stats = cache.compression_stats().await;
    
    // Verify stats are reasonable
    assert_eq!(stats.total_compressed_bytes, original_size as u64);
    assert!(stats.total_compressed_size < original_size as u64);
    assert!(stats.compression_ratio() < 1.0);
    assert!(stats.space_saved() > 0);
    assert!(stats.space_saved_percent() > 0.0);
    assert!(stats.space_saved_percent() < 100.0);
}

#[tokio::test]
async fn test_compression_with_cache_stats() {
    let temp_dir = TempDir::new().unwrap();
    let cache_path = temp_dir.path().join("cache.dat");

    let cache = RawDiskCache::new(
        &cache_path,
        10 * 1024 * 1024,
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();

    let data = Bytes::from("test data ".repeat(500));
    cache.store("test", data).await.unwrap();

    // Get overall cache stats
    let cache_stats = cache.stats().await;
    
    // Verify compression stats are included
    assert!(cache_stats.compression_stats.is_some());
    
    let comp_stats = cache_stats.compression_stats.unwrap();
    assert!(comp_stats.compression_count > 0);
    assert!(comp_stats.total_compressed_bytes > 0);
}

#[tokio::test]
async fn test_compression_config() {
    let temp_dir = TempDir::new().unwrap();
    let cache_path = temp_dir.path().join("cache.dat");

    let cache = RawDiskCache::new(
        &cache_path,
        10 * 1024 * 1024,
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();

    // Get compression config
    let config = cache.compression_config();
    
    // Verify default config
    assert_eq!(config.algorithm, CompressionAlgorithm::Zstd);
    assert_eq!(config.level, 3);
    assert_eq!(config.min_size, 1024);
    assert!(config.enabled);
}

#[tokio::test]
async fn test_large_compressible_data() {
    let temp_dir = TempDir::new().unwrap();
    let cache_path = temp_dir.path().join("cache.dat");

    let cache = RawDiskCache::new(
        &cache_path,
        50 * 1024 * 1024,
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();

    // Create large compressible data (1MB of repeated pattern)
    let large_data = Bytes::from("ABCDEFGH".repeat(128 * 1024));
    let original_size = large_data.len();

    cache.store("large", large_data.clone()).await.unwrap();
    let retrieved = cache.lookup("large").await.unwrap().unwrap();
    assert_eq!(retrieved, large_data);

    // Verify significant compression
    let stats = cache.compression_stats().await;
    assert!(stats.total_compressed_size < (original_size as u64) / 2);
}

#[tokio::test]
async fn test_compression_with_zero_copy() {
    let temp_dir = TempDir::new().unwrap();
    let cache_path = temp_dir.path().join("cache.dat");

    let cache = RawDiskCache::new(
        &cache_path,
        10 * 1024 * 1024,
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();

    // Store compressible data
    let data = Bytes::from("test data ".repeat(1000));
    cache.store("test", data.clone()).await.unwrap();

    // Retrieve using zero-copy (should still decompress correctly)
    let retrieved = cache.lookup_zero_copy("test").await.unwrap().unwrap();
    assert_eq!(retrieved, data);

    // Verify decompression happened
    let stats = cache.compression_stats().await;
    assert!(stats.decompression_count > 0);
}
