use bytes::Bytes;
use pingora_slice::raw_disk::{AccessPattern, PrefetchConfig, RawDiskCache};
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== Raw Disk Cache Prefetch Example ===\n");

    // Create a temporary file for the cache
    let temp_file = tempfile::NamedTempFile::new()?;
    let cache_path = temp_file.path();

    // Configure prefetch
    let prefetch_config = PrefetchConfig {
        enabled: true,
        max_prefetch_entries: 5,
        cache_size: 50,
        pattern_window_size: 20,
        sequential_threshold: 0.7,
        temporal_threshold: 0.5,
    };

    println!("Creating cache with prefetch configuration:");
    println!("  - Max prefetch entries: {}", prefetch_config.max_prefetch_entries);
    println!("  - Prefetch cache size: {}", prefetch_config.cache_size);
    println!("  - Pattern window size: {}", prefetch_config.pattern_window_size);
    println!();

    // Create cache
    let cache = RawDiskCache::new_with_prefetch(
        cache_path,
        50 * 1024 * 1024, // 50MB
        4096,
        Duration::from_secs(3600),
        prefetch_config,
    )
    .await?;

    // Populate cache with test data
    println!("Populating cache with 100 entries...");
    for i in 0..100 {
        let key = format!("key_{:03}", i);
        let data = Bytes::from(format!("This is test data for entry {}", i).repeat(100));
        cache.store(&key, data).await?;
    }
    println!("Cache populated.\n");

    // Demonstrate sequential access pattern
    println!("=== Sequential Access Pattern ===");
    demonstrate_sequential_access(&cache).await?;

    // Clear prefetch cache
    cache.clear_prefetch_cache().await;

    // Demonstrate temporal access pattern
    println!("\n=== Temporal Access Pattern ===");
    demonstrate_temporal_access(&cache).await?;

    // Clear prefetch cache
    cache.clear_prefetch_cache().await;

    // Demonstrate random access pattern
    println!("\n=== Random Access Pattern ===");
    demonstrate_random_access(&cache).await?;

    // Show final statistics
    println!("\n=== Final Cache Statistics ===");
    let stats = cache.stats().await;
    println!("Total entries: {}", stats.entries);
    println!("Cache hits: {}", stats.hits);
    println!("Cache misses: {}", stats.misses);
    println!("Hit rate: {:.2}%", (stats.hits as f64 / (stats.hits + stats.misses) as f64) * 100.0);

    if let Some(prefetch_stats) = stats.prefetch_stats {
        println!("\nPrefetch Statistics:");
        println!("  Cache size: {}/{}", prefetch_stats.cache_size, prefetch_stats.max_size);
        println!("  Prefetch hits: {}", prefetch_stats.hits);
        println!("  Prefetch misses: {}", prefetch_stats.misses);
        println!("  Prefetch hit rate: {:.2}%", prefetch_stats.hit_rate * 100.0);
    }

    Ok(())
}

async fn demonstrate_sequential_access(cache: &RawDiskCache) -> Result<(), Box<dyn std::error::Error>> {
    println!("Accessing keys sequentially (key_000 to key_029)...");

    let start = Instant::now();
    for i in 0..30 {
        let key = format!("key_{:03}", i);
        let result = cache.lookup(&key).await?;
        assert!(result.is_some());
    }
    let duration = start.elapsed();

    // Give prefetch time to work
    tokio::time::sleep(Duration::from_millis(100)).await;

    let pattern = cache.access_pattern().await;
    let prefetch_stats = cache.prefetch_stats().await;

    println!("Access time: {:?}", duration);
    println!("Detected pattern: {:?}", pattern);
    println!("Prefetch cache size: {}", prefetch_stats.cache_size);
    println!("Prefetch hits: {}", prefetch_stats.hits);
    println!("Prefetch misses: {}", prefetch_stats.misses);

    // Now access the next batch - should benefit from prefetch
    println!("\nAccessing next batch (key_030 to key_039) - should benefit from prefetch...");
    let start = Instant::now();
    for i in 30..40 {
        let key = format!("key_{:03}", i);
        let result = cache.lookup(&key).await?;
        assert!(result.is_some());
    }
    let duration = start.elapsed();

    let prefetch_stats = cache.prefetch_stats().await;
    println!("Access time: {:?}", duration);
    println!("Prefetch hits: {}", prefetch_stats.hits);

    Ok(())
}

async fn demonstrate_temporal_access(cache: &RawDiskCache) -> Result<(), Box<dyn std::error::Error>> {
    println!("Accessing same keys repeatedly (temporal pattern)...");

    let hot_keys = vec!["key_010", "key_020", "key_030", "key_040"];

    let start = Instant::now();
    for _ in 0..10 {
        for key in &hot_keys {
            let result = cache.lookup(key).await?;
            assert!(result.is_some());
        }
    }
    let duration = start.elapsed();

    // Give prefetch time to work
    tokio::time::sleep(Duration::from_millis(100)).await;

    let pattern = cache.access_pattern().await;
    let prefetch_stats = cache.prefetch_stats().await;

    println!("Access time: {:?}", duration);
    println!("Detected pattern: {:?}", pattern);
    println!("Prefetch cache size: {}", prefetch_stats.cache_size);
    println!("Prefetch hits: {}", prefetch_stats.hits);

    Ok(())
}

async fn demonstrate_random_access(cache: &RawDiskCache) -> Result<(), Box<dyn std::error::Error>> {
    println!("Accessing keys randomly...");

    let random_keys = vec![
        "key_005", "key_087", "key_023", "key_056", "key_012",
        "key_091", "key_034", "key_067", "key_045", "key_078",
    ];

    let start = Instant::now();
    for key in &random_keys {
        let result = cache.lookup(key).await?;
        assert!(result.is_some());
    }
    let duration = start.elapsed();

    let pattern = cache.access_pattern().await;
    let prefetch_stats = cache.prefetch_stats().await;

    println!("Access time: {:?}", duration);
    println!("Detected pattern: {:?}", pattern);
    println!("Prefetch cache size: {}", prefetch_stats.cache_size);
    println!("Note: Random access typically doesn't benefit from prefetch");

    Ok(())
}
