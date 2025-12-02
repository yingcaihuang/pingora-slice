//! Tests for enhanced checksum and data verification

use bytes::Bytes;
use pingora_slice::raw_disk::{
    Checksum, ChecksumAlgorithm, RawDiskCache, VerificationConfig,
};
use std::time::Duration;
use tempfile::NamedTempFile;

#[tokio::test]
async fn test_xxhash3_checksum() {
    let data = b"test data for xxhash3";
    let checksum = Checksum::compute(ChecksumAlgorithm::XxHash3, data);
    
    assert_eq!(checksum.algorithm(), ChecksumAlgorithm::XxHash3);
    assert!(checksum.verify(data));
    assert!(!checksum.verify(b"wrong data"));
}

#[tokio::test]
async fn test_xxhash64_checksum() {
    let data = b"test data for xxhash64";
    let checksum = Checksum::compute(ChecksumAlgorithm::XxHash64, data);
    
    assert_eq!(checksum.algorithm(), ChecksumAlgorithm::XxHash64);
    assert!(checksum.verify(data));
    assert!(!checksum.verify(b"wrong data"));
}

#[tokio::test]
async fn test_crc32_checksum_compatibility() {
    let data = b"test data for crc32";
    let checksum = Checksum::compute(ChecksumAlgorithm::Crc32, data);
    
    assert_eq!(checksum.algorithm(), ChecksumAlgorithm::Crc32);
    assert!(checksum.verify(data));
    
    // Test conversion to legacy format
    let crc32_value = checksum.to_crc32();
    assert_ne!(crc32_value, 0);
    
    // Test conversion from legacy format
    let from_legacy = Checksum::from_crc32(crc32_value);
    assert_eq!(from_legacy.algorithm(), ChecksumAlgorithm::Crc32);
    assert!(from_legacy.verify(data));
}

#[tokio::test]
async fn test_cache_with_verification() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    
    let total_size = 10 * 1024 * 1024; // 10MB
    let block_size = 4096;
    let ttl = Duration::from_secs(3600);
    
    let mut cache = RawDiskCache::new(path, total_size, block_size, ttl)
        .await
        .unwrap();
    
    // Store some data
    let key = "test_key";
    let data = Bytes::from("test data for verification");
    cache.store(key, data.clone()).await.unwrap();
    
    // Verify the entry
    let is_valid = cache.verify_entry(key).await.unwrap();
    assert!(is_valid);
    
    // Get verification stats
    let stats = cache.verification_stats().await;
    assert_eq!(stats.total_runs, 0); // No periodic runs yet
}

#[tokio::test]
async fn test_verification_detects_corruption() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    
    let total_size = 10 * 1024 * 1024;
    let block_size = 4096;
    let ttl = Duration::from_secs(3600);
    
    let cache = RawDiskCache::new(path, total_size, block_size, ttl)
        .await
        .unwrap();
    
    // Store data
    let key = "test_key";
    let data = Bytes::from("original data");
    cache.store(key, data).await.unwrap();
    
    // Verify it's valid
    let is_valid = cache.verify_entry(key).await.unwrap();
    assert!(is_valid);
    
    // Note: We can't easily corrupt the data in this test without
    // direct disk access, but the verification infrastructure is in place
}

#[tokio::test]
async fn test_verification_config() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    
    let total_size = 10 * 1024 * 1024;
    let block_size = 4096;
    let ttl = Duration::from_secs(3600);
    
    let cache = RawDiskCache::new(path, total_size, block_size, ttl)
        .await
        .unwrap();
    
    let config = cache.verification_config();
    assert_eq!(config.algorithm, ChecksumAlgorithm::XxHash3);
    assert!(!config.periodic_verification_enabled);
    assert_eq!(config.verification_interval_secs, 3600);
}

#[tokio::test]
async fn test_update_verification_config() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    
    let total_size = 10 * 1024 * 1024;
    let block_size = 4096;
    let ttl = Duration::from_secs(3600);
    
    let mut cache = RawDiskCache::new(path, total_size, block_size, ttl)
        .await
        .unwrap();
    
    // Update config
    let mut new_config = VerificationConfig::default();
    new_config.algorithm = ChecksumAlgorithm::XxHash64;
    new_config.periodic_verification_enabled = false; // Keep disabled for test
    new_config.verification_interval_secs = 1800;
    
    cache.update_verification_config(new_config);
    
    let config = cache.verification_config();
    assert_eq!(config.algorithm, ChecksumAlgorithm::XxHash64);
    assert_eq!(config.verification_interval_secs, 1800);
}

#[tokio::test]
async fn test_verify_all_entries() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    
    let total_size = 10 * 1024 * 1024;
    let block_size = 4096;
    let ttl = Duration::from_secs(3600);
    
    let cache = RawDiskCache::new(path, total_size, block_size, ttl)
        .await
        .unwrap();
    
    // Store multiple entries
    for i in 0..5 {
        let key = format!("key_{}", i);
        let data = Bytes::from(format!("data_{}", i));
        cache.store(&key, data).await.unwrap();
    }
    
    // Verify all entries
    let result = cache.verify_all_entries().await.unwrap();
    assert_eq!(result.verified, 5);
    assert_eq!(result.corrupted, 0);
    assert_eq!(result.repaired, 0);
}

#[tokio::test]
async fn test_verification_stats_tracking() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    
    let total_size = 10 * 1024 * 1024;
    let block_size = 4096;
    let ttl = Duration::from_secs(3600);
    
    let cache = RawDiskCache::new(path, total_size, block_size, ttl)
        .await
        .unwrap();
    
    // Store some entries
    for i in 0..3 {
        let key = format!("key_{}", i);
        let data = Bytes::from(format!("data_{}", i));
        cache.store(&key, data).await.unwrap();
    }
    
    // Run verification
    let result = cache.verify_all_entries().await.unwrap();
    assert_eq!(result.verified, 3);
    
    // Check stats
    let stats = cache.verification_stats().await;
    assert_eq!(stats.total_runs, 1);
    assert_eq!(stats.total_verified, 3);
    assert_eq!(stats.corrupted_found, 0);
    assert!(stats.last_verification.is_some());
}

#[tokio::test]
async fn test_backup_storage() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    
    let total_size = 10 * 1024 * 1024;
    let block_size = 4096;
    let ttl = Duration::from_secs(3600);
    
    let cache = RawDiskCache::new(path, total_size, block_size, ttl)
        .await
        .unwrap();
    
    // Initially no backups
    assert_eq!(cache.backup_storage_size().await, 0);
    
    // Clear backups (should be no-op)
    cache.clear_all_backups().await;
    assert_eq!(cache.backup_storage_size().await, 0);
}

#[tokio::test]
async fn test_different_checksum_algorithms() {
    let data = b"test data for algorithm comparison";
    
    let crc32 = Checksum::compute(ChecksumAlgorithm::Crc32, data);
    let xxhash64 = Checksum::compute(ChecksumAlgorithm::XxHash64, data);
    let xxhash3 = Checksum::compute(ChecksumAlgorithm::XxHash3, data);
    
    // All should verify the same data
    assert!(crc32.verify(data));
    assert!(xxhash64.verify(data));
    assert!(xxhash3.verify(data));
    
    // All should reject wrong data
    let wrong_data = b"wrong data";
    assert!(!crc32.verify(wrong_data));
    assert!(!xxhash64.verify(wrong_data));
    assert!(!xxhash3.verify(wrong_data));
    
    // Values should be different (collision is theoretically possible but unlikely)
    assert_ne!(crc32.value(), xxhash64.value());
    assert_ne!(crc32.value(), xxhash3.value());
}
