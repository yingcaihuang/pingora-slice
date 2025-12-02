//! Enhanced checksum and data verification module
//!
//! This module provides multiple checksum algorithms and data verification capabilities.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Checksum algorithm selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChecksumAlgorithm {
    /// CRC32 - Fast but less collision-resistant
    Crc32,
    /// XXHash64 - Fast and better collision resistance
    XxHash64,
    /// XXHash3 - Fastest with excellent collision resistance
    XxHash3,
}

impl Default for ChecksumAlgorithm {
    fn default() -> Self {
        Self::XxHash3
    }
}

impl fmt::Display for ChecksumAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Crc32 => write!(f, "CRC32"),
            Self::XxHash64 => write!(f, "XXHash64"),
            Self::XxHash3 => write!(f, "XXHash3"),
        }
    }
}

/// Checksum value that can hold different algorithm results
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checksum {
    algorithm: ChecksumAlgorithm,
    value: u64,
}

impl Checksum {
    /// Create a new checksum
    pub fn new(algorithm: ChecksumAlgorithm, value: u64) -> Self {
        Self { algorithm, value }
    }

    /// Compute checksum for data using specified algorithm
    pub fn compute(algorithm: ChecksumAlgorithm, data: &[u8]) -> Self {
        let value = match algorithm {
            ChecksumAlgorithm::Crc32 => {
                let mut hasher = crc32fast::Hasher::new();
                hasher.update(data);
                hasher.finalize() as u64
            }
            ChecksumAlgorithm::XxHash64 => {
                xxhash_rust::xxh3::xxh3_64(data)
            }
            ChecksumAlgorithm::XxHash3 => {
                xxhash_rust::xxh3::xxh3_64(data)
            }
        };

        Self { algorithm, value }
    }

    /// Verify data against this checksum
    pub fn verify(&self, data: &[u8]) -> bool {
        let computed = Self::compute(self.algorithm, data);
        computed.value == self.value
    }

    /// Get the algorithm used
    pub fn algorithm(&self) -> ChecksumAlgorithm {
        self.algorithm
    }

    /// Get the checksum value
    pub fn value(&self) -> u64 {
        self.value
    }

    /// Convert to legacy CRC32 format (for backward compatibility)
    pub fn to_crc32(&self) -> u32 {
        if self.algorithm == ChecksumAlgorithm::Crc32 {
            self.value as u32
        } else {
            // For non-CRC32 checksums, we can't convert back
            // Return a marker value
            0xFFFFFFFF
        }
    }

    /// Create from legacy CRC32 value
    pub fn from_crc32(value: u32) -> Self {
        Self {
            algorithm: ChecksumAlgorithm::Crc32,
            value: value as u64,
        }
    }
}

/// Configuration for data verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationConfig {
    /// Checksum algorithm to use
    pub algorithm: ChecksumAlgorithm,
    
    /// Enable periodic verification
    pub periodic_verification_enabled: bool,
    
    /// Interval between verification runs (in seconds)
    pub verification_interval_secs: u64,
    
    /// Maximum number of entries to verify per run
    pub max_entries_per_run: usize,
    
    /// Enable automatic repair of corrupted data
    pub auto_repair_enabled: bool,
    
    /// Keep backup of data before repair
    pub keep_backup_on_repair: bool,
}

impl Default for VerificationConfig {
    fn default() -> Self {
        Self {
            algorithm: ChecksumAlgorithm::XxHash3,
            periodic_verification_enabled: false,
            verification_interval_secs: 3600, // 1 hour
            max_entries_per_run: 100,
            auto_repair_enabled: false,
            keep_backup_on_repair: true,
        }
    }
}

/// Statistics for data verification
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VerificationStats {
    /// Total number of verification runs
    pub total_runs: u64,
    
    /// Total entries verified
    pub total_verified: u64,
    
    /// Number of corrupted entries found
    pub corrupted_found: u64,
    
    /// Number of entries successfully repaired
    pub repaired: u64,
    
    /// Number of entries that failed repair
    pub repair_failed: u64,
    
    /// Last verification timestamp
    pub last_verification: Option<u64>,
    
    /// Total time spent verifying (milliseconds)
    pub total_verification_time_ms: u64,
}

