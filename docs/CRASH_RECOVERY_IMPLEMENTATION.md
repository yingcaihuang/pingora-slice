# Crash Recovery Implementation

## Overview

This document describes the crash recovery functionality implemented for the Raw Disk Cache system. The crash recovery mechanism ensures that the cache can recover its state after unexpected shutdowns or crashes.

## Features Implemented

### 1. Automatic Recovery on Startup

When a `RawDiskCache` is created, it automatically attempts to recover the cache state by:

1. **Loading Metadata**: Attempts to load the metadata from disk
2. **Verifying Integrity**: Validates the loaded metadata and checks data integrity
3. **Rebuilding Allocator**: Reconstructs the block allocator state from the directory
4. **Handling Corruption**: Detects and removes corrupted entries

### 2. Metadata Verification

The recovery process verifies each cache entry by:

- Checking that offsets are within valid ranges
- Reading data from disk and verifying checksums
- Removing entries that fail verification
- Rebuilding the allocator state based on valid entries

### 3. Graceful Degradation

The system handles various failure scenarios:

- **Missing Metadata**: Starts with an empty cache
- **Corrupted Metadata**: Attempts to recover what it can, removes corrupted entries
- **Partial Corruption**: Removes only the corrupted entries, preserves valid ones
- **Fresh Cache**: Recognizes new caches and doesn't perform unnecessary scans

### 4. Disk Scanning (Future Enhancement)

The implementation includes a disk scanning capability that can:

- Scan the entire disk for valid block headers
- Recover block allocation state even without metadata
- Note: Cannot recover cache keys without metadata (keys are not stored on disk)

## Implementation Details

### Key Methods

#### `recover()`

Main recovery entry point that orchestrates the recovery process:

```rust
pub async fn recover(&self) -> Result<(), RawDiskError>
```

#### `verify_and_rebuild_allocator()`

Verifies metadata integrity and rebuilds allocator state:

```rust
async fn verify_and_rebuild_allocator(&self) -> Result<(), RawDiskError>
```

#### `scan_and_rebuild()`

Scans disk for valid entries (used when metadata is corrupted):

```rust
async fn scan_and_rebuild(&self) -> Result<(), RawDiskError>
```

### Allocator Enhancement

Added `mark_used()` method to `BlockAllocator` for recovery:

```rust
pub fn mark_used(&mut self, offset: u64, blocks: usize) -> Result<(), RawDiskError>
```

This allows the recovery process to reconstruct the allocator state from the directory.

### Metadata Size Adjustment

Adjusted the minimum metadata size from 1MB to 64KB to support smaller caches:

```rust
// Allocate 1% of total size for metadata, min 64KB, max 100MB
let metadata_size = ((total_size / 100).max(64 * 1024)).min(100 * 1024 * 1024);
```

## Testing

Comprehensive tests were added to verify crash recovery functionality:

### Test Coverage

#### Crash Recovery Tests (`tests/test_crash_recovery.rs`)

1. **test_normal_shutdown_and_restart**: Verifies clean recovery after proper shutdown with metadata save
   - Creates cache with 20 entries
   - Saves metadata before shutdown
   - Verifies all entries are recovered correctly

2. **test_crash_recovery_without_metadata_save**: Tests recovery when metadata wasn't saved (simulating crash)
   - Creates cache with 10 entries
   - Does NOT save metadata (simulating crash)
   - Verifies cache starts empty but disk space is preserved

3. **test_corrupted_metadata_recovery**: Tests handling of corrupted metadata
   - Creates cache with 5 entries and saves metadata
   - Corrupts metadata by writing garbage bytes
   - Verifies cache starts with empty state after corruption

4. **test_partial_metadata_corruption**: Tests recovery when some data blocks are corrupted
   - Creates cache with 10 entries
   - Corrupts one data block (not metadata)
   - Verifies corrupted entry is detected and removed

5. **test_recovery_with_empty_cache**: Tests recovery of empty caches
   - Creates empty cache and saves metadata
   - Verifies recovery works correctly with no entries

6. **test_recovery_preserves_allocator_state**: Verifies allocator state is correctly restored
   - Creates cache with large entries (50KB each)
   - Verifies free/used blocks are preserved after recovery
   - Confirms new allocations work after recovery

