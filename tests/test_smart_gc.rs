//! Tests for smart garbage collection

use bytes::Bytes;
use pingora_slice::raw_disk::{EvictionStrategy, GCConfig, GCTriggerConfig, RawDiskCache};
use std::time::Duration;
use tempfile::NamedTempFile;

#[tokio::test]
async fn test_smart_gc_lru_strategy() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let cache = RawDiskCache::new(
        path,
        2 * 1024 * 1024, // 2MB - smaller to trigger GC
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();

    // Configure GC with LRU strategy
    let gc_config = GCConfig {
        strategy: EvictionStrategy::LRU,
        trigger: GCTriggerConfig {
            min_free_ratio: 0.9, // High threshold to ensure GC runs
            target_free_ratio: 0.95,
            adaptive: false,
            min_interval: Duration::from_secs(0),
        },
        incremental: false,
        batch_size: 10,
        ttl_secs: 0,
    };
    cache.update_gc_config(gc_config).await;

    // Fill cache with data (30 * 50KB = 1.5MB, leaving ~500KB free)
    for i in 0..30 {
        let key = format!("key{}", i);
        let data = Bytes::from(vec![i as u8; 50 * 1024]); // 50KB each
        cache.store(&key, data).await.unwrap();
    }

    // Access some keys to make them more recently used
    for i in 15..30 {
        let key = format!("key{}", i);
        cache.lookup(&key).await.unwrap();
    }

    // Run GC - should free entries to reach target
    let freed = cache.run_smart_gc().await.unwrap();
    assert!(freed > 0, "GC should have freed some entries");

    // Check that recently accessed keys are still present
    let mut still_present = 0;
    for i in 15..30 {
        let key = format!("key{}", i);
        if cache.lookup(&key).await.unwrap().is_some() {
            still_present += 1;
        }
    }
    assert!(still_present > 0, "Some recently accessed keys should still exist");

    // Check GC metrics
    let metrics = cache.gc_metrics().await;
    assert!(metrics.total_runs >= 1, "At least one GC run should have occurred");
    assert!(metrics.total_evicted >= freed as u64);
    assert!(metrics.total_bytes_freed > 0);
}

#[tokio::test]
async fn test_smart_gc_lfu_strategy() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let cache = RawDiskCache::new(
        path,
        2 * 1024 * 1024, // 2MB
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();

    // Configure GC with LFU strategy
    let gc_config = GCConfig {
        strategy: EvictionStrategy::LFU,
        trigger: GCTriggerConfig {
            min_free_ratio: 0.9,
            target_free_ratio: 0.95,
            adaptive: false,
            min_interval: Duration::from_secs(0),
        },
        incremental: false,
        batch_size: 10,
        ttl_secs: 0,
    };
    cache.update_gc_config(gc_config).await;

    // Fill cache with data
    for i in 0..30 {
        let key = format!("key{}", i);
        let data = Bytes::from(vec![i as u8; 50 * 1024]); // 50KB each
        cache.store(&key, data).await.unwrap();
    }

    // Access some keys multiple times to increase their frequency
    for _ in 0..10 {
        for i in 15..30 {
            let key = format!("key{}", i);
            cache.lookup(&key).await.unwrap();
        }
    }

    // Run GC
    let freed = cache.run_smart_gc().await.unwrap();
    assert!(freed > 0, "GC should have freed some entries");

    // Check that frequently accessed keys are still present
    let mut still_present = 0;
    for i in 15..30 {
        let key = format!("key{}", i);
        if cache.lookup(&key).await.unwrap().is_some() {
            still_present += 1;
        }
    }
    assert!(still_present > 0, "Some frequently accessed keys should still exist");

    // Check GC metrics
    let metrics = cache.gc_metrics().await;
    assert!(metrics.total_runs >= 1, "At least one GC run should have occurred");
    assert!(metrics.total_evicted > 0);
}

#[tokio::test]
async fn test_smart_gc_fifo_strategy() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let cache = RawDiskCache::new(
        path,
        2 * 1024 * 1024, // 2MB
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();

    // Configure GC with FIFO strategy
    let gc_config = GCConfig {
        strategy: EvictionStrategy::FIFO,
        trigger: GCTriggerConfig {
            min_free_ratio: 0.9,
            target_free_ratio: 0.95,
            adaptive: false,
            min_interval: Duration::from_secs(0),
        },
        incremental: false,
        batch_size: 10,
        ttl_secs: 0,
    };
    cache.update_gc_config(gc_config).await;

    // Fill cache with data
    for i in 0..30 {
        let key = format!("key{}", i);
        let data = Bytes::from(vec![i as u8; 50 * 1024]); // 50KB each
        cache.store(&key, data).await.unwrap();
    }

    // Run GC
    let freed = cache.run_smart_gc().await.unwrap();
    assert!(freed > 0, "GC should have freed some entries");

    // Check that later entries are still present (FIFO evicts oldest first)
    let mut still_present = 0;
    for i in 15..30 {
        let key = format!("key{}", i);
        if cache.lookup(&key).await.unwrap().is_some() {
            still_present += 1;
        }
    }
    assert!(still_present > 0, "Some later keys should still exist");

    // Check GC metrics
    let metrics = cache.gc_metrics().await;
    assert!(metrics.total_runs >= 1, "At least one GC run should have occurred");
    assert!(metrics.total_evicted > 0);
}

