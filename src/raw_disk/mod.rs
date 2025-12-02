//! Raw Disk Cache Implementation
//!
//! This module implements a high-performance cache that directly manages
//! disk blocks without relying on the filesystem, similar to Apache Traffic Server.

pub mod allocator;
pub mod batch_io;
pub mod checksum;
pub mod compression;
pub mod defrag;
pub mod directory;
pub mod disk_io;
pub mod gc;
pub mod metrics;
pub mod prefetch;
pub mod superblock;
pub mod types;
pub mod verification;
pub mod zero_copy;

// io_uring support (Linux only)
#[cfg(target_os = "linux")]
pub mod io_uring;
#[cfg(target_os = "linux")]
pub mod io_uring_batch;

pub use allocator::BlockAllocator;
pub use batch_io::{BatchIOManager, BufferStats};
pub use checksum::{Checksum, ChecksumAlgorithm, VerificationConfig, VerificationStats};
pub use compression::{CompressionAlgorithm, CompressionConfig, CompressionManager, CompressionStats};
pub use defrag::{DefragConfig, DefragManager, DefragStats};
pub use directory::CacheDirectory;
pub use disk_io::DiskIOManager;
pub use gc::{EvictionStrategy, GCConfig, GCMetrics, GCTriggerConfig, SmartGC};
pub use metrics::{RawDiskMetrics, RawDiskMetricsSnapshot};
pub use prefetch::{AccessPattern, PrefetchConfig, PrefetchManager, PrefetchStats};
pub use superblock::Superblock;
pub use types::*;
pub use verification::{VerificationManager, VerificationResult};
pub use zero_copy::{ZeroCopyConfig, ZeroCopyManager, ZeroCopyStats};

#[cfg(target_os = "linux")]
pub use io_uring::{IoUringConfig, IoUringManager};
#[cfg(target_os = "linux")]
pub use io_uring_batch::IoUringBatchManager;

use bytes::Bytes;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// I/O backend selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IOBackend {
    /// Standard async I/O
    Standard,
    /// io_uring (Linux only)
    #[cfg(target_os = "linux")]
    IoUring,
}

/// Raw disk cache implementation
pub struct RawDiskCache {
    superblock: Arc<RwLock<Superblock>>,
    directory: Arc<RwLock<CacheDirectory>>,
    allocator: Arc<RwLock<BlockAllocator>>,
    disk_io: Arc<DiskIOManager>,
    batch_io: Arc<BatchIOManager>,
    #[cfg(target_os = "linux")]
    io_uring_batch: Option<Arc<IoUringBatchManager>>,
    prefetch_manager: Arc<PrefetchManager>,
    zero_copy_manager: Option<Arc<ZeroCopyManager>>,
    zero_copy_stats: Arc<RwLock<ZeroCopyStats>>,
    compression_manager: Arc<CompressionManager>,
    compression_stats: Arc<RwLock<CompressionStats>>,
    io_backend: IOBackend,
    ttl: Duration,
    smart_gc: Arc<RwLock<SmartGC>>,
    defrag_manager: Arc<RwLock<DefragManager>>,
    verification_manager: Arc<VerificationManager>,
    verification_task: Option<tokio::task::JoinHandle<()>>,
    metrics: Arc<RawDiskMetrics>,
}

impl RawDiskCache {
    /// Create a new raw disk cache
    pub async fn new(
        device_path: impl AsRef<Path>,
        total_size: u64,
        block_size: usize,
        ttl: Duration,
    ) -> Result<Self, RawDiskError> {
        Self::new_with_backend_and_prefetch(
            device_path,
            total_size,
            block_size,
            ttl,
            IOBackend::Standard,
            true,
            PrefetchConfig::default(),
        )
        .await
    }
    
    /// Create a new raw disk cache with explicit O_DIRECT control
    pub async fn new_with_options(
        device_path: impl AsRef<Path>,
        total_size: u64,
        block_size: usize,
        ttl: Duration,
        use_direct_io: bool,
    ) -> Result<Self, RawDiskError> {
        Self::new_with_backend_and_prefetch(
            device_path,
            total_size,
            block_size,
            ttl,
            IOBackend::Standard,
            use_direct_io,
            PrefetchConfig::default(),
        )
        .await
    }

    /// Create a new raw disk cache with prefetch configuration
    pub async fn new_with_prefetch(
        device_path: impl AsRef<Path>,
        total_size: u64,
        block_size: usize,
        ttl: Duration,
        prefetch_config: PrefetchConfig,
    ) -> Result<Self, RawDiskError> {
        Self::new_with_backend_and_prefetch(
            device_path,
            total_size,
            block_size,
            ttl,
            IOBackend::Standard,
            true,
            prefetch_config,
        )
        .await
    }
    
    /// Create a new raw disk cache with io_uring support (Linux only)
    #[cfg(target_os = "linux")]
    pub async fn new_with_io_uring(
        device_path: impl AsRef<Path>,
        total_size: u64,
        block_size: usize,
        ttl: Duration,
        io_uring_config: IoUringConfig,
    ) -> Result<Self, RawDiskError> {
        Self::new_with_backend_and_config(
            device_path,
            total_size,
            block_size,
            ttl,
            IOBackend::IoUring,
            true,
            Some(io_uring_config),
            PrefetchConfig::default(),
        )
        .await
    }

    /// Create a new raw disk cache with specified backend and prefetch
    async fn new_with_backend_and_prefetch(
        device_path: impl AsRef<Path>,
        total_size: u64,
        block_size: usize,
        ttl: Duration,
        io_backend: IOBackend,
        use_direct_io: bool,
        prefetch_config: PrefetchConfig,
    ) -> Result<Self, RawDiskError> {
        Self::new_with_backend_and_config(
            device_path,
            total_size,
            block_size,
            ttl,
            io_backend,
            use_direct_io,
            None,
            prefetch_config,
        )
        .await
    }
    
    /// Create a new raw disk cache with full configuration
    async fn new_with_backend_and_config(
        device_path: impl AsRef<Path>,
        total_size: u64,
        block_size: usize,
        ttl: Duration,
        io_backend: IOBackend,
        use_direct_io: bool,
        #[cfg(target_os = "linux")]
        io_uring_config: Option<IoUringConfig>,
        #[cfg(not(target_os = "linux"))]
        #[allow(unused_variables)]
        io_uring_config: Option<()>,
        prefetch_config: PrefetchConfig,
    ) -> Result<Self, RawDiskError> {
        info!(
            "Initializing raw disk cache: path={}, size={}, block_size={}, backend={:?}, direct_io={}",
            device_path.as_ref().display(),
            total_size,
            block_size,
            io_backend,
            use_direct_io
        );

        // Initialize disk I/O manager (always needed for superblock operations)
        let disk_io = Arc::new(
            DiskIOManager::new_with_options(&device_path, block_size, use_direct_io).await?
        );

        // Ensure file is large enough
        let current_size = disk_io.size().await.unwrap_or(0);
        if current_size < total_size {
            info!("Pre-allocating disk space: {} bytes", total_size);
            // Write a byte at the end to extend the file
            disk_io.write_at(total_size - 1, &[0]).await?;
        }

        // Try to load existing superblock or create new one
        let superblock = match Superblock::load(&disk_io).await {
            Ok(sb) => {
                info!("Loaded existing superblock");
                sb
            }
            Err(_) => {
                info!("Creating new superblock");
                let sb = Superblock::new(total_size, block_size as u32);
                sb.save(&disk_io).await?;
                sb
            }
        };

        let total_blocks = superblock.total_blocks() as usize;

        // Initialize allocator
        let allocator = BlockAllocator::new(block_size, total_blocks);

        // Initialize directory
        let directory = CacheDirectory::new();

        // Initialize batch I/O manager
        // Default: batch up to 32 operations or 4MB of data
        let batch_io = Arc::new(BatchIOManager::new(
            disk_io.clone(),
            32,
            4 * 1024 * 1024,
        ));

        // Initialize io_uring batch manager if requested (Linux only)
        #[cfg(target_os = "linux")]
        let io_uring_batch = if io_backend == IOBackend::IoUring {
            let config = io_uring_config.unwrap_or_default();
            info!("Initializing io_uring with queue_depth={}", config.queue_depth);
            Some(Arc::new(IoUringBatchManager::new(&device_path, config).await?))
        } else {
            None
        };

        // Initialize prefetch manager
        info!(
            "Initializing prefetch manager: enabled={}, cache_size={}",
            prefetch_config.enabled, prefetch_config.cache_size
        );
        let prefetch_manager = Arc::new(PrefetchManager::new(prefetch_config));

        // Initialize zero-copy manager
        // Open a separate file handle for zero-copy operations
        let zero_copy_manager = match std::fs::File::open(&device_path) {
            Ok(file) => {
                let config = ZeroCopyConfig::default();
                info!("Initializing zero-copy manager with mmap_threshold={} bytes", config.mmap_threshold);
                Some(Arc::new(ZeroCopyManager::new(file, config)))
            }
            Err(e) => {
                warn!("Failed to open file for zero-copy operations: {}", e);
                None
            }
        };

        // Initialize smart GC with TTL from cache config
        let gc_config = GCConfig {
            ttl_secs: ttl.as_secs(),
            ..Default::default()
        };
        let smart_gc = Arc::new(RwLock::new(SmartGC::new(gc_config)));

        // Initialize defragmentation manager
        let defrag_manager = Arc::new(RwLock::new(DefragManager::new(DefragConfig::default())));

        // Initialize compression manager
        let compression_config = CompressionConfig::default();
        info!(
            "Initializing compression: algorithm={:?}, level={}, min_size={}, enabled={}",
            compression_config.algorithm,
            compression_config.level,
            compression_config.min_size,
            compression_config.enabled
        );
        let compression_manager = Arc::new(CompressionManager::new(compression_config));

        // Initialize verification manager
        let verification_config = VerificationConfig::default();
        info!(
            "Initializing verification: algorithm={:?}, periodic={}, interval={}s",
            verification_config.algorithm,
            verification_config.periodic_verification_enabled,
            verification_config.verification_interval_secs
        );
        let verification_manager = Arc::new(VerificationManager::new(
            verification_config,
            disk_io.clone(),
        ));

        let cache = Self {
            superblock: Arc::new(RwLock::new(superblock)),
            directory: Arc::new(RwLock::new(directory)),
            allocator: Arc::new(RwLock::new(allocator)),
            disk_io,
            batch_io,
            #[cfg(target_os = "linux")]
            io_uring_batch,
            prefetch_manager,
            zero_copy_manager,
            zero_copy_stats: Arc::new(RwLock::new(ZeroCopyStats::new())),
            compression_manager,
            compression_stats: Arc::new(RwLock::new(CompressionStats::new())),
            io_backend,
            ttl,
            smart_gc,
            defrag_manager,
            verification_manager,
            verification_task: None,
            metrics: Arc::new(RawDiskMetrics::new()),
        };

        // Attempt crash recovery
        cache.recover().await?;

        Ok(cache)
    }

