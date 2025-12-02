use bytes::Bytes;
use pingora_slice::raw_disk::{AccessPattern, PrefetchConfig, RawDiskCache};
use std::time::Duration;
use tempfile::NamedTempFile;

#[tokio::test]
async fn test_prefetch_sequential_pattern() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    // Create cache with prefetch enabled
    let prefetch_config = PrefetchConfig {
        enabled: true,
        max_prefetch_entries: 3,
        cache_size: 10,
        pattern_window_size: 10,
        sequential_threshold: 0.7,
        temporal_threshold: 0.5,
    };

    let cache = RawDiskCache::new_with_prefetch(
        path,
        10 * 1024 * 1024, // 10MB
        4096,
        Duration::from_secs(3600),
        prefetch_config,
    )
    .await
    .unwrap();

    // Store sequential data
    for i in 0..20 {
        let key = format!("key_{}", i);
        let data = Bytes::from(format!("data_{}", i));
        cache.store(&key, data).await.unwrap();
    }

    // Access sequentially to establish pattern
    for i in 0..10 {
        let key = format!("key_{}", i);
        let result = cache.lookup(&key).await.unwrap();
        assert!(result.is_some());
    }

    // Give prefetch time to work
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Check that pattern is detected as sequential
    let pattern = cache.access_pattern().await;
    assert_eq!(pattern, AccessPattern::Sequential);

    // Check prefetch stats
    let stats = cache.prefetch_stats().await;
    assert!(stats.cache_size > 0, "Prefetch cache should have entries");
    println!("Prefetch stats: {:?}", stats);
}

#[tokio::test]
async fn test_prefetch_temporal_pattern() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let prefetch_config = PrefetchConfig {
        enabled: true,
        max_prefetch_entries: 3,
        cache_size: 10,
        pattern_window_size: 20,
        sequential_threshold: 0.7,
        temporal_threshold: 0.5,
    };

    let cache = RawDiskCache::new_with_prefetch(
        path,
        10 * 1024 * 1024,
        4096,
        Duration::from_secs(3600),
        prefetch_config,
    )
    .await
    .unwrap();

    // Store data
    for i in 0..10 {
        let key = format!("key_{}", i);
        let data = Bytes::from(format!("data_{}", i));
        cache.store(&key, data).await.unwrap();
    }

    // Access same keys repeatedly (temporal pattern)
    for _ in 0..5 {
        cache.lookup("key_1").await.unwrap();
        cache.lookup("key_5").await.unwrap();
        cache.lookup("key_1").await.unwrap();
        cache.lookup("key_9").await.unwrap();
    }

    // Give prefetch time to work
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Check that pattern is detected as temporal
    let pattern = cache.access_pattern().await;
    assert_eq!(pattern, AccessPattern::Temporal);

    let stats = cache.prefetch_stats().await;
    println!("Temporal prefetch stats: {:?}", stats);
}

#[tokio::test]
async fn test_prefetch_cache_hit() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let prefetch_config = PrefetchConfig {
        enabled: true,
        max_prefetch_entries: 5,
        cache_size: 20,
        pattern_window_size: 10,
        sequential_threshold: 0.7,
        temporal_threshold: 0.5,
    };

    let cache = RawDiskCache::new_with_prefetch(
        path,
        10 * 1024 * 1024,
        4096,
        Duration::from_secs(3600),
        prefetch_config,
    )
    .await
    .unwrap();

    // Store sequential data
    for i in 0..30 {
        let key = format!("key_{}", i);
        let data = Bytes::from(format!("data_{}", i));
        cache.store(&key, data).await.unwrap();
    }

    // Access sequentially to trigger prefetch
    for i in 0..10 {
        let key = format!("key_{}", i);
        cache.lookup(&key).await.unwrap();
    }

    // Give prefetch time to work
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Get initial stats
    let stats_before = cache.prefetch_stats().await;
    let hits_before = stats_before.hits;

    // Access keys that should be prefetched
    for i in 10..15 {
        let key = format!("key_{}", i);
        let result = cache.lookup(&key).await.unwrap();
        assert!(result.is_some());
    }

    // Check that we got prefetch hits
    let stats_after = cache.prefetch_stats().await;
    let hits_after = stats_after.hits;

    println!(
        "Prefetch hits: before={}, after={}, delta={}",
        hits_before,
        hits_after,
        hits_after - hits_before
    );

    // We should have at least some prefetch hits
    assert!(
        hits_after > hits_before,
        "Should have prefetch cache hits"
    );
}