7. **test_full_crash_recovery_workflow**: End-to-end test of multiple crash/recovery cycles
   - Phase 1: Create cache with 20 entries
   - Phase 2: Recover and add 5 more entries
   - Phase 3: Recover again and verify all 25 entries

8. **test_recovery_after_multiple_crashes**: Tests multiple crash-recovery cycles
   - Simulates 3 crash-recovery cycles
   - Each cycle adds 5 new entries
   - Verifies all data from all cycles is preserved

9. **test_recovery_with_large_entries**: Tests recovery with large cache entries
   - Creates 5 entries of 100KB each
   - Verifies large data integrity after recovery
   - Confirms checksums are validated correctly

10. **test_recovery_with_mixed_operations**: Tests recovery after mixed operations
    - Adds 20 entries, removes 5, updates 5
    - Verifies correct state after recovery
    - Confirms removed entries stay removed

11. **test_recovery_with_superblock_intact**: Verifies superblock integrity after recovery
    - Checks that superblock parameters are preserved
    - Verifies operations continue normally after recovery

#### Metadata Persistence Tests (`tests/test_metadata_persistence.rs`)

1. **test_metadata_save_and_load**: Tests basic metadata save/load functionality
   - Creates cache with 10 entries
   - Saves and loads metadata
   - Verifies all entries are accessible

2. **test_metadata_persistence_with_updates**: Tests metadata persistence with modifications
   - Creates cache with 5 entries
   - Adds 5 more and removes 2
   - Verifies correct state after reload

3. **test_metadata_load_on_empty_cache**: Tests loading metadata from empty cache
   - Verifies no errors when loading from empty cache
   - Confirms cache starts with 0 entries

### Test Results

All tests pass successfully:

```
running 11 tests
test test_recovery_with_empty_cache ... ok
test test_corrupted_metadata_recovery ... ok
test test_recovery_preserves_allocator_state ... ok
test test_recovery_with_large_entries ... ok
test test_partial_metadata_corruption ... ok
test test_crash_recovery_without_metadata_save ... ok
test test_recovery_with_superblock_intact ... ok
test test_recovery_after_multiple_crashes ... ok
test test_normal_shutdown_and_restart ... ok
test test_recovery_with_mixed_operations ... ok
test test_full_crash_recovery_workflow ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Test Scenarios Covered

- ✅ Normal shutdown and restart
- ✅ Crash without metadata save
- ✅ Metadata corruption recovery
- ✅ Partial data corruption
- ✅ Empty cache recovery
- ✅ Allocator state preservation
- ✅ Multiple crash-recovery cycles
- ✅ Large entry recovery
- ✅ Mixed operations (add/remove/update)
- ✅ Superblock integrity
- ✅ Metadata serialization/deserialization

## Usage

The crash recovery is automatic and requires no additional code. Simply create a `RawDiskCache` instance:

```rust
let cache = RawDiskCache::new(
    "/path/to/cache",
    10 * 1024 * 1024,  // 10MB
    4096,               // 4KB blocks
    Duration::from_secs(3600),
).await?;

// Cache automatically recovers on creation
// Continue normal operations
```

## Best Practices

1. **Periodic Metadata Saves**: Call `save_metadata()` periodically to minimize data loss
2. **Graceful Shutdown**: Always call `save_metadata()` before shutdown when possible
3. **Monitor Logs**: Check logs for recovery warnings and corrupted entries
4. **Adequate Metadata Space**: Ensure metadata area is large enough for your cache size

## Limitations

1. **Key Recovery**: Cache keys cannot be recovered from disk scan (keys are not stored on disk)
2. **Scan Performance**: Full disk scans can be slow for large caches
3. **Partial Recovery**: Without metadata, only block allocation state can be recovered

## Future Enhancements

1. **Write-Ahead Logging**: Add WAL for better crash consistency
2. **Key Storage**: Optionally store keys on disk for full recovery
3. **Incremental Scanning**: Optimize disk scanning for large caches
4. **Recovery Metrics**: Add metrics for recovery operations
