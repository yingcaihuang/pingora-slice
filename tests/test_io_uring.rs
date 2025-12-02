//! Tests for io_uring support

#[cfg(target_os = "linux")]
mod linux_tests {
    use bytes::Bytes;
    use pingora_slice::raw_disk::{IoUringConfig, IoUringManager, IoUringBatchManager, RawDiskCache, IOBackend};
    use std::time::Duration;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_io_uring_basic_read_write() {
        // Create a temporary file
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Create io_uring manager
        let config = IoUringConfig::default();
        let manager = IoUringManager::new(path, config).await.unwrap();

        // Write data
        let data = b"Hello, io_uring!";
        manager.write_at(0, data).await.unwrap();

        // Read data back
        let read_data = manager.read_at(0, data.len()).await.unwrap();
        assert_eq!(&read_data[..], data);
    }

    #[tokio::test]
    async fn test_io_uring_multiple_writes() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let config = IoUringConfig::default();
        let manager = IoUringManager::new(path, config).await.unwrap();

        // Write multiple chunks
        let chunks = vec![
            (0, b"chunk1"),
            (100, b"chunk2"),
            (200, b"chunk3"),
        ];

        for (offset, data) in &chunks {
            manager.write_at(*offset, *data).await.unwrap();
        }

        // Read back and verify
        for (offset, expected) in &chunks {
            let data = manager.read_at(*offset, expected.len()).await.unwrap();
            assert_eq!(&data[..], *expected);
        }
    }

    #[tokio::test]
    async fn test_io_uring_batch_operations() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let config = IoUringConfig {
            queue_depth: 64,
            ..Default::default()
        };
        let batch_manager = IoUringBatchManager::new(path, config).await.unwrap();

        // Batch write
        let writes = vec![
            (0, Bytes::from("data1")),
            (100, Bytes::from("data2")),
            (200, Bytes::from("data3")),
        ];

        batch_manager.write_batch(writes.clone()).await.unwrap();

        // Batch read
        let locations: Vec<_> = writes.iter().map(|(offset, data)| (*offset, data.len())).collect();
        let results = batch_manager.read_batch(locations).await.unwrap();

        // Verify
        for (i, (_, expected)) in writes.iter().enumerate() {
            assert_eq!(&results[i], expected);
        }
    }

    #[tokio::test]
    async fn test_io_uring_config() {
        let config = IoUringConfig {
            queue_depth: 256,
            use_sqpoll: true,
            use_iopoll: false,
            block_size: 8192,
        };

        assert_eq!(config.queue_depth, 256);
        assert!(config.use_sqpoll);
        assert!(!config.use_iopoll);
        assert_eq!(config.block_size, 8192);
    }

    #[tokio::test]
    async fn test_raw_disk_cache_with_io_uring() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let config = IoUringConfig {
            queue_depth: 128,
            ..Default::default()
        };

        // Create cache with io_uring
        let cache = RawDiskCache::new_with_io_uring(
            path,
            10 * 1024 * 1024, // 10MB
            4096,
            Duration::from_secs(3600),
            config,
        ).await.unwrap();

        // Verify backend
        assert_eq!(cache.io_backend(), IOBackend::IoUring);

        // Store data
        let key = "test_key";
        let data = Bytes::from("test data with io_uring");
        cache.store_with_io_uring(key, data.clone()).await.unwrap();

        // Lookup data
        let result = cache.lookup_with_io_uring(key).await.unwrap();
        assert_eq!(result, Some(data));
    }

    #[tokio::test]
    async fn test_io_uring_large_data() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let config = IoUringConfig::default();
        let manager = IoUringManager::new(path, config).await.unwrap();

        // Write large data (1MB)
        let large_data = vec![0xAB; 1024 * 1024];
        manager.write_at(0, &large_data).await.unwrap();

        // Read back
        let read_data = manager.read_at(0, large_data.len()).await.unwrap();
        assert_eq!(read_data.len(), large_data.len());
        assert_eq!(&read_data[..], &large_data[..]);
    }

    #[tokio::test]
    async fn test_io_uring_sync() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let config = IoUringConfig::default();
        let manager = IoUringManager::new(path, config).await.unwrap();

        // Write and sync
        let data = b"sync test";
        manager.write_at(0, data).await.unwrap();
        manager.sync().await.unwrap();

        // Verify data persisted
        let read_data = manager.read_at(0, data.len()).await.unwrap();
        assert_eq!(&read_data[..], data);
    }

    #[tokio::test]
    async fn test_io_uring_batch_buffered_operations() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let config = IoUringConfig {
            queue_depth: 32,
            ..Default::default()
        };
        let batch_manager = IoUringBatchManager::new(path, config).await.unwrap();

        // Add buffered writes
        for i in 0..10 {
            let offset = i * 100;
            let data = Bytes::from(format!("buffered_{}", i));
            batch_manager.write_buffered(offset, data).await.unwrap();
        }

        // Flush
        let results = batch_manager.flush().await.unwrap();
        assert_eq!(results.len(), 10);

        // Verify all writes succeeded
        for result in results {
            match result {
                pingora_slice::raw_disk::io_uring_batch::OpResult::Write(Ok(())) => {},
                _ => panic!("Write operation failed"),
            }
        }
    }

    #[tokio::test]
    async fn test_io_uring_performance_comparison() {
        let temp_file1 = NamedTempFile::new().unwrap();
        let temp_file2 = NamedTempFile::new().unwrap();

        // Create cache with standard I/O
        let cache_standard = RawDiskCache::new(
            temp_file1.path(),
            10 * 1024 * 1024,
            4096,
            Duration::from_secs(3600),
        ).await.unwrap();

        // Create cache with io_uring
        let cache_io_uring = RawDiskCache::new_with_io_uring(
            temp_file2.path(),
            10 * 1024 * 1024,
            4096,
            Duration::from_secs(3600),
            IoUringConfig::default(),
        ).await.unwrap();

        // Test data
        let test_data = Bytes::from(vec![0xAB; 64 * 1024]); // 64KB

        // Store with standard I/O
        let start = std::time::Instant::now();
        for i in 0..100 {
            cache_standard.store(&format!("key_{}", i), test_data.clone()).await.unwrap();
        }
        let standard_duration = start.elapsed();

        // Store with io_uring
        let start = std::time::Instant::now();
        for i in 0..100 {
            cache_io_uring.store_with_io_uring(&format!("key_{}", i), test_data.clone()).await.unwrap();
        }
        let io_uring_duration = start.elapsed();

        println!("Standard I/O: {:?}", standard_duration);
        println!("io_uring: {:?}", io_uring_duration);

        // Note: io_uring should be faster, but we don't assert this in tests
        // as performance can vary based on system load
    }
}

#[cfg(not(target_os = "linux"))]
#[test]
fn test_io_uring_not_supported() {
    // On non-Linux platforms, io_uring should not be available
    // This test just ensures the code compiles on all platforms
    println!("io_uring is only supported on Linux");
}
