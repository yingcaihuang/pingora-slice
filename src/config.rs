//! Configuration management for the Pingora Slice module

use crate::error::{Result, SliceError};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Configuration for the Slice module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SliceConfig {
    /// Size of each slice in bytes (default: 1MB)
    /// Valid range: 64KB to 10MB
    #[serde(default = "default_slice_size")]
    pub slice_size: usize,

    /// Maximum number of concurrent subrequests (default: 4)
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_subrequests: usize,

    /// Maximum number of retries for failed subrequests (default: 3)
    #[serde(default = "default_max_retries")]
    pub max_retries: usize,

    /// URL patterns that should enable slicing (regex patterns)
    #[serde(default)]
    pub slice_patterns: Vec<String>,

    /// Whether to enable caching (default: true)
    #[serde(default = "default_true")]
    pub enable_cache: bool,

    /// Cache TTL in seconds (default: 3600 = 1 hour)
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl: u64,

    /// L1 (memory) cache size in bytes (default: 100MB)
    #[serde(default = "default_l1_cache_size")]
    pub l1_cache_size_bytes: usize,

    /// L2 (disk) cache directory (default: /var/cache/pingora-slice)
    #[serde(default = "default_l2_cache_dir")]
    pub l2_cache_dir: String,

    /// Whether to enable L2 disk cache (default: true)
    #[serde(default = "default_true")]
    pub enable_l2_cache: bool,

    /// Upstream server address
    #[serde(default = "default_upstream")]
    pub upstream_address: String,

    /// Metrics endpoint configuration (optional)
    #[serde(default)]
    pub metrics_endpoint: Option<MetricsEndpointConfig>,
}

/// Configuration for the metrics HTTP endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsEndpointConfig {
    /// Whether to enable the metrics endpoint (default: false)
    #[serde(default)]
    pub enabled: bool,

    /// Address to bind the metrics endpoint to (default: "127.0.0.1:9090")
    #[serde(default = "default_metrics_address")]
    pub address: String,
}

impl Default for MetricsEndpointConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            address: default_metrics_address(),
        }
    }
}

// Default value functions for serde
fn default_slice_size() -> usize {
    1024 * 1024 // 1MB
}

fn default_max_concurrent() -> usize {
    4
}

fn default_max_retries() -> usize {
    3
}

fn default_true() -> bool {
    true
}

fn default_cache_ttl() -> u64 {
    3600 // 1 hour
}

fn default_l1_cache_size() -> usize {
    100 * 1024 * 1024 // 100MB
}

fn default_l2_cache_dir() -> String {
    "/var/cache/pingora-slice".to_string()
}

fn default_upstream() -> String {
    "127.0.0.1:8080".to_string()
}

fn default_metrics_address() -> String {
    "127.0.0.1:9090".to_string()
}

impl Default for SliceConfig {
    fn default() -> Self {
        SliceConfig {
            slice_size: default_slice_size(),
            max_concurrent_subrequests: default_max_concurrent(),
            max_retries: default_max_retries(),
            slice_patterns: Vec::new(),
            enable_cache: default_true(),
            cache_ttl: default_cache_ttl(),
            upstream_address: default_upstream(),
            metrics_endpoint: None,
        }
    }
}

impl SliceConfig {
    /// Load configuration from a YAML file
    ///
    /// # Arguments
    /// * `path` - Path to the YAML configuration file
    ///
    /// # Returns
    /// * `Ok(SliceConfig)` if loading and validation succeed
    /// * `Err(SliceError)` if file cannot be read or config is invalid
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path.as_ref()).map_err(|e| {
            SliceError::ConfigError(format!("Failed to read config file: {}", e))
        })?;

        let config: SliceConfig = serde_yaml::from_str(&content).map_err(|e| {
            SliceError::ConfigError(format!("Failed to parse config file: {}", e))
        })?;

        config.validate()?;
        Ok(config)
    }

    /// Validate the configuration
    ///
    /// # Returns
    /// * `Ok(())` if configuration is valid
    /// * `Err(SliceError)` if any validation fails
    ///
    /// # Validation Rules
    /// - slice_size must be between 64KB and 10MB
    /// - max_concurrent_subrequests must be > 0
    /// - max_retries must be >= 0
    /// - cache_ttl must be > 0
    pub fn validate(&self) -> Result<()> {
        const MIN_SLICE_SIZE: usize = 64 * 1024; // 64KB
        const MAX_SLICE_SIZE: usize = 10 * 1024 * 1024; // 10MB

        // Validate slice size
        if self.slice_size < MIN_SLICE_SIZE || self.slice_size > MAX_SLICE_SIZE {
            return Err(SliceError::ConfigError(format!(
                "slice_size must be between {}KB and {}MB, got {} bytes",
                MIN_SLICE_SIZE / 1024,
                MAX_SLICE_SIZE / (1024 * 1024),
                self.slice_size
            )));
        }

        // Validate max concurrent subrequests
        if self.max_concurrent_subrequests == 0 {
            return Err(SliceError::ConfigError(
                "max_concurrent_subrequests must be greater than 0".to_string(),
            ));
        }

        // Validate cache TTL
        if self.enable_cache && self.cache_ttl == 0 {
            return Err(SliceError::ConfigError(
                "cache_ttl must be greater than 0 when caching is enabled".to_string(),
            ));
        }

        Ok(())
    }

    /// Create a new SliceConfig with custom values
    pub fn new(
        slice_size: usize,
        max_concurrent_subrequests: usize,
        max_retries: usize,
    ) -> Result<Self> {
        let config = SliceConfig {
            slice_size,
            max_concurrent_subrequests,
            max_retries,
            ..Default::default()
        };
        config.validate()?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SliceConfig::default();
        assert_eq!(config.slice_size, 1024 * 1024);
        assert_eq!(config.max_concurrent_subrequests, 4);
        assert_eq!(config.max_retries, 3);
        assert!(config.enable_cache);
        assert_eq!(config.cache_ttl, 3600);
    }

    #[test]
    fn test_validate_valid_config() {
        let config = SliceConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_slice_size_too_small() {
        let mut config = SliceConfig::default();
        config.slice_size = 1024; // 1KB, too small
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_slice_size_too_large() {
        let mut config = SliceConfig::default();
        config.slice_size = 20 * 1024 * 1024; // 20MB, too large
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_zero_concurrent() {
        let mut config = SliceConfig::default();
        config.max_concurrent_subrequests = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_zero_cache_ttl() {
        let mut config = SliceConfig::default();
        config.cache_ttl = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_new_config() {
        let config = SliceConfig::new(512 * 1024, 8, 5).unwrap();
        assert_eq!(config.slice_size, 512 * 1024);
        assert_eq!(config.max_concurrent_subrequests, 8);
        assert_eq!(config.max_retries, 5);
    }

    #[test]
    fn test_new_config_invalid() {
        let result = SliceConfig::new(1024, 8, 5); // Too small
        assert!(result.is_err());
    }
}
