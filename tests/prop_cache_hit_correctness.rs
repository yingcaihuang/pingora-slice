// Feature: pingora-slice, Property 13: 缓存命中正确性
// **Validates: Requirements 7.4**
//
// Property: For any cached slice, when retrieved from cache,
// the data should be identical to the data originally stored

use bytes::Bytes;
use pingora_slice::{ByteRange, SliceCache};
use proptest::prelude::*;
use std::time::Duration;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property 13: Cache hit correctness - round trip
    /// 
    /// For any URL, byte range, and data, storing the data in cache
    /// and then retrieving it should return the exact same data.
    #[test]
    fn prop_cache_round_trip(
        url in "[a-z]{3,10}\\.[a-z]{3}",
        start in 0u64..=10_000_000u64,
        end in 0u64..=10_000_000u64,
        data in prop::collection::vec(any::<u8>(), 1..10000),
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let (start, end) = if start <= end {
                (start, end)
            } else {
                (end, start)
            };
            
            let range = ByteRange::new(start, end)
                .expect("Range should be valid");
            
            let cache = SliceCache::new(Duration::from_secs(3600));
            let full_url = format!("http://{}/file.bin", url);
            let original_data = Bytes::from(data);
            
            // Store the data
            cache.store_slice(&full_url, &range, original_data.clone())
                .await
                .expect("Store should succeed");
            
            // Retrieve the data
            let retrieved = cache.lookup_slice(&full_url, &range)
                .await
                .expect("Lookup should succeed")
                .expect("Data should be in cache");
            
            // Verify data is identical
            prop_assert_eq!(
                retrieved,
                original_data,
                "Retrieved data should be identical to stored data"
            );
            
            Ok(())
        })?;
    }

    /// Property 13: Cache hit correctness - multiple slices
    /// 
    /// For any set of slices stored in cache, each slice should
    /// be retrievable with its exact original data.
    #[test]
    fn prop_cache_multiple_slices_correctness(
        url in "[a-z]{3,10}\\.[a-z]{3}",
        slices in prop::collection::vec(
            (0u64..=1_000_000u64, 0u64..=1_000_000u64, prop::collection::vec(any::<u8>(), 1..1000)),
            1..10
        ),
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let cache = SliceCache::new(Duration::from_secs(3600));
            let full_url = format!("http://{}/file.bin", url);
            
            // Store all slices
            let mut stored_slices = Vec::new();
            for (start, end, data) in slices {
                let (start, end) = if start <= end {
                    (start, end)
                } else {
                    (end, start)
                };
                
                let range = ByteRange::new(start, end)
                    .expect("Range should be valid");
                let bytes_data = Bytes::from(data);
                
                cache.store_slice(&full_url, &range, bytes_data.clone())
                    .await
                    .expect("Store should succeed");
                
                stored_slices.push((range, bytes_data));
            }
            
            // Retrieve and verify each slice
            for (range, original_data) in stored_slices {
                let retrieved = cache.lookup_slice(&full_url, &range)
                    .await
                    .expect("Lookup should succeed")
                    .expect("Data should be in cache");
                
                prop_assert_eq!(
                    retrieved,
                    original_data,
                    "Retrieved data for range {}-{} should match stored data",
                    range.start,
                    range.end
                );
            }
            
            Ok(())
        })?;
    }

    /// Property 13: Cache hit correctness - data integrity
    /// 
    /// For any data stored in cache, the byte-by-byte content
    /// should be preserved exactly.
    #[test]
    fn prop_cache_data_integrity(
        url in "[a-z]{3,10}\\.[a-z]{3}",
        start in 0u64..=1_000_000u64,
        end in 0u64..=1_000_000u64,
        data_size in 1usize..10000,
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let (start, end) = if start <= end {
                (start, end)
            } else {
                (end, start)
            };
            
            let range = ByteRange::new(start, end)
                .expect("Range should be valid");
            
            let cache = SliceCache::new(Duration::from_secs(3600));
            let full_url = format!("http://{}/file.bin", url);
            
            // Create data with specific pattern for verification
            let mut data = Vec::with_capacity(data_size);
            for i in 0..data_size {
                data.push((i % 256) as u8);
            }
            let original_data = Bytes::from(data);
            
            // Store the data
            cache.store_slice(&full_url, &range, original_data.clone())
                .await
                .expect("Store should succeed");
            
            // Retrieve the data
            let retrieved = cache.lookup_slice(&full_url, &range)
                .await
                .expect("Lookup should succeed")
                .expect("Data should be in cache");
            
            // Verify length
            prop_assert_eq!(
                retrieved.len(),
                original_data.len(),
                "Retrieved data length should match original"
            );
            
            // Verify byte-by-byte
            for (i, (original_byte, retrieved_byte)) in 
                original_data.iter().zip(retrieved.iter()).enumerate() 
            {
                prop_assert_eq!(
                    retrieved_byte,
                    original_byte,
                    "Byte at position {} should match: expected {}, got {}",
                    i,
                    original_byte,
                    retrieved_byte
                );
            }
            
            Ok(())
        })?;
    }

    /// Property 13: Cache hit correctness - empty data
    /// 
    /// Even empty data should be stored and retrieved correctly.
    #[test]
    fn prop_cache_empty_data(
        url in "[a-z]{3,10}\\.[a-z]{3}",
        start in 0u64..=1_000_000u64,
        end in 0u64..=1_000_000u64,
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let (start, end) = if start <= end {
                (start, end)
            } else {
                (end, start)
            };
            
            let range = ByteRange::new(start, end)
                .expect("Range should be valid");
            
            let cache = SliceCache::new(Duration::from_secs(3600));
            let full_url = format!("http://{}/file.bin", url);
            let empty_data = Bytes::new();
            
            // Store empty data
            cache.store_slice(&full_url, &range, empty_data.clone())
                .await
                .expect("Store should succeed");
            
            // Retrieve the data
            let retrieved = cache.lookup_slice(&full_url, &range)
                .await
                .expect("Lookup should succeed")
                .expect("Data should be in cache");
            
            // Verify it's still empty
            prop_assert_eq!(
                retrieved.len(),
                0,
                "Retrieved empty data should still be empty"
            );
            prop_assert_eq!(
                retrieved,
                empty_data,
                "Retrieved empty data should match stored empty data"
            );
            
            Ok(())
        })?;
    }

    /// Property 13: Cache hit correctness - overwrite behavior
    /// 
    /// When the same slice is stored twice with different data,
    /// the latest data should be retrieved.
    #[test]
    fn prop_cache_overwrite_correctness(
        url in "[a-z]{3,10}\\.[a-z]{3}",
        start in 0u64..=1_000_000u64,
        end in 0u64..=1_000_000u64,
        data1 in prop::collection::vec(any::<u8>(), 1..1000),
        data2 in prop::collection::vec(any::<u8>(), 1..1000),
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            // Skip if data is the same
            prop_assume!(data1 != data2);
            
            let (start, end) = if start <= end {
                (start, end)
            } else {
                (end, start)
            };
            
            let range = ByteRange::new(start, end)
                .expect("Range should be valid");
            
            let cache = SliceCache::new(Duration::from_secs(3600));
            let full_url = format!("http://{}/file.bin", url);
            let first_data = Bytes::from(data1);
            let second_data = Bytes::from(data2);
            
            // Store first data
            cache.store_slice(&full_url, &range, first_data.clone())
                .await
                .expect("First store should succeed");
            
            // Store second data (overwrite)
            cache.store_slice(&full_url, &range, second_data.clone())
                .await
                .expect("Second store should succeed");
            
            // Retrieve the data
            let retrieved = cache.lookup_slice(&full_url, &range)
                .await
                .expect("Lookup should succeed")
                .expect("Data should be in cache");
            
            // Should get the second (latest) data
            prop_assert_eq!(
                &retrieved,
                &second_data,
                "Retrieved data should be the latest stored data"
            );
            
            prop_assert_ne!(
                &retrieved,
                &first_data,
                "Retrieved data should not be the old data"
            );
            
            Ok(())
        })?;
    }

    /// Property 13: Cache hit correctness - concurrent access
    /// 
    /// When multiple slices are stored and retrieved concurrently,
    /// each should maintain data integrity.
    #[test]
    fn prop_cache_concurrent_correctness(
        url in "[a-z]{3,10}\\.[a-z]{3}",
        slices in prop::collection::vec(
            (0u64..=100_000u64, prop::collection::vec(any::<u8>(), 100..1000)),
            2..5
        ),
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            use std::sync::Arc;
            
            let cache = Arc::new(SliceCache::new(Duration::from_secs(3600)));
            let full_url = format!("http://{}/file.bin", url);
            
            // Create non-overlapping ranges
            let mut stored_slices = Vec::new();
            for (i, (start, data)) in slices.into_iter().enumerate() {
                let range_start = start + (i as u64 * 100_000);
                let range_end = range_start + data.len() as u64 - 1;
                
                let range = ByteRange::new(range_start, range_end)
                    .expect("Range should be valid");
                let bytes_data = Bytes::from(data);
                
                stored_slices.push((range, bytes_data));
            }
            
            // Store all slices concurrently
            let mut store_handles = Vec::new();
            for (range, data) in stored_slices.clone() {
                let cache_clone = cache.clone();
                let url_clone = full_url.clone();
                
                let handle = tokio::spawn(async move {
                    cache_clone.store_slice(&url_clone, &range, data)
                        .await
                        .expect("Store should succeed");
                });
                
                store_handles.push(handle);
            }
            
            // Wait for all stores to complete
            for handle in store_handles {
                handle.await.expect("Store task should complete");
            }
            
            // Retrieve all slices concurrently
            let mut retrieve_handles = Vec::new();
            for (range, expected_data) in stored_slices {
                let cache_clone = cache.clone();
                let url_clone = full_url.clone();
                
                let handle = tokio::spawn(async move {
                    let retrieved = cache_clone.lookup_slice(&url_clone, &range)
                        .await
                        .expect("Lookup should succeed")
                        .expect("Data should be in cache");
                    
                    (range, retrieved, expected_data)
                });
                
                retrieve_handles.push(handle);
            }
            
            // Verify all retrieved data
            for handle in retrieve_handles {
                let (range, retrieved, expected) = handle.await.expect("Retrieve task should complete");
                
                prop_assert_eq!(
                    retrieved,
                    expected,
                    "Retrieved data for range {}-{} should match stored data",
                    range.start,
                    range.end
                );
            }
            
            Ok(())
        })?;
    }

    /// Property 13: Cache hit correctness - large data
    /// 
    /// Large data chunks should be stored and retrieved correctly.
    #[test]
    fn prop_cache_large_data_correctness(
        url in "[a-z]{3,10}\\.[a-z]{3}",
        start in 0u64..=1_000_000u64,
        end in 0u64..=1_000_000u64,
        pattern in any::<u8>(),
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let (start, end) = if start <= end {
                (start, end)
            } else {
                (end, start)
            };
            
            let range = ByteRange::new(start, end)
                .expect("Range should be valid");
            
            let cache = SliceCache::new(Duration::from_secs(3600));
            let full_url = format!("http://{}/file.bin", url);
            
            // Create large data (1MB)
            let large_data = Bytes::from(vec![pattern; 1024 * 1024]);
            
            // Store the large data
            cache.store_slice(&full_url, &range, large_data.clone())
                .await
                .expect("Store should succeed");
            
            // Retrieve the data
            let retrieved = cache.lookup_slice(&full_url, &range)
                .await
                .expect("Lookup should succeed")
                .expect("Data should be in cache");
            
            // Verify size and content
            prop_assert_eq!(
                retrieved.len(),
                large_data.len(),
                "Retrieved large data should have same size"
            );
            
            prop_assert_eq!(
                retrieved,
                large_data,
                "Retrieved large data should be identical to stored data"
            );
            
            Ok(())
        })?;
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[tokio::test]
    async fn test_simple_round_trip() {
        let cache = SliceCache::new(Duration::from_secs(3600));
        let url = "http://example.com/file.bin";
        let range = ByteRange::new(0, 1023).unwrap();
        let data = Bytes::from(vec![1, 2, 3, 4, 5]);
        
        // Store
        cache.store_slice(url, &range, data.clone()).await.unwrap();
        
        // Retrieve
        let retrieved = cache.lookup_slice(url, &range).await.unwrap().unwrap();
        
        // Verify
        assert_eq!(retrieved, data);
    }

    #[tokio::test]
    async fn test_multiple_slices_independence() {
        let cache = SliceCache::new(Duration::from_secs(3600));
        let url = "http://example.com/file.bin";
        
        let range1 = ByteRange::new(0, 1023).unwrap();
        let range2 = ByteRange::new(1024, 2047).unwrap();
        let data1 = Bytes::from(vec![1; 1024]);
        let data2 = Bytes::from(vec![2; 1024]);
        
        // Store both
        cache.store_slice(url, &range1, data1.clone()).await.unwrap();
        cache.store_slice(url, &range2, data2.clone()).await.unwrap();
        
        // Retrieve both
        let retrieved1 = cache.lookup_slice(url, &range1).await.unwrap().unwrap();
        let retrieved2 = cache.lookup_slice(url, &range2).await.unwrap().unwrap();
        
        // Verify independence
        assert_eq!(retrieved1, data1);
        assert_eq!(retrieved2, data2);
        assert_ne!(retrieved1, retrieved2);
    }

    #[tokio::test]
    async fn test_overwrite_updates_data() {
        let cache = SliceCache::new(Duration::from_secs(3600));
        let url = "http://example.com/file.bin";
        let range = ByteRange::new(0, 1023).unwrap();
        
        let data1 = Bytes::from(vec![1; 1024]);
        let data2 = Bytes::from(vec![2; 1024]);
        
        // Store first
        cache.store_slice(url, &range, data1.clone()).await.unwrap();
        
        // Verify first
        let retrieved1 = cache.lookup_slice(url, &range).await.unwrap().unwrap();
        assert_eq!(retrieved1, data1);
        
        // Overwrite
        cache.store_slice(url, &range, data2.clone()).await.unwrap();
        
        // Verify second
        let retrieved2 = cache.lookup_slice(url, &range).await.unwrap().unwrap();
        assert_eq!(retrieved2, data2);
        assert_ne!(retrieved2, data1);
    }

    #[tokio::test]
    async fn test_empty_data_handling() {
        let cache = SliceCache::new(Duration::from_secs(3600));
        let url = "http://example.com/file.bin";
        let range = ByteRange::new(0, 0).unwrap();
        let empty = Bytes::new();
        
        // Store empty
        cache.store_slice(url, &range, empty.clone()).await.unwrap();
        
        // Retrieve
        let retrieved = cache.lookup_slice(url, &range).await.unwrap().unwrap();
        
        // Verify
        assert_eq!(retrieved.len(), 0);
        assert_eq!(retrieved, empty);
    }

    #[tokio::test]
    async fn test_binary_data_integrity() {
        let cache = SliceCache::new(Duration::from_secs(3600));
        let url = "http://example.com/file.bin";
        let range = ByteRange::new(0, 255).unwrap();
        
        // Create data with all byte values
        let data: Vec<u8> = (0..=255).collect();
        let bytes_data = Bytes::from(data);
        
        // Store
        cache.store_slice(url, &range, bytes_data.clone()).await.unwrap();
        
        // Retrieve
        let retrieved = cache.lookup_slice(url, &range).await.unwrap().unwrap();
        
        // Verify all bytes
        assert_eq!(retrieved.len(), 256);
        for (i, byte) in retrieved.iter().enumerate() {
            assert_eq!(*byte, i as u8, "Byte at position {} should be {}", i, i);
        }
    }
}