    /// Store data in cache
    pub async fn store(&self, key: &str, data: Bytes) -> Result<(), RawDiskError> {
        let start = Instant::now();
        let original_size = data.len();
        
        // Try to compress data
        let (data_to_store, was_compressed) = self.compression_manager
            .compress(&data)
            .map_err(|e| RawDiskError::CompressionError(e.to_string()))?;
        
        let data_len = data_to_store.len();
        
        // Update compression stats
        if was_compressed {
            let mut stats = self.compression_stats.write().await;
            stats.record_compression(original_size, data_len);
            drop(stats);
        } else if original_size >= self.compression_manager.config().min_size {
            // Data was large enough but compression didn't help
            let mut stats = self.compression_stats.write().await;
            stats.record_expansion();
            drop(stats);
        } else {
            // Data was too small to compress
            let mut stats = self.compression_stats.write().await;
            stats.record_skipped();
            drop(stats);
        }
        
        // Calculate blocks needed
        let superblock = self.superblock.read().await;
        let block_size = superblock.block_size() as usize;
        let data_offset = superblock.data_offset();
        drop(superblock);
        
        let blocks_needed = (data_len + block_size - 1) / block_size;

        // Check if GC should be triggered before allocation
        let allocator = self.allocator.read().await;
        let free_ratio = allocator.free_blocks() as f64 / allocator.total_blocks() as f64;
        drop(allocator);

        let mut gc = self.smart_gc.write().await;
        if gc.should_trigger(free_ratio) {
            drop(gc);
            // Run GC in background to avoid blocking
            let cache = self.clone_for_gc();
            tokio::spawn(async move {
                if let Err(e) = cache.run_smart_gc().await {
                    warn!("Background GC failed: {}", e);
                }
            });
        } else {
            drop(gc);
        }

        // Allocate space
        let mut allocator = self.allocator.write().await;
        let allocation_result = allocator.allocate(blocks_needed);
        drop(allocator);

        let temp_location = match allocation_result {
            Ok(loc) => {
                // Record successful allocation
                let mut gc = self.smart_gc.write().await;
                gc.record_allocation(true);
                drop(gc);
                loc
            }
            Err(e) => {
                // Record failed allocation
                let mut gc = self.smart_gc.write().await;
                gc.record_allocation(false);
                drop(gc);
                
                // Record failed store
                let duration = start.elapsed();
                self.metrics.record_store(false, duration, 0);
                
                return Err(e);
            }
        };

        // Calculate absolute disk offset
        let absolute_offset = data_offset + temp_location.offset;

        // Write data to disk
        self.disk_io.write_at(absolute_offset, &data_to_store).await?;

        // Create location with correct checksum and absolute offset
        let location = if was_compressed {
            DiskLocation::new_compressed(absolute_offset, &data_to_store, original_size as u32)
        } else {
            DiskLocation::new(absolute_offset, &data_to_store)
        };

        // Update directory and record insertion
        let mut directory = self.directory.write().await;
        let offset = location.offset; // Save for logging
        directory.insert(key.to_string(), location);
        drop(directory);

        // Record insertion for GC tracking
        let mut gc = self.smart_gc.write().await;
        gc.record_insertion(key.to_string());
        drop(gc);

        if was_compressed {
            info!(
                "Stored {} bytes (compressed from {} bytes, {:.1}% reduction) at offset {} ({} blocks)",
                data_len,
                original_size,
                (1.0 - data_len as f64 / original_size as f64) * 100.0,
                offset,
                blocks_needed
            );
        } else {
            info!("Stored {} bytes at offset {} ({} blocks)", data_len, offset, blocks_needed);
        }

        // Record metrics
        let duration = start.elapsed();
        self.metrics.record_store(true, duration, data_len as u64);
        
        // Update cache state
        let directory = self.directory.read().await;
        let allocator = self.allocator.read().await;
        self.metrics.update_cache_state(
            directory.len() as u64,
            allocator.used_blocks() as u64,
            allocator.free_blocks() as u64,
        );

        Ok(())
    }

    /// Lookup data in cache
    pub async fn lookup(&self, key: &str) -> Result<Option<Bytes>, RawDiskError> {
        let start = Instant::now();
        
        // Check prefetch cache first
        if let Some(data) = self.prefetch_manager.get_prefetched(key).await {
            debug!("Prefetch cache hit for key: {}", key);
            
            // Update LRU
            let mut directory = self.directory.write().await;
            directory.touch(key);
            
            // Record hit
            let duration = start.elapsed();
            self.metrics.record_lookup(true, duration, data.len() as u64);
            
            return Ok(Some(data));
        }

        // Check directory
        let directory = self.directory.read().await;
        let location = match directory.get(key) {
            Some(loc) => loc.clone(),
            None => {
                drop(directory);
                // Record miss
                let duration = start.elapsed();
                self.metrics.record_lookup(false, duration, 0);
                return Ok(None);
            }
        };
        drop(directory);

        // Check if entry is expired
        if location.is_expired(self.ttl.as_secs()) {
            debug!("Entry expired for key: {}", key);
            // Remove expired entry
            self.remove(key).await?;
            return Ok(None);
        }

        // Record access for pattern detection
        self.prefetch_manager
            .record_access(key.to_string(), location.offset)
            .await;

        // Read from disk
        let data = self.disk_io.read_at(location.offset, location.size as usize).await?;

        // Verify checksum
        if !location.verify_checksum(&data) {
            warn!("Checksum mismatch for key: {}", key);
            return Err(RawDiskError::ChecksumMismatch);
        }
        
        // Decompress if needed
        let data = if location.compressed {
            let decompressed = self.compression_manager
                .decompress(&data, true)
                .map_err(|e| RawDiskError::CompressionError(e.to_string()))?;
            
            // Update decompression stats
            let mut stats = self.compression_stats.write().await;
            stats.record_decompression(decompressed.len());
            drop(stats);
            
            decompressed
        } else {
            data
        };

        // Update LRU
        let mut directory = self.directory.write().await;
        directory.touch(key);
        drop(directory);

        // Record access for GC
        let mut gc = self.smart_gc.write().await;
        gc.record_access(key);
        drop(gc);

        // Trigger prefetch for predicted keys
        self.trigger_prefetch(key).await;

        // Record hit
        let duration = start.elapsed();
        self.metrics.record_lookup(true, duration, data.len() as u64);

        Ok(Some(data))
    }

