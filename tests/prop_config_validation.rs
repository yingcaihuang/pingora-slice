// Feature: pingora-slice, Property 1: 配置值范围验证
// **Validates: Requirements 1.2, 1.4**
//
// Property: For any slice size configuration value, if the value is outside 
// the range [64KB, 10MB], then validation should fail and the module should 
// refuse to start

use pingora_slice::config::SliceConfig;
use proptest::prelude::*;

const MIN_SLICE_SIZE: usize = 64 * 1024; // 64KB
const MAX_SLICE_SIZE: usize = 10 * 1024 * 1024; // 10MB

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property 1: Configuration value range validation
    /// 
    /// For any slice_size value outside [64KB, 10MB], validation must fail.
    /// For any slice_size value inside [64KB, 10MB], validation must succeed.
    #[test]
    fn prop_slice_size_validation(slice_size in any::<usize>()) {
        let mut config = SliceConfig::default();
        config.slice_size = slice_size;
        
        let validation_result = config.validate();
        
        if slice_size < MIN_SLICE_SIZE || slice_size > MAX_SLICE_SIZE {
            // Outside valid range - validation MUST fail
            prop_assert!(
                validation_result.is_err(),
                "Validation should fail for slice_size={} (outside range [{}, {}])",
                slice_size,
                MIN_SLICE_SIZE,
                MAX_SLICE_SIZE
            );
        } else {
            // Inside valid range - validation MUST succeed
            prop_assert!(
                validation_result.is_ok(),
                "Validation should succeed for slice_size={} (inside range [{}, {}])",
                slice_size,
                MIN_SLICE_SIZE,
                MAX_SLICE_SIZE
            );
        }
    }

    /// Property 1 (boundary test): Exact boundary values
    /// 
    /// Test that the exact boundary values are handled correctly.
    #[test]
    fn prop_slice_size_boundaries(
        offset in 0usize..1000
    ) {
        // Test just below minimum
        if offset < MIN_SLICE_SIZE {
            let mut config = SliceConfig::default();
            config.slice_size = MIN_SLICE_SIZE - offset - 1;
            prop_assert!(
                config.validate().is_err(),
                "Validation should fail for slice_size={} (below minimum)",
                config.slice_size
            );
        }
        
        // Test at minimum (should pass)
        let mut config = SliceConfig::default();
        config.slice_size = MIN_SLICE_SIZE;
        prop_assert!(
            config.validate().is_ok(),
            "Validation should succeed for slice_size={} (at minimum)",
            MIN_SLICE_SIZE
        );
        
        // Test at maximum (should pass)
        config.slice_size = MAX_SLICE_SIZE;
        prop_assert!(
            config.validate().is_ok(),
            "Validation should succeed for slice_size={} (at maximum)",
            MAX_SLICE_SIZE
        );
        
        // Test just above maximum
        if offset < usize::MAX - MAX_SLICE_SIZE {
            config.slice_size = MAX_SLICE_SIZE + offset + 1;
            prop_assert!(
                config.validate().is_err(),
                "Validation should fail for slice_size={} (above maximum)",
                config.slice_size
            );
        }
    }

    /// Property 1 (extended): Other configuration parameters
    /// 
    /// Ensure that other configuration validations also work correctly.
    #[test]
    fn prop_other_config_validations(
        max_concurrent in any::<usize>(),
        cache_ttl in any::<u64>()
    ) {
        let mut config = SliceConfig::default();
        
        // Test max_concurrent_subrequests validation
        config.max_concurrent_subrequests = max_concurrent;
        let result = config.validate();
        
        if max_concurrent == 0 {
            prop_assert!(
                result.is_err(),
                "Validation should fail for max_concurrent_subrequests=0"
            );
        } else {
            // Reset to valid value for next test
            config.max_concurrent_subrequests = 4;
        }
        
        // Test cache_ttl validation (only matters when cache is enabled)
        config.enable_cache = true;
        config.cache_ttl = cache_ttl;
        let result = config.validate();
        
        if cache_ttl == 0 {
            prop_assert!(
                result.is_err(),
                "Validation should fail for cache_ttl=0 when caching is enabled"
            );
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_min_boundary() {
        let mut config = SliceConfig::default();
        config.slice_size = MIN_SLICE_SIZE;
        assert!(config.validate().is_ok(), "Minimum boundary should be valid");
    }

    #[test]
    fn test_max_boundary() {
        let mut config = SliceConfig::default();
        config.slice_size = MAX_SLICE_SIZE;
        assert!(config.validate().is_ok(), "Maximum boundary should be valid");
    }

    #[test]
    fn test_below_min() {
        let mut config = SliceConfig::default();
        config.slice_size = MIN_SLICE_SIZE - 1;
        assert!(config.validate().is_err(), "Below minimum should be invalid");
    }

    #[test]
    fn test_above_max() {
        let mut config = SliceConfig::default();
        config.slice_size = MAX_SLICE_SIZE + 1;
        assert!(config.validate().is_err(), "Above maximum should be invalid");
    }
}
