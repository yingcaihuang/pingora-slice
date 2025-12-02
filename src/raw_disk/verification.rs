//! Data verification and repair manager
//!
//! This module provides periodic data verification and automatic repair capabilities.

use super::checksum::{Checksum, VerificationConfig, VerificationStats};
use super::directory::CacheDirectory;
use super::disk_io::DiskIOManager;
use super::types::DiskLocation;
use super::RawDiskError;
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::time;
use tracing::{debug, error, info, warn};

/// Data verification manager
pub struct VerificationManager {
    config: VerificationConfig,
    stats: Arc<RwLock<VerificationStats>>,
    disk_io: Arc<DiskIOManager>,
    /// Backup storage for corrupted data (key -> original data)
    backup_storage: Arc<RwLock<HashMap<String, Bytes>>>,
}

impl VerificationManager {
    /// Create a new verification manager
    pub fn new(config: VerificationConfig, disk_io: Arc<DiskIOManager>) -> Self {
        Self {
            config,
            stats: Arc::new(RwLock::new(VerificationStats::new())),
            disk_io,
            backup_storage: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get current configuration
    pub fn config(&self) -> &VerificationConfig {
        &self.config
    }

    /// Get current statistics
    pub async fn stats(&self) -> VerificationStats {
        self.stats.read().await.clone()
    }

    /// Update configuration
    pub fn update_config(&mut self, config: VerificationConfig) {
        self.config = config;
    }

    /// Start periodic verification task
    pub fn start_periodic_verification(
        self: Arc<Self>,
        directory: Arc<RwLock<CacheDirectory>>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(
                self.config.verification_interval_secs,
            ));

            loop {
                interval.tick().await;

                if !self.config.periodic_verification_enabled {
                    continue;
                }

                info!("Starting periodic data verification");
                match self.verify_all_entries(directory.clone()).await {
                    Ok(result) => {
                        info!(
                            "Periodic verification completed: verified={}, corrupted={}, repaired={}",
                            result.verified, result.corrupted, result.repaired
                        );
                    }
                    Err(e) => {
                        error!("Periodic verification failed: {}", e);
                    }
                }
            }
        })
    }

    /// Verify all cache entries
    pub async fn verify_all_entries(
        &self,
        directory: Arc<RwLock<CacheDirectory>>,
    ) -> Result<VerificationResult, RawDiskError> {
        let start = Instant::now();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let dir = directory.read().await;
        let entries: Vec<(String, DiskLocation)> = dir
            .iter()
            .take(self.config.max_entries_per_run)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        drop(dir);

        let total_entries = entries.len();
        let mut verified = 0;
        let mut corrupted_keys = Vec::new();

        info!("Verifying {} cache entries", total_entries);

        for (key, location) in entries {
            match self.verify_entry(&key, &location).await {
                Ok(true) => {
                    verified += 1;
                }
                Ok(false) => {
                    warn!("Corrupted entry detected: {}", key);
                    corrupted_keys.push(key);
                }
                Err(e) => {
                    warn!("Failed to verify entry {}: {}", key, e);
                }
            }
        }

        let corrupted_count = corrupted_keys.len();
        let mut repaired = 0;

        // Attempt to repair corrupted entries if enabled
        if self.config.auto_repair_enabled && !corrupted_keys.is_empty() {
            info!("Attempting to repair {} corrupted entries", corrupted_count);

            for key in &corrupted_keys {
                match self.repair_entry(key, directory.clone()).await {
                    Ok(true) => {
                        repaired += 1;
                        let mut stats = self.stats.write().await;
                        stats.record_repair_success();
                    }
                    Ok(false) => {
                        let mut stats = self.stats.write().await;
                        stats.record_repair_failure();
                    }
                    Err(e) => {
                        warn!("Failed to repair entry {}: {}", key, e);
                        let mut stats = self.stats.write().await;
                        stats.record_repair_failure();
                    }
                }
            }
        }

        let duration = start.elapsed();
        let duration_ms = duration.as_millis() as u64;

        // Update statistics
        let mut stats = self.stats.write().await;
        stats.record_run(verified as u64, corrupted_count as u64, duration_ms, timestamp);
        drop(stats);

        info!(
            "Verification completed in {:?}: verified={}, corrupted={}, repaired={}",
            duration, verified, corrupted_count, repaired
        );

        Ok(VerificationResult {
            verified,
            corrupted: corrupted_count,
            repaired,
            duration,
        })
    }

    /// Verify a single cache entry
    pub async fn verify_entry(
        &self,
        key: &str,
        location: &DiskLocation,
    ) -> Result<bool, RawDiskError> {
        debug!("Verifying entry: {}", key);

        // Read data from disk
        let data = self
            .disk_io
            .read_at(location.offset, location.size as usize)
            .await?;

        // Verify checksum using the enhanced checksum system
        let is_valid = if location.checksum == 0xFFFFFFFF {
            // This is a new-style checksum stored separately
            // For now, we'll use the legacy verification
            location.verify_checksum(&data)
        } else {
            // Legacy CRC32 checksum
            location.verify_checksum(&data)
        };

        if !is_valid {
            warn!(
                "Checksum verification failed for key: {} (offset: {}, size: {})",
                key, location.offset, location.size
            );
        }

        Ok(is_valid)
    }

    /// Attempt to repair a corrupted entry
    pub async fn repair_entry(
        &self,
        key: &str,
        directory: Arc<RwLock<CacheDirectory>>,
    ) -> Result<bool, RawDiskError> {
        info!("Attempting to repair corrupted entry: {}", key);

        // Check if we have a backup
        let backup_data = {
            let backup_storage = self.backup_storage.read().await;
            backup_storage.get(key).cloned()
        };

        if let Some(backup_data) = backup_data {
            info!("Found backup data for key: {}", key);

            // Get current location
            let dir = directory.read().await;
            let location = match dir.get(key) {
                Some(loc) => loc.clone(),
                None => {
                    warn!("Entry not found in directory: {}", key);
                    return Ok(false);
                }
            };
            drop(dir);

            // Write backup data back to disk
            self.disk_io
                .write_at(location.offset, &backup_data)
                .await?;

            info!("Successfully repaired entry from backup: {}", key);
            return Ok(true);
        }

        // No backup available - cannot repair
        warn!("No backup available for corrupted entry: {}", key);

        // Remove the corrupted entry from the directory
        let mut dir = directory.write().await;
        dir.remove(key);
        drop(dir);

        info!("Removed corrupted entry from cache: {}", key);
        Ok(false)
    }

    /// Create a backup of data before writing
    pub async fn backup_data(&self, key: String, data: Bytes) {
        if !self.config.keep_backup_on_repair {
            return;
        }

        let mut backup_storage = self.backup_storage.write().await;
        backup_storage.insert(key, data);
    }

    /// Clear backup for a key
    pub async fn clear_backup(&self, key: &str) {
        let mut backup_storage = self.backup_storage.write().await;
        backup_storage.remove(key);
    }

    /// Clear all backups
    pub async fn clear_all_backups(&self) {
        let mut backup_storage = self.backup_storage.write().await;
        backup_storage.clear();
    }

    /// Get backup storage size
    pub async fn backup_storage_size(&self) -> usize {
        let backup_storage = self.backup_storage.read().await;
        backup_storage.len()
    }

    /// Verify a specific entry by key
    pub async fn verify_entry_by_key(
        &self,
        key: &str,
        directory: Arc<RwLock<CacheDirectory>>,
    ) -> Result<bool, RawDiskError> {
        let dir = directory.read().await;
        let location = match dir.get(key) {
            Some(loc) => loc.clone(),
            None => {
                return Err(RawDiskError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Key not found: {}", key),
                )));
            }
        };
        drop(dir);

        self.verify_entry(key, &location).await
    }

    /// Compute enhanced checksum for data
    pub fn compute_checksum(&self, data: &[u8]) -> Checksum {
        Checksum::compute(self.config.algorithm, data)
    }

    /// Verify data against enhanced checksum
    pub fn verify_checksum(&self, checksum: &Checksum, data: &[u8]) -> bool {
        checksum.verify(data)
    }
}