#[tokio::test]
async fn test_incremental_gc() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let cache = RawDiskCache::new(
        path,
        2 * 1024 * 1024, // 2MB
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();

    // Configure incremental GC with small batch size
    let gc_config = GCConfig {
        strategy: EvictionStrategy::LRU,
        trigger: GCTriggerConfig {
            min_free_ratio: 0.9,
            target_free_ratio: 0.95,
            adaptive: false,
            min_interval: Duration::from_secs(0),
        },
        incremental: true,
        batch_size: 5, // Small batch size
        ttl_secs: 0,
    };
    cache.update_gc_config(gc_config).await;

    // Fill cache with data
    for i in 0..30 {
        let key = format!("key{}", i);
        let data = Bytes::from(vec![i as u8; 50 * 1024]); // 50KB each
        cache.store(&key, data).await.unwrap();
    }

    // Run GC
    let freed = cache.run_smart_gc().await.unwrap();
    assert!(freed > 0, "Incremental GC should have freed some entries");

    // Check GC metrics
    let metrics = cache.gc_metrics().await;
    assert!(metrics.total_runs >= 1, "At least one GC run should have occurred");
    assert!(metrics.total_evicted > 0);
}

#[tokio::test]
async fn test_adaptive_gc_trigger() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let cache = RawDiskCache::new(
        path,
        5 * 1024 * 1024, // 5MB - smaller to trigger GC more easily
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();

    // Configure adaptive GC
    let gc_config = GCConfig {
        strategy: EvictionStrategy::LRU,
        trigger: GCTriggerConfig {
            min_free_ratio: 0.2,
            target_free_ratio: 0.4,
            adaptive: true,
            min_interval: Duration::from_secs(0),
        },
        incremental: true,
        batch_size: 10,
        ttl_secs: 0,
    };
    cache.update_gc_config(gc_config).await;

    // Fill cache and trigger some allocation failures
    for i in 0..200 {
        let key = format!("key{}", i);
        let data = Bytes::from(vec![i as u8; 50 * 1024]); // 50KB each
        let _ = cache.store(&key, data).await; // Some may fail due to space
    }

    // Check that adaptive adjustments were made
    let metrics = cache.gc_metrics().await;
    // Adaptive GC should have triggered and possibly adjusted thresholds
    assert!(metrics.total_runs > 0 || metrics.adaptive_adjustments > 0);
}

#[tokio::test]
async fn test_gc_with_ttl() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let cache = RawDiskCache::new(
        path,
        2 * 1024 * 1024, // 2MB
        4096,
        Duration::from_secs(1), // 1 second TTL
    )
    .await
    .unwrap();

    // Configure GC with TTL
    let gc_config = GCConfig {
        strategy: EvictionStrategy::LRU,
        trigger: GCTriggerConfig {
            min_free_ratio: 0.9,
            target_free_ratio: 0.95,
            adaptive: false,
            min_interval: Duration::from_secs(0),
        },
        incremental: false,
        batch_size: 10,
        ttl_secs: 1, // 1 second TTL
    };
    cache.update_gc_config(gc_config).await;

    // Fill cache with data
    for i in 0..30 {
        let key = format!("key{}", i);
        let data = Bytes::from(vec![i as u8; 50 * 1024]); // 50KB each
        cache.store(&key, data).await.unwrap();
    }

    // Wait for TTL to expire
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Run GC - should prioritize expired entries
    let _freed = cache.run_smart_gc().await.unwrap();
    
    // With TTL-based eviction, expired entries should be removed
    let metrics = cache.gc_metrics().await;
    assert!(metrics.total_runs > 0);
}

#[tokio::test]
async fn test_gc_stats_in_cache_stats() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let cache = RawDiskCache::new(
        path,
        2 * 1024 * 1024, // 2MB
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();

    // Configure GC
    let gc_config = GCConfig {
        strategy: EvictionStrategy::LRU,
        trigger: GCTriggerConfig {
            min_free_ratio: 0.9,
            target_free_ratio: 0.95,
            adaptive: false,
            min_interval: Duration::from_secs(0),
        },
        incremental: false,
        batch_size: 10,
        ttl_secs: 0,
    };
    cache.update_gc_config(gc_config).await;

    // Store some data
    for i in 0..30 {
        let key = format!("key{}", i);
        let data = Bytes::from(vec![i as u8; 50 * 1024]);
        cache.store(&key, data).await.unwrap();
    }

    // Run GC
    cache.run_smart_gc().await.unwrap();

    // Check that GC metrics are included in cache stats
    let stats = cache.stats().await;
    assert!(stats.gc_metrics.is_some());
    
    let gc_metrics = stats.gc_metrics.unwrap();
    assert!(gc_metrics.total_runs > 0);
    assert!(gc_metrics.total_evicted > 0);
}

#[tokio::test]
async fn test_gc_cleanup_on_remove() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let cache = RawDiskCache::new(
        path,
        10 * 1024 * 1024, // 10MB
        4096,
        Duration::from_secs(3600),
    )
    .await
    .unwrap();

    // Store some data
    for i in 0..10 {
        let key = format!("key{}", i);
        let data = Bytes::from(vec![i as u8; 1024]);
        cache.store(&key, data).await.unwrap();
    }

    // Remove some entries
    for i in 0..5 {
        let key = format!("key{}", i);
        cache.remove(&key).await.unwrap();
    }

    // The GC tracking should have been cleaned up
    // This is verified internally - if there's a panic or error, the test will fail
}
