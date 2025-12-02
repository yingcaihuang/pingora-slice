//! Example demonstrating defragmentation functionality
//!
//! This example shows how to:
//! - Detect fragmentation in the cache
//! - Configure defragmentation settings
//! - Run defragmentation manually or in background
//! - Monitor defragmentation statistics

use bytes::Bytes;
use pingora_slice::raw_disk::{DefragConfig, RawDiskCache};
use std::time::Duration;
use tempfile::NamedTempFile;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    println!("=== Raw Disk Cache Defragmentation Example ===\n");
    
    // Create a temporary file for the cache
    let temp_file = NamedTempFile::new()?;
    let path = temp_file.path();
    
    println!("Creating cache at: {:?}", path);
    
    // Create cache with 50MB size
    let cache = RawDiskCache::new(
        path,
        50 * 1024 * 1024,
        4096,
        Duration::from_secs(3600),
    )
    .await?;
    
    println!("Cache created successfully\n");
    
    // Configure defragmentation
    let defrag_config = DefragConfig {
        fragmentation_threshold: 0.2, // Trigger at 20% fragmentation
        batch_size: 50,
        incremental: true,
        min_free_space_ratio: 0.1,
        target_compaction_ratio: 0.95,
    };
    cache.update_defrag_config(defrag_config).await;
    
    println!("Defragmentation configuration:");
    let config = cache.defrag_config().await;
    println!("  Fragmentation threshold: {:.1}%", config.fragmentation_threshold * 100.0);
    println!("  Batch size: {}", config.batch_size);
    println!("  Incremental: {}", config.incremental);
    println!("  Min free space ratio: {:.1}%", config.min_free_space_ratio * 100.0);
    println!("  Target compaction ratio: {:.1}%\n", config.target_compaction_ratio * 100.0);
    
    // Phase 1: Add initial entries
    println!("Phase 1: Adding 100 entries...");
    for i in 0..100 {
        let key = format!("entry_{:03}", i);
        let data = vec![i as u8; 5000]; // 5KB each
        cache.store(&key, Bytes::from(data)).await?;
    }
    
    let stats = cache.stats().await;
    println!("  Entries: {}", stats.entries);
    println!("  Used blocks: {}", stats.used_blocks);
    println!("  Free blocks: {}", stats.free_blocks);
    println!("  Fragmentation: {:.2}%\n", stats.fragmentation_ratio * 100.0);
    
    // Phase 2: Remove every third entry to create fragmentation
    println!("Phase 2: Removing every third entry to create fragmentation...");
    for i in (0..100).step_by(3) {
        let key = format!("entry_{:03}", i);
        cache.remove(&key).await?;
    }
    
    let stats = cache.stats().await;
    println!("  Entries: {}", stats.entries);
    println!("  Used blocks: {}", stats.used_blocks);
    println!("  Free blocks: {}", stats.free_blocks);
    println!("  Fragmentation: {:.2}%\n", stats.fragmentation_ratio * 100.0);
    
    // Phase 3: Add more entries (will be placed in gaps and at the end)
    println!("Phase 3: Adding 50 more entries...");
    for i in 100..150 {
        let key = format!("entry_{:03}", i);
        let data = vec![i as u8; 5000];
        cache.store(&key, Bytes::from(data)).await?;
    }
    
    let stats = cache.stats().await;
    println!("  Entries: {}", stats.entries);
    println!("  Used blocks: {}", stats.used_blocks);
    println!("  Free blocks: {}", stats.free_blocks);
    println!("  Fragmentation: {:.2}%\n", stats.fragmentation_ratio * 100.0);
    
    // Check if defragmentation should be triggered
    let should_defrag = cache.should_defragment().await;
    println!("Should defragment: {}\n", should_defrag);
    
    if should_defrag {
        // Phase 4: Run defragmentation
        println!("Phase 4: Running defragmentation...");
        let start = std::time::Instant::now();
        let moved = cache.defragment().await?;
        let duration = start.elapsed();
        
        println!("  Entries moved: {}", moved);
        println!("  Duration: {:?}\n", duration);
        
        let stats = cache.stats().await;
        println!("After defragmentation:");
        println!("  Entries: {}", stats.entries);
        println!("  Used blocks: {}", stats.used_blocks);
        println!("  Free blocks: {}", stats.free_blocks);
        println!("  Fragmentation: {:.2}%\n", stats.fragmentation_ratio * 100.0);
        
        // Show defragmentation statistics
        if let Some(defrag_stats) = stats.defrag_stats {
            println!("Defragmentation statistics:");
            println!("  Total runs: {}", defrag_stats.total_runs);
            println!("  Total entries moved: {}", defrag_stats.total_entries_moved);
            println!("  Total bytes moved: {} KB", defrag_stats.total_bytes_moved / 1024);
            println!("  Total duration: {:?}", defrag_stats.total_duration);
            println!("  Failed moves: {}", defrag_stats.failed_moves);
            if defrag_stats.last_run.is_some() {
                println!("  Last fragmentation: {:.2}% -> {:.2}%",
                         defrag_stats.last_fragmentation_before * 100.0,
                         defrag_stats.last_fragmentation_after * 100.0);
            }
            println!();
        }
    }
    
    // Phase 5: Verify data integrity
    println!("Phase 5: Verifying data integrity...");
    let mut verified = 0;
    let mut errors = 0;
    
    for i in 0..100 {
        if i % 3 == 0 {
            continue; // These were removed
        }
        let key = format!("entry_{:03}", i);
        match cache.lookup(&key).await? {
            Some(data) => {
                if data.len() == 5000 && data[0] == i as u8 {
                    verified += 1;
                } else {
                    errors += 1;
                    println!("  ERROR: Data mismatch for {}", key);
                }
            }
            None => {
                errors += 1;
                println!("  ERROR: Entry not found: {}", key);
            }
        }
    }
    
    for i in 100..150 {
        let key = format!("entry_{:03}", i);
        match cache.lookup(&key).await? {
            Some(data) => {
                if data.len() == 5000 && data[0] == i as u8 {
                    verified += 1;
                } else {
                    errors += 1;
                    println!("  ERROR: Data mismatch for {}", key);
                }
            }
            None => {
                errors += 1;
                println!("  ERROR: Entry not found: {}", key);
            }
        }
    }
    
    println!("  Verified: {}", verified);
    println!("  Errors: {}\n", errors);
    
    // Phase 6: Demonstrate background defragmentation
    println!("Phase 6: Demonstrating background defragmentation...");
    
    // Create more fragmentation
    for i in (1..100).step_by(3) {
        let key = format!("entry_{:03}", i);
        cache.remove(&key).await?;
    }
    
    println!("  Created more fragmentation");
    let frag_before = cache.fragmentation_ratio().await;
    println!("  Fragmentation: {:.2}%", frag_before * 100.0);
    
    // Run in background
    cache.defragment_background().await;
    println!("  Background defragmentation started");
    
    // Wait a bit for it to complete
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    let frag_after = cache.fragmentation_ratio().await;
    println!("  Fragmentation after: {:.2}%\n", frag_after * 100.0);
    
    println!("=== Example completed successfully ===");
    
    Ok(())
}
