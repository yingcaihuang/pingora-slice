//! Example demonstrating enhanced checksum and data verification features
//!
//! This example shows how to:
//! - Use different checksum algorithms (CRC32, XXHash64, XXHash3)
//! - Enable periodic data verification
//! - Manually verify cache entries
//! - Configure automatic repair
//! - Monitor verification statistics

use bytes::Bytes;
use pingora_slice::raw_disk::{
    ChecksumAlgorithm, RawDiskCache, VerificationConfig,
};
use std::time::Duration;
use tempfile::NamedTempFile;
use tracing::{info, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("=== Enhanced Checksum Example ===");

    // Create a temporary file for the cache
    let temp_file = NamedTempFile::new()?;
    let path = temp_file.path();

    let total_size = 100 * 1024 * 1024; // 100MB
    let block_size = 4096;
    let ttl = Duration::from_secs(3600);

    info!("Creating cache at: {}", path.display());

    // Create cache with default verification config (XXHash3)
    let mut cache = RawDiskCache::new(path, total_size, block_size, ttl).await?;

    info!("Cache created successfully");

    // Display initial verification config
    let config = cache.verification_config();
    info!("Verification algorithm: {}", config.algorithm);
    info!("Periodic verification: {}", config.periodic_verification_enabled);
    info!("Verification interval: {}s", config.verification_interval_secs);

    // Store some test data
    info!("\n=== Storing Test Data ===");
    for i in 0..10 {
        let key = format!("test_key_{}", i);
        let data = Bytes::from(format!("Test data for entry {}", i));
        cache.store(&key, data).await?;
        info!("Stored: {}", key);
    }

    // Verify a single entry
    info!("\n=== Verifying Single Entry ===");
    let key = "test_key_0";
    let is_valid = cache.verify_entry(key).await?;
    info!("Entry '{}' is valid: {}", key, is_valid);

    // Verify all entries
    info!("\n=== Verifying All Entries ===");
    let result = cache.verify_all_entries().await?;
    info!("Verification complete:");
    info!("  Verified: {}", result.verified);
    info!("  Corrupted: {}", result.corrupted);
    info!("  Repaired: {}", result.repaired);
    info!("  Duration: {:?}", result.duration);

    // Get verification statistics
    info!("\n=== Verification Statistics ===");
    let stats = cache.verification_stats().await;
    info!("Total runs: {}", stats.total_runs);
    info!("Total verified: {}", stats.total_verified);
    info!("Corrupted found: {}", stats.corrupted_found);
    info!("Repaired: {}", stats.repaired);
    info!("Repair failed: {}", stats.repair_failed);
    info!("Corruption rate: {:.4}%", stats.corruption_rate() * 100.0);
    info!("Repair success rate: {:.2}%", stats.repair_success_rate() * 100.0);

    // Demonstrate different checksum algorithms
    info!("\n=== Checksum Algorithm Comparison ===");
    let test_data = b"Sample data for checksum comparison";
    
    use pingora_slice::raw_disk::Checksum;
    
    let crc32 = Checksum::compute(ChecksumAlgorithm::Crc32, test_data);
    let xxhash64 = Checksum::compute(ChecksumAlgorithm::XxHash64, test_data);
    let xxhash3 = Checksum::compute(ChecksumAlgorithm::XxHash3, test_data);
    
    info!("CRC32:    {} (value: {})", crc32.algorithm(), crc32.value());
    info!("XXHash64: {} (value: {})", xxhash64.algorithm(), xxhash64.value());
    info!("XXHash3:  {} (value: {})", xxhash3.algorithm(), xxhash3.value());

    // Update verification config to use XXHash64
    info!("\n=== Updating Verification Config ===");
    let mut new_config = VerificationConfig::default();
    new_config.algorithm = ChecksumAlgorithm::XxHash64;
    new_config.periodic_verification_enabled = false; // Keep disabled for example
    new_config.verification_interval_secs = 1800; // 30 minutes
    new_config.auto_repair_enabled = true;
    new_config.max_entries_per_run = 50;
    
    cache.update_verification_config(new_config);
    
    let updated_config = cache.verification_config();
    info!("Updated algorithm: {}", updated_config.algorithm);
    info!("Auto repair enabled: {}", updated_config.auto_repair_enabled);
    info!("Max entries per run: {}", updated_config.max_entries_per_run);

    // Store more data with new algorithm
    info!("\n=== Storing More Data with XXHash64 ===");
    for i in 10..15 {
        let key = format!("test_key_{}", i);
        let data = Bytes::from(format!("Test data for entry {}", i));
        cache.store(&key, data).await?;
        info!("Stored: {}", key);
    }

    // Verify all entries again
    info!("\n=== Final Verification ===");
    let final_result = cache.verify_all_entries().await?;
    info!("Final verification complete:");
    info!("  Verified: {}", final_result.verified);
    info!("  Corrupted: {}", final_result.corrupted);
    info!("  Repaired: {}", final_result.repaired);
    info!("  Duration: {:?}", final_result.duration);

    // Get final statistics
    info!("\n=== Final Statistics ===");
    let final_stats = cache.verification_stats().await;
    info!("Total runs: {}", final_stats.total_runs);
    info!("Total verified: {}", final_stats.total_verified);
    info!("Total verification time: {}ms", final_stats.total_verification_time_ms);

    // Get cache statistics including verification
    info!("\n=== Cache Statistics ===");
    let cache_stats = cache.stats().await;
    info!("Total entries: {}", cache_stats.entries);
    info!("Cache hits: {}", cache_stats.hits);
    info!("Cache misses: {}", cache_stats.misses);
    
    if let Some(ver_stats) = cache_stats.verification_stats {
        info!("Verification runs: {}", ver_stats.total_runs);
        info!("Entries verified: {}", ver_stats.total_verified);
    }

    info!("\n=== Example Complete ===");
    info!("Enhanced checksum and verification features demonstrated successfully!");

    Ok(())
}
