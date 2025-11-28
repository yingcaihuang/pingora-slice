// Feature: pingora-slice, Property 14: 部分缓存命中优化
// **Validates: Requirements 7.4**
//
// Property: For any request where some slices are cached and some are not,
// only the non-cached slices should result in subrequests to the origin

use bytes::Bytes;
use pingora_slice::{ByteRange, SliceCache, SliceCalculator};
use proptest::prelude::*;
use std::collections::HashSet;
use std::time::Duration;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property 14: Partial cache hit optimization - only fetch missing slices
    /// 
    /// For any file with multiple slices where some are cached and some are not,
    /// looking up all slices should return only the cached ones, indicating that
    /// only the non-cached slices need to be fetched from origin.
    #[test]
    fn prop_partial_cache_only_fetches_missing(
        url in "[a-z]{3,10}\\.[a-z]{3}",
        file_size in 1024u64..=10_000_000u64,
        slice_size in 512usize..=2048usize,
        cache_ratio in 0.1f64..=0.9f64, // Percentage of slices to cache
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let calculator = SliceCalculator::new(slice_size);
            let slices = calculator.calculate_slices(file_size, None)
                .expect("Should calculate slices");
            
            // Skip if we have too few slices
            prop_assume!(slices.len() >= 3);
            
            let cache = SliceCache::new(Duration::from_secs(3600));
            let full_url = format!("http://{}/file.bin", url);
            
            // Determine which slices to cache
            let num_to_cache = ((slices.len() as f64) * cache_ratio).ceil() as usize;
            let num_to_cache = num_to_cache.max(1).min(slices.len() - 1); // At least 1, at most len-1
            
            // Cache some slices (not all)
            let mut cached_indices = HashSet::new();
            for i in 0..num_to_cache {
                let slice = &slices[i];
                let data = Bytes::from(vec![(i % 256) as u8; slice.range.size() as usize]);
                
                cache.store_slice(&full_url, &slice.range, data)
                    .await
                    .expect("Store should succeed");
                
                cached_indices.insert(i);
            }
            
            // Collect all ranges
            let all_ranges: Vec<ByteRange> = slices.iter().map(|s| s.range).collect();
            
            // Lookup all slices
            let cached_slices = cache.lookup_multiple(&full_url, &all_ranges).await;
            
            // Verify: only the cached slices should be returned
            prop_assert_eq!(
                cached_slices.len(),
                num_to_cache,
                "Should return exactly {} cached slices, got {}",
                num_to_cache,
                cached_slices.len()
            );
            
            // Verify: all returned indices should be in our cached set
            for idx in cached_slices.keys() {
                prop_assert!(
                    cached_indices.contains(idx),
                    "Index {} should be in cached set",
                    idx
                );
            }
            
            // Verify: all cached indices should be returned
            for idx in &cached_indices {
                prop_assert!(
                    cached_slices.contains_key(idx),
                    "Cached index {} should be in results",
                    idx
                );
            }
            
            // Calculate which slices would need to be fetched
            let slices_to_fetch: Vec<usize> = (0..slices.len())
                .filter(|i| !cached_slices.contains_key(i))
                .collect();
            
            // Verify: the number of slices to fetch is correct
            let expected_to_fetch = slices.len() - num_to_cache;
            prop_assert_eq!(
                slices_to_fetch.len(),
                expected_to_fetch,
                "Should need to fetch {} slices, got {}",
                expected_to_fetch,
                slices_to_fetch.len()
            );
            
            Ok(())
        })?;
    }

    /// Property 14: Partial cache hit - no redundant fetches
    /// 
    /// For any set of slices where some are cached, the set of slices
    /// that need to be fetched should be exactly the complement of the cached set.
    #[test]
    fn prop_partial_cache_complement_set(
        url in "[a-z]{3,10}\\.[a-z]{3}",
        num_slices in 3usize..=20usize,
        cached_indices in prop::collection::hash_set(0usize..20usize, 1..10),
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            // Filter cached indices to be within range
            let cached_indices: HashSet<usize> = cached_indices
                .into_iter()
                .filter(|&i| i < num_slices)
                .collect();
            
            // Skip if no indices or all indices are cached
            prop_assume!(!cached_indices.is_empty());
            prop_assume!(cached_indices.len() < num_slices);
            
            let cache = SliceCache::new(Duration::from_secs(3600));
            let full_url = format!("http://{}/file.bin", url);
            
            // Create slices
            let mut slices = Vec::new();
            for i in 0..num_slices {
                let start = (i as u64) * 1024;
                let end = start + 1023;
                let range = ByteRange::new(start, end).unwrap();
                slices.push(range);
            }
            
            // Cache the specified slices
            for &idx in &cached_indices {
                let data = Bytes::from(vec![(idx % 256) as u8; 1024]);
                cache.store_slice(&full_url, &slices[idx], data)
                    .await
                    .expect("Store should succeed");
            }
            
            // Lookup all slices
            let cached_slices = cache.lookup_multiple(&full_url, &slices).await;
            
            // Calculate the complement (slices to fetch)
            let mut slices_to_fetch = HashSet::new();
            for i in 0..num_slices {
                if !cached_slices.contains_key(&i) {
                    slices_to_fetch.insert(i);
                }
            }
            
            // Verify: cached + to_fetch = all slices
            prop_assert_eq!(
                cached_slices.len() + slices_to_fetch.len(),
                num_slices,
                "Cached + to_fetch should equal total slices"
            );
            
            // Verify: no overlap between cached and to_fetch
            for idx in cached_slices.keys() {
                prop_assert!(
                    !slices_to_fetch.contains(idx),
                    "Index {} should not be in both cached and to_fetch",
                    idx
                );
            }
            
            // Verify: union of cached and to_fetch covers all indices
            let mut all_indices: HashSet<usize> = HashSet::new();
            all_indices.extend(cached_slices.keys());
            all_indices.extend(&slices_to_fetch);
            
            prop_assert_eq!(
                all_indices.len(),
                num_slices,
                "Union should cover all {} slices",
                num_slices
            );
            
            Ok(())
        })?;
    }

    /// Property 14: Partial cache hit - data integrity for cached slices
    /// 
    /// For any partially cached file, the cached slices should return
    /// correct data while non-cached slices return None.
    #[test]
    fn prop_partial_cache_data_integrity(
        url in "[a-z]{3,10}\\.[a-z]{3}",
        file_size in 5000u64..=50_000u64,
        slice_size in 1000usize..=2000usize,
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let calculator = SliceCalculator::new(slice_size);
            let slices = calculator.calculate_slices(file_size, None)
                .expect("Should calculate slices");
            
            prop_assume!(slices.len() >= 3);
            
            let cache = SliceCache::new(Duration::from_secs(3600));
            let full_url = format!("http://{}/file.bin", url);
            
            // Cache every other slice with unique data
            let mut expected_data = std::collections::HashMap::new();
            for (i, slice) in slices.iter().enumerate() {
                if i % 2 == 0 {
                    // Create unique data for this slice
                    let data = Bytes::from(vec![(i % 256) as u8; slice.range.size() as usize]);
                    expected_data.insert(i, data.clone());
                    
                    cache.store_slice(&full_url, &slice.range, data)
                        .await
                        .expect("Store should succeed");
                }
            }
            
            // Lookup each slice individually
            for (i, slice) in slices.iter().enumerate() {
                let result = cache.lookup_slice(&full_url, &slice.range).await
                    .expect("Lookup should not error");
                
                if i % 2 == 0 {
                    // Should be cached
                    prop_assert!(
                        result.is_some(),
                        "Slice {} should be cached",
                        i
                    );
                    
                    let data = result.unwrap();
                    let expected = expected_data.get(&i).unwrap();
                    prop_assert_eq!(
                        &data,
                        expected,
                        "Cached slice {} should have correct data",
                        i
                    );
                } else {
                    // Should not be cached
                    prop_assert!(
                        result.is_none(),
                        "Slice {} should not be cached",
                        i
                    );
                }
            }
            
            Ok(())
        })?;
    }

    /// Property 14: Partial cache hit - batch lookup efficiency
    /// 
    /// For any file with partial cache hits, batch lookup should be
    /// equivalent to individual lookups but more efficient.
    #[test]
    fn prop_partial_cache_batch_lookup_correctness(
        url in "[a-z]{3,10}\\.[a-z]{3}",
        num_slices in 5usize..=15usize,
        cache_pattern in prop::collection::vec(any::<bool>(), 5..=15),
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let cache_pattern: Vec<bool> = cache_pattern
                .into_iter()
                .take(num_slices)
                .collect();
            
            prop_assume!(cache_pattern.len() == num_slices);
            
            // Ensure at least one cached and one not cached
            let num_cached = cache_pattern.iter().filter(|&&b| b).count();
            prop_assume!(num_cached > 0 && num_cached < num_slices);
            
            let cache = SliceCache::new(Duration::from_secs(3600));
            let full_url = format!("http://{}/file.bin", url);
            
            // Create slices and cache according to pattern
            let mut slices = Vec::new();
            let mut expected_cached = std::collections::HashMap::new();
            
            for i in 0..num_slices {
                let start = (i as u64) * 1024;
                let end = start + 1023;
                let range = ByteRange::new(start, end).unwrap();
                slices.push(range);
                
                if cache_pattern[i] {
                    let data = Bytes::from(vec![(i % 256) as u8; 1024]);
                    expected_cached.insert(i, data.clone());
                    
                    cache.store_slice(&full_url, &range, data)
                        .await
                        .expect("Store should succeed");
                }
            }
            
            // Batch lookup
            let batch_results = cache.lookup_multiple(&full_url, &slices).await;
            
            // Individual lookups
            let mut individual_results = std::collections::HashMap::new();
            for (i, range) in slices.iter().enumerate() {
                if let Ok(Some(data)) = cache.lookup_slice(&full_url, range).await {
                    individual_results.insert(i, data);
                }
            }
            
            // Verify batch and individual results match
            prop_assert_eq!(
                batch_results.len(),
                individual_results.len(),
                "Batch and individual lookup should return same number of results"
            );
            
            for (idx, batch_data) in &batch_results {
                prop_assert!(
                    individual_results.contains_key(idx),
                    "Individual lookup should contain index {}",
                    idx
                );
                
                let individual_data = individual_results.get(idx).unwrap();
                prop_assert_eq!(
                    batch_data,
                    individual_data,
                    "Data for index {} should match between batch and individual",
                    idx
                );
            }
            
            // Verify results match expected
            prop_assert_eq!(
                batch_results.len(),
                expected_cached.len(),
                "Should return exactly the cached slices"
            );
            
            for (idx, expected_data) in &expected_cached {
                prop_assert!(
                    batch_results.contains_key(idx),
                    "Should contain cached index {}",
                    idx
                );
                
                let actual_data = batch_results.get(idx).unwrap();
                prop_assert_eq!(
                    actual_data,
                    expected_data,
                    "Data for index {} should match expected",
                    idx
                );
            }
            
            Ok(())
        })?;
    }

    /// Property 14: Partial cache hit - optimization verification
    /// 
    /// For any file request, the number of slices that need to be fetched
    /// should be exactly (total_slices - cached_slices).
    #[test]
    fn prop_partial_cache_fetch_count(
        url in "[a-z]{3,10}\\.[a-z]{3}",
        file_size in 10_000u64..=100_000u64,
        slice_size in 1000usize..=5000usize,
        cache_percentage in 10u32..=90u32,
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let calculator = SliceCalculator::new(slice_size);
            let slices = calculator.calculate_slices(file_size, None)
                .expect("Should calculate slices");
            
            prop_assume!(slices.len() >= 5);
            
            let cache = SliceCache::new(Duration::from_secs(3600));
            let full_url = format!("http://{}/file.bin", url);
            
            // Cache a percentage of slices
            let num_to_cache = ((slices.len() as u32 * cache_percentage) / 100) as usize;
            let num_to_cache = num_to_cache.max(1).min(slices.len() - 1);
            
            // Cache first N slices
            for i in 0..num_to_cache {
                let slice = &slices[i];
                let data = Bytes::from(vec![i as u8; slice.range.size() as usize]);
                
                cache.store_slice(&full_url, &slice.range, data)
                    .await
                    .expect("Store should succeed");
            }
            
            // Lookup all slices
            let all_ranges: Vec<ByteRange> = slices.iter().map(|s| s.range).collect();
            let cached_slices = cache.lookup_multiple(&full_url, &all_ranges).await;
            
            // Calculate slices to fetch
            let total_slices = slices.len();
            let cached_count = cached_slices.len();
            let to_fetch_count = total_slices - cached_count;
            
            // Verify the optimization: only fetch what's not cached
            prop_assert_eq!(
                cached_count,
                num_to_cache,
                "Should have {} cached slices",
                num_to_cache
            );
            
            prop_assert_eq!(
                to_fetch_count,
                total_slices - num_to_cache,
                "Should need to fetch {} slices",
                total_slices - num_to_cache
            );
            
            // Verify: cached + to_fetch = total
            prop_assert_eq!(
                cached_count + to_fetch_count,
                total_slices,
                "Cached + to_fetch should equal total"
            );
            
            // Verify: we're not fetching cached slices
            for idx in cached_slices.keys() {
                prop_assert!(
                    *idx < num_to_cache,
                    "Cached index {} should be in the cached range",
                    idx
                );
            }
            
            Ok(())
        })?;
    }

    /// Property 14: Partial cache hit - sparse cache pattern
    /// 
    /// For any file with a sparse cache pattern (random slices cached),
    /// only the non-cached slices should need to be fetched.
    #[test]
    fn prop_partial_cache_sparse_pattern(
        url in "[a-z]{3,10}\\.[a-z]{3}",
        num_slices in 10usize..=20usize,
        seed in any::<u64>(),
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            use std::collections::hash_map::RandomState;
            use std::hash::{BuildHasher, Hash, Hasher};
            
            let cache = SliceCache::new(Duration::from_secs(3600));
            let full_url = format!("http://{}/file.bin", url);
            
            // Create slices
            let mut slices = Vec::new();
            for i in 0..num_slices {
                let start = (i as u64) * 1024;
                let end = start + 1023;
                let range = ByteRange::new(start, end).unwrap();
                slices.push(range);
            }
            
            // Create a sparse cache pattern using the seed
            let mut cached_indices = HashSet::new();
            let build_hasher = RandomState::new();
            
            for i in 0..num_slices {
                let mut hasher = build_hasher.build_hasher();
                seed.hash(&mut hasher);
                i.hash(&mut hasher);
                let hash = hasher.finish();
                
                // Cache approximately 50% of slices in a deterministic but sparse pattern
                if hash % 2 == 0 {
                    cached_indices.insert(i);
                }
            }
            
            // Ensure we have at least one cached and one not cached
            prop_assume!(!cached_indices.is_empty());
            prop_assume!(cached_indices.len() < num_slices);
            
            // Cache the selected slices
            for &idx in &cached_indices {
                let data = Bytes::from(vec![(idx % 256) as u8; 1024]);
                cache.store_slice(&full_url, &slices[idx], data)
                    .await
                    .expect("Store should succeed");
            }
            
            // Lookup all slices
            let cached_slices = cache.lookup_multiple(&full_url, &slices).await;
            
            // Verify: only cached indices are returned
            prop_assert_eq!(
                cached_slices.len(),
                cached_indices.len(),
                "Should return exactly the cached slices"
            );
            
            for idx in cached_slices.keys() {
                prop_assert!(
                    cached_indices.contains(idx),
                    "Returned index {} should be in cached set",
                    idx
                );
            }
            
            // Calculate slices to fetch
            let slices_to_fetch: HashSet<usize> = (0..num_slices)
                .filter(|i| !cached_slices.contains_key(i))
                .collect();
            
            // Verify: to_fetch is the complement of cached
            prop_assert_eq!(
                slices_to_fetch.len(),
                num_slices - cached_indices.len(),
                "Should need to fetch the non-cached slices"
            );
            
            // Verify: no overlap
            for idx in &slices_to_fetch {
                prop_assert!(
                    !cached_indices.contains(idx),
                    "To-fetch index {} should not be cached",
                    idx
                );
            }
            
            Ok(())
        })?;
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[tokio::test]
    async fn test_partial_cache_simple() {
        let cache = SliceCache::new(Duration::from_secs(3600));
        let url = "http://example.com/file.bin";
        
        // Create 5 slices
        let mut ranges = Vec::new();
        for i in 0..5 {
            let start = (i as u64) * 1024;
            let end = start + 1023;
            ranges.push(ByteRange::new(start, end).unwrap());
        }
        
        // Cache slices 0, 2, 4
        for &i in &[0, 2, 4] {
            let data = Bytes::from(vec![i as u8; 1024]);
            cache.store_slice(url, &ranges[i], data).await.unwrap();
        }
        
        // Lookup all
        let cached = cache.lookup_multiple(url, &ranges).await;
        
        // Should have 3 cached
        assert_eq!(cached.len(), 3);
        assert!(cached.contains_key(&0));
        assert!(cached.contains_key(&2));
        assert!(cached.contains_key(&4));
        
        // Should not have 1, 3
        assert!(!cached.contains_key(&1));
        assert!(!cached.contains_key(&3));
    }

    #[tokio::test]
    async fn test_partial_cache_all_cached() {
        let cache = SliceCache::new(Duration::from_secs(3600));
        let url = "http://example.com/file.bin";
        
        let mut ranges = Vec::new();
        for i in 0..3 {
            let start = (i as u64) * 1024;
            let end = start + 1023;
            ranges.push(ByteRange::new(start, end).unwrap());
        }
        
        // Cache all slices
        for i in 0..3 {
            let data = Bytes::from(vec![i as u8; 1024]);
            cache.store_slice(url, &ranges[i], data).await.unwrap();
        }
        
        // Lookup all
        let cached = cache.lookup_multiple(url, &ranges).await;
        
        // Should have all 3
        assert_eq!(cached.len(), 3);
    }

    #[tokio::test]
    async fn test_partial_cache_none_cached() {
        let cache = SliceCache::new(Duration::from_secs(3600));
        let url = "http://example.com/file.bin";
        
        let mut ranges = Vec::new();
        for i in 0..3 {
            let start = (i as u64) * 1024;
            let end = start + 1023;
            ranges.push(ByteRange::new(start, end).unwrap());
        }
        
        // Don't cache anything
        
        // Lookup all
        let cached = cache.lookup_multiple(url, &ranges).await;
        
        // Should have none
        assert_eq!(cached.len(), 0);
    }

    #[tokio::test]
    async fn test_partial_cache_data_correctness() {
        let cache = SliceCache::new(Duration::from_secs(3600));
        let url = "http://example.com/file.bin";
        
        let mut ranges = Vec::new();
        let mut expected_data = std::collections::HashMap::new();
        
        for i in 0..5 {
            let start = (i as u64) * 1024;
            let end = start + 1023;
            ranges.push(ByteRange::new(start, end).unwrap());
            
            // Cache odd indices
            if i % 2 == 1 {
                let data = Bytes::from(vec![i as u8; 1024]);
                expected_data.insert(i, data.clone());
                cache.store_slice(url, &ranges[i], data).await.unwrap();
            }
        }
        
        // Lookup all
        let cached = cache.lookup_multiple(url, &ranges).await;
        
        // Verify count
        assert_eq!(cached.len(), 2); // indices 1 and 3
        
        // Verify data
        for (idx, data) in &cached {
            let expected = expected_data.get(idx).unwrap();
            assert_eq!(data, expected);
        }
    }

    #[tokio::test]
    async fn test_partial_cache_optimization_count() {
        let cache = SliceCache::new(Duration::from_secs(3600));
        let url = "http://example.com/file.bin";
        
        let total_slices = 10;
        let mut ranges = Vec::new();
        
        for i in 0..total_slices {
            let start = (i as u64) * 1024;
            let end = start + 1023;
            ranges.push(ByteRange::new(start, end).unwrap());
        }
        
        // Cache 3 slices
        let num_cached = 3;
        for i in 0..num_cached {
            let data = Bytes::from(vec![i as u8; 1024]);
            cache.store_slice(url, &ranges[i], data).await.unwrap();
        }
        
        // Lookup all
        let cached = cache.lookup_multiple(url, &ranges).await;
        
        // Verify optimization
        assert_eq!(cached.len(), num_cached);
        
        let to_fetch = total_slices - cached.len();
        assert_eq!(to_fetch, total_slices - num_cached);
        assert_eq!(cached.len() + to_fetch, total_slices);
    }
}
