//! Example demonstrating cache purge operations
//!
//! This example shows how to:
//! - Store data in the tiered cache
//! - Purge specific cache entries
//! - Purge all entries for a URL
//! - Purge all cache entries

use bytes::Bytes;
use pingora_slice::models::ByteRange;
use pingora_slice::tiered_cache::TieredCache;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create a temporary directory for the cache
    let cache_dir = tempfile::tempdir()?;
    println!("Cache directory: {:?}", cache_dir.path());

    // Create a tiered cache
    let cache = TieredCache::new(
        Duration::from_secs(3600), // 1 hour TTL
        10 * 1024 * 1024,          // 10MB L1 cache
        cache_dir.path(),
    )
    .await?;

    println!("\n=== Example 1: Purge a specific cache entry ===");
    
    // Store some data
    let url1 = "http://example.com/video.mp4";
    let range1 = ByteRange::new(0, 1023)?;
    let data1 = Bytes::from(vec![1u8; 1024]);
    
    cache.store(url1, &range1, data1.clone())?;
    println!("Stored: {} bytes={}-{}", url1, range1.start, range1.end);
    
    // Verify it's cached
    let result = cache.lookup(url1, &range1).await?;
    println!("Lookup result: {}", if result.is_some() { "HIT" } else { "MISS" });
    
    // Purge the specific entry
    let purged = cache.purge(url1, &range1).await?;
    println!("Purged: {}", purged);
    
    // Verify it's gone
    let result = cache.lookup(url1, &range1).await?;
    println!("Lookup after purge: {}", if result.is_some() { "HIT" } else { "MISS" });

    println!("\n=== Example 2: Purge all slices for a URL ===");
    
    // Store multiple slices for the same URL
    let url2 = "http://example.com/largefile.bin";
    let ranges = vec![
        ByteRange::new(0, 1023)?,
        ByteRange::new(1024, 2047)?,
        ByteRange::new(2048, 3071)?,
        ByteRange::new(3072, 4095)?,
    ];
    
    for range in &ranges {
        let data = Bytes::from(vec![2u8; 1024]);
        cache.store(url2, range, data)?;
        println!("Stored: {} bytes={}-{}", url2, range.start, range.end);
    }
    
    // Verify all are cached
    for range in &ranges {
        let result = cache.lookup(url2, range).await?;
        println!("Lookup {}-{}: {}", range.start, range.end, 
                 if result.is_some() { "HIT" } else { "MISS" });
    }
    
    // Purge all slices for this URL
    let purged_count = cache.purge_url(url2).await?;
    println!("\nPurged {} entries for URL: {}", purged_count, url2);
    
    // Verify they're all gone
    for range in &ranges {
        let result = cache.lookup(url2, range).await?;
        println!("Lookup after purge {}-{}: {}", range.start, range.end,
                 if result.is_some() { "HIT" } else { "MISS" });
    }

    println!("\n=== Example 3: Purge all cache entries ===");
    
    // Store data for multiple URLs
    let urls = vec![
        "http://example.com/file1.dat",
        "http://example.com/file2.dat",
        "http://example.com/file3.dat",
    ];
    
    let range = ByteRange::new(0, 1023)?;
    for url in &urls {
        let data = Bytes::from(vec![3u8; 1024]);
        cache.store(url, &range, data)?;
        println!("Stored: {}", url);
    }
    
    // Show cache stats
    let stats = cache.get_stats();
    println!("\nCache stats before purge:");
    println!("  L1 entries: {}", stats.l1_entries);
    println!("  L1 bytes: {}", stats.l1_bytes);
    println!("  L1 hits: {}", stats.l1_hits);
    println!("  L2 hits: {}", stats.l2_hits);
    println!("  Misses: {}", stats.misses);
    
    // Purge all entries
    let purged_count = cache.purge_all().await?;
    println!("\nPurged all {} entries", purged_count);
    
    // Show cache stats after purge
    let stats = cache.get_stats();
    println!("\nCache stats after purge:");
    println!("  L1 entries: {}", stats.l1_entries);
    println!("  L1 bytes: {}", stats.l1_bytes);
    
    // Verify all are gone
    for url in &urls {
        let result = cache.lookup(url, &range).await?;
        println!("Lookup {}: {}", url, if result.is_some() { "HIT" } else { "MISS" });
    }

    println!("\n=== Example 4: Purge with pattern matching ===");
    
    // Store data for multiple URLs with different patterns
    let test_urls = vec![
        ("http://cdn.example.com/videos/movie1.mp4", ByteRange::new(0, 1023)?),
        ("http://cdn.example.com/videos/movie2.mp4", ByteRange::new(0, 1023)?),
        ("http://cdn.example.com/images/photo1.jpg", ByteRange::new(0, 1023)?),
        ("http://api.example.com/data.json", ByteRange::new(0, 1023)?),
    ];
    
    for (url, range) in &test_urls {
        let data = Bytes::from(vec![4u8; 1024]);
        cache.store(url, range, data)?;
        println!("Stored: {}", url);
    }
    
    // Purge only video files
    println!("\nPurging video files...");
    for (url, _) in &test_urls {
        if url.contains("/videos/") {
            let purged = cache.purge_url(url).await?;
            println!("Purged {} entries for: {}", purged, url);
        }
    }
    
    // Verify selective purge
    println!("\nVerifying selective purge:");
    for (url, range) in &test_urls {
        let result = cache.lookup(url, range).await?;
        println!("  {}: {}", url, if result.is_some() { "HIT" } else { "MISS" });
    }

    println!("\n=== Cache Purge Examples Complete ===");
    
    Ok(())
}
