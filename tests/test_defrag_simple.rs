//! Simple defragmentation test

use bytes::Bytes;
use pingora_slice::raw_disk::{DefragConfig, RawDiskCache};
use std::time::Duration;
use tempfile::NamedTempFile;

#[tokio::test]
async fn test_defrag_basic() {
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
    
    // Test fragmentation detection
    let frag = cache.fragmentation_ratio().await;
    assert_eq!(frag, 0.0);
    
    // Test configuration
    let config = DefragConfig::default();
    cache.update_defrag_config(config).await;
    
    let retrieved_config = cache.defrag_config().await;
    assert_eq!(retrieved_config.fragmentation_threshold, 0.3);
    
    // Test stats
    let stats = cache.defrag_stats().await;
    assert_eq!(stats.total_runs, 0);
    
    println!("Basic defragmentation test passed!");
}
