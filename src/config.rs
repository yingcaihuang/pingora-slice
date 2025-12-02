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

    /// L2 cache backend type (default: "file")
    /// Options: "file" (filesystem-based) or "raw_disk" (raw disk cache)
    #[serde(default = "default_l2_backend")]
    pub l2_backend: String,

    /// Raw disk cache configuration (optional, only used when l2_backend = "raw_disk")
    #[serde(default)]
    pub raw_disk_cache: Option<RawDiskCacheConfig>,

    /// Upstream server address
    #[serde(default = "default_upstream")]
    pub upstream_address: String,

    /// Metrics endpoint configuration (optional)
    #[serde(default)]
    pub metrics_endpoint: Option<MetricsEndpointConfig>,

    /// Purge configuration (optional)
    #[serde(default)]
    pub purge: Option<PurgeConfig>,
}

/// Configuration for the metrics HTTP endpoint
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetricsEndpointConfig {
    /// Whether to enable the metrics endpoint (default: false)
    #[serde(default)]
    pub enabled: bool,

    /// Address to bind the metrics endpoint to (default: "127.0.0.1:9090")
    #[serde(default = "default_metrics_address")]
    pub address: String,
}

/// Configuration for cache purge functionality
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PurgeConfig {
    /// Whether to enable purge functionality (default: false)
    #[serde(default)]
    pub enabled: bool,

    /// Authentication token for purge requests (optional)
    /// If not set, purge requests will not require authentication
    pub auth_token: Option<String>,

    /// Whether to enable Prometheus metrics for purge operations (default: true)
    #[serde(default = "default_true")]
    pub enable_metrics: bool,
}

/// Configuration for raw disk cache
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawDiskCacheConfig {
    /// Path to the raw disk cache device/file
    #[serde(default = "default_raw_disk_path")]
    pub device_path: String,

    /// Total size of the raw disk cache in bytes (default: 10GB)
    #[serde(default = "default_raw_disk_size")]
    pub total_size: u64,

    /// Block size in bytes (default: 4KB)
    #[serde(default = "default_raw_disk_block_size")]
    pub block_size: usize,

    /// Whether to use O_DIRECT for disk I/O (default: true)
    #[serde(default = "default_true")]
    pub use_direct_io: bool,

    /// Whether to enable compression (default: true)
    #[serde(default = "default_true")]
    pub enable_compression: bool,

    /// Whether to enable prefetching (default: true)
    #[serde(default = "default_true")]
    pub enable_prefetch: bool,

    /// Whether to enable zero-copy operations (default: true)
    #[serde(default = "default_true")]
    pub enable_zero_copy: bool,
}

impl Default for RawDiskCacheConfig {
    fn default() -> Self {
        Self {
            device_path: default_raw_disk_path(),
            total_size: default_raw_disk_size(),
            block_size: default_raw_disk_block_size(),
            use_direct_io: default_true(),
            enable_compression: default_true(),
            enable_prefetch: default_true(),
            enable_zero_copy: default_true(),
        }
    }
}

