//! Example demonstrating the SliceCache functionality

use bytes::Bytes;
use pingora_slice::{ByteRange, SliceCache};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("=== Pingora Slice Cache Example ===\n");

    // Create a cache with 1 hour TTL
    let cache = SliceCache::new(Duration::from_secs(3600));

    let url = "http://example.com/large-file.bin";
    
    // Define some byte ranges
    let ranges = vec![
        ByteRange::new(0, 1048575)?,      // First 1MB
        ByteRange::new(1048576, 2097151)?, // Second 1MB
        ByteRange::new(2097152, 3145727)?, // Third 1MB
    ];

    println!("1. Storing slices in cache...");
    for (idx, range) in ranges.iter().enumerate() {
        let data = Bytes::from(vec![idx as u8; 1024]); // Dummy data
        cache.store_slice(url, range, data).await?;
        println!("   Stored slice {}: bytes {}-{}", idx, range.start, range.end);
    }

    println!("\n2. Looking up individual slices...");
    for (idx, range) in ranges.iter().enumerate() {
        match cache.lookup_slice(url, range).await? {
            Some(data) => {
                println!("   Cache HIT for slice {}: {} bytes", idx, data.len());
            }
            None => {
                println!("   Cache MISS for slice {}", idx);
            }
        }
    }

    println!("\n3. Batch lookup of multiple slices...");
    let cached_slices = cache.lookup_multiple(url, &ranges).await;
    println!("   Found {} out of {} slices in cache", cached_slices.len(), ranges.len());
    for (idx, data) in cached_slices.iter() {
        println!("   Slice {}: {} bytes", idx, data.len());
    }

    println!("\n4. Testing cache key uniqueness...");
    let key1 = cache.generate_cache_key("http://example.com/file1.bin", &ranges[0]);
    let key2 = cache.generate_cache_key("http://example.com/file2.bin", &ranges[0]);
    let key3 = cache.generate_cache_key("http://example.com/file1.bin", &ranges[1]);
    
    println!("   Key for file1, range0: {}", key1);
    println!("   Key for file2, range0: {}", key2);
    println!("   Key for file1, range1: {}", key3);
    println!("   All keys are unique: {}", 
        key1 != key2 && key1 != key3 && key2 != key3);

    println!("\n5. Testing cache expiration...");
    let short_ttl_cache = SliceCache::new(Duration::from_millis(500));
    let test_range = ByteRange::new(0, 1023)?;
    let test_data = Bytes::from(vec![42; 1024]);
    
    short_ttl_cache.store_slice(url, &test_range, test_data).await?;
    println!("   Stored slice with 500ms TTL");
    
    match short_ttl_cache.lookup_slice(url, &test_range).await? {
        Some(_) => println!("   Immediate lookup: Cache HIT"),
        None => println!("   Immediate lookup: Cache MISS"),
    }
    
    println!("   Waiting 600ms for expiration...");
    tokio::time::sleep(Duration::from_millis(600)).await;
    
    match short_ttl_cache.lookup_slice(url, &test_range).await? {
        Some(_) => println!("   After expiration: Cache HIT"),
        None => println!("   After expiration: Cache MISS (expired)"),
    }

    println!("\n=== Example Complete ===");
    Ok(())
}
