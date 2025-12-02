//! Cache directory for metadata management

use super::{DiskLocation, RawDiskError};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

/// Serializable metadata structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMetadata {
    /// Version of metadata format
    pub version: u32,
    
    /// Cache entries (using BTreeMap for deterministic serialization)
    pub entries: std::collections::BTreeMap<String, DiskLocation>,
    
    /// LRU order
    pub lru_order: Vec<String>,
    
    /// Statistics
    pub hits: u64,
    pub misses: u64,
    
    /// Checksum of metadata
    pub checksum: u32,
}

/// Cache directory manages metadata
pub struct CacheDirectory {
    index: HashMap<String, DiskLocation>,
    lru: VecDeque<String>,
    hits: u64,
    misses: u64,
}

impl CacheDirectory {
    pub fn new() -> Self {
        Self {
            index: HashMap::new(),
            lru: VecDeque::new(),
            hits: 0,
            misses: 0,
        }
    }
    
    pub fn insert(&mut self, key: String, location: DiskLocation) {
        self.index.insert(key.clone(), location);
        self.lru.push_back(key);
    }
    
    pub fn get(&self, key: &str) -> Option<&DiskLocation> {
        if let Some(loc) = self.index.get(key) {
            Some(loc)
        } else {
            None
        }
    }
    
    pub fn remove(&mut self, key: &str) -> Option<DiskLocation> {
        if let Some(loc) = self.index.remove(key) {
            self.lru.retain(|k| k != key);
            Some(loc)
        } else {
            None
        }
    }
    
    pub fn touch(&mut self, key: &str) {
        if self.index.contains_key(key) {
            self.hits += 1;
            // Move to back of LRU
            self.lru.retain(|k| k != key);
            self.lru.push_back(key.to_string());
        } else {
            self.misses += 1;
        }
    }
    
    pub fn select_lru_victims(&self, ratio: f64) -> Vec<String> {
        let count = (self.index.len() as f64 * ratio).ceil() as usize;
        self.lru.iter().take(count).cloned().collect()
    }
    
    pub fn len(&self) -> usize {
        self.index.len()
    }
    
    pub fn hits(&self) -> u64 {
        self.hits
    }
    
    pub fn misses(&self) -> u64 {
        self.misses
    }
    
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &DiskLocation)> {
        self.index.iter()
    }
    
    /// Serialize metadata to bytes
    pub fn serialize(&self) -> Result<Vec<u8>, RawDiskError> {
        // Create metadata without checksum (convert HashMap to BTreeMap for deterministic order)
        let metadata_without_checksum = CacheMetadata {
            version: 1,
            entries: self.index.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            lru_order: self.lru.iter().cloned().collect(),
            hits: self.hits,
            misses: self.misses,
            checksum: 0,
        };
        
        // Serialize to calculate checksum
        let data_for_checksum = bincode::serialize(&metadata_without_checksum)
            .map_err(|e| RawDiskError::SerializationError(e.to_string()))?;
        
        // Calculate checksum of the data
        let checksum = crc32fast::hash(&data_for_checksum);
        
        // Create final metadata with checksum
        let metadata = CacheMetadata {
            checksum,
            ..metadata_without_checksum
        };
        
        // Serialize final version
        bincode::serialize(&metadata)
            .map_err(|e| RawDiskError::SerializationError(e.to_string()))
    }
    
    /// Deserialize metadata from bytes
    pub fn deserialize(data: &[u8]) -> Result<Self, RawDiskError> {
        let metadata: CacheMetadata = bincode::deserialize(data)
            .map_err(|e| RawDiskError::SerializationError(e.to_string()))?;
        
        // Verify version
        if metadata.version != 1 {
            return Err(RawDiskError::InvalidMetadata(format!(
                "Unsupported metadata version: {}",
                metadata.version
            )));
        }
        
        // Verify checksum by re-serializing without checksum
        let metadata_for_check = CacheMetadata {
            version: metadata.version,
            entries: metadata.entries.clone(),
            lru_order: metadata.lru_order.clone(),
            hits: metadata.hits,
            misses: metadata.misses,
            checksum: 0,
        };
        
        let data_for_check = bincode::serialize(&metadata_for_check)
            .map_err(|e| RawDiskError::SerializationError(e.to_string()))?;
        let calculated_checksum = crc32fast::hash(&data_for_check);
        
        if calculated_checksum != metadata.checksum {
            return Err(RawDiskError::ChecksumMismatch);
        }
        
        // Reconstruct directory (convert BTreeMap back to HashMap)
        Ok(Self {
            index: metadata.entries.into_iter().collect(),
            lru: metadata.lru_order.into_iter().collect(),
            hits: metadata.hits,
            misses: metadata.misses,
        })
    }
    
    /// Get metadata size estimate
    pub fn metadata_size_estimate(&self) -> usize {
        // Rough estimate: 100 bytes per entry + overhead
        self.index.len() * 100 + 1024
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_directory_serialization() {
        let mut dir = CacheDirectory::new();
        
        // Add some entries
        let loc1 = DiskLocation::new(1000, b"data1");
        let loc2 = DiskLocation::new(2000, b"data2");
        let loc3 = DiskLocation::new(3000, b"data3");
        
        dir.insert("key1".to_string(), loc1);
        dir.insert("key2".to_string(), loc2);
        dir.insert("key3".to_string(), loc3);
        
        // Touch some keys to update stats
        dir.touch("key1");
        dir.touch("key2");
        dir.touch("nonexistent");
        
        // Serialize
        let serialized = dir.serialize().unwrap();
        assert!(serialized.len() > 0);
        
        // Deserialize
        let restored = CacheDirectory::deserialize(&serialized).unwrap();
        
        // Verify
        assert_eq!(restored.len(), 3);
        assert_eq!(restored.hits(), 2);
        assert_eq!(restored.misses(), 1);
        
        // Verify entries
        assert!(restored.get("key1").is_some());
        assert!(restored.get("key2").is_some());
        assert!(restored.get("key3").is_some());
        assert!(restored.get("nonexistent").is_none());
    }
    
    #[test]
    fn test_metadata_checksum_validation() {
        let mut dir = CacheDirectory::new();
        dir.insert("key1".to_string(), DiskLocation::new(1000, b"data"));
        
        let mut serialized = dir.serialize().unwrap();
        
        // Corrupt the data
        if serialized.len() > 10 {
            serialized[10] ^= 0xFF;
        }
        
        // Should fail checksum validation
        let result = CacheDirectory::deserialize(&serialized);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_empty_directory_serialization() {
        let dir = CacheDirectory::new();
        
        let serialized = dir.serialize().unwrap();
        let restored = CacheDirectory::deserialize(&serialized).unwrap();
        
        assert_eq!(restored.len(), 0);
        assert_eq!(restored.hits(), 0);
        assert_eq!(restored.misses(), 0);
    }
    
    #[test]
    fn test_large_directory_serialization() {
        let mut dir = CacheDirectory::new();
        
        // Add many entries
        for i in 0..1000 {
            let key = format!("key_{}", i);
            let data = format!("data_{}", i);
            let loc = DiskLocation::new(i * 1000, data.as_bytes());
            dir.insert(key, loc);
        }
        
        let serialized = dir.serialize().unwrap();
        let restored = CacheDirectory::deserialize(&serialized).unwrap();
        
        assert_eq!(restored.len(), 1000);
        
        // Verify some entries
        assert!(restored.get("key_0").is_some());
        assert!(restored.get("key_500").is_some());
        assert!(restored.get("key_999").is_some());
    }
}
