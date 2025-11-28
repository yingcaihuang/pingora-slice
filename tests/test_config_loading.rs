use pingora_slice::config::SliceConfig;

#[test]
fn test_load_example_config() {
    let config = SliceConfig::from_file("examples/pingora_slice.yaml");
    assert!(config.is_ok(), "Failed to load example config: {:?}", config.err());
    
    let config = config.unwrap();
    assert_eq!(config.slice_size, 1048576);
    assert_eq!(config.max_concurrent_subrequests, 4);
    assert_eq!(config.max_retries, 3);
    assert!(config.enable_cache);
    assert_eq!(config.cache_ttl, 3600);
    assert_eq!(config.upstream_address, "origin.example.com:80");
    assert_eq!(config.slice_patterns.len(), 5);
}

#[test]
fn test_load_minimal_config() {
    // Create a minimal config file
    let minimal_yaml = r#"
slice_size: 524288
"#;
    
    std::fs::write("test_minimal.yaml", minimal_yaml).unwrap();
    
    let config = SliceConfig::from_file("test_minimal.yaml");
    assert!(config.is_ok());
    
    let config = config.unwrap();
    assert_eq!(config.slice_size, 524288);
    // Check defaults are applied
    assert_eq!(config.max_concurrent_subrequests, 4);
    assert_eq!(config.max_retries, 3);
    assert!(config.enable_cache);
    
    // Cleanup
    std::fs::remove_file("test_minimal.yaml").unwrap();
}

#[test]
fn test_load_invalid_config() {
    let invalid_yaml = r#"
slice_size: 1024
"#;
    
    std::fs::write("test_invalid.yaml", invalid_yaml).unwrap();
    
    let config = SliceConfig::from_file("test_invalid.yaml");
    assert!(config.is_err(), "Should fail validation for slice_size < 64KB");
    
    // Cleanup
    std::fs::remove_file("test_invalid.yaml").unwrap();
}

#[test]
fn test_load_nonexistent_file() {
    let config = SliceConfig::from_file("nonexistent.yaml");
    assert!(config.is_err(), "Should fail when file doesn't exist");
}