impl RawDiskCacheConfig {
    /// Validate the raw disk cache configuration
    ///
    /// # Returns
    /// * `Ok(())` if configuration is valid
    /// * `Err(SliceError)` if any validation fails
    ///
    /// # Validation Rules
    /// - device_path must not be empty
    /// - total_size must be at least 1MB
    /// - block_size must be a power of 2 between 512 bytes and 1MB
    /// - total_size must be at least 10x block_size
    pub fn validate(&self) -> Result<()> {
        const MIN_TOTAL_SIZE: u64 = 1024 * 1024; // 1MB
        const MIN_BLOCK_SIZE: usize = 512; // 512 bytes
        const MAX_BLOCK_SIZE: usize = 1024 * 1024; // 1MB

        // Validate device path
        if self.device_path.is_empty() {
            return Err(SliceError::ConfigError(
                "raw_disk device_path must not be empty".to_string(),
            ));
        }

        // Validate total size
        if self.total_size < MIN_TOTAL_SIZE {
            return Err(SliceError::ConfigError(format!(
                "raw_disk total_size must be at least {}MB, got {} bytes",
                MIN_TOTAL_SIZE / (1024 * 1024),
                self.total_size
            )));
        }

        // Validate block size
        if self.block_size < MIN_BLOCK_SIZE || self.block_size > MAX_BLOCK_SIZE {
            return Err(SliceError::ConfigError(format!(
                "raw_disk block_size must be between {} bytes and {}KB, got {} bytes",
                MIN_BLOCK_SIZE,
                MAX_BLOCK_SIZE / 1024,
                self.block_size
            )));
        }

        // Validate block size is power of 2
        if !self.block_size.is_power_of_two() {
            return Err(SliceError::ConfigError(format!(
                "raw_disk block_size must be a power of 2, got {}",
                self.block_size
            )));
        }

        // Validate total size is sufficient for block size
        let min_total_for_blocks = (self.block_size as u64) * 10;
        if self.total_size < min_total_for_blocks {
            return Err(SliceError::ConfigError(format!(
                "raw_disk total_size must be at least 10x block_size ({}), got {}",
                min_total_for_blocks, self.total_size
            )));
        }

        Ok(())
    }
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

fn default_l2_backend() -> String {
    "file".to_string()
}

fn default_raw_disk_path() -> String {
    "/var/cache/pingora-slice-raw".to_string()
}

fn default_raw_disk_size() -> u64 {
    10 * 1024 * 1024 * 1024 // 10GB
}

fn default_raw_disk_block_size() -> usize {
    4096 // 4KB
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
            l1_cache_size_bytes: default_l1_cache_size(),
            l2_cache_dir: default_l2_cache_dir(),
            enable_l2_cache: default_true(),
            l2_backend: default_l2_backend(),
            raw_disk_cache: None,
            upstream_address: default_upstream(),
            metrics_endpoint: None,
            purge: None,
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
    /// - raw_disk configuration must be valid if l2_backend is "raw_disk"
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

        // Validate L2 backend configuration
        if self.enable_l2_cache {
            match self.l2_backend.as_str() {
                "file" => {
                    // File backend is always valid
                }
                "raw_disk" => {
                    // Validate raw_disk configuration
                    if let Some(ref raw_disk_config) = self.raw_disk_cache {
                        raw_disk_config.validate()?;
                    } else {
                        return Err(SliceError::ConfigError(
                            "raw_disk_cache configuration is required when l2_backend is 'raw_disk'"
                                .to_string(),
                        ));
                    }
                }
                other => {
                    return Err(SliceError::ConfigError(format!(
                        "Invalid l2_backend '{}', must be 'file' or 'raw_disk'",
                        other
                    )));
                }
            }
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

    /// Update configuration from another config
    ///
    /// This method allows hot-reloading of configuration by merging
    /// changes from a new configuration while preserving runtime state.
    ///
    /// # Arguments
    /// * `new_config` - The new configuration to apply
    ///
    /// # Returns
    /// * `Ok(ConfigChanges)` - Description of what changed
    /// * `Err(SliceError)` - If the new configuration is invalid
    pub fn update_from(&mut self, new_config: &SliceConfig) -> Result<ConfigChanges> {
        // Validate new config first
        new_config.validate()?;

        let mut changes = ConfigChanges::default();

        // Track changes
        if self.slice_size != new_config.slice_size {
            changes.slice_size_changed = true;
            self.slice_size = new_config.slice_size;
        }

        if self.max_concurrent_subrequests != new_config.max_concurrent_subrequests {
            changes.max_concurrent_changed = true;
            self.max_concurrent_subrequests = new_config.max_concurrent_subrequests;
        }

        if self.max_retries != new_config.max_retries {
            changes.max_retries_changed = true;
            self.max_retries = new_config.max_retries;
        }

        if self.slice_patterns != new_config.slice_patterns {
            changes.slice_patterns_changed = true;
            self.slice_patterns = new_config.slice_patterns.clone();
        }

        if self.enable_cache != new_config.enable_cache {
            changes.cache_enabled_changed = true;
            self.enable_cache = new_config.enable_cache;
        }

        if self.cache_ttl != new_config.cache_ttl {
            changes.cache_ttl_changed = true;
            self.cache_ttl = new_config.cache_ttl;
        }

        if self.l1_cache_size_bytes != new_config.l1_cache_size_bytes {
            changes.l1_cache_size_changed = true;
            self.l1_cache_size_bytes = new_config.l1_cache_size_bytes;
        }

        if self.l2_cache_dir != new_config.l2_cache_dir {
            changes.l2_cache_dir_changed = true;
            self.l2_cache_dir = new_config.l2_cache_dir.clone();
        }

        if self.enable_l2_cache != new_config.enable_l2_cache {
            changes.l2_cache_enabled_changed = true;
            self.enable_l2_cache = new_config.enable_l2_cache;
        }

        if self.l2_backend != new_config.l2_backend {
            changes.l2_backend_changed = true;
            self.l2_backend = new_config.l2_backend.clone();
        }

        if self.raw_disk_cache != new_config.raw_disk_cache {
            changes.raw_disk_config_changed = true;
            self.raw_disk_cache = new_config.raw_disk_cache.clone();
        }

        if self.upstream_address != new_config.upstream_address {
            changes.upstream_changed = true;
            self.upstream_address = new_config.upstream_address.clone();
        }

        if self.metrics_endpoint != new_config.metrics_endpoint {
            changes.metrics_endpoint_changed = true;
            self.metrics_endpoint = new_config.metrics_endpoint.clone();
        }

        if self.purge != new_config.purge {
            changes.purge_config_changed = true;
            self.purge = new_config.purge.clone();
        }

        Ok(changes)
    }

    /// Reload configuration from file and apply changes
    ///
    /// This method reloads the configuration from the specified file
    /// and applies the changes to the current configuration.
    ///
    /// # Arguments
    /// * `path` - Path to the YAML configuration file
    ///
    /// # Returns
    /// * `Ok(ConfigChanges)` - Description of what changed
    /// * `Err(SliceError)` - If file cannot be read or config is invalid
    pub fn reload_from_file<P: AsRef<Path>>(&mut self, path: P) -> Result<ConfigChanges> {
        let new_config = Self::from_file(path)?;
        self.update_from(&new_config)
    }
}

/// Description of configuration changes after hot reload
#[derive(Debug, Default, Clone)]
pub struct ConfigChanges {
    pub slice_size_changed: bool,
    pub max_concurrent_changed: bool,
    pub max_retries_changed: bool,
    pub slice_patterns_changed: bool,
    pub cache_enabled_changed: bool,
    pub cache_ttl_changed: bool,
    pub l1_cache_size_changed: bool,
    pub l2_cache_dir_changed: bool,
    pub l2_cache_enabled_changed: bool,
    pub l2_backend_changed: bool,
    pub raw_disk_config_changed: bool,
    pub upstream_changed: bool,
    pub metrics_endpoint_changed: bool,
    pub purge_config_changed: bool,
}

impl ConfigChanges {
    /// Check if any changes were made
    pub fn has_changes(&self) -> bool {
        self.slice_size_changed
            || self.max_concurrent_changed
            || self.max_retries_changed
            || self.slice_patterns_changed
            || self.cache_enabled_changed
            || self.cache_ttl_changed
            || self.l1_cache_size_changed
            || self.l2_cache_dir_changed
            || self.l2_cache_enabled_changed
            || self.l2_backend_changed
            || self.raw_disk_config_changed
            || self.upstream_changed
            || self.metrics_endpoint_changed
            || self.purge_config_changed
    }

    /// Check if cache-related settings changed (requires cache restart)
    pub fn requires_cache_restart(&self) -> bool {
        self.cache_ttl_changed
            || self.l1_cache_size_changed
            || self.l2_cache_dir_changed
            || self.l2_cache_enabled_changed
            || self.l2_backend_changed
            || self.raw_disk_config_changed
    }

    /// Get a summary of changes
    pub fn summary(&self) -> Vec<String> {
        let mut changes = Vec::new();

        if self.slice_size_changed {
            changes.push("slice_size".to_string());
        }
        if self.max_concurrent_changed {
            changes.push("max_concurrent_subrequests".to_string());
        }
        if self.max_retries_changed {
            changes.push("max_retries".to_string());
        }
        if self.slice_patterns_changed {
            changes.push("slice_patterns".to_string());
        }
        if self.cache_enabled_changed {
            changes.push("enable_cache".to_string());
        }
        if self.cache_ttl_changed {
            changes.push("cache_ttl".to_string());
        }
        if self.l1_cache_size_changed {
            changes.push("l1_cache_size_bytes".to_string());
        }
        if self.l2_cache_dir_changed {
            changes.push("l2_cache_dir".to_string());
        }
        if self.l2_cache_enabled_changed {
            changes.push("enable_l2_cache".to_string());
        }
        if self.l2_backend_changed {
            changes.push("l2_backend".to_string());
        }
        if self.raw_disk_config_changed {
            changes.push("raw_disk_cache".to_string());
        }
        if self.upstream_changed {
            changes.push("upstream_address".to_string());
        }
        if self.metrics_endpoint_changed {
            changes.push("metrics_endpoint".to_string());
        }
        if self.purge_config_changed {
            changes.push("purge".to_string());
        }

        changes
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

    #[test]
    fn test_raw_disk_config_validation() {
        // Valid config
        let config = RawDiskCacheConfig::default();
        assert!(config.validate().is_ok());

        // Empty device path
        let mut config = RawDiskCacheConfig::default();
        config.device_path = String::new();
        assert!(config.validate().is_err());

        // Total size too small
        let mut config = RawDiskCacheConfig::default();
        config.total_size = 512 * 1024; // 512KB, too small
        assert!(config.validate().is_err());

        // Block size too small
        let mut config = RawDiskCacheConfig::default();
        config.block_size = 256; // Too small
        assert!(config.validate().is_err());

        // Block size too large
        let mut config = RawDiskCacheConfig::default();
        config.block_size = 2 * 1024 * 1024; // 2MB, too large
        assert!(config.validate().is_err());

        // Block size not power of 2
        let mut config = RawDiskCacheConfig::default();
        config.block_size = 3000; // Not power of 2
        assert!(config.validate().is_err());

        // Total size too small for block size
        let mut config = RawDiskCacheConfig::default();
        config.block_size = 1024 * 1024; // 1MB
        config.total_size = 5 * 1024 * 1024; // 5MB, less than 10x block size
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_l2_backend_validation() {
        // File backend is always valid
        let mut config = SliceConfig::default();
        config.l2_backend = "file".to_string();
        config.enable_l2_cache = true;
        assert!(config.validate().is_ok());

        // Raw disk backend requires raw_disk_cache config
        let mut config = SliceConfig::default();
        config.l2_backend = "raw_disk".to_string();
        config.enable_l2_cache = true;
        config.raw_disk_cache = None;
        assert!(config.validate().is_err());

        // Raw disk backend with valid config
        let mut config = SliceConfig::default();
        config.l2_backend = "raw_disk".to_string();
        config.enable_l2_cache = true;
        config.raw_disk_cache = Some(RawDiskCacheConfig::default());
        assert!(config.validate().is_ok());

        // Invalid backend type
        let mut config = SliceConfig::default();
        config.l2_backend = "invalid".to_string();
        config.enable_l2_cache = true;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_hot_reload() {
        let mut config = SliceConfig::default();
        let mut new_config = SliceConfig::default();

        // No changes
        let changes = config.update_from(&new_config).unwrap();
        assert!(!changes.has_changes());

        // Change slice size
        new_config.slice_size = 2 * 1024 * 1024; // 2MB
        let changes = config.update_from(&new_config).unwrap();
        assert!(changes.has_changes());
        assert!(changes.slice_size_changed);
        assert_eq!(config.slice_size, 2 * 1024 * 1024);

        // Change max concurrent
        new_config.max_concurrent_subrequests = 8;
        let changes = config.update_from(&new_config).unwrap();
        assert!(changes.max_concurrent_changed);
        assert_eq!(config.max_concurrent_subrequests, 8);

        // Change cache TTL (requires restart)
        new_config.cache_ttl = 7200;
        let changes = config.update_from(&new_config).unwrap();
        assert!(changes.cache_ttl_changed);
        assert!(changes.requires_cache_restart());
        assert_eq!(config.cache_ttl, 7200);
    }

    #[test]
    fn test_config_hot_reload_validation() {
        let mut config = SliceConfig::default();
        let mut new_config = SliceConfig::default();

        // Invalid new config should fail
        new_config.slice_size = 1024; // Too small
        let result = config.update_from(&new_config);
        assert!(result.is_err());

        // Original config should be unchanged
        assert_eq!(config.slice_size, 1024 * 1024);
    }

    #[test]
    fn test_config_changes_summary() {
        let mut changes = ConfigChanges::default();
        assert!(!changes.has_changes());
        assert!(changes.summary().is_empty());

        changes.slice_size_changed = true;
        changes.cache_ttl_changed = true;
        assert!(changes.has_changes());
        assert!(changes.requires_cache_restart());
        
        let summary = changes.summary();
        assert_eq!(summary.len(), 2);
        assert!(summary.contains(&"slice_size".to_string()));
        assert!(summary.contains(&"cache_ttl".to_string()));
    }

    #[test]
    fn test_raw_disk_config_changes() {
        let mut config = SliceConfig::default();
        config.l2_backend = "raw_disk".to_string();
        config.enable_l2_cache = true;
        config.raw_disk_cache = Some(RawDiskCacheConfig::default());

        let mut new_config = config.clone();
        
        // Change raw disk config
        let mut raw_disk = RawDiskCacheConfig::default();
        raw_disk.block_size = 8192;
        new_config.raw_disk_cache = Some(raw_disk);

        let changes = config.update_from(&new_config).unwrap();
        assert!(changes.raw_disk_config_changed);
        assert!(changes.requires_cache_restart());
        assert_eq!(config.raw_disk_cache.unwrap().block_size, 8192);
    }
}
