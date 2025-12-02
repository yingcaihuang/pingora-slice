//! Example demonstrating configuration hot reload
//!
//! This example shows how to:
//! - Load configuration from a file
//! - Reload configuration and detect changes
//! - Handle configuration updates

use pingora_slice::config::SliceConfig;
use std::error::Error;
use std::fs;
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn Error>> {
    println!("=== Configuration Hot Reload Example ===\n");

    // Create a temporary config file
    let config_path = "/tmp/pingora_slice_example.yaml";
    let initial_config = r#"
slice_size: 1048576
max_concurrent_subrequests: 4
max_retries: 3
enable_cache: true
cache_ttl: 3600
l1_cache_size_bytes: 104857600
l2_cache_dir: "/var/cache/pingora-slice"
enable_l2_cache: true
l2_backend: "file"
upstream_address: "127.0.0.1:8080"
"#;

    fs::write(config_path, initial_config)?;
    println!("Created initial configuration at {}", config_path);

    // Load initial configuration
    let mut config = SliceConfig::from_file(config_path)?;
    println!("\n✓ Initial configuration loaded:");
    println!("  - slice_size: {}", config.slice_size);
    println!("  - max_concurrent_subrequests: {}", config.max_concurrent_subrequests);
    println!("  - cache_ttl: {}", config.cache_ttl);
    println!("  - l2_backend: {}", config.l2_backend);

    // Simulate running for a bit
    println!("\n⏳ Simulating server running...");
    thread::sleep(Duration::from_secs(2));

    // Update configuration file
    let updated_config = r#"
slice_size: 2097152
max_concurrent_subrequests: 8
max_retries: 5
enable_cache: true
cache_ttl: 7200
l1_cache_size_bytes: 209715200
l2_cache_dir: "/var/cache/pingora-slice"
enable_l2_cache: true
l2_backend: "raw_disk"
upstream_address: "127.0.0.1:8080"
raw_disk_cache:
  device_path: "/var/cache/pingora-slice-raw"
  total_size: 10737418240
  block_size: 4096
  use_direct_io: true
  enable_compression: true
  enable_prefetch: true
  enable_zero_copy: true
"#;

    fs::write(config_path, updated_config)?;
    println!("\n📝 Configuration file updated");

    // Reload configuration
    println!("\n🔄 Reloading configuration...");
    match config.reload_from_file(config_path) {
        Ok(changes) => {
            if changes.has_changes() {
                println!("\n✓ Configuration reloaded successfully!");
                println!("\n📊 Changes detected:");
                for change in changes.summary() {
                    println!("  - {}", change);
                }

                println!("\n📋 Updated values:");
                println!("  - slice_size: {} (was 1048576)", config.slice_size);
                println!(
                    "  - max_concurrent_subrequests: {} (was 4)",
                    config.max_concurrent_subrequests
                );
                println!("  - cache_ttl: {} (was 3600)", config.cache_ttl);
                println!("  - l2_backend: {} (was file)", config.l2_backend);

                if changes.requires_cache_restart() {
                    println!("\n⚠️  Warning: Some changes require cache restart:");
                    println!("  - Cache will need to be reinitialized");
                    println!("  - Existing cached data may be invalidated");
                }

                // Show raw disk configuration
                if let Some(ref raw_disk) = config.raw_disk_cache {
                    println!("\n🔧 Raw Disk Cache Configuration:");
                    println!("  - device_path: {}", raw_disk.device_path);
                    println!("  - total_size: {} bytes ({} GB)", 
                        raw_disk.total_size,
                        raw_disk.total_size / (1024 * 1024 * 1024)
                    );
                    println!("  - block_size: {} bytes", raw_disk.block_size);
                    println!("  - use_direct_io: {}", raw_disk.use_direct_io);
                    println!("  - enable_compression: {}", raw_disk.enable_compression);
                    println!("  - enable_prefetch: {}", raw_disk.enable_prefetch);
                    println!("  - enable_zero_copy: {}", raw_disk.enable_zero_copy);
                }
            } else {
                println!("\n✓ Configuration reloaded, but no changes detected");
            }
        }
        Err(e) => {
            println!("\n✗ Failed to reload configuration: {}", e);
            println!("  Configuration remains unchanged");
        }
    }

    // Test invalid configuration
    println!("\n\n=== Testing Invalid Configuration ===");
    let invalid_config = r#"
slice_size: 1024
max_concurrent_subrequests: 4
max_retries: 3
enable_cache: true
cache_ttl: 3600
l1_cache_size_bytes: 104857600
l2_cache_dir: "/var/cache/pingora-slice"
enable_l2_cache: true
l2_backend: "file"
upstream_address: "127.0.0.1:8080"
"#;

    fs::write(config_path, invalid_config)?;
    println!("\n📝 Updated config with invalid slice_size (1024 bytes, too small)");

    println!("\n🔄 Attempting to reload...");
    match config.reload_from_file(config_path) {
        Ok(_) => {
            println!("\n✗ Unexpected: Invalid configuration was accepted!");
        }
        Err(e) => {
            println!("\n✓ Configuration validation caught the error:");
            println!("  {}", e);
            println!("\n✓ Previous valid configuration is still active");
            println!("  - slice_size: {}", config.slice_size);
        }
    }

    // Clean up
    fs::remove_file(config_path)?;
    println!("\n\n✓ Cleaned up temporary config file");

    println!("\n=== Example Complete ===");
    println!("\nKey Takeaways:");
    println!("  1. Configuration can be reloaded without restarting");
    println!("  2. Changes are validated before being applied");
    println!("  3. Invalid configurations are rejected, keeping the current config");
    println!("  4. You can detect which settings changed and if restart is needed");

    Ok(())
}