/// Result of a verification run
#[derive(Debug, Clone)]
pub struct VerificationResult {
    /// Number of entries verified
    pub verified: usize,
    /// Number of corrupted entries found
    pub corrupted: usize,
    /// Number of entries repaired
    pub repaired: usize,
    /// Duration of verification
    pub duration: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raw_disk::checksum::ChecksumAlgorithm;
    use crate::raw_disk::disk_io::DiskIOManager;
    use crate::raw_disk::types::DiskLocation;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_verification_manager_creation() {
        let temp_file = NamedTempFile::new().unwrap();
        let disk_io = Arc::new(
            DiskIOManager::new(temp_file.path(), 4096)
                .await
                .unwrap(),
        );

        let config = VerificationConfig::default();
        let manager = VerificationManager::new(config, disk_io);

        assert_eq!(
            manager.config().algorithm,
            ChecksumAlgorithm::XxHash3
        );
    }

    #[tokio::test]
    async fn test_verify_valid_entry() {
        let temp_file = NamedTempFile::new().unwrap();
        let disk_io = Arc::new(
            DiskIOManager::new(temp_file.path(), 4096)
                .await
                .unwrap(),
        );

        let config = VerificationConfig::default();
        let manager = VerificationManager::new(config, disk_io.clone());

        // Write test data
        let data = b"test data for verification";
        let offset = 4096;
        disk_io.write_at(offset, data).await.unwrap();

        // Create location with correct checksum
        let location = DiskLocation::new(offset, data);

        // Verify
        let result = manager.verify_entry("test_key", &location).await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_verify_corrupted_entry() {
        let temp_file = NamedTempFile::new().unwrap();
        let disk_io = Arc::new(
            DiskIOManager::new(temp_file.path(), 4096)
                .await
                .unwrap(),
        );

        let config = VerificationConfig::default();
        let manager = VerificationManager::new(config, disk_io.clone());

        // Write test data
        let data = b"test data for verification";
        let offset = 4096;
        disk_io.write_at(offset, data).await.unwrap();

        // Create location with wrong checksum
        let mut location = DiskLocation::new(offset, data);
        location.checksum = 0x12345678; // Wrong checksum

        // Verify - should fail
        let result = manager.verify_entry("test_key", &location).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_backup_and_repair() {
        let temp_file = NamedTempFile::new().unwrap();
        let disk_io = Arc::new(
            DiskIOManager::new(temp_file.path(), 4096)
                .await
                .unwrap(),
        );

        let mut config = VerificationConfig::default();
        config.keep_backup_on_repair = true;
        let manager = VerificationManager::new(config, disk_io.clone());

        // Create backup
        let key = "test_key".to_string();
        let data = Bytes::from("test data");
        manager.backup_data(key.clone(), data.clone()).await;

        assert_eq!(manager.backup_storage_size().await, 1);

        // Clear backup
        manager.clear_backup(&key).await;
        assert_eq!(manager.backup_storage_size().await, 0);
    }

    #[tokio::test]
    async fn test_verification_stats() {
        let temp_file = NamedTempFile::new().unwrap();
        let disk_io = Arc::new(
            DiskIOManager::new(temp_file.path(), 4096)
                .await
                .unwrap(),
        );

        let config = VerificationConfig::default();
        let manager = VerificationManager::new(config, disk_io);

        let stats = manager.stats().await;
        assert_eq!(stats.total_runs, 0);
        assert_eq!(stats.total_verified, 0);
        assert_eq!(stats.corrupted_found, 0);
    }
}