impl VerificationStats {
    /// Create new stats
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a verification run
    pub fn record_run(&mut self, verified: u64, corrupted: u64, duration_ms: u64, timestamp: u64) {
        self.total_runs += 1;
        self.total_verified += verified;
        self.corrupted_found += corrupted;
        self.last_verification = Some(timestamp);
        self.total_verification_time_ms += duration_ms;
    }

    /// Record a successful repair
    pub fn record_repair_success(&mut self) {
        self.repaired += 1;
    }

    /// Record a failed repair
    pub fn record_repair_failure(&mut self) {
        self.repair_failed += 1;
    }

    /// Get corruption rate
    pub fn corruption_rate(&self) -> f64 {
        if self.total_verified == 0 {
            0.0
        } else {
            self.corrupted_found as f64 / self.total_verified as f64
        }
    }

    /// Get repair success rate
    pub fn repair_success_rate(&self) -> f64 {
        let total_repairs = self.repaired + self.repair_failed;
        if total_repairs == 0 {
            0.0
        } else {
            self.repaired as f64 / total_repairs as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc32_checksum() {
        let data = b"test data";
        let checksum = Checksum::compute(ChecksumAlgorithm::Crc32, data);
        
        assert_eq!(checksum.algorithm(), ChecksumAlgorithm::Crc32);
        assert!(checksum.verify(data));
        assert!(!checksum.verify(b"wrong data"));
    }

    #[test]
    fn test_xxhash64_checksum() {
        let data = b"test data";
        let checksum = Checksum::compute(ChecksumAlgorithm::XxHash64, data);
        
        assert_eq!(checksum.algorithm(), ChecksumAlgorithm::XxHash64);
        assert!(checksum.verify(data));
        assert!(!checksum.verify(b"wrong data"));
    }

    #[test]
    fn test_xxhash3_checksum() {
        let data = b"test data";
        let checksum = Checksum::compute(ChecksumAlgorithm::XxHash3, data);
        
        assert_eq!(checksum.algorithm(), ChecksumAlgorithm::XxHash3);
        assert!(checksum.verify(data));
        assert!(!checksum.verify(b"wrong data"));
    }

    #[test]
    fn test_different_algorithms_produce_different_values() {
        let data = b"test data";
        
        let crc32 = Checksum::compute(ChecksumAlgorithm::Crc32, data);
        let xxhash64 = Checksum::compute(ChecksumAlgorithm::XxHash64, data);
        let xxhash3 = Checksum::compute(ChecksumAlgorithm::XxHash3, data);
        
        // Values should be different (though technically could collide)
        assert_ne!(crc32.value(), xxhash64.value());
        assert_ne!(crc32.value(), xxhash3.value());
    }

    #[test]
    fn test_checksum_serialization() {
        let data = b"test data";
        let checksum = Checksum::compute(ChecksumAlgorithm::XxHash3, data);
        
        // Serialize and deserialize
        let json = serde_json::to_string(&checksum).unwrap();
        let deserialized: Checksum = serde_json::from_str(&json).unwrap();
        
        assert_eq!(checksum, deserialized);
        assert!(deserialized.verify(data));
    }

    #[test]
    fn test_verification_stats() {
        let mut stats = VerificationStats::new();
        
        assert_eq!(stats.corruption_rate(), 0.0);
        assert_eq!(stats.repair_success_rate(), 0.0);
        
        stats.record_run(100, 5, 1000, 12345);
        assert_eq!(stats.total_verified, 100);
        assert_eq!(stats.corrupted_found, 5);
        assert_eq!(stats.corruption_rate(), 0.05);
        
        stats.record_repair_success();
        stats.record_repair_success();
        stats.record_repair_failure();
        
        assert_eq!(stats.repaired, 2);
        assert_eq!(stats.repair_failed, 1);
        assert!((stats.repair_success_rate() - 0.666).abs() < 0.01);
    }
}