    /// Lookup data using zero-copy mmap for large files
    pub async fn lookup_zero_copy(&self, key: &str) -> Result<Option<Bytes>, RawDiskError> {
        // Check if zero-copy is available
        let zero_copy_manager = match &self.zero_copy_manager {
            Some(mgr) => mgr,
            None => {
                // Fall back to regular lookup
                return self.lookup(key).await;
            }
        };

        // Check prefetch cache first
        if let Some(data) = self.prefetch_manager.get_prefetched(key).await {
            debug!("Prefetch cache hit for key: {}", key);
            
            // Update LRU
            let mut directory = self.directory.write().await;
            directory.touch(key);
            
            return Ok(Some(data));
        }

        // Check directory
        let directory = self.directory.read().await;
        let location = match directory.get(key) {
            Some(loc) => loc.clone(),
            None => return Ok(None),
        };
        drop(directory);

        // Check if entry is expired
        if location.is_expired(self.ttl.as_secs()) {
            debug!("Entry expired for key: {}", key);
            // Remove expired entry
            self.remove(key).await?;
            return Ok(None);
        }

        // Record access for pattern detection
        self.prefetch_manager
            .record_access(key.to_string(), location.offset)
            .await;

        // Decide whether to use mmap based on size
        let data = if location.size as usize >= zero_copy_manager.mmap_threshold() {
            debug!("Using mmap for large file: {} bytes", location.size);
            let result = zero_copy_manager.mmap_read(location.offset, location.size as usize).await?;
            
            // Record stats
            let mut stats = self.zero_copy_stats.write().await;
            stats.record_mmap_read(location.size as usize);
            drop(stats);
            
            result
        } else {
            debug!("Using regular read for small file: {} bytes", location.size);
            let result = self.disk_io.read_at(location.offset, location.size as usize).await?;
            
            // Record stats
            let mut stats = self.zero_copy_stats.write().await;
            stats.record_mmap_skipped();
            drop(stats);
            
            result
        };

        // Verify checksum
        if !location.verify_checksum(&data) {
            warn!("Checksum mismatch for key: {}", key);
            return Err(RawDiskError::ChecksumMismatch);
        }
        
        // Decompress if needed
        let data = if location.compressed {
            let decompressed = self.compression_manager
                .decompress(&data, true)
                .map_err(|e| RawDiskError::CompressionError(e.to_string()))?;
            
            // Update decompression stats
            let mut stats = self.compression_stats.write().await;
            stats.record_decompression(decompressed.len());
            drop(stats);
            
            decompressed
        } else {
            data
        };

        // Update LRU
        let mut directory = self.directory.write().await;
        directory.touch(key);
        drop(directory);

        // Record access for GC
        let mut gc = self.smart_gc.write().await;
        gc.record_access(key);
        drop(gc);

        // Trigger prefetch for predicted keys
        self.trigger_prefetch(key).await;

        Ok(Some(data))
    }

