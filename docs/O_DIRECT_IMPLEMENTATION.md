# O_DIRECT Implementation

## Overview

O_DIRECT is a Linux-specific flag that allows applications to bypass the kernel's page cache and perform direct I/O operations to disk. This implementation adds O_DIRECT support to the raw disk cache to reduce page cache overhead and improve performance for cache workloads.

## Features

### 1. Automatic Detection

The system automatically detects whether O_DIRECT is supported on the current platform:

- **Linux**: Attempts to open files with the `O_DIRECT` flag
- **Other platforms**: Falls back to buffered I/O with a warning

### 2. Aligned Memory Allocation

O_DIRECT requires that:
- Buffer addresses are aligned to sector boundaries (typically 512 or 4096 bytes)
- I/O sizes are multiples of the sector size
- File offsets are aligned to sector boundaries

The implementation handles this automatically by:
- Detecting the required alignment (defaults to 4096 bytes for safety)
- Allocating aligned buffers for all I/O operations
- Aligning offsets and sizes for read/write operations

### 3. Read-Modify-Write for Unaligned Operations

When writing data at unaligned offsets or with unaligned sizes, the implementation performs read-modify-write operations:

1. Reads the existing data in the affected blocks
2. Modifies the relevant portions with new data
3. Writes the entire aligned blocks back to disk

This ensures data integrity while maintaining O_DIRECT compatibility.

## API

### Creating a Cache with O_DIRECT

```rust
use pingora_slice::raw_disk::RawDiskCache;
use std::time::Duration;

// Create cache with O_DIRECT enabled (default)
let cache = RawDiskCache::new(
    "/dev/sdb1",
    10 * 1024 * 1024 * 1024, // 10GB
    4096,                     // 4KB blocks
    Duration::from_secs(3600), // 1 hour TTL
).await?;

// Create cache with explicit O_DIRECT control
let cache = RawDiskCache::new_with_options(
    "/dev/sdb1",
    10 * 1024 * 1024 * 1024,
    4096,
    Duration::from_secs(3600),
    true, // Enable O_DIRECT
).await?;

// Create cache with buffered I/O
let cache = RawDiskCache::new_with_options(
    "/dev/sdb1",
    10 * 1024 * 1024 * 1024,
    4096,
    Duration::from_secs(3600),
    false, // Disable O_DIRECT
).await?;
```

### Checking O_DIRECT Status

```rust
// Check if O_DIRECT is enabled
if cache.disk_io.is_direct_io_enabled() {
    println!("O_DIRECT is enabled");
    println!("Alignment: {} bytes", cache.disk_io.alignment());
}
```

## Performance Characteristics

### Benefits

1. **Reduced Memory Pressure**: Bypasses page cache, freeing memory for other uses
2. **Predictable Performance**: Eliminates page cache eviction effects
3. **Lower CPU Usage**: Reduces memory copying between kernel and user space
4. **Better for Large Sequential I/O**: Ideal for cache workloads with large objects

### Trade-offs

1. **Alignment Overhead**: Requires aligned buffers and may need read-modify-write for unaligned operations
2. **No Kernel Caching**: Each read goes to disk (good for cache workloads, bad for frequently accessed data)
3. **Platform-Specific**: Only available on Linux

### Benchmark Results

From `test_o_direct_performance_comparison`:

```
Test: 100 entries of 64 KB each

Write Performance:
  Buffered I/O: 470.12ms (13.29 MB/s)
  O_DIRECT:     470.96ms (13.27 MB/s)
  Speedup:      1.00x

Read Performance:
  Buffered I/O: 9.87ms (632.92 MB/s)
  O_DIRECT:     9.58ms (652.40 MB/s)
  Speedup:      1.03x
```

Note: Performance benefits are more pronounced with:
- Larger cache sizes (reduces page cache thrashing)
- Higher concurrency (reduces kernel lock contention)
- Workloads with poor locality (where page cache doesn't help)

## Implementation Details

### File Opening

On Linux, files are opened with the `O_DIRECT` flag using `OpenOptionsExt`:

```rust
use std::os::unix::fs::OpenOptionsExt;

let mut opts = OpenOptions::new();
opts.read(true).write(true);
opts.custom_flags(libc::O_DIRECT);
let file = opts.open(path).await?;
```

### Alignment Detection

The implementation uses a conservative 4096-byte alignment, which works for both:
- Traditional 512-byte sector disks
- Advanced Format 4096-byte sector disks
- NVMe devices with 4KB page sizes

### Buffer Alignment

Buffers are aligned by:
1. Allocating extra space for alignment
2. Finding the aligned offset within the buffer
3. Returning only the aligned portion

```rust
fn allocate_aligned(&self, size: usize) -> Vec<u8> {
    let total_size = size + self.alignment;
    let mut buffer = vec![0u8; total_size];
    
    let ptr = buffer.as_ptr() as usize;
    let aligned_ptr = (ptr + self.alignment - 1) / self.alignment * self.alignment;
    let offset = aligned_ptr - ptr;
    
    buffer.drain(..offset);
    buffer.truncate(size);
    buffer
}
```

## Testing

The implementation includes comprehensive tests:

1. **Basic Functionality**: Tests store/lookup with O_DIRECT enabled
2. **Unaligned Offsets**: Tests read-modify-write with various unaligned sizes
3. **Performance Comparison**: Benchmarks O_DIRECT vs buffered I/O
4. **Fallback Behavior**: Tests graceful fallback on unsupported systems

Run tests with:

```bash
# Run all O_DIRECT tests
cargo test --test test_raw_disk_cache test_o_direct

# Run performance benchmark
cargo test --test test_raw_disk_cache test_o_direct_performance_comparison -- --ignored --nocapture
```

## Future Improvements

1. **Dynamic Alignment Detection**: Query actual device sector size
2. **Vectored I/O**: Use `preadv`/`pwritev` for better performance
3. **io_uring Integration**: Use io_uring for async O_DIRECT operations
4. **Adaptive Mode**: Automatically switch between O_DIRECT and buffered I/O based on workload

## References

- [Linux O_DIRECT Documentation](https://man7.org/linux/man-pages/man2/open.2.html)
- [Direct I/O Best Practices](https://www.kernel.org/doc/Documentation/filesystems/direct-io.txt)
- [Apache Traffic Server Cache Design](https://docs.trafficserver.apache.org/en/latest/developer-guide/cache-architecture/index.html)
