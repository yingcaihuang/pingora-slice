//! Example demonstrating the SubrequestManager
//!
//! This example shows how to use the SubrequestManager to fetch slices
//! from an origin server with retry logic and concurrent fetching.

use pingora_slice::{ByteRange, SliceSpec, SubrequestManager};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    println!("SubrequestManager Example");
    println!("=========================\n");

    // Create a SubrequestManager with 4 concurrent requests and 3 retries
    let manager = SubrequestManager::new(4, 3);
    println!("Created SubrequestManager with:");
    println!("  - Max concurrent requests: 4");
    println!("  - Max retries: 3\n");

    // Create some example slices
    let slices = vec![
        SliceSpec::new(0, ByteRange::new(0, 1023)?),
        SliceSpec::new(1, ByteRange::new(1024, 2047)?),
        SliceSpec::new(2, ByteRange::new(2048, 3071)?),
    ];

    println!("Example slices to fetch:");
    for slice in &slices {
        println!(
            "  Slice {}: bytes {}-{} (size: {} bytes)",
            slice.index,
            slice.range.start,
            slice.range.end,
            slice.range.size()
        );
    }
    println!();

    // Example URL - using httpbin.org which supports Range requests
    let url = "https://httpbin.org/bytes/4096";
    println!("Fetching from: {}\n", url);

    // Fetch all slices concurrently
    println!("Fetching slices...");
    match manager.fetch_slices(slices, url).await {
        Ok(results) => {
            println!("✓ Successfully fetched {} slices\n", results.len());
            
            for result in results {
                println!(
                    "Slice {}: {} bytes received (status: {})",
                    result.slice_index,
                    result.data.len(),
                    result.status
                );
                
                // Show Content-Range header if present
                if let Some(content_range) = result.headers.get("content-range") {
                    println!("  Content-Range: {:?}", content_range);
                }
            }
        }
        Err(e) => {
            eprintln!("✗ Error fetching slices: {}", e);
        }
    }

    println!("\nExample completed!");
    Ok(())
}
