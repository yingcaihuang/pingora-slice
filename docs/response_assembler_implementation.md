# ResponseAssembler Implementation

## Overview

The `ResponseAssembler` is responsible for assembling slices received from the origin server and preparing them for streaming to the client. It handles:

1. Building appropriate HTTP response headers (200 or 206)
2. Assembling slices that may arrive out of order
3. Validating completeness of received slices
4. Preparing slices for ordered streaming

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                  ResponseAssembler                      │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  ┌───────────────────────────────────────────────┐    │
│  │  build_response_header()                      │    │
│  │  - Determines status code (200 or 206)        │    │
│  │  - Sets Content-Length                        │    │
│  │  - Sets Content-Range (for 206)               │    │
│  │  - Sets Content-Type, ETag, etc.              │    │
│  └───────────────────────────────────────────────┘    │
│                                                         │
│  ┌───────────────────────────────────────────────┐    │
│  │  assemble_slices()                            │    │
│  │  - Takes SubrequestResults (may be unordered) │    │
│  │  - Organizes into BTreeMap by index           │    │
│  │  - Ensures correct ordering                   │    │
│  └───────────────────────────────────────────────┘    │
│                                                         │
│  ┌───────────────────────────────────────────────┐    │
│  │  stream_slices()                              │    │
│  │  - Converts BTreeMap to ordered Vec           │    │
│  │  - Ready for sequential streaming             │    │
│  └───────────────────────────────────────────────┘    │
│                                                         │
│  ┌───────────────────────────────────────────────┐    │
│  │  validate_completeness()                      │    │
│  │  - Checks all expected slices present         │    │
│  │  - Verifies contiguous indices                │    │
│  └───────────────────────────────────────────────┘    │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

## Key Features

### 1. Response Header Building

The assembler builds appropriate HTTP response headers based on whether the client requested a range or the full file:

**Full File Response (200 OK):**
- Status: 200 OK
- Content-Length: Total file size
- Content-Type: From metadata
- Accept-Ranges: bytes
- ETag, Last-Modified: From metadata

**Range Response (206 Partial Content):**
- Status: 206 Partial Content
- Content-Length: Size of requested range
- Content-Range: bytes start-end/total
- Content-Type: From metadata
- Accept-Ranges: bytes
- ETag, Last-Modified: From metadata

### 2. Out-of-Order Slice Handling

Slices may arrive out of order due to concurrent fetching. The assembler uses a `BTreeMap<usize, Bytes>` to:
- Store slices by their index
- Automatically maintain sorted order
- Enable efficient ordered iteration

**Example:**
```rust
// Slices arrive: [2, 0, 1]
let results = vec![
    SubrequestResult { slice_index: 2, data: ... },
    SubrequestResult { slice_index: 0, data: ... },
    SubrequestResult { slice_index: 1, data: ... },
];

// BTreeMap ensures correct order: [0, 1, 2]
let assembled = assembler.assemble_slices(results);
```

### 3. Completeness Validation

Before streaming, the assembler can validate that all expected slices are present:
- Checks that the count matches expected
- Verifies indices are contiguous (0, 1, 2, ..., n-1)
- Returns error if any slices are missing

### 4. Streaming Preparation

The `stream_slices()` method converts the BTreeMap into a Vec for sequential streaming:
```rust
let assembled = assembler.assemble_slices(results);
let ordered_slices = assembler.stream_slices(assembled);

// Now ready for streaming to client
for slice_data in ordered_slices {
    // Send to client
}
```

## Usage Example

```rust
use pingora_slice::{ResponseAssembler, FileMetadata, ByteRange};

// Create assembler
let assembler = ResponseAssembler::new();

// Build response headers for full file
let metadata = FileMetadata::new(10240, true);
let (status, headers) = assembler.build_response_header(&metadata, None)?;

// Or for a range request
let range = ByteRange::new(0, 1023)?;
let (status, headers) = assembler.build_response_header(&metadata, Some(range))?;

// Assemble slices (may be out of order)
let assembled = assembler.assemble_slices(subrequest_results);

// Validate completeness
assembler.validate_completeness(&assembled, expected_count)?;

// Prepare for streaming
let ordered_slices = assembler.stream_slices(assembled);
```

## Implementation Details

### Data Structures

```rust
pub struct ResponseAssembler;

impl ResponseAssembler {
    pub fn new() -> Self;
    
    pub fn build_response_header(
        &self,
        metadata: &FileMetadata,
        client_range: Option<ByteRange>,
    ) -> Result<(StatusCode, HeaderMap)>;
    
    pub fn assemble_slices(
        &self,
        slice_results: Vec<SubrequestResult>
    ) -> BTreeMap<usize, Bytes>;
    
    pub fn stream_slices(
        &self,
        assembled_slices: BTreeMap<usize, Bytes>
    ) -> Vec<Bytes>;
    
    pub fn validate_completeness(
        &self,
        assembled_slices: &BTreeMap<usize, Bytes>,
        expected_count: usize,
    ) -> Result<()>;
}
```

### Error Handling

The assembler returns `SliceError::AssemblyError` for:
- Invalid header values
- Invalid ranges (beyond file size)
- Missing slices
- Non-contiguous slice indices

### Performance Considerations

1. **BTreeMap for Ordering**: O(log n) insertion, O(n) iteration in order
2. **Zero-copy where possible**: Uses `Bytes` for efficient data handling
3. **Lazy validation**: Completeness check is optional, only when needed
4. **Minimal buffering**: Slices can be streamed as soon as they're assembled

## Integration with Other Components

### With SubrequestManager

```rust
// Fetch slices concurrently
let results = subrequest_manager.fetch_slices(slices, url).await?;

// Assemble (handles out-of-order arrival)
let assembled = assembler.assemble_slices(results);
```

### With Cache

```rust
// Combine cached and fetched slices
let mut all_slices = BTreeMap::new();

// Add cached slices
for (idx, data) in cached_slices {
    all_slices.insert(idx, data);
}

// Add newly fetched slices
let assembled = assembler.assemble_slices(fetch_results);
for (idx, data) in assembled {
    all_slices.insert(idx, data);
}

// Stream all slices
let ordered = assembler.stream_slices(all_slices);
```

## Testing

The implementation includes comprehensive unit tests:

1. **Header Building Tests**
   - Full file response (200)
   - Range response (206)
   - Invalid range handling
   - Header completeness

2. **Assembly Tests**
   - Ordered slices
   - Out-of-order slices
   - Empty results
   - Single slice

3. **Validation Tests**
   - Complete slice sets
   - Missing slices
   - Wrong counts
   - Non-contiguous indices

4. **Streaming Tests**
   - Correct order preservation
   - Data integrity

## Requirements Satisfied

This implementation satisfies the following requirements from the design document:

- **Requirement 6.1**: Immediate streaming capability
- **Requirement 6.2**: Correct byte order maintenance
- **Requirement 6.3**: Out-of-order slice buffering
- **Requirement 6.4**: Complete response assembly
- **Requirement 6.5**: Appropriate response headers
- **Requirement 10.4**: 206 response format for range requests

## Future Enhancements

1. **Streaming Optimization**: Stream slices as they arrive if they're in order
2. **Memory Management**: Implement backpressure for large files
3. **Partial Streaming**: Start streaming before all slices arrive
4. **Compression**: Support for compressed slice streaming
5. **Metrics**: Track assembly time and buffer sizes

## See Also

- [SubrequestManager Implementation](./subrequest_manager_implementation.md)
- [Cache Implementation](./cache_implementation.md)
- [Design Document](../.kiro/specs/pingora-slice/design.md)