    /// Remove entry from cache
    pub async fn remove(&self, key: &str) -> Result<bool, RawDiskError> {
        let start = Instant::now();
        let mut directory = self.directory.write().await;
        
        if let Some(location) = directory.remove(key) {
            drop(directory);
            
            // Free blocks
            let mut allocator = self.allocator.write().await;
            let block_size = self.superblock.read().await.block_size() as usize;
            let blocks = (location.size as usize + block_size - 1) / block_size;
            allocator.free(location.offset, blocks)?;
            drop(allocator);
            
            // Clean up GC tracking
            let mut gc = self.smart_gc.write().await;
            gc.cleanup_removed_keys(&[key.to_string()]);
            drop(gc);
            
            // Record metrics
            let duration = start.elapsed();
            self.metrics.record_remove(duration);
            
            // Update cache state
            let directory = self.directory.read().await;
            let allocator = self.allocator.read().await;
            self.metrics.update_cache_state(
                directory.len() as u64,
                allocator.used_blocks() as u64,
                allocator.free_blocks() as u64,
            );
            
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        let directory = self.directory.read().await;
        let allocator = self.allocator.read().await;
        let buffer_stats = self.batch_io.buffer_stats().await;
        let prefetch_stats = Some(self.prefetch_manager.stats().await);
        let zero_copy_stats = Some(self.zero_copy_stats.read().await.clone());
        let compression_stats = Some(self.compression_stats.read().await.clone());
        let gc_metrics = Some(self.smart_gc.read().await.metrics().clone());
        let defrag_stats = Some(self.defrag_manager.read().await.stats().clone());
        let verification_stats = Some(self.verification_manager.stats().await);
        
        // Collect values before dropping locks
        let entries = directory.len();
        let used_blocks = allocator.used_blocks();
        let free_blocks = allocator.free_blocks();
        let total_blocks = allocator.total_blocks();
        let hits = directory.hits();
        let misses = directory.misses();
        let block_size = allocator.block_size;
        
        let superblock = self.superblock.read().await;
        let data_offset = superblock.data_offset();
        drop(superblock);
        
        let defrag = self.defrag_manager.read().await;
        let fragmentation_ratio = defrag.calculate_fragmentation(&directory, total_blocks, block_size, data_offset);
        drop(defrag);
        drop(directory);
        drop(allocator);

        CacheStats {
            entries,
            used_blocks,
            free_blocks,
            total_blocks,
            hits,
            misses,
            pending_writes: buffer_stats.pending_operations,
            buffered_bytes: buffer_stats.buffered_bytes,
            prefetch_stats,
            zero_copy_stats,
            compression_stats,
            gc_metrics,
            defrag_stats,
            fragmentation_ratio,
            verification_stats,
        }
    }

    /// Get raw disk metrics
    pub fn metrics(&self) -> Arc<RawDiskMetrics> {
        self.metrics.clone()
    }

    /// Get metrics snapshot
    /// 
    /// Note: This returns the current metrics snapshot. Cache state metrics
    /// (entries, blocks) are updated periodically by operations, not on-demand.
    pub fn metrics_snapshot(&self) -> RawDiskMetricsSnapshot {
        self.metrics.get_stats()
    }
    
    /// Update and get metrics snapshot
    /// 
    /// This async version updates cache state metrics before returning the snapshot.
    pub async fn metrics_snapshot_async(&self) -> RawDiskMetricsSnapshot {
        // Update cache state metrics before returning snapshot
        let directory = self.directory.read().await;
        let allocator = self.allocator.read().await;
        
        self.metrics.update_cache_state(
            directory.len() as u64,
            allocator.used_blocks() as u64,
            allocator.free_blocks() as u64,
        );
        
        drop(directory);
        drop(allocator);
        
        self.metrics.get_stats()
    }

    /// Perform health check
    /// 
    /// Returns true if the cache is healthy, false otherwise.
    /// Checks include:
    /// - Disk I/O is accessible
    /// - Allocator has free space
    /// - No critical errors
    pub async fn health_check(&self) -> bool {
        // Check if we can read the superblock
        if Superblock::load(&self.disk_io).await.is_err() {
            return false;
        }

        // Check if we have free space
        let allocator = self.allocator.read().await;
        if allocator.free_blocks() == 0 {
            return false;
        }

        // Check store success rate (if we have operations)
        let snapshot = self.metrics.get_stats();
        if snapshot.store_operations > 100 {
            let success_rate = snapshot.store_success_rate();
            if success_rate < 50.0 {
                return false;
            }
        }

        true
    }

    /// Transfer data directly to a socket using sendfile (Linux only)
    /// 
    /// This provides zero-copy transfer from disk to socket, avoiding
    /// copying data through user space.
    #[cfg(target_os = "linux")]
    pub async fn sendfile_to_socket(
        &self,
        key: &str,
        socket_fd: i32,
    ) -> Result<usize, RawDiskError> {
        // Check if zero-copy is available
        let zero_copy_manager = match &self.zero_copy_manager {
            Some(mgr) => mgr,
            None => {
                return Err(RawDiskError::Io(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "Zero-copy not available"
                )));
            }
        };

        // Check directory
        let directory = self.directory.read().await;
        let location = match directory.get(key) {
            Some(loc) => loc.clone(),
            None => {
                return Err(RawDiskError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Key not found"
                )));
            }
        };
        drop(directory);

        // Use sendfile to transfer data
        let bytes_sent = zero_copy_manager
            .sendfile_to_socket(socket_fd, location.offset, location.size as usize)
            .await?;

        // Record stats
        let mut stats = self.zero_copy_stats.write().await;
        stats.record_sendfile(bytes_sent);
        drop(stats);

        // Update LRU
        let mut directory = self.directory.write().await;
        directory.touch(key);
        drop(directory);

        // Record access for GC
        let mut gc = self.smart_gc.write().await;
        gc.record_access(key);
        drop(gc);

        Ok(bytes_sent)
    }

    /// Transfer data to socket (stub for non-Linux platforms)
    #[cfg(not(target_os = "linux"))]
    pub async fn sendfile_to_socket(
        &self,
        _key: &str,
        _socket_fd: i32,
    ) -> Result<usize, RawDiskError> {
        Err(RawDiskError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "sendfile is only supported on Linux"
        )))
    }

    /// Get zero-copy statistics
    pub async fn zero_copy_stats(&self) -> ZeroCopyStats {
        self.zero_copy_stats.read().await.clone()
    }

    /// Check if zero-copy operations are available
    pub fn is_zero_copy_available(&self) -> bool {
        self.zero_copy_manager.is_some()
    }
    
    /// Get compression statistics
    pub async fn compression_stats(&self) -> CompressionStats {
        self.compression_stats.read().await.clone()
    }
    
    /// Get compression configuration
    pub fn compression_config(&self) -> CompressionConfig {
        self.compression_manager.config().clone()
    }
    
    /// Update compression configuration
    pub async fn update_compression_config(&self, _config: CompressionConfig) {
        // Note: This creates a new manager since CompressionManager doesn't have interior mutability
        // In a production system, you might want to make CompressionManager use Arc<RwLock<>> internally
        warn!("Compression config update requires cache restart to take effect");
    }
    
    /// Store data using batch I/O (buffered write)
    pub async fn store_buffered(&self, key: &str, data: Bytes) -> Result<(), RawDiskError> {
        let data_len = data.len();
        
        // Calculate blocks needed
        let superblock = self.superblock.read().await;
        let block_size = superblock.block_size() as usize;
        let data_offset = superblock.data_offset();
        drop(superblock);
        
        let blocks_needed = (data_len + block_size - 1) / block_size;

        // Allocate space
        let mut allocator = self.allocator.write().await;
        let temp_location = allocator.allocate(blocks_needed)?;
        drop(allocator);

        // Calculate absolute disk offset
        let absolute_offset = data_offset + temp_location.offset;

        // Create location with correct checksum and absolute offset
        let location = DiskLocation::new(absolute_offset, &data);

        // Write data to disk using batch I/O
        self.batch_io.write_buffered(absolute_offset, data).await?;

        // Update directory
        let mut directory = self.directory.write().await;
        let offset = location.offset; // Save for logging
        directory.insert(key.to_string(), location);
        drop(directory);

        // Record insertion for GC tracking
        let mut gc = self.smart_gc.write().await;
        gc.record_insertion(key.to_string());
        drop(gc);

        info!("Buffered store: {} bytes at offset {} ({} blocks)", data_len, offset, blocks_needed);

        Ok(())
    }
    
    /// Flush all pending writes
    pub async fn flush_writes(&self) -> Result<usize, RawDiskError> {
        self.batch_io.flush().await
    }
    
    /// Batch lookup multiple keys
    pub async fn lookup_batch(&self, keys: &[String]) -> Result<Vec<Option<Bytes>>, RawDiskError> {
        // Collect locations for all keys
        let directory = self.directory.read().await;
        let mut locations = Vec::new();
        let mut location_map = Vec::new();
        
        for key in keys {
            if let Some(loc) = directory.get(key) {
                locations.push((loc.offset, loc.size as usize));
                location_map.push(Some(loc.clone()));
            } else {
                location_map.push(None);
            }
        }
        drop(directory);
        
        if locations.is_empty() {
            return Ok(vec![None; keys.len()]);
        }
        
        // Batch read all locations using appropriate backend
        let data_results = self.batch_read_internal(locations).await?;
        
        // Verify checksums and build results
        let mut results = Vec::new();
        let mut data_idx = 0;
        
        for (i, key) in keys.iter().enumerate() {
            if let Some(location) = &location_map[i] {
                let data = &data_results[data_idx];
                data_idx += 1;
                
                // Verify checksum
                if location.verify_checksum(data) {
                    results.push(Some(data.clone()));
                    
                    // Update LRU
                    let mut directory = self.directory.write().await;
                    directory.touch(key);
                    drop(directory);
                    
                    // Record access for GC
                    let mut gc = self.smart_gc.write().await;
                    gc.record_access(key);
                    drop(gc);
                } else {
                    warn!("Checksum mismatch for key: {}", key);
                    results.push(None);
                }
            } else {
                results.push(None);
            }
        }
        
        Ok(results)
    }
    
    /// Internal batch read using appropriate backend
    async fn batch_read_internal(&self, locations: Vec<(u64, usize)>) -> Result<Vec<Bytes>, RawDiskError> {
        #[cfg(target_os = "linux")]
        if let Some(io_uring_batch) = &self.io_uring_batch {
            return io_uring_batch.read_batch(locations).await;
        }
        
        // Fall back to standard batch I/O
        self.batch_io.read_batch(locations).await
    }
    
    /// Store data using io_uring (Linux only)
    #[cfg(target_os = "linux")]
    pub async fn store_with_io_uring(&self, key: &str, data: Bytes) -> Result<(), RawDiskError> {
        if self.io_uring_batch.is_none() {
            return Err(RawDiskError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "io_uring not initialized"
            )));
        }
        
        let data_len = data.len();
        
        // Calculate blocks needed
        let superblock = self.superblock.read().await;
        let block_size = superblock.block_size() as usize;
        let data_offset = superblock.data_offset();
        drop(superblock);
        
        let blocks_needed = (data_len + block_size - 1) / block_size;

        // Allocate space
        let mut allocator = self.allocator.write().await;
        let temp_location = allocator.allocate(blocks_needed)?;
        drop(allocator);

        // Calculate absolute disk offset
        let absolute_offset = data_offset + temp_location.offset;

        // Create location with correct checksum and absolute offset
        let location = DiskLocation::new(absolute_offset, &data);

        // Write data using io_uring
        if let Some(io_uring_batch) = &self.io_uring_batch {
            io_uring_batch.io_uring().write_at(absolute_offset, &data).await?;
            io_uring_batch.io_uring().sync().await?;
        }

        // Update directory
        let mut directory = self.directory.write().await;
        let offset = location.offset;
        directory.insert(key.to_string(), location);
        drop(directory);

        // Record insertion for GC tracking
        let mut gc = self.smart_gc.write().await;
        gc.record_insertion(key.to_string());
        drop(gc);

        info!("Stored with io_uring: {} bytes at offset {} ({} blocks)", data_len, offset, blocks_needed);

        Ok(())
    }
    
    /// Lookup data using io_uring (Linux only)
    #[cfg(target_os = "linux")]
    pub async fn lookup_with_io_uring(&self, key: &str) -> Result<Option<Bytes>, RawDiskError> {
        if self.io_uring_batch.is_none() {
            return Err(RawDiskError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "io_uring not initialized"
            )));
        }
        
        // Check directory
        let directory = self.directory.read().await;
        let location = match directory.get(key) {
            Some(loc) => loc.clone(),
            None => return Ok(None),
        };
        drop(directory);

        // Check if entry is expired
        if location.is_expired(self.ttl.as_secs()) {
            debug!("Entry expired for key: {}", key);
            // Remove expired entry
            self.remove(key).await?;
            return Ok(None);
        }

        // Read from disk using io_uring
        let data = if let Some(io_uring_batch) = &self.io_uring_batch {
            io_uring_batch.io_uring().read_at(location.offset, location.size as usize).await?
        } else {
            return Err(RawDiskError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "io_uring not initialized"
            )));
        };

        // Verify checksum
        if !location.verify_checksum(&data) {
            warn!("Checksum mismatch for key: {}", key);
            return Err(RawDiskError::ChecksumMismatch);
        }

        // Update LRU
        let mut directory = self.directory.write().await;
        directory.touch(key);
        drop(directory);

        // Record access for GC
        let mut gc = self.smart_gc.write().await;
        gc.record_access(key);
        drop(gc);

        Ok(Some(data))
    }
    
    /// Get the I/O backend being used
    pub fn io_backend(&self) -> IOBackend {
        self.io_backend
    }

    /// Trigger prefetch for predicted keys
    async fn trigger_prefetch(&self, current_key: &str) {
        // Get all keys from directory
        let directory = self.directory.read().await;
        let all_keys: Vec<String> = directory.iter().map(|(k, _)| k.clone()).collect();
        drop(directory);

        // Get predicted keys
        let predicted_keys = self
            .prefetch_manager
            .predict_prefetch_keys(current_key, &all_keys)
            .await;

        if predicted_keys.is_empty() {
            return;
        }

        debug!(
            "Prefetching {} keys after access to {}",
            predicted_keys.len(),
            current_key
        );

        // Prefetch in background
        let cache = self.clone_for_prefetch();
        tokio::spawn(async move {
            for key in predicted_keys {
                if let Err(e) = cache.prefetch_key(&key).await {
                    debug!("Prefetch failed for key {}: {}", key, e);
                }
            }
        });
    }

    /// Clone necessary components for prefetch task
    fn clone_for_prefetch(&self) -> Self {
        Self {
            superblock: self.superblock.clone(),
            directory: self.directory.clone(),
            allocator: self.allocator.clone(),
            disk_io: self.disk_io.clone(),
            batch_io: self.batch_io.clone(),
            #[cfg(target_os = "linux")]
            io_uring_batch: self.io_uring_batch.clone(),
            prefetch_manager: self.prefetch_manager.clone(),
            zero_copy_manager: self.zero_copy_manager.clone(),
            zero_copy_stats: self.zero_copy_stats.clone(),
            compression_manager: self.compression_manager.clone(),
            compression_stats: self.compression_stats.clone(),
            io_backend: self.io_backend,
            ttl: self.ttl,
            smart_gc: self.smart_gc.clone(),
            defrag_manager: self.defrag_manager.clone(),
            verification_manager: self.verification_manager.clone(),
            verification_task: None,
            metrics: self.metrics.clone(),
        }
    }

    /// Clone necessary components for GC task
    fn clone_for_gc(&self) -> Self {
        Self {
            superblock: self.superblock.clone(),
            directory: self.directory.clone(),
            allocator: self.allocator.clone(),
            disk_io: self.disk_io.clone(),
            batch_io: self.batch_io.clone(),
            #[cfg(target_os = "linux")]
            io_uring_batch: self.io_uring_batch.clone(),
            prefetch_manager: self.prefetch_manager.clone(),
            zero_copy_manager: self.zero_copy_manager.clone(),
            zero_copy_stats: self.zero_copy_stats.clone(),
            compression_manager: self.compression_manager.clone(),
            compression_stats: self.compression_stats.clone(),
            io_backend: self.io_backend,
            ttl: self.ttl,
            smart_gc: self.smart_gc.clone(),
            defrag_manager: self.defrag_manager.clone(),
            verification_manager: self.verification_manager.clone(),
            verification_task: None,
            metrics: self.metrics.clone(),
        }
    }

    /// Prefetch a single key
    async fn prefetch_key(&self, key: &str) -> Result<(), RawDiskError> {
        // Check if already in prefetch cache
        if self.prefetch_manager.get_prefetched(key).await.is_some() {
            return Ok(());
        }

        // Get location from directory
        let directory = self.directory.read().await;
        let location = match directory.get(key) {
            Some(loc) => loc.clone(),
            None => return Ok(()), // Key doesn't exist, skip
        };
        drop(directory);

        // Read from disk
        let data = self
            .disk_io
            .read_at(location.offset, location.size as usize)
            .await?;

        // Verify checksum
        if !location.verify_checksum(&data) {
            warn!("Checksum mismatch during prefetch for key: {}", key);
            return Ok(()); // Don't fail, just skip
        }
        
        // Decompress if needed before storing in prefetch cache
        let data = if location.compressed {
            self.compression_manager
                .decompress(&data, true)
                .map_err(|e| RawDiskError::CompressionError(e.to_string()))?
        } else {
            data
        };

        // Store in prefetch cache (decompressed)
        self.prefetch_manager
            .store_prefetched(key.to_string(), data, location)
            .await;

        debug!("Prefetched key: {}", key);
        Ok(())
    }

    /// Get prefetch statistics
    pub async fn prefetch_stats(&self) -> PrefetchStats {
        self.prefetch_manager.stats().await
    }

    /// Get current access pattern
    pub async fn access_pattern(&self) -> AccessPattern {
        self.prefetch_manager.current_pattern().await
    }

    /// Clear prefetch cache
    pub async fn clear_prefetch_cache(&self) {
        self.prefetch_manager.clear_cache().await;
    }

    /// Perform garbage collection (legacy method - uses smart GC internally)
    pub async fn gc(&self, target_free_ratio: f64) -> Result<usize, RawDiskError> {
        let allocator = self.allocator.read().await;
        let current_free_ratio = allocator.free_blocks() as f64 / allocator.total_blocks() as f64;
        drop(allocator);

        if current_free_ratio >= target_free_ratio {
            return Ok(0);
        }

        self.run_smart_gc().await
    }

    /// Run smart garbage collection
    pub async fn run_smart_gc(&self) -> Result<usize, RawDiskError> {
        let start = Instant::now();
        
        let allocator = self.allocator.read().await;
        let current_free_ratio = allocator.free_blocks() as f64 / allocator.total_blocks() as f64;
        let total_blocks = allocator.total_blocks();
        drop(allocator);

        let gc = self.smart_gc.read().await;
        let target_free_ratio = gc.config().trigger.target_free_ratio;
        let incremental = gc.config().incremental;
        let batch_size = gc.config().batch_size;
        drop(gc);

        if current_free_ratio >= target_free_ratio {
            return Ok(0);
        }

        info!(
            "Starting smart GC: current_free={:.2}%, target={:.2}%",
            current_free_ratio * 100.0,
            target_free_ratio * 100.0
        );

        // Calculate how many entries to evict
        let blocks_to_free = ((target_free_ratio - current_free_ratio) * total_blocks as f64) as usize;
        let superblock = self.superblock.read().await;
        let block_size = superblock.block_size() as usize;
        drop(superblock);

        let directory = self.directory.read().await;
        let avg_entry_size = if directory.len() > 0 {
            let allocator = self.allocator.read().await;
            let used_blocks = allocator.used_blocks();
            drop(allocator);
            (used_blocks * block_size) / directory.len()
        } else {
            block_size
        };
        drop(directory);

        let target_evictions = (blocks_to_free * block_size / avg_entry_size).max(1);

        let mut total_freed = 0;
        let mut total_bytes_freed = 0u64;
        let data_offset = self.superblock.read().await.data_offset();

        if incremental {
            // Incremental GC - process in batches
            let mut remaining = target_evictions;
            
            while remaining > 0 {
                let batch = remaining.min(batch_size);
                
                // Select victims
                let directory = self.directory.read().await;
                let mut gc = self.smart_gc.write().await;
                let victims = gc.select_victims(&directory, batch, block_size);
                drop(gc);
                drop(directory);

                if victims.is_empty() {
                    break;
                }

                // Evict victims
                let mut directory = self.directory.write().await;
                let mut allocator = self.allocator.write().await;
                let mut evicted_keys = Vec::new();

                for (key, _location) in victims {
                    if let Some(loc) = directory.remove(&key) {
                        let blocks = (loc.size as usize + block_size - 1) / block_size;
                        let relative_offset = loc.offset - data_offset;
                        
                        if let Err(e) = allocator.free(relative_offset, blocks) {
                            warn!("Failed to free blocks for key {}: {}", key, e);
                            continue;
                        }
                        
                        total_freed += 1;
                        total_bytes_freed += loc.size as u64;
                        evicted_keys.push(key);
                    }
                }

                drop(allocator);
                drop(directory);

                // Clean up GC tracking
                let mut gc = self.smart_gc.write().await;
                gc.cleanup_removed_keys(&evicted_keys);
                drop(gc);

                remaining = remaining.saturating_sub(batch);

                // Yield to allow other operations
                tokio::task::yield_now().await;
            }
        } else {
            // Full GC - process all at once
            let directory = self.directory.read().await;
            let mut gc = self.smart_gc.write().await;
            let victims = gc.select_victims(&directory, target_evictions, block_size);
            drop(gc);
            drop(directory);

            let mut directory = self.directory.write().await;
            let mut allocator = self.allocator.write().await;
            let mut evicted_keys = Vec::new();

            for (key, _location) in victims {
                if let Some(loc) = directory.remove(&key) {
                    let blocks = (loc.size as usize + block_size - 1) / block_size;
                    let relative_offset = loc.offset - data_offset;
                    
                    if let Err(e) = allocator.free(relative_offset, blocks) {
                        warn!("Failed to free blocks for key {}: {}", key, e);
                        continue;
                    }
                    
                    total_freed += 1;
                    total_bytes_freed += loc.size as u64;
                    evicted_keys.push(key);
                }
            }

            drop(allocator);
            drop(directory);

            // Clean up GC tracking
            let mut gc = self.smart_gc.write().await;
            gc.cleanup_removed_keys(&evicted_keys);
            drop(gc);
        }

        let duration = start.elapsed();

        // Record metrics
        let mut gc = self.smart_gc.write().await;
        gc.metrics_mut().record_run(total_freed, total_bytes_freed, duration);
        drop(gc);

        info!(
            "Smart GC completed: freed {} entries ({} bytes) in {:?}",
            total_freed, total_bytes_freed, duration
        );

        Ok(total_freed)
    }

    /// Get GC metrics
    pub async fn gc_metrics(&self) -> GCMetrics {
        self.smart_gc.read().await.metrics().clone()
    }

    /// Get GC configuration
    pub async fn gc_config(&self) -> GCConfig {
        self.smart_gc.read().await.config().clone()
    }

    /// Update GC configuration
    pub async fn update_gc_config(&self, config: GCConfig) {
        let mut gc = self.smart_gc.write().await;
        gc.update_config(config);
    }
    
    /// Calculate current fragmentation ratio
    pub async fn fragmentation_ratio(&self) -> f64 {
        let directory = self.directory.read().await;
        let allocator = self.allocator.read().await;
        let superblock = self.superblock.read().await;
        
        let total_blocks = allocator.total_blocks();
        let block_size = superblock.block_size() as usize;
        let data_offset = superblock.data_offset();
        
        drop(superblock);
        drop(allocator);
        
        let defrag = self.defrag_manager.read().await;
        let ratio = defrag.calculate_fragmentation(&directory, total_blocks, block_size, data_offset);
        drop(defrag);
        drop(directory);
        
        ratio
    }
    
    /// Check if defragmentation should be triggered
    pub async fn should_defragment(&self) -> bool {
        let allocator = self.allocator.read().await;
        let free_ratio = allocator.free_blocks() as f64 / allocator.total_blocks() as f64;
        drop(allocator);
        
        let frag_ratio = self.fragmentation_ratio().await;
        
        let defrag = self.defrag_manager.read().await;
        let should = defrag.should_defragment(frag_ratio, free_ratio);
        drop(defrag);
        
        should
    }
    
    /// Run defragmentation
    /// 
    /// This moves entries from the end of the disk to fill gaps at the beginning,
    /// compacting data and reducing fragmentation.
    pub async fn defragment(&self) -> Result<usize, RawDiskError> {
        let start = Instant::now();
        
        info!("Starting defragmentation");
        
        // Get current fragmentation
        let frag_before = self.fragmentation_ratio().await;
        
        // Check if we should defragment
        let allocator = self.allocator.read().await;
        let free_ratio = allocator.free_blocks() as f64 / allocator.total_blocks() as f64;
        let total_blocks = allocator.total_blocks();
        let block_size = allocator.block_size;
        drop(allocator);
        
        let defrag = self.defrag_manager.read().await;
        if !defrag.should_defragment(frag_before, free_ratio) {
            info!("Defragmentation not needed: frag={:.2}%, free={:.2}%", 
                  frag_before * 100.0, free_ratio * 100.0);
            return Ok(0);
        }
        
        let incremental = defrag.config().incremental;
        let batch_size = defrag.config().batch_size;
        drop(defrag);
        
        info!(
            "Defragmentation triggered: frag={:.2}%, free={:.2}%, incremental={}",
            frag_before * 100.0, free_ratio * 100.0, incremental
        );
        
        let superblock = self.superblock.read().await;
        let data_offset = superblock.data_offset();
        drop(superblock);
        
        let mut total_moved = 0;
        let mut total_bytes_moved = 0u64;
        
        if incremental {
            // Incremental defragmentation - process in batches
            loop {
                let directory = self.directory.read().await;
                let defrag = self.defrag_manager.read().await;
                let entries_to_move = defrag.select_entries_to_move(
                    &directory,
                    total_blocks,
                    block_size,
                    data_offset,
                    batch_size,
                );
                drop(defrag);
                drop(directory);
                
                if entries_to_move.is_empty() {
                    break;
                }
                
                // Move entries
                let moved = self.move_entries(entries_to_move, data_offset).await?;
                total_moved += moved.0;
                total_bytes_moved += moved.1;
                
                // Yield to allow other operations
                tokio::task::yield_now().await;
                
                // Check if we've improved enough
                let frag_current = self.fragmentation_ratio().await;
                let defrag = self.defrag_manager.read().await;
                let target = defrag.config().target_compaction_ratio;
                drop(defrag);
                
                if frag_current < frag_before * (1.0 - target) {
                    break;
                }
            }
        } else {
            // Full defragmentation - process all at once
            let directory = self.directory.read().await;
            let defrag = self.defrag_manager.read().await;
            let entries_to_move = defrag.select_entries_to_move(
                &directory,
                total_blocks,
                block_size,
                data_offset,
                usize::MAX, // Move all possible entries
            );
            drop(defrag);
            drop(directory);
            
            let moved = self.move_entries(entries_to_move, data_offset).await?;
            total_moved += moved.0;
            total_bytes_moved += moved.1;
        }
        
        let duration = start.elapsed();
        let frag_after = self.fragmentation_ratio().await;
        
        // Record metrics
        let mut defrag = self.defrag_manager.write().await;
        defrag.stats_mut().record_run(
            total_moved,
            total_bytes_moved,
            duration,
            frag_before,
            frag_after,
        );
        drop(defrag);
        
        info!(
            "Defragmentation completed: moved {} entries ({} bytes) in {:?}, frag {:.2}% -> {:.2}%",
            total_moved, total_bytes_moved, duration,
            frag_before * 100.0, frag_after * 100.0
        );
        
        Ok(total_moved)
    }
    
    /// Move entries to new locations
    async fn move_entries(
        &self,
        entries: Vec<(String, DiskLocation)>,
        data_offset: u64,
    ) -> Result<(usize, u64), RawDiskError> {
        let mut moved_count = 0;
        let mut bytes_moved = 0u64;
        
        let superblock = self.superblock.read().await;
        let block_size = superblock.block_size() as usize;
        drop(superblock);
        
        for (key, old_location) in entries {
            // Read data from old location
            let data = match self.disk_io.read_at(old_location.offset, old_location.size as usize).await {
                Ok(d) => d,
                Err(e) => {
                    warn!("Failed to read entry {} during defragmentation: {}", key, e);
                    let mut defrag = self.defrag_manager.write().await;
                    defrag.stats_mut().record_failed_move();
                    drop(defrag);
                    continue;
                }
            };
            
            // Verify checksum
            if !old_location.verify_checksum(&data) {
                warn!("Checksum mismatch for entry {} during defragmentation", key);
                let mut defrag = self.defrag_manager.write().await;
                defrag.stats_mut().record_failed_move();
                drop(defrag);
                continue;
            }
            
            // Free old blocks
            let old_blocks = (old_location.size as usize + block_size - 1) / block_size;
            let old_relative_offset = old_location.offset - data_offset;
            
            let mut allocator = self.allocator.write().await;
            if let Err(e) = allocator.free(old_relative_offset, old_blocks) {
                warn!("Failed to free old blocks for {}: {}", key, e);
                drop(allocator);
                let mut defrag = self.defrag_manager.write().await;
                defrag.stats_mut().record_failed_move();
                drop(defrag);
                continue;
            }
            
            // Allocate new space
            let new_blocks = (data.len() + block_size - 1) / block_size;
            let new_location = match allocator.allocate(new_blocks) {
                Ok(loc) => loc,
                Err(e) => {
                    warn!("Failed to allocate new space for {}: {}", key, e);
                    // Try to restore old allocation
                    let _ = allocator.mark_used(old_relative_offset, old_blocks);
                    drop(allocator);
                    let mut defrag = self.defrag_manager.write().await;
                    defrag.stats_mut().record_failed_move();
                    drop(defrag);
                    continue;
                }
            };
            drop(allocator);
            
            // Calculate absolute offset
            let new_absolute_offset = data_offset + new_location.offset;
            
            // Write data to new location
            if let Err(e) = self.disk_io.write_at(new_absolute_offset, &data).await {
                warn!("Failed to write entry {} to new location: {}", key, e);
                // Free the newly allocated space
                let mut allocator = self.allocator.write().await;
                let _ = allocator.free(new_location.offset, new_blocks);
                // Try to restore old allocation
                let _ = allocator.mark_used(old_relative_offset, old_blocks);
                drop(allocator);
                let mut defrag = self.defrag_manager.write().await;
                defrag.stats_mut().record_failed_move();
                drop(defrag);
                continue;
            }
            
            // Update directory with new location
            let new_disk_location = DiskLocation::new(new_absolute_offset, &data);
            let mut directory = self.directory.write().await;
            directory.insert(key.clone(), new_disk_location);
            drop(directory);
            
            moved_count += 1;
            bytes_moved += data.len() as u64;
            
            debug!("Moved entry {}: {} -> {}", key, old_location.offset, new_absolute_offset);
        }
        
        Ok((moved_count, bytes_moved))
    }
    
    /// Run defragmentation in background
    pub async fn defragment_background(&self) {
        let cache = self.clone_for_defrag();
        tokio::spawn(async move {
            if let Err(e) = cache.defragment().await {
                warn!("Background defragmentation failed: {}", e);
            }
        });
    }
    
    /// Clone necessary components for defragmentation task
    fn clone_for_defrag(&self) -> Self {
        Self {
            superblock: self.superblock.clone(),
            directory: self.directory.clone(),
            allocator: self.allocator.clone(),
            disk_io: self.disk_io.clone(),
            batch_io: self.batch_io.clone(),
            #[cfg(target_os = "linux")]
            io_uring_batch: self.io_uring_batch.clone(),
            prefetch_manager: self.prefetch_manager.clone(),
            zero_copy_manager: self.zero_copy_manager.clone(),
            zero_copy_stats: self.zero_copy_stats.clone(),
            compression_manager: self.compression_manager.clone(),
            compression_stats: self.compression_stats.clone(),
            io_backend: self.io_backend,
            ttl: self.ttl,
            smart_gc: self.smart_gc.clone(),
            defrag_manager: self.defrag_manager.clone(),
            verification_manager: self.verification_manager.clone(),
            verification_task: None,
            metrics: self.metrics.clone(),
        }
    }
    
    /// Get defragmentation statistics
    pub async fn defrag_stats(&self) -> DefragStats {
        self.defrag_manager.read().await.stats().clone()
    }
    
    /// Get defragmentation configuration
    pub async fn defrag_config(&self) -> DefragConfig {
        self.defrag_manager.read().await.config().clone()
    }
    
    /// Update defragmentation configuration
    pub async fn update_defrag_config(&self, config: DefragConfig) {
        let mut defrag = self.defrag_manager.write().await;
        defrag.update_config(config);
    }

    /// Clean up expired entries
    /// 
    /// Scans all cache entries and removes those that have exceeded their TTL.
    /// Returns the number of entries removed.
    pub async fn cleanup_expired(&self) -> Result<usize, RawDiskError> {
        let start = Instant::now();
        let ttl_secs = self.ttl.as_secs();
        
        if ttl_secs == 0 {
            // TTL disabled
            return Ok(0);
        }

        info!("Starting TTL-based cleanup (TTL: {} seconds)", ttl_secs);

        // Collect expired keys
        let directory = self.directory.read().await;
        let expired_keys: Vec<String> = directory
            .iter()
            .filter(|(_, location)| location.is_expired(ttl_secs))
            .map(|(key, _)| key.clone())
            .collect();
        drop(directory);

        let expired_count = expired_keys.len();
        
        if expired_count == 0 {
            debug!("No expired entries found");
            return Ok(0);
        }

        info!("Found {} expired entries", expired_count);

        // Remove expired entries
        let mut removed_count = 0;
        let mut total_bytes_freed = 0u64;
        
        let superblock = self.superblock.read().await;
        let block_size = superblock.block_size() as usize;
        let data_offset = superblock.data_offset();
        drop(superblock);

        for key in &expired_keys {
            let mut directory = self.directory.write().await;
            if let Some(location) = directory.remove(key) {
                drop(directory);
                
                // Free blocks
                let blocks = (location.size as usize + block_size - 1) / block_size;
                let relative_offset = location.offset - data_offset;
                
                let mut allocator = self.allocator.write().await;
                if let Err(e) = allocator.free(relative_offset, blocks) {
                    warn!("Failed to free blocks for expired key {}: {}", key, e);
                    drop(allocator);
                    continue;
                }
                drop(allocator);
                
                removed_count += 1;
                total_bytes_freed += location.size as u64;
                
                debug!("Removed expired entry: {} ({} bytes)", key, location.size);
            } else {
                drop(directory);
            }
        }

        // Clean up GC tracking
        let mut gc = self.smart_gc.write().await;
        gc.cleanup_removed_keys(&expired_keys);
        drop(gc);

        let duration = start.elapsed();
        info!(
            "TTL cleanup completed: removed {} entries ({} bytes) in {:?}",
            removed_count, total_bytes_freed, duration
        );

        Ok(removed_count)
    }

    /// Run TTL cleanup in background
    pub async fn cleanup_expired_background(&self) {
        let cache = self.clone_for_gc();
        tokio::spawn(async move {
            if let Err(e) = cache.cleanup_expired().await {
                warn!("Background TTL cleanup failed: {}", e);
            }
        });
    }

    /// Start periodic data verification
    /// 
    /// This starts a background task that periodically verifies cache entries
    /// and optionally repairs corrupted data.
    pub fn start_periodic_verification(&mut self) {
        if self.verification_task.is_some() {
            warn!("Periodic verification already running");
            return;
        }

        info!("Starting periodic data verification");
        let task = self.verification_manager.clone().start_periodic_verification(
            self.directory.clone(),
        );
        self.verification_task = Some(task);
    }

    /// Stop periodic data verification
    pub fn stop_periodic_verification(&mut self) {
        if let Some(task) = self.verification_task.take() {
            task.abort();
            info!("Stopped periodic data verification");
        }
    }

    /// Verify all cache entries
    /// 
    /// This performs a one-time verification of all cache entries.
    pub async fn verify_all_entries(&self) -> Result<VerificationResult, RawDiskError> {
        self.verification_manager
            .verify_all_entries(self.directory.clone())
            .await
    }

    /// Verify a specific cache entry
    pub async fn verify_entry(&self, key: &str) -> Result<bool, RawDiskError> {
        self.verification_manager
            .verify_entry_by_key(key, self.directory.clone())
            .await
    }

    /// Get verification statistics
    pub async fn verification_stats(&self) -> VerificationStats {
        self.verification_manager.stats().await
    }

    /// Get verification configuration
    pub fn verification_config(&self) -> &VerificationConfig {
        self.verification_manager.config()
    }

    /// Update verification configuration
    pub fn update_verification_config(&mut self, config: VerificationConfig) {
        // Stop existing task if running
        self.stop_periodic_verification();

        // Update config
        let verification_manager = Arc::new(VerificationManager::new(
            config,
            self.disk_io.clone(),
        ));
        self.verification_manager = verification_manager;

        // Restart if periodic verification was enabled
        if self.verification_manager.config().periodic_verification_enabled {
            self.start_periodic_verification();
        }
    }

    /// Repair a corrupted entry
    pub async fn repair_entry(&self, key: &str) -> Result<bool, RawDiskError> {
        self.verification_manager
            .repair_entry(key, self.directory.clone())
            .await
    }

    /// Get backup storage size
    pub async fn backup_storage_size(&self) -> usize {
        self.verification_manager.backup_storage_size().await
    }

    /// Clear all backups
    pub async fn clear_all_backups(&self) {
        self.verification_manager.clear_all_backups().await
    }
    
    /// Save metadata to disk
    pub async fn save_metadata(&self) -> Result<(), RawDiskError> {
        let directory = self.directory.read().await;
        let superblock = self.superblock.read().await;
        
        // Serialize metadata
        let metadata_bytes = directory.serialize()?;
        
        // Prepare data with length header (8 bytes for u64 length)
        let total_size = 8 + metadata_bytes.len();
        
        // Check if metadata fits in allocated space
        if total_size as u64 > superblock.metadata_size() {
            return Err(RawDiskError::MetadataTooLarge {
                size: total_size,
                max_size: superblock.metadata_size() as usize,
            });
        }
        
        // Create buffer with length header
        let mut buffer = Vec::with_capacity(total_size);
        buffer.extend_from_slice(&(metadata_bytes.len() as u64).to_le_bytes());
        buffer.extend_from_slice(&metadata_bytes);
        
        // Write metadata to disk
        let offset = superblock.metadata_offset();
        self.disk_io.write_at(offset, &buffer).await?;
        
        info!("Saved metadata: {} bytes at offset {}", buffer.len(), offset);
        Ok(())
    }
    
    /// Load metadata from disk
    pub async fn load_metadata(&self) -> Result<(), RawDiskError> {
        let superblock = self.superblock.read().await;
        let offset = superblock.metadata_offset();
        
        // Read length header (8 bytes)
        let header = match self.disk_io.read_at(offset, 8).await {
            Ok(h) => h,
            Err(_) => {
                warn!("No metadata found, starting with empty cache");
                return Ok(());
            }
        };
        
        if header.len() < 8 {
            warn!("Metadata header too small, starting with empty cache");
            return Ok(());
        }
        
        // Parse length
        let metadata_len = u64::from_le_bytes(header[0..8].try_into().unwrap()) as usize;
        
        if metadata_len == 0 || metadata_len > superblock.metadata_size() as usize {
            warn!("Invalid metadata length: {}, starting with empty cache", metadata_len);
            return Ok(());
        }
        
        // Read actual metadata
        let metadata_bytes = self.disk_io.read_at(offset + 8, metadata_len).await?;
        
        // Try to deserialize
        match CacheDirectory::deserialize(&metadata_bytes) {
            Ok(loaded_directory) => {
                // Replace current directory
                let mut directory = self.directory.write().await;
                *directory = loaded_directory;
                
                info!("Loaded metadata: {} entries", directory.len());
                Ok(())
            }
            Err(e) => {
                warn!("Failed to load metadata: {}, starting with empty cache", e);
                Ok(()) // Don't fail, just start with empty cache
            }
        }
    }

    /// Recover cache after crash
    /// 
    /// This method attempts to recover the cache state by:
    /// 1. Loading metadata from disk
    /// 2. Verifying metadata integrity
    /// 3. Rebuilding allocator state from directory
    /// 4. Scanning disk for orphaned entries if metadata is corrupted
    pub async fn recover(&self) -> Result<(), RawDiskError> {
        info!("Starting crash recovery");

        // Try to load metadata
        let metadata_loaded = match self.load_metadata().await {
            Ok(_) => {
                let directory = self.directory.read().await;
                let has_entries = directory.len() > 0;
                drop(directory);
                has_entries
            }
            Err(e) => {
                warn!("Metadata load failed: {}", e);
                false
            }
        };

        if metadata_loaded {
            // Verify and rebuild allocator from directory
            info!("Verifying metadata integrity");
            match self.verify_and_rebuild_allocator().await {
                Ok(_) => {
                    info!("Metadata verified and allocator rebuilt successfully");
                    return Ok(());
                }
                Err(e) => {
                    warn!("Metadata verification failed: {}, attempting disk scan", e);
                    // If verification failed, scan disk
                    info!("Scanning disk for cache entries");
                    self.scan_and_rebuild().await?;
                }
            }
        } else {
            // No metadata found - this is either a fresh cache or metadata was lost
            // Don't scan disk for fresh caches (would be wasteful)
            info!("No metadata found, starting with empty cache");
        }

        info!("Crash recovery completed");
        Ok(())
    }

    /// Verify metadata integrity and rebuild allocator state
    async fn verify_and_rebuild_allocator(&self) -> Result<(), RawDiskError> {
        let directory = self.directory.read().await;
        let superblock = self.superblock.read().await;
        let block_size = superblock.block_size() as usize;
        let data_offset = superblock.data_offset();
        drop(superblock);

        // Verify each entry and rebuild allocator
        let mut allocator = self.allocator.write().await;
        let mut valid_entries = Vec::new();
        let mut corrupted_keys = Vec::new();

        for (key, location) in directory.iter() {
            // Verify location is within valid range
            if location.offset < data_offset {
                warn!("Invalid offset for key {}: {} < {}", key, location.offset, data_offset);
                corrupted_keys.push(key.clone());
                continue;
            }

            // Try to read and verify data
            match self.disk_io.read_at(location.offset, location.size as usize).await {
                Ok(data) => {
                    if location.verify_checksum(&data) {
                        // Valid entry - mark blocks as used
                        let blocks_needed = (location.size as usize + block_size - 1) / block_size;
                        let relative_offset = location.offset - data_offset;
                        
                        // Mark blocks as used in allocator
                        if let Err(e) = allocator.mark_used(relative_offset, blocks_needed) {
                            warn!("Failed to mark blocks as used for key {}: {}", key, e);
                            corrupted_keys.push(key.clone());
                        } else {
                            valid_entries.push(key.clone());
                        }
                    } else {
                        warn!("Checksum mismatch for key: {}", key);
                        corrupted_keys.push(key.clone());
                    }
                }
                Err(e) => {
                    warn!("Failed to read data for key {}: {}", key, e);
                    corrupted_keys.push(key.clone());
                }
            }
        }

        drop(allocator);
        drop(directory);

        // Remove corrupted entries from directory
        if !corrupted_keys.is_empty() {
            warn!("Removing {} corrupted entries", corrupted_keys.len());
            let mut directory = self.directory.write().await;
            for key in corrupted_keys {
                directory.remove(&key);
            }
        }

        info!("Verified {} valid entries", valid_entries.len());
        Ok(())
    }

    /// Scan disk and rebuild cache from scratch
    async fn scan_and_rebuild(&self) -> Result<(), RawDiskError> {
        warn!("Rebuilding cache from disk scan (this may take a while)");

        let superblock = self.superblock.read().await;
        let block_size = superblock.block_size() as usize;
        let data_offset = superblock.data_offset();
        let total_blocks = superblock.total_blocks() as usize;
        drop(superblock);

        // Clear existing directory
        let mut directory = self.directory.write().await;
        *directory = CacheDirectory::new();
        drop(directory);

        // Reset allocator
        let mut allocator = self.allocator.write().await;
        *allocator = BlockAllocator::new(block_size, total_blocks);
        drop(allocator);

        // Scan disk for valid block headers
        let mut recovered = 0;
        let mut scanned = 0;

        for block_idx in 0..total_blocks {
            scanned += 1;
            if scanned % 10000 == 0 {
                info!("Scanned {} blocks, recovered {} entries", scanned, recovered);
            }

            let offset = data_offset + (block_idx * block_size) as u64;

            // Try to read block header
            let header_data = match self.disk_io.read_at(offset, BlockHeader::SIZE).await {
                Ok(data) => data,
                Err(_) => continue,
            };

            // Try to parse header
            let header = match BlockHeader::from_bytes(&header_data) {
                Some(h) => h,
                None => continue,
            };

            // Read data
            let data = match self.disk_io.read_at(
                offset + BlockHeader::SIZE as u64,
                header.data_size as usize,
            ).await {
                Ok(d) => d,
                Err(_) => continue,
            };

            // Verify checksum
            let mut hasher = crc32fast::Hasher::new();
            hasher.update(&data);
            if hasher.finalize() != header.checksum {
                continue;
            }

            // Valid entry found - but we don't have the original key
            // We can only recover the location and mark blocks as used
            let blocks_needed = (header.data_size as usize + block_size - 1) / block_size;
            let relative_offset = offset - data_offset;

            let mut allocator = self.allocator.write().await;
            if let Err(e) = allocator.mark_used(relative_offset, blocks_needed) {
                warn!("Failed to mark blocks as used during scan: {}", e);
            }
            drop(allocator);

            recovered += 1;
        }

        info!("Disk scan completed: scanned {} blocks, recovered {} entries", scanned, recovered);
        
        // Note: We can't fully recover the directory without keys
        // But we've at least marked the used blocks correctly
        warn!("Note: Cache keys cannot be recovered from disk scan. Cache directory is empty.");
        
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub entries: usize,
    pub used_blocks: usize,
    pub free_blocks: usize,
    pub total_blocks: usize,
    pub hits: u64,
    pub misses: u64,
    pub pending_writes: usize,
    pub buffered_bytes: usize,
    pub prefetch_stats: Option<PrefetchStats>,
    pub zero_copy_stats: Option<ZeroCopyStats>,
    pub compression_stats: Option<CompressionStats>,
    pub gc_metrics: Option<GCMetrics>,
    pub defrag_stats: Option<DefragStats>,
    pub fragmentation_ratio: f64,
    pub verification_stats: Option<VerificationStats>,
}

#[derive(Debug, thiserror::Error)]
pub enum RawDiskError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("No space available")]
    NoSpace,
    
    #[error("Invalid block size")]
    InvalidBlockSize,
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Invalid metadata: {0}")]
    InvalidMetadata(String),
    
    #[error("Metadata too large: {size} bytes (max: {max_size})")]
    MetadataTooLarge { size: usize, max_size: usize },
    
    #[error("Checksum mismatch")]
    ChecksumMismatch,
    
    #[error("Invalid superblock")]
    InvalidSuperblock,
    
    #[error("Allocation error: {0}")]
    AllocationError(String),
    
    #[error("Compression error: {0}")]
    CompressionError(String),
}
