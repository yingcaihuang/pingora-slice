//! Integration tests for SliceCache with other components

use bytes::Bytes;
use pingora_slice::{ByteRange, SliceCache, SliceCalculator};
use std::time::Duration;

#[tokio::test]
async fn test_cache_with_slice_calculator() {
    // Create a cache
    let cache = SliceCache::new(Duration::from_secs(3600));
    
    // Create a slice calculator
    let calculator = SliceCalculator::new(1024 * 1024); // 1MB slices
    
    // Calculate slices for a 5MB file
    let file_size = 5 * 1024 * 1024;
    let slices = calculator.calculate_slices(file_size, None).unwrap();
    
    assert_eq!(slices.len(), 5);
    
    // Simulate storing some slices in cache
    let url = "http://example.com/file.bin";
    for (idx, slice) in slices.iter().enumerate() {
        if idx % 2 == 0 {
            // Store even-indexed slices
            let data = Bytes::from(vec![idx as u8; 1024]);
            cache.store_slice(url, &slice.range, data).await.unwrap();
        }
    }
    
    // Look up all slices
    let ranges: Vec<ByteRange> = slices.iter().map(|s| s.range).collect();
    let cached = cache.lookup_multiple(url, &ranges).await;
    
    // Should have 3 cached slices (indices 0, 2, 4)
    assert_eq!(cached.len(), 3);
    assert!(cached.contains_key(&0));
    assert!(cached.contains_key(&2));
    assert!(cached.contains_key(&4));
    assert!(!cached.contains_key(&1));
    assert!(!cached.contains_key(&3));
}

#[tokio::test]
async fn test_cache_with_partial_request() {
    let cache = SliceCache::new(Duration::from_secs(3600));
    let calculator = SliceCalculator::new(1024 * 1024); // 1MB slices
    
    // Calculate slices for a partial request (bytes 1MB-3MB of a 10MB file)
    let client_range = ByteRange::new(1024 * 1024, 3 * 1024 * 1024 - 1).unwrap();
    let slices = calculator.calculate_slices(10 * 1024 * 1024, Some(client_range)).unwrap();
    
    // Should only have 2 slices for the requested range
    assert_eq!(slices.len(), 2);
    
    // Store first slice in cache
    let url = "http://example.com/file.bin";
    let data = Bytes::from(vec![1; 1024]);
    cache.store_slice(url, &slices[0].range, data).await.unwrap();
    
    // Look up both slices
    let ranges: Vec<ByteRange> = slices.iter().map(|s| s.range).collect();
    let cached = cache.lookup_multiple(url, &ranges).await;
    
    // Should have 1 cached slice
    assert_eq!(cached.len(), 1);
    assert!(cached.contains_key(&0));
    assert!(!cached.contains_key(&1));
}

#[tokio::test]
async fn test_cache_key_collision_prevention() {
    let cache = SliceCache::new(Duration::from_secs(3600));
    
    // Create overlapping ranges for different files
    let range1 = ByteRange::new(0, 1023).unwrap();
    let range2 = ByteRange::new(0, 1023).unwrap();
    
    let url1 = "http://example.com/file1.bin";
    let url2 = "http://example.com/file2.bin";
    
    // Store data for both files with same range
    let data1 = Bytes::from(vec![1; 1024]);
    let data2 = Bytes::from(vec![2; 1024]);
    
    cache.store_slice(url1, &range1, data1.clone()).await.unwrap();
    cache.store_slice(url2, &range2, data2.clone()).await.unwrap();
    
    // Look up both - should get different data
    let cached1 = cache.lookup_slice(url1, &range1).await.unwrap().unwrap();
    let cached2 = cache.lookup_slice(url2, &range2).await.unwrap().unwrap();
    
    assert_eq!(cached1, data1);
    assert_eq!(cached2, data2);
    assert_ne!(cached1, cached2);
}

#[tokio::test]
async fn test_cache_concurrent_access() {
    use std::sync::Arc;
    
    let cache = Arc::new(SliceCache::new(Duration::from_secs(3600)));
    let url = "http://example.com/file.bin";
    
    // Spawn multiple tasks that store and lookup slices concurrently
    let mut handles = vec![];
    
    for i in 0..10 {
        let cache_clone = cache.clone();
        let url_clone = url.to_string();
        
        let handle = tokio::spawn(async move {
            let range = ByteRange::new(i * 1024, (i + 1) * 1024 - 1).unwrap();
            let data = Bytes::from(vec![i as u8; 1024]);
            
            // Store
            cache_clone.store_slice(&url_clone, &range, data.clone()).await.unwrap();
            
            // Lookup
            let cached = cache_clone.lookup_slice(&url_clone, &range).await.unwrap();
            assert!(cached.is_some());
            assert_eq!(cached.unwrap(), data);
        });
        
        handles.push(handle);
    }
    
    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }
}

#[tokio::test]
async fn test_cache_error_handling() {
    let cache = SliceCache::new(Duration::from_secs(3600));
    let url = "http://example.com/file.bin";
    let range = ByteRange::new(0, 1023).unwrap();
    
    // Lookup non-existent slice should return None, not error
    let result = cache.lookup_slice(url, &range).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
    
    // Store should always succeed (errors are logged but not returned)
    let data = Bytes::from(vec![1; 1024]);
    let result = cache.store_slice(url, &range, data).await;
    assert!(result.is_ok());
}
