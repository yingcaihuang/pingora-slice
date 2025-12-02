//! Tests for raw disk cache metrics and monitoring

use bytes::Bytes;
use pingora_slice::raw_disk::{RawDiskCache, metrics::format_prometheus_metrics};
use std::time::Duration;
use tempfile::tempdir;

#[tokio::test]
async fn test_metrics_store_operations() {
    let temp_dir = tempdir().unwrap();
    let cache_file = temp_dir.path().join("test_metrics_store.dat");

    let cache = RawDiskCache::new(
        &cache_file,
        1024 * 1024, // 1MB
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();

    // Store some data
    cache.store("key1", Bytes::from("value1")).await.unwrap();
    cache.store("key2", Bytes::from("value2")).await.unwrap();

    let snapshot = cache.metrics_snapshot();
    assert_eq!(snapshot.store_operations, 2);
    assert_eq!(snapshot.store_successes, 2);
    assert_eq!(snapshot.store_failures, 0);
    assert_eq!(snapshot.store_success_rate(), 100.0);
}

#[tokio::test]
async fn test_metrics_lookup_operations() {
    let temp_dir = tempdir().unwrap();
    let cache_file = temp_dir.path().join("test_metrics_lookup.dat");

    let cache = RawDiskCache::new(
        &cache_file,
        1024 * 1024,
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();

    // Store and lookup
    cache.store("key1", Bytes::from("value1")).await.unwrap();
    cache.lookup("key1").await.unwrap();
    cache.lookup("key2").await.unwrap(); // Miss

    let snapshot = cache.metrics_snapshot();
    assert_eq!(snapshot.lookup_operations, 2);
    assert_eq!(snapshot.lookup_hits, 1);
    assert_eq!(snapshot.lookup_misses, 1);
    assert_eq!(snapshot.cache_hit_rate(), 50.0);
}

#[tokio::test]
async fn test_metrics_remove_operations() {
    let temp_dir = tempdir().unwrap();
    let cache_file = temp_dir.path().join("test_metrics_remove.dat");

    let cache = RawDiskCache::new(
        &cache_file,
        1024 * 1024,
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();

    // Store and remove
    cache.store("key1", Bytes::from("value1")).await.unwrap();
    cache.remove("key1").await.unwrap();

    let snapshot = cache.metrics_snapshot();
    assert_eq!(snapshot.remove_operations, 1);
}

#[tokio::test]
async fn test_metrics_cache_state() {
    let temp_dir = tempdir().unwrap();
    let cache_file = temp_dir.path().join("test_metrics_state.dat");

    let cache = RawDiskCache::new(
        &cache_file,
        1024 * 1024,
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();

    // Store some data
    for i in 0..5 {
        cache.store(&format!("key{}", i), Bytes::from("value")).await.unwrap();
    }

    let snapshot = cache.metrics_snapshot();
    assert_eq!(snapshot.current_entries, 5);
    assert!(snapshot.used_blocks > 0);
    assert!(snapshot.free_blocks > 0);
    assert!(snapshot.space_utilization() > 0.0);
    assert!(snapshot.space_utilization() < 100.0);
}

#[tokio::test]
async fn test_metrics_bytes_tracking() {
    let temp_dir = tempdir().unwrap();
    let cache_file = temp_dir.path().join("test_metrics_bytes.dat");

    let cache = RawDiskCache::new(
        &cache_file,
        1024 * 1024,
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();

    let data = Bytes::from(vec![0u8; 1000]);
    cache.store("key1", data).await.unwrap();
    cache.lookup("key1").await.unwrap();

    let snapshot = cache.metrics_snapshot();
    assert!(snapshot.bytes_written > 0);
    assert!(snapshot.bytes_read > 0);
}

#[tokio::test]
async fn test_health_check_healthy() {
    let temp_dir = tempdir().unwrap();
    let cache_file = temp_dir.path().join("test_health_healthy.dat");

    let cache = RawDiskCache::new(
        &cache_file,
        1024 * 1024,
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();

    let is_healthy = cache.health_check().await;
    assert!(is_healthy);
}

#[tokio::test]
async fn test_prometheus_format() {
    let temp_dir = tempdir().unwrap();
    let cache_file = temp_dir.path().join("test_prometheus.dat");

    let cache = RawDiskCache::new(
        &cache_file,
        1024 * 1024,
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();

    // Generate some metrics
    cache.store("key1", Bytes::from("value1")).await.unwrap();
    cache.lookup("key1").await.unwrap();

    let snapshot = cache.metrics_snapshot();
    let output = format_prometheus_metrics(&snapshot);

    // Verify Prometheus format
    assert!(output.contains("# HELP raw_disk_cache_store_operations_total"));
    assert!(output.contains("# TYPE raw_disk_cache_store_operations_total counter"));
    assert!(output.contains("raw_disk_cache_store_operations_total"));
    assert!(output.contains("raw_disk_cache_lookup_operations_total"));
    assert!(output.contains("raw_disk_cache_hit_rate"));
    assert!(output.contains("raw_disk_cache_entries"));
}

#[tokio::test]
async fn test_metrics_latency_tracking() {
    let temp_dir = tempdir().unwrap();
    let cache_file = temp_dir.path().join("test_metrics_latency.dat");

    let cache = RawDiskCache::new(
        &cache_file,
        1024 * 1024,
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();

    // Perform operations
    cache.store("key1", Bytes::from("value1")).await.unwrap();
    cache.lookup("key1").await.unwrap();
    cache.remove("key1").await.unwrap();

    let snapshot = cache.metrics_snapshot();
    
    // Latencies should be recorded (non-zero)
    assert!(snapshot.total_store_duration_us > 0);
    assert!(snapshot.total_lookup_duration_us > 0);
    assert!(snapshot.total_remove_duration_us > 0);
    
    // Average durations should be calculable
    assert!(snapshot.avg_store_duration_ms() >= 0.0);
    assert!(snapshot.avg_lookup_duration_ms() >= 0.0);
    assert!(snapshot.avg_remove_duration_ms() >= 0.0);
}