#[tokio::test]
async fn test_prefetch_disabled() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let prefetch_config = PrefetchConfig {
        enabled: false,
        ..Default::default()
    };

    let cache = RawDiskCache::new_with_prefetch(
        path,
        10 * 1024 * 1024,
        4096,
        Duration::from_secs(3600),
        prefetch_config,
    )
    .await
    .unwrap();

    // Store and access data
    for i in 0..10 {
        let key = format!("key_{}", i);
        let data = Bytes::from(format!("data_{}", i));
        cache.store(&key, data).await.unwrap();
        cache.lookup(&key).await.unwrap();
    }

    // Prefetch should not be active
    let stats = cache.prefetch_stats().await;
    assert_eq!(stats.hits, 0);
    assert_eq!(stats.misses, 0);
    assert_eq!(stats.cache_size, 0);
}

#[tokio::test]
async fn test_prefetch_clear_cache() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let cache = RawDiskCache::new_with_prefetch(
        path,
        10 * 1024 * 1024,
        4096,
        Duration::from_secs(3600),
        PrefetchConfig::default(),
    )
    .await
    .unwrap();

    // Store and access data to populate prefetch cache
    for i in 0..10 {
        let key = format!("key_{}", i);
        let data = Bytes::from(format!("data_{}", i));
        cache.store(&key, data).await.unwrap();
        cache.lookup(&key).await.unwrap();
    }

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Clear prefetch cache
    cache.clear_prefetch_cache().await;

    // Check that cache is empty
    let stats = cache.prefetch_stats().await;
    assert_eq!(stats.cache_size, 0);
}

#[tokio::test]
async fn test_prefetch_with_cache_stats() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let cache = RawDiskCache::new_with_prefetch(
        path,
        10 * 1024 * 1024,
        4096,
        Duration::from_secs(3600),
        PrefetchConfig::default(),
    )
    .await
    .unwrap();

    // Store data
    for i in 0..5 {
        let key = format!("key_{}", i);
        let data = Bytes::from(format!("data_{}", i));
        cache.store(&key, data).await.unwrap();
    }

    // Access data
    for i in 0..5 {
        let key = format!("key_{}", i);
        cache.lookup(&key).await.unwrap();
    }

    // Get overall cache stats
    let stats = cache.stats().await;

    // Check that prefetch stats are included
    assert!(stats.prefetch_stats.is_some());

    println!("Cache stats: {:?}", stats);

    if let Some(prefetch_stats) = &stats.prefetch_stats {
        println!("Prefetch stats: {:?}", prefetch_stats);
    }

    // Basic sanity checks
    assert_eq!(stats.entries, 5);
}

#[tokio::test]
async fn test_prefetch_random_pattern() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let cache = RawDiskCache::new_with_prefetch(
        path,
        10 * 1024 * 1024,
        4096,
        Duration::from_secs(3600),
        PrefetchConfig::default(),
    )
    .await
    .unwrap();

    // Store data
    for i in 0..20 {
        let key = format!("key_{}", i);
        let data = Bytes::from(format!("data_{}", i));
        cache.store(&key, data).await.unwrap();
    }

    // Access randomly
    let random_order = vec![5, 1, 15, 3, 18, 7, 12, 2, 19, 9];
    for i in random_order {
        let key = format!("key_{}", i);
        cache.lookup(&key).await.unwrap();
    }

    // Pattern should be random
    let pattern = cache.access_pattern().await;
    assert_eq!(pattern, AccessPattern::Random);

    // Random pattern should not trigger much prefetching
    let stats = cache.prefetch_stats().await;
    println!("Random pattern prefetch stats: {:?}", stats);
}
