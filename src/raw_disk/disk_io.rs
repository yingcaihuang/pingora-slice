//! Disk I/O manager for raw disk operations

use super::RawDiskError;
use bytes::Bytes;
use std::path::Path;
use std::sync::Arc;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, SeekFrom};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

#[cfg(target_os = "linux")]
use std::os::unix::fs::OpenOptionsExt;

#[cfg(target_os = "linux")]
use nix::fcntl::OFlag;

/// Disk I/O manager handles low-level disk operations
pub struct DiskIOManager {
    file: Arc<Mutex<File>>,
    block_size: usize,
    use_direct_io: bool,
    alignment: usize,
}

impl DiskIOManager {
    /// Create a new disk I/O manager
    pub async fn new(
        device_path: impl AsRef<Path>, 
        block_size: usize
    ) -> Result<Self, RawDiskError> {
        Self::new_with_options(device_path, block_size, true).await
    }
    
    /// Create a new disk I/O manager with explicit O_DIRECT control
    pub async fn new_with_options(
        device_path: impl AsRef<Path>, 
        block_size: usize,
        try_direct_io: bool,
    ) -> Result<Self, RawDiskError> {
        let path = device_path.as_ref();
        
        debug!("Opening disk device: {}", path.display());
        
        // Detect O_DIRECT support
        let (use_direct_io, alignment) = if try_direct_io {
            Self::detect_direct_io_support(path).await
        } else {
            (false, 512)
        };
        
        if use_direct_io {
            info!("O_DIRECT enabled with alignment: {} bytes", alignment);
        } else {
            info!("O_DIRECT disabled, using buffered I/O");
        }
        
        // Open file with appropriate flags
        let file = Self::open_file(path, use_direct_io).await?;
        
        Ok(Self {
            file: Arc::new(Mutex::new(file)),
            block_size,
            use_direct_io,
            alignment,
        })
    }
    
    /// Detect if O_DIRECT is supported on this system
    #[cfg(target_os = "linux")]
    async fn detect_direct_io_support(path: &Path) -> (bool, usize) {
        // Try to open with O_DIRECT
        let test_path = if path.exists() {
            path.to_path_buf()
        } else {
            // Create a temporary file for testing
            path.to_path_buf()
        };
        
        // Try opening with O_DIRECT
        let mut opts = OpenOptions::new();
        opts.read(true).write(true);
        
        if !test_path.exists() {
            opts.create(true);
        }
        
        // Add O_DIRECT flag
        opts.custom_flags(libc::O_DIRECT);
        
        match opts.open(&test_path).await {
            Ok(_) => {
                info!("O_DIRECT is supported on this system");
                // Most systems use 512-byte alignment, but some use 4096
                let alignment = Self::detect_alignment();
                (true, alignment)
            }
            Err(e) => {
                warn!("O_DIRECT not supported: {}", e);
                (false, 512)
            }
        }
    }
    
    /// Detect if O_DIRECT is supported on non-Linux systems
    #[cfg(not(target_os = "linux"))]
    async fn detect_direct_io_support(_path: &Path) -> (bool, usize) {
        warn!("O_DIRECT is only supported on Linux");
        (false, 512)
    }
    
    /// Detect the required alignment for O_DIRECT
    #[cfg(target_os = "linux")]
    fn detect_alignment() -> usize {
        // Try to get the logical block size from the system
        // Most modern systems use 512 bytes, but some use 4096
        // For safety, we'll use 4096 which works for both
        4096
    }
    
    /// Open file with appropriate flags
    #[cfg(target_os = "linux")]
    async fn open_file(path: &Path, use_direct_io: bool) -> Result<File, RawDiskError> {
        let mut opts = OpenOptions::new();
        opts.read(true).write(true);
        
        if !path.exists() {
            warn!("Device {} not found, creating file for testing", path.display());
            opts.create(true);
        }
        
        if use_direct_io {
            opts.custom_flags(libc::O_DIRECT);
        }
        
        Ok(opts.open(path).await?)
    }
    
    /// Open file with appropriate flags (non-Linux)
    #[cfg(not(target_os = "linux"))]
    async fn open_file(path: &Path, _use_direct_io: bool) -> Result<File, RawDiskError> {
        let mut opts = OpenOptions::new();
        opts.read(true).write(true);
        
        if !path.exists() {
            warn!("Device {} not found, creating file for testing", path.display());
            opts.create(true);
        }
        
        Ok(opts.open(path).await?)
    }
    
    /// Read data from disk at offset
    pub async fn read_at(&self, offset: u64, size: usize) -> Result<Bytes, RawDiskError> {
        debug!("Reading {} bytes at offset {}", size, offset);
        
        if self.use_direct_io {
            self.read_at_direct(offset, size).await
        } else {
            self.read_at_buffered(offset, size).await
        }
    }
    
    /// Read data using buffered I/O
    async fn read_at_buffered(&self, offset: u64, size: usize) -> Result<Bytes, RawDiskError> {
        let mut file = self.file.lock().await;
        
        // Seek to position
        file.seek(SeekFrom::Start(offset)).await?;
        
        // Read data
        let mut buffer = vec![0u8; size];
        file.read_exact(&mut buffer).await?;
        
        Ok(Bytes::from(buffer))
    }
    
