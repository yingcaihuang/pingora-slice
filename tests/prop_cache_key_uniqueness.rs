// Feature: pingora-slice, Property 12: 缓存键唯一性
// **Validates: Requirements 7.2**
//
// Property: For any two different slice specifications (different URL or different byte range),
// their cache keys should be different

use pingora_slice::{ByteRange, SliceCache};
use proptest::prelude::*;
use std::time::Duration;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property 12: Cache key uniqueness for different URLs
    /// 
    /// For any two different URLs with the same byte range,
    /// the generated cache keys should be different.
    #[test]
    fn prop_cache_key_unique_urls(
        url1 in "[a-z]{3,10}\\.[a-z]{3}",
        url2 in "[a-z]{3,10}\\.[a-z]{3}",
        start in 0u64..=1_000_000u64,
        end in 0u64..=1_000_000u64,
    ) {
        // Skip if URLs are the same
        prop_assume!(url1 != url2);
        
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        
        let range = ByteRange::new(start, end)
            .expect("Range should be valid");
        
        let cache = SliceCache::new(Duration::from_secs(3600));
        
        let full_url1 = format!("http://{}/file.bin", url1);
        let full_url2 = format!("http://{}/file.bin", url2);
        
        let key1 = cache.generate_cache_key(&full_url1, &range);
        let key2 = cache.generate_cache_key(&full_url2, &range);
        
        prop_assert_ne!(
            key1,
            key2,
            "Cache keys should be different for different URLs: '{}' vs '{}'",
            full_url1,
            full_url2
        );
    }

    /// Property 12: Cache key uniqueness for different ranges
    /// 
    /// For any single URL with two different byte ranges,
    /// the generated cache keys should be different.
    #[test]
    fn prop_cache_key_unique_ranges(
        url in "[a-z]{3,10}\\.[a-z]{3}",
        start1 in 0u64..=1_000_000u64,
        end1 in 0u64..=1_000_000u64,
        start2 in 0u64..=1_000_000u64,
        end2 in 0u64..=1_000_000u64,
    ) {
        let (start1, end1) = if start1 <= end1 {
            (start1, end1)
        } else {
            (end1, start1)
        };
        
        let (start2, end2) = if start2 <= end2 {
            (start2, end2)
        } else {
            (end2, start2)
        };
        
        // Skip if ranges are the same
        prop_assume!(start1 != start2 || end1 != end2);
        
        let range1 = ByteRange::new(start1, end1)
            .expect("Range 1 should be valid");
        let range2 = ByteRange::new(start2, end2)
            .expect("Range 2 should be valid");
        
        let cache = SliceCache::new(Duration::from_secs(3600));
        let full_url = format!("http://{}/file.bin", url);
        
        let key1 = cache.generate_cache_key(&full_url, &range1);
        let key2 = cache.generate_cache_key(&full_url, &range2);
        
        prop_assert_ne!(
            key1,
            key2,
            "Cache keys should be different for different ranges: {}-{} vs {}-{}",
            start1, end1, start2, end2
        );
    }

    /// Property 12: Cache key uniqueness for different URLs and ranges
    /// 
    /// For any two different combinations of URL and byte range,
    /// the generated cache keys should be different.
    #[test]
    fn prop_cache_key_unique_combinations(
        url1 in "[a-z]{3,10}\\.[a-z]{3}",
        url2 in "[a-z]{3,10}\\.[a-z]{3}",
        start1 in 0u64..=1_000_000u64,
        end1 in 0u64..=1_000_000u64,
        start2 in 0u64..=1_000_000u64,
        end2 in 0u64..=1_000_000u64,
    ) {
        let (start1, end1) = if start1 <= end1 {
            (start1, end1)
        } else {
            (end1, start1)
        };
        
        let (start2, end2) = if start2 <= end2 {
            (start2, end2)
        } else {
            (end2, start2)
        };
        
        // Skip if both URL and range are the same
        prop_assume!(url1 != url2 || start1 != start2 || end1 != end2);
        
        let range1 = ByteRange::new(start1, end1)
            .expect("Range 1 should be valid");
        let range2 = ByteRange::new(start2, end2)
            .expect("Range 2 should be valid");
        
        let cache = SliceCache::new(Duration::from_secs(3600));
        
        let full_url1 = format!("http://{}/file.bin", url1);
        let full_url2 = format!("http://{}/file.bin", url2);
        
        let key1 = cache.generate_cache_key(&full_url1, &range1);
        let key2 = cache.generate_cache_key(&full_url2, &range2);
        
        prop_assert_ne!(
            key1,
            key2,
            "Cache keys should be different for different URL/range combinations"
        );
    }

    /// Property 12: Cache key determinism
    /// 
    /// For any URL and byte range, generating the cache key multiple times
    /// should always produce the same result (deterministic).
    #[test]
    fn prop_cache_key_deterministic(
        url in "[a-z]{3,10}\\.[a-z]{3}",
        start in 0u64..=1_000_000u64,
        end in 0u64..=1_000_000u64,
    ) {
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        
        let range = ByteRange::new(start, end)
            .expect("Range should be valid");
        
        let cache = SliceCache::new(Duration::from_secs(3600));
        let full_url = format!("http://{}/file.bin", url);
        
        // Generate key multiple times
        let key1 = cache.generate_cache_key(&full_url, &range);
        let key2 = cache.generate_cache_key(&full_url, &range);
        let key3 = cache.generate_cache_key(&full_url, &range);
        
        prop_assert_eq!(
            &key1,
            &key2,
            "Cache key should be deterministic (first vs second)"
        );
        prop_assert_eq!(
            &key2,
            &key3,
            "Cache key should be deterministic (second vs third)"
        );
    }

    /// Property 12: Cache key format consistency
    /// 
    /// For any URL and byte range, the cache key should follow
    /// the expected format: {url}:slice:{start}:{end}
    #[test]
    fn prop_cache_key_format(
        url in "[a-z]{3,10}\\.[a-z]{3}",
        start in 0u64..=1_000_000u64,
        end in 0u64..=1_000_000u64,
    ) {
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        
        let range = ByteRange::new(start, end)
            .expect("Range should be valid");
        
        let cache = SliceCache::new(Duration::from_secs(3600));
        let full_url = format!("http://{}/file.bin", url);
        
        let key = cache.generate_cache_key(&full_url, &range);
        
        // Verify format
        prop_assert!(
            key.contains(":slice:"),
            "Cache key should contain ':slice:' separator, got: {}",
            key
        );
        
        prop_assert!(
            key.starts_with(&full_url),
            "Cache key should start with URL, got: {}",
            key
        );
        
        // Verify expected format
        let expected = format!("{}:slice:{}:{}", full_url, start, end);
        prop_assert_eq!(
            key,
            expected,
            "Cache key should follow format {{url}}:slice:{{start}}:{{end}}"
        );
    }

    /// Property 12: Cache key collision resistance
    /// 
    /// For any set of different slice specifications, all cache keys
    /// should be unique (no collisions).
    #[test]
    fn prop_cache_key_no_collisions(
        urls in prop::collection::vec("[a-z]{3,10}\\.[a-z]{3}", 2..10),
        ranges in prop::collection::vec((0u64..=100_000u64, 0u64..=100_000u64), 2..10),
    ) {
        let cache = SliceCache::new(Duration::from_secs(3600));
        let mut keys = std::collections::HashSet::new();
        
        // Generate keys for all combinations
        for url in &urls {
            for (start, end) in &ranges {
                let (start, end) = if start <= end {
                    (*start, *end)
                } else {
                    (*end, *start)
                };
                
                let range = ByteRange::new(start, end)
                    .expect("Range should be valid");
                
                let full_url = format!("http://{}/file.bin", url);
                let key = cache.generate_cache_key(&full_url, &range);
                
                // Check for collision
                prop_assert!(
                    !keys.contains(&key),
                    "Cache key collision detected: {}",
                    key
                );
                
                keys.insert(key);
            }
        }
        
        // Verify we generated the expected number of unique keys
        let expected_keys = urls.len() * ranges.len();
        prop_assert_eq!(
            keys.len(),
            expected_keys,
            "Should have {} unique keys, got {}",
            expected_keys,
            keys.len()
        );
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_different_urls_different_keys() {
        let cache = SliceCache::new(Duration::from_secs(3600));
        let range = ByteRange::new(0, 1023).unwrap();
        
        let key1 = cache.generate_cache_key("http://example.com/file.bin", &range);
        let key2 = cache.generate_cache_key("http://other.com/file.bin", &range);
        
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_different_ranges_different_keys() {
        let cache = SliceCache::new(Duration::from_secs(3600));
        let url = "http://example.com/file.bin";
        
        let range1 = ByteRange::new(0, 1023).unwrap();
        let range2 = ByteRange::new(1024, 2047).unwrap();
        
        let key1 = cache.generate_cache_key(url, &range1);
        let key2 = cache.generate_cache_key(url, &range2);
        
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_same_url_and_range_same_key() {
        let cache = SliceCache::new(Duration::from_secs(3600));
        let url = "http://example.com/file.bin";
        let range = ByteRange::new(0, 1023).unwrap();
        
        let key1 = cache.generate_cache_key(url, &range);
        let key2 = cache.generate_cache_key(url, &range);
        
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_key_format() {
        let cache = SliceCache::new(Duration::from_secs(3600));
        let url = "http://example.com/file.bin";
        let range = ByteRange::new(100, 200).unwrap();
        
        let key = cache.generate_cache_key(url, &range);
        
        assert_eq!(key, "http://example.com/file.bin:slice:100:200");
    }

    #[test]
    fn test_multiple_slices_unique_keys() {
        let cache = SliceCache::new(Duration::from_secs(3600));
        let url = "http://example.com/file.bin";
        
        let mut keys = std::collections::HashSet::new();
        
        // Generate keys for 10 different slices
        for i in 0..10 {
            let range = ByteRange::new(i * 1024, (i + 1) * 1024 - 1).unwrap();
            let key = cache.generate_cache_key(url, &range);
            
            // Should be unique
            assert!(!keys.contains(&key), "Duplicate key detected: {}", key);
            keys.insert(key);
        }
        
        assert_eq!(keys.len(), 10);
    }

    #[test]
    fn test_url_path_variations() {
        let cache = SliceCache::new(Duration::from_secs(3600));
        let range = ByteRange::new(0, 1023).unwrap();
        
        let key1 = cache.generate_cache_key("http://example.com/file1.bin", &range);
        let key2 = cache.generate_cache_key("http://example.com/file2.bin", &range);
        let key3 = cache.generate_cache_key("http://example.com/dir/file1.bin", &range);
        
        // All should be different
        assert_ne!(key1, key2);
        assert_ne!(key1, key3);
        assert_ne!(key2, key3);
    }

    #[test]
    fn test_query_parameters_in_url() {
        let cache = SliceCache::new(Duration::from_secs(3600));
        let range = ByteRange::new(0, 1023).unwrap();
        
        let key1 = cache.generate_cache_key("http://example.com/file.bin?v=1", &range);
        let key2 = cache.generate_cache_key("http://example.com/file.bin?v=2", &range);
        
        // Different query parameters should result in different keys
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_large_range_values() {
        let cache = SliceCache::new(Duration::from_secs(3600));
        let url = "http://example.com/file.bin";
        
        let range1 = ByteRange::new(0, u64::MAX - 1).unwrap();
        let range2 = ByteRange::new(1, u64::MAX - 1).unwrap();
        
        let key1 = cache.generate_cache_key(url, &range1);
        let key2 = cache.generate_cache_key(url, &range2);
        
        // Should be different even with large values
        assert_ne!(key1, key2);
    }
}
