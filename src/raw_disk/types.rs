//! Common types for raw disk cache

use crc32fast::Hasher;
use std::time::{SystemTime, UNIX_EPOCH};

/// Magic number for superblock identification
pub const MAGIC_NUMBER: u32 = 0x50494E47; // "PING"

/// Current version
pub const VERSION: u32 = 1;

/// Disk location information
#[derive(Debug, Clone)]
pub struct DiskLocation {
    /// Offset in bytes from start of disk
    pub offset: u64,
    
    /// Size of data in bytes
    pub size: u32,
    
    /// CRC32 checksum
    pub checksum: u32,
    
    /// Unix timestamp
    pub timestamp: u64,
}

impl DiskLocation {
    /// Create a new disk location
    pub fn new(offset: u64, data: &[u8]) -> Self {
        let mut hasher = Hasher::new();
        hasher.update(data);
        let checksum = hasher.finalize();
        
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Self {
            offset,
            size: data.len() as u32,
            checksum,
            timestamp,
        }
    }
    
    /// Verify checksum of data
    pub fn verify_checksum(&self, data: &[u8]) -> bool {
        let mut hasher = Hasher::new();
        hasher.update(data);
        hasher.finalize() == self.checksum
    }
    
    /// Check if entry is expired
    pub fn is_expired(&self, ttl_secs: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        now - self.timestamp > ttl_secs
    }
}

/// Block header stored on disk
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BlockHeader {
    /// Magic number for validation
    pub magic: u32,
    
    /// Hash of the key
    pub key_hash: u64,
    
    /// Size of data
    pub data_size: u32,
    
    /// CRC32 checksum
    pub checksum: u32,
    
    /// Timestamp
    pub timestamp: u64,
    
    /// Next block offset (for large objects)
    pub next_block: u64,
    
    /// Reserved for future use
    pub reserved: [u8; 28],
}

impl BlockHeader {
    pub const SIZE: usize = 64;
    
    pub fn new(key_hash: u64, data_size: u32, checksum: u32) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Self {
            magic: MAGIC_NUMBER,
            key_hash,
            data_size,
            checksum,
            timestamp,
            next_block: 0,
            reserved: [0; 28],
        }
    }
    
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(Self::SIZE);
        bytes.extend_from_slice(&self.magic.to_le_bytes());
        bytes.extend_from_slice(&self.key_hash.to_le_bytes());
        bytes.extend_from_slice(&self.data_size.to_le_bytes());
        bytes.extend_from_slice(&self.checksum.to_le_bytes());
        bytes.extend_from_slice(&self.timestamp.to_le_bytes());
        bytes.extend_from_slice(&self.next_block.to_le_bytes());
        bytes.extend_from_slice(&self.reserved);
        bytes
    }
    
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::SIZE {
            return None;
        }
        
        let magic = u32::from_le_bytes(bytes[0..4].try_into().ok()?);
        if magic != MAGIC_NUMBER {
            return None;
        }
        
        let key_hash = u64::from_le_bytes(bytes[4..12].try_into().ok()?);
        let data_size = u32::from_le_bytes(bytes[12..16].try_into().ok()?);
        let checksum = u32::from_le_bytes(bytes[16..20].try_into().ok()?);
        let timestamp = u64::from_le_bytes(bytes[20..28].try_into().ok()?);
        let next_block = u64::from_le_bytes(bytes[28..36].try_into().ok()?);
        
        let mut reserved = [0u8; 28];
        reserved.copy_from_slice(&bytes[36..64]);
        
        Some(Self {
            magic,
            key_hash,
            data_size,
            checksum,
            timestamp,
            next_block,
            reserved,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_disk_location() {
        let data = b"test data";
        let loc = DiskLocation::new(1024, data);
        
        assert_eq!(loc.offset, 1024);
        assert_eq!(loc.size, 9);
        assert!(loc.verify_checksum(data));
        assert!(!loc.verify_checksum(b"wrong data"));
    }
    
    #[test]
    fn test_block_header_serialization() {
        let header = BlockHeader::new(12345, 1024, 0xABCDEF);
        let bytes = header.to_bytes();
        
        assert_eq!(bytes.len(), BlockHeader::SIZE);
        
        let decoded = BlockHeader::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.magic, MAGIC_NUMBER);
        assert_eq!(decoded.key_hash, 12345);
        assert_eq!(decoded.data_size, 1024);
        assert_eq!(decoded.checksum, 0xABCDEF);
    }
}
