//! Example demonstrating the MetadataFetcher usage

use pingora_slice::MetadataFetcher;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a new MetadataFetcher
    let fetcher = MetadataFetcher::new()?;

    // Example URL (using httpbin for demonstration)
    let url = "https://httpbin.org/bytes/1024";

    println!("Fetching metadata for: {}", url);

    // Fetch metadata
    match fetcher.fetch_metadata(url).await {
        Ok(metadata) => {
            println!("\n✓ Metadata fetched successfully:");
            println!("  Content-Length: {} bytes", metadata.content_length);
            println!("  Supports Range: {}", metadata.supports_range);
            
            if let Some(content_type) = &metadata.content_type {
                println!("  Content-Type: {}", content_type);
            }
            
            if let Some(etag) = &metadata.etag {
                println!("  ETag: {}", etag);
            }
            
            if let Some(last_modified) = &metadata.last_modified {
                println!("  Last-Modified: {}", last_modified);
            }
        }
        Err(e) => {
            eprintln!("\n✗ Failed to fetch metadata: {}", e);
        }
    }

    Ok(())
}
