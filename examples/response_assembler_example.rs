//! Example demonstrating the ResponseAssembler component
//!
//! This example shows how to:
//! 1. Build response headers for full file and range requests
//! 2. Assemble slices that may arrive out of order
//! 3. Stream slices in the correct order
//! 4. Validate completeness of assembled slices

use bytes::Bytes;
use http::HeaderMap;
use pingora_slice::{ByteRange, FileMetadata, ResponseAssembler, SubrequestResult};
use std::collections::BTreeMap;

fn main() {
    println!("=== ResponseAssembler Example ===\n");

    // Create a ResponseAssembler
    let assembler = ResponseAssembler::new();

    // Example 1: Build response headers for a full file request
    println!("Example 1: Full file response headers");
    let metadata = FileMetadata::with_headers(
        10240,
        true,
        Some("application/octet-stream".to_string()),
        Some("\"abc123\"".to_string()),
        Some("Wed, 21 Oct 2015 07:28:00 GMT".to_string()),
    );

    match assembler.build_response_header(&metadata, None) {
        Ok((status, headers)) => {
            println!("  Status: {}", status);
            println!("  Content-Length: {:?}", headers.get("content-length"));
            println!("  Content-Type: {:?}", headers.get("content-type"));
            println!("  Accept-Ranges: {:?}", headers.get("accept-ranges"));
            println!("  ETag: {:?}", headers.get("etag"));
        }
        Err(e) => println!("  Error: {}", e),
    }

    // Example 2: Build response headers for a range request
    println!("\nExample 2: Range request response headers (206)");
    let range = ByteRange::new(0, 1023).unwrap();
    match assembler.build_response_header(&metadata, Some(range)) {
        Ok((status, headers)) => {
            println!("  Status: {}", status);
            println!("  Content-Length: {:?}", headers.get("content-length"));
            println!("  Content-Range: {:?}", headers.get("content-range"));
            println!("  Accept-Ranges: {:?}", headers.get("accept-ranges"));
        }
        Err(e) => println!("  Error: {}", e),
    }

    // Example 3: Assemble slices that arrive out of order
    println!("\nExample 3: Assembling out-of-order slices");
    
    // Simulate slices arriving out of order
    let results = vec![
        SubrequestResult {
            slice_index: 2,
            data: Bytes::from("Third slice data"),
            status: 206,
            headers: HeaderMap::new(),
        },
        SubrequestResult {
            slice_index: 0,
            data: Bytes::from("First slice data"),
            status: 206,
            headers: HeaderMap::new(),
        },
        SubrequestResult {
            slice_index: 1,
            data: Bytes::from("Second slice data"),
            status: 206,
            headers: HeaderMap::new(),
        },
    ];

    println!("  Slices arrived in order: [2, 0, 1]");
    let assembled = assembler.assemble_slices(results);
    println!("  Assembled {} slices", assembled.len());

    // Example 4: Stream slices in correct order
    println!("\nExample 4: Streaming slices in order");
    let streamed = assembler.stream_slices(assembled.clone());
    for (i, data) in streamed.iter().enumerate() {
        println!("  Slice {}: {:?}", i, String::from_utf8_lossy(data));
    }

    // Example 5: Validate completeness
    println!("\nExample 5: Validating slice completeness");
    match assembler.validate_completeness(&assembled, 3) {
        Ok(()) => println!("  ✓ All slices present and accounted for"),
        Err(e) => println!("  ✗ Validation failed: {}", e),
    }

    // Example 6: Detect missing slices
    println!("\nExample 6: Detecting missing slices");
    let mut incomplete_slices = BTreeMap::new();
    incomplete_slices.insert(0, Bytes::from("First slice"));
    incomplete_slices.insert(2, Bytes::from("Third slice"));
    // Missing slice 1

    match assembler.validate_completeness(&incomplete_slices, 3) {
        Ok(()) => println!("  ✓ All slices present"),
        Err(e) => println!("  ✗ Validation failed (expected): {}", e),
    }

    // Example 7: Invalid range handling
    println!("\nExample 7: Handling invalid range requests");
    let invalid_range = ByteRange::new(10240, 20000).unwrap();
    match assembler.build_response_header(&metadata, Some(invalid_range)) {
        Ok(_) => println!("  Unexpected success"),
        Err(e) => println!("  ✓ Correctly rejected invalid range: {}", e),
    }

    println!("\n=== Example Complete ===");
}