    /// Read data using O_DIRECT
    async fn read_at_direct(&self, offset: u64, size: usize) -> Result<Bytes, RawDiskError> {
        // Align offset and size
        let aligned_offset = self.align_down(offset);
        let offset_adjustment = (offset - aligned_offset) as usize;
        let aligned_size = self.align_up((offset_adjustment + size) as u64) as usize;
        
        // Allocate aligned buffer
        let mut aligned_buffer = self.allocate_aligned(aligned_size);
        
        let mut file = self.file.lock().await;
        
        // Seek to aligned position
        file.seek(SeekFrom::Start(aligned_offset)).await?;
        
        // Read aligned data
        file.read_exact(&mut aligned_buffer).await?;
        
        // Extract the requested portion
        let data = aligned_buffer[offset_adjustment..offset_adjustment + size].to_vec();
        
        Ok(Bytes::from(data))
    }
    
    /// Write data to disk at offset
    pub async fn write_at(&self, offset: u64, data: &[u8]) -> Result<(), RawDiskError> {
        debug!("Writing {} bytes at offset {}", data.len(), offset);
        
        if self.use_direct_io {
            self.write_at_direct(offset, data).await
        } else {
            self.write_at_buffered(offset, data).await
        }
    }
    
    /// Write data using buffered I/O
    async fn write_at_buffered(&self, offset: u64, data: &[u8]) -> Result<(), RawDiskError> {
        let mut file = self.file.lock().await;
        
        // Seek to position
        file.seek(SeekFrom::Start(offset)).await?;
        
        // Write data
        file.write_all(data).await?;
        
        // Sync to disk
        file.sync_data().await?;
        
        Ok(())
    }
    
    /// Write data using O_DIRECT
    async fn write_at_direct(&self, offset: u64, data: &[u8]) -> Result<(), RawDiskError> {
        // For O_DIRECT writes, we need aligned offset, size, and buffer
        let aligned_offset = self.align_down(offset);
        let offset_adjustment = (offset - aligned_offset) as usize;
        let total_size = offset_adjustment + data.len();
        let aligned_size = self.align_up(total_size as u64) as usize;
        
        // Allocate aligned buffer
        let mut aligned_buffer = self.allocate_aligned(aligned_size);
        
        // If we're not writing at an aligned offset, we need to read-modify-write
        if offset_adjustment > 0 {
            let mut file = self.file.lock().await;
            file.seek(SeekFrom::Start(aligned_offset)).await?;
            // Read existing data for the first partial block
            let read_size = std::cmp::min(self.alignment, aligned_size);
            file.read_exact(&mut aligned_buffer[..read_size]).await?;
            drop(file);
        }
        
        // Copy data into aligned buffer
        aligned_buffer[offset_adjustment..offset_adjustment + data.len()].copy_from_slice(data);
        
        // If the data doesn't fill the last block, read it first
        if total_size % self.alignment != 0 && aligned_size > self.alignment {
            let last_block_offset = aligned_offset + ((aligned_size - self.alignment) as u64);
            let mut file = self.file.lock().await;
            file.seek(SeekFrom::Start(last_block_offset)).await?;
            let last_block_start = aligned_size - self.alignment;
            // Try to read, but don't fail if we're at the end of file
            let _ = file.read_exact(&mut aligned_buffer[last_block_start..]).await;
            drop(file);
            // Re-copy our data in case we overwrote it
            aligned_buffer[offset_adjustment..offset_adjustment + data.len()].copy_from_slice(data);
        }
        
        let mut file = self.file.lock().await;
        
        // Seek to aligned position
        file.seek(SeekFrom::Start(aligned_offset)).await?;
        
        // Write aligned data
        file.write_all(&aligned_buffer).await?;
        
        // With O_DIRECT, sync is not strictly necessary but we'll keep it for consistency
        file.sync_data().await?;
        
        Ok(())
    }
    
    /// Sync data to disk
    pub async fn sync(&self) -> Result<(), RawDiskError> {
        let file = self.file.lock().await;
        file.sync_data().await?;
        Ok(())
    }
    
    /// Get file size
    pub async fn size(&self) -> Result<u64, RawDiskError> {
        let file = self.file.lock().await;
        let metadata = file.metadata().await?;
        Ok(metadata.len())
    }
    
    /// Allocate an aligned buffer for O_DIRECT
    fn allocate_aligned(&self, size: usize) -> Vec<u8> {
        // Allocate extra space for alignment
        let total_size = size + self.alignment;
        let mut buffer = vec![0u8; total_size];
        
        // Find aligned offset within buffer
        let ptr = buffer.as_ptr() as usize;
        let aligned_ptr = (ptr + self.alignment - 1) / self.alignment * self.alignment;
        let offset = aligned_ptr - ptr;
        
        // Return aligned portion
        buffer.drain(..offset);
        buffer.truncate(size);
        buffer
    }
    
    /// Align offset down to alignment boundary
    fn align_down(&self, offset: u64) -> u64 {
        (offset / self.alignment as u64) * self.alignment as u64
    }
    
    /// Align size up to alignment boundary
    fn align_up(&self, size: u64) -> u64 {
        ((size + self.alignment as u64 - 1) / self.alignment as u64) * self.alignment as u64
    }
    
    /// Prefetch data (hint to OS)
    pub async fn prefetch(&self, _offset: u64, _size: usize) -> Result<(), RawDiskError> {
        // TODO: Implement prefetch using posix_fadvise or similar
        Ok(())
    }
    
    /// Get block size
    pub fn block_size(&self) -> usize {
        self.block_size
    }
    
    /// Check if O_DIRECT is enabled
    pub fn is_direct_io_enabled(&self) -> bool {
        self.use_direct_io
    }
    
    /// Get alignment requirement
    pub fn alignment(&self) -> usize {
        self.alignment
    }
}
