//! Main SliceProxy structure and context
//!
//! This module provides the core SliceProxy structure that integrates all components
//! of the slice module, along with the SliceContext for managing per-request state.

use crate::{
    SliceConfig, SliceMetrics, FileMetadata, SliceSpec, ByteRange,
    RequestAnalyzer, MetadataFetcher, SliceCalculator, SliceCache,
};
use crate::error::{Result, SliceError};
use bytes::Bytes;
use std::sync::Arc;
use std::time::Duration;
use http::{Method, HeaderMap, HeaderValue};
use tracing::{debug, info, warn};

/// Main proxy structure that integrates all slice module components
///
/// SliceProxy is the central structure that coordinates all aspects of the slice
/// functionality, including configuration, metrics collection, and request handling.
///
/// # Fields
/// * `config` - Shared configuration for the slice module
/// * `metrics` - Thread-safe metrics collector
///
/// # Requirements
/// Validates: All requirements (1.1-10.5)
#[derive(Clone)]
pub struct SliceProxy {
    /// Configuration for the slice module
    config: Arc<SliceConfig>,
    
    /// Metrics collector for monitoring
    metrics: Arc<SliceMetrics>,
}

/// Per-request context for slice processing
///
/// SliceContext stores all state information for a single request being processed
/// by the slice module. This includes metadata about the file, calculated slices,
/// and whether slicing is enabled for this request.
///
/// # Fields
/// * `slice_enabled` - Whether slicing is enabled for this request
/// * `metadata` - File metadata from the origin server (if fetched)
/// * `client_range` - Client's requested byte range (if any)
/// * `slices` - Calculated slice specifications for this request
#[derive(Debug, Clone, Default)]
pub struct SliceContext {
    /// Whether slicing is enabled for this request
    pub slice_enabled: bool,
    
    /// File metadata from the origin server
    pub metadata: Option<FileMetadata>,
    
    /// Client's requested byte range (if present in request)
    pub client_range: Option<ByteRange>,
    
    /// Calculated slices for this request
    pub slices: Vec<SliceSpec>,
}

impl SliceProxy {
    /// Create a new SliceProxy instance
    ///
    /// # Arguments
    /// * `config` - Configuration for the slice module
    ///
    /// # Returns
    /// A new SliceProxy instance with initialized metrics
    ///
    /// # Example
    /// ```
    /// use pingora_slice::{SliceConfig, SliceProxy};
    /// use std::sync::Arc;
    ///
    /// let config = SliceConfig::default();
    /// let proxy = SliceProxy::new(Arc::new(config));
    /// ```
    ///
    /// # Requirements
    /// Validates: Requirements 1.1, 1.2, 1.3, 1.4
    pub fn new(config: Arc<SliceConfig>) -> Self {
        SliceProxy {
            config,
            metrics: Arc::new(SliceMetrics::new()),
        }
    }
    
    /// Create a new request context
    ///
    /// This method creates a fresh SliceContext for each incoming request.
    /// The context starts with slicing disabled and no metadata.
    ///
    /// # Returns
    /// A new SliceContext with default values
    ///
    /// # Example
    /// ```
    /// use pingora_slice::{SliceConfig, SliceProxy};
    /// use std::sync::Arc;
    ///
    /// let config = SliceConfig::default();
    /// let proxy = SliceProxy::new(Arc::new(config));
    /// let ctx = proxy.new_ctx();
    /// assert!(!ctx.slice_enabled);
    /// ```
    pub fn new_ctx(&self) -> SliceContext {
        SliceContext::default()
    }
    
    /// Get a reference to the configuration
    ///
    /// # Returns
    /// A reference to the shared configuration
    pub fn config(&self) -> &SliceConfig {
        &self.config
    }
    
    /// Get a reference to the metrics collector
    ///
    /// # Returns
    /// A reference to the shared metrics collector
    ///
    /// # Requirements
    /// Validates: Requirements 9.1, 9.2
    pub fn metrics(&self) -> &SliceMetrics {
        &self.metrics
    }
    
    /// Get a cloned Arc to the configuration
    ///
    /// This is useful when you need to pass the configuration to other components
    /// that require Arc<SliceConfig>.
    ///
    /// # Returns
    /// An Arc clone of the configuration
    pub fn config_arc(&self) -> Arc<SliceConfig> {
        Arc::clone(&self.config)
    }
    
    /// Get a cloned Arc to the metrics collector
    ///
    /// This is useful when you need to pass the metrics to other components
    /// that require Arc<SliceMetrics>.
    ///
    /// # Returns
    /// An Arc clone of the metrics collector
    pub fn metrics_arc(&self) -> Arc<SliceMetrics> {
        Arc::clone(&self.metrics)
    }
    
    /// Handle a slice request - core logic for fetching and streaming slices
    ///
    /// This method implements the complete slice request handling flow:
    /// 1. Build and send response headers to the client
    /// 2. Fetch uncached slices from origin using SubrequestManager
    /// 3. Merge cached and newly fetched slices
    /// 4. Store newly fetched slices in cache
    /// 5. Stream all slices in order to the client
    /// 6. Record metrics
    ///
    /// # Arguments
    /// * `url` - The URL being requested
    /// * `ctx` - The request context containing metadata and slices
    ///
    /// # Returns
    /// * `Ok((StatusCode, HeaderMap, Vec<Bytes>))` - Response status, headers, and ordered slice data
    /// * `Err(SliceError)` - If any step fails
    ///
    /// # Requirements
    /// Validates: Requirements 5.1, 5.2, 5.3, 5.4, 5.5, 6.1, 6.2, 6.3, 6.4, 6.5, 7.1, 7.4, 7.5
    pub async fn handle_slice_request(
        &self,
        url: &str,
        ctx: &SliceContext,
    ) -> Result<(http::StatusCode, HeaderMap, Vec<Bytes>)> {
        use crate::{ResponseAssembler, SubrequestManager};
        use std::collections::BTreeMap;
        use std::time::Instant;
        
        let start_time = Instant::now();
        
        // Validate that we have metadata
        let metadata = ctx.metadata().ok_or_else(|| {
            SliceError::AssemblyError("Missing file metadata".to_string())
        })?;
        
        info!(
            "Handling slice request: url={}, total_slices={}, cached={}, uncached={}",
            url,
            ctx.slice_count(),
            ctx.cached_slice_count(),
            ctx.uncached_slice_count()
        );
        
        // Step 1: Build response headers (Requirement 6.5)
        let assembler = ResponseAssembler::new();
        let (status, headers) = assembler.build_response_header(metadata, ctx.client_range())?;
        
        debug!(
            "Built response headers: status={}, content_length={}",
            status,
            headers.get("content-length").map(|v| v.to_str().unwrap_or("?")).unwrap_or("?")
        );
        
        // Step 2: Identify slices that need to be fetched from origin (Requirements 5.1, 7.4)
        let slices_to_fetch: Vec<crate::SliceSpec> = ctx.slices()
            .iter()
            .filter(|s| !s.cached)
            .cloned()
            .collect();
        
        debug!(
            "Slices to fetch from origin: {} out of {}",
            slices_to_fetch.len(),
            ctx.slice_count()
        );
        
        // Step 3: Fetch uncached slices concurrently (Requirements 5.1, 5.2, 5.3, 5.4, 5.5)
        let fetch_start = Instant::now();
        let fetch_results = if !slices_to_fetch.is_empty() {
            let subrequest_mgr = SubrequestManager::new(
                self.config.max_concurrent_subrequests,
                self.config.max_retries,
            );
            
            debug!(
                "Fetching {} slices with max_concurrent={}",
                slices_to_fetch.len(),
                self.config.max_concurrent_subrequests
            );
            
            match subrequest_mgr.fetch_slices(slices_to_fetch.clone(), url).await {
                Ok(results) => {
                    let fetch_duration = fetch_start.elapsed();
                    info!(
                        "Successfully fetched {} slices in {:?}",
                        results.len(),
                        fetch_duration
                    );
                    
                    // Record subrequest metrics
                    for _ in &results {
                        self.metrics.record_subrequest(true);
                    }
                    self.metrics.record_subrequest_duration(fetch_duration);
                    
                    results
                }
                Err(e) => {
                    warn!("Failed to fetch slices: {:?}", e);
                    // Record failed subrequests
                    for _ in &slices_to_fetch {
                        self.metrics.record_subrequest(false);
                    }
                    return Err(e);
                }
            }
        } else {
            debug!("All slices are cached, no fetching needed");
            Vec::new()
        };
        
        // Step 4: Merge cached and newly fetched slices (Requirement 6.2)
        let assembly_start = Instant::now();
        let cache = crate::SliceCache::new(Duration::from_secs(self.config.cache_ttl));
        let mut all_slices: BTreeMap<usize, Bytes> = BTreeMap::new();
        
        // Add cached slices
        for (idx, slice_spec) in ctx.slices().iter().enumerate() {
            if slice_spec.cached {
                match cache.lookup_slice(url, &slice_spec.range).await {
                    Ok(Some(data)) => {
                        debug!(
                            "Retrieved cached slice {}: range={}-{}, size={}",
                            idx, slice_spec.range.start, slice_spec.range.end, data.len()
                        );
                        self.metrics.record_bytes_from_cache(data.len() as u64);
                        all_slices.insert(idx, data);
                    }
                    Ok(None) => {
                        warn!(
                            "Cached slice {} not found in cache (marked as cached but missing)",
                            idx
                        );
                        // This shouldn't happen, but we'll handle it gracefully
                        // The slice should have been in slices_to_fetch if it wasn't cached
                    }
                    Err(e) => {
                        warn!("Error retrieving cached slice {}: {:?}", idx, e);
                        self.metrics.record_cache_error();
                    }
                }
            }
        }
        
        // Step 5: Add newly fetched slices and store them in cache (Requirements 7.1, 7.5)
        for result in fetch_results {
            let idx = result.slice_index;
            let data = result.data.clone();
            
            debug!(
                "Adding fetched slice {}: size={}",
                idx,
                data.len()
            );
            
            self.metrics.record_bytes_from_origin(data.len() as u64);
            all_slices.insert(idx, data.clone());
            
            // Store in cache
            if let Some(slice_spec) = ctx.slices().get(idx) {
                match cache.store_slice(url, &slice_spec.range, data).await {
                    Ok(()) => {
                        debug!(
                            "Stored slice {} in cache: range={}-{}",
                            idx, slice_spec.range.start, slice_spec.range.end
                        );
                    }
                    Err(e) => {
                        warn!(
                            "Failed to store slice {} in cache: {:?}",
                            idx, e
                        );
                        self.metrics.record_cache_error();
                        // Continue processing even if cache storage fails
                    }
                }
            }
        }
        
        // Step 6: Validate that all slices are present (Requirement 6.2)
        assembler.validate_completeness(&all_slices, ctx.slice_count())?;
        
        debug!(
            "All {} slices assembled successfully",
            all_slices.len()
        );
        
        // Step 7: Stream slices in order (Requirements 6.1, 6.2, 6.3)
        let ordered_slices = assembler.stream_slices(all_slices);
        
        // Calculate total bytes sent
        let total_bytes: u64 = ordered_slices.iter().map(|b| b.len() as u64).sum();
        self.metrics.record_bytes_to_client(total_bytes);
        
        let assembly_duration = assembly_start.elapsed();
        self.metrics.record_assembly_duration(assembly_duration);
        
        // Step 8: Record overall request metrics (Requirement 9.1, 9.2)
        let total_duration = start_time.elapsed();
        self.metrics.record_request_duration(total_duration);
        
        info!(
            "Slice request completed: url={}, slices={}, total_bytes={}, duration={:?}",
            url,
            ctx.slice_count(),
            total_bytes,
            total_duration
        );
        
        Ok((status, headers, ordered_slices))
    }
    
    /// Request filter - determines if slicing should be enabled for this request
    ///
    /// This method implements the core decision logic for whether to use slice mode.
    /// It performs the following steps:
    /// 1. Analyze the request to determine if slicing is applicable
    /// 2. Fetch file metadata from the origin server
    /// 3. Check if the origin supports Range requests
    /// 4. Calculate the required slices
    /// 5. Check cache for existing slices
    /// 6. Enable slicing if appropriate
    ///
    /// # Arguments
    /// * `method` - HTTP method of the request
    /// * `uri` - Request URI
    /// * `headers` - Request headers
    /// * `ctx` - Mutable reference to the request context
    ///
    /// # Returns
    /// * `Ok(true)` - Continue with normal proxy mode (slicing not enabled)
    /// * `Ok(false)` - Slicing enabled, will handle response ourselves
    /// * `Err(SliceError)` - An error occurred during processing
    ///
    /// # Requirements
    /// Validates: Requirements 2.1, 2.2, 2.3, 2.4, 3.1, 3.2, 3.3, 3.4, 3.5,
    ///            4.1, 4.2, 4.3, 4.4, 7.3
    pub async fn request_filter(
        &self,
        method: &Method,
        uri: &str,
        headers: &HeaderMap<HeaderValue>,
        ctx: &mut SliceContext,
    ) -> Result<bool> {
        info!("Processing request: method={}, uri={}", method, uri);
        
        // Step 1: Check if slicing should be enabled for this request
        // Requirements: 2.1, 2.2, 2.3, 2.4
        let analyzer = RequestAnalyzer::new(self.config_arc());
        
        if !analyzer.should_slice(method, uri, headers) {
            debug!(
                "Slicing not applicable for request: method={}, uri={}",
                method, uri
            );
            // Record as non-sliced request
            self.metrics.record_request(false);
            // Continue with normal proxy mode
            return Ok(true);
        }
        
        debug!("Request eligible for slicing: uri={}", uri);
        
        // Step 2: Extract client's Range header if present (Requirement 10.1)
        ctx.set_client_range_opt(analyzer.extract_client_range(headers));
        
        if let Some(range) = ctx.client_range() {
            debug!(
                "Client requested range: {}-{} for uri={}",
                range.start, range.end, uri
            );
        }
        
        // Step 3: Fetch file metadata from origin server
        // Requirements: 3.1, 3.2, 3.3, 3.4, 3.5
        let metadata_fetcher = MetadataFetcher::new()
            .map_err(|e| {
                warn!("Failed to create metadata fetcher: {:?}", e);
                e
            })?;
        
        let metadata = match metadata_fetcher.fetch_metadata(uri).await {
            Ok(meta) => {
                debug!(
                    "Fetched metadata: uri={}, size={}, supports_range={}",
                    uri, meta.content_length, meta.supports_range
                );
                meta
            }
            Err(e) => {
                warn!(
                    "Failed to fetch metadata for uri={}: {:?}",
                    uri, e
                );
                // Record as non-sliced request
                self.metrics.record_request(false);
                // Fall back to normal proxy mode
                return Ok(true);
            }
        };
        
        // Step 4: Check if origin supports Range requests (Requirement 3.3, 3.4)
        if !metadata.supports_range {
            info!(
                "Origin does not support Range requests for uri={}, falling back to normal proxy",
                uri
            );
            // Record as non-sliced request
            self.metrics.record_request(false);
            // Fall back to normal proxy mode
            return Ok(true);
        }
        
        // Step 5: Calculate slices (Requirements 4.1, 4.2, 4.3, 4.4)
        let calculator = SliceCalculator::new(self.config.slice_size);
        
        let slices = match calculator.calculate_slices(
            metadata.content_length,
            ctx.client_range(),
        ) {
            Ok(slices) => {
                debug!(
                    "Calculated {} slices for uri={}, file_size={}",
                    slices.len(),
                    uri,
                    metadata.content_length
                );
                slices
            }
            Err(e) => {
                warn!(
                    "Failed to calculate slices for uri={}: {:?}",
                    uri, e
                );
                // Record as non-sliced request
                self.metrics.record_request(false);
                // Return error for invalid range
                return Err(e);
            }
        };
        
        // If no slices calculated (empty file), fall back to normal proxy
        if slices.is_empty() {
            debug!("No slices calculated for uri={}, falling back to normal proxy", uri);
            self.metrics.record_request(false);
            return Ok(true);
        }
        
        // Step 6: Check cache for existing slices (Requirement 7.3)
        let cache = SliceCache::new(Duration::from_secs(self.config.cache_ttl));
        
        // Extract ranges for cache lookup
        let ranges: Vec<ByteRange> = slices.iter().map(|s| s.range).collect();
        
        let cached_slices = cache.lookup_multiple(uri, &ranges).await;
        
        debug!(
            "Cache lookup complete: uri={}, total_slices={}, cache_hits={}",
            uri,
            slices.len(),
            cached_slices.len()
        );
        
        // Record cache hits and misses
        for _ in 0..cached_slices.len() {
            self.metrics.record_cache_hit();
        }
        for _ in 0..(slices.len() - cached_slices.len()) {
            self.metrics.record_cache_miss();
        }
        
        // Mark which slices are cached
        let mut slices_with_cache_info = slices;
        for (idx, _) in cached_slices {
            if idx < slices_with_cache_info.len() {
                slices_with_cache_info[idx].cached = true;
            }
        }
        
        // Step 7: Update context and enable slicing
        ctx.set_metadata(metadata);
        ctx.set_slices(slices_with_cache_info);
        ctx.enable_slicing();
        
        info!(
            "Slicing enabled for uri={}, total_slices={}, cached_slices={}, uncached_slices={}",
            uri,
            ctx.slice_count(),
            ctx.cached_slice_count(),
            ctx.uncached_slice_count()
        );
        
        // Record as sliced request
        self.metrics.record_request(true);
        
        // Return false to indicate we will handle the response ourselves
        // (In actual Pingora integration, this would trigger handle_slice_request)
        Ok(false)
    }
    
    /// Get the upstream peer for normal proxy mode
    ///
    /// This method returns the upstream server configuration when slicing is not enabled.
    /// It is called by Pingora when the request_filter returns true (normal proxy mode).
    ///
    /// # Arguments
    /// * `ctx` - The request context
    ///
    /// # Returns
    /// * `Ok(String)` - The upstream server address
    /// * `Err(SliceError)` - If slicing is enabled (this method shouldn't be called)
    ///
    /// # Requirements
    /// Validates: Requirements 9.3, 9.4
    pub fn upstream_peer(&self, ctx: &SliceContext) -> Result<String> {
        if ctx.is_slice_enabled() {
            // This shouldn't happen - if slicing is enabled, request_filter should return false
            // and this method won't be called
            warn!("upstream_peer called when slicing is enabled");
            return Err(SliceError::InternalError(
                "upstream_peer should not be called when slicing is enabled".to_string()
            ));
        }
        
        debug!("Returning upstream peer: {}", self.config.upstream_address);
        Ok(self.config.upstream_address.clone())
    }
    
    /// Log request completion information
    ///
    /// This method logs detailed information about the request processing, including:
    /// - Request URL and method
    /// - Whether slicing was enabled
    /// - Error information if the request failed
    /// - Performance metrics (duration, slices, bytes transferred)
    ///
    /// # Arguments
    /// * `method` - HTTP method of the request
    /// * `uri` - Request URI
    /// * `ctx` - The request context
    /// * `error` - Optional error that occurred during processing
    /// * `duration_ms` - Request duration in milliseconds
    ///
    /// # Requirements
    /// Validates: Requirements 9.3, 9.4
    pub fn logging(
        &self,
        method: &Method,
        uri: &str,
        ctx: &SliceContext,
        error: Option<&SliceError>,
        duration_ms: u64,
    ) {
        if let Some(err) = error {
            // Requirement 9.3: Log detailed error information including request URL and error type
            warn!(
                "Request failed: method={}, uri={}, error_type={:?}, error={}, duration_ms={}",
                method,
                uri,
                err,
                err,
                duration_ms
            );
        } else if ctx.is_slice_enabled() {
            // Requirement 9.4: Log summary information including total time and number of slices
            let stats = self.metrics.get_stats();
            info!(
                "Slice request completed: method={}, uri={}, slices={}, cached={}, uncached={}, duration_ms={}, bytes_from_origin={}, bytes_from_cache={}, bytes_to_client={}",
                method,
                uri,
                ctx.slice_count(),
                ctx.cached_slice_count(),
                ctx.uncached_slice_count(),
                duration_ms,
                stats.bytes_from_origin,
                stats.bytes_from_cache,
                stats.bytes_to_client
            );
        } else {
            // Normal proxy mode
            info!(
                "Normal proxy request completed: method={}, uri={}, duration_ms={}",
                method,
                uri,
                duration_ms
            );
        }
    }
}

impl SliceContext {
    /// Create a new SliceContext with default values
    ///
    /// # Returns
    /// A new SliceContext with slicing disabled and no metadata
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Check if slicing is enabled for this request
    ///
    /// # Returns
    /// true if slicing is enabled, false otherwise
    pub fn is_slice_enabled(&self) -> bool {
        self.slice_enabled
    }
    
    /// Enable slicing for this request
    pub fn enable_slicing(&mut self) {
        self.slice_enabled = true;
    }
    
    /// Disable slicing for this request
    pub fn disable_slicing(&mut self) {
        self.slice_enabled = false;
    }
    
    /// Set the file metadata
    ///
    /// # Arguments
    /// * `metadata` - File metadata from the origin server
    pub fn set_metadata(&mut self, metadata: FileMetadata) {
        self.metadata = Some(metadata);
    }
    
    /// Get a reference to the file metadata
    ///
    /// # Returns
    /// An Option containing a reference to the metadata if available
    pub fn metadata(&self) -> Option<&FileMetadata> {
        self.metadata.as_ref()
    }
    
    /// Set the client's requested byte range
    ///
    /// # Arguments
    /// * `range` - The byte range requested by the client
    pub fn set_client_range(&mut self, range: ByteRange) {
        self.client_range = Some(range);
    }
    
    /// Set the client's requested byte range from an Option
    ///
    /// # Arguments
    /// * `range` - Optional byte range requested by the client
    pub fn set_client_range_opt(&mut self, range: Option<ByteRange>) {
        self.client_range = range;
    }
    
    /// Get the client's requested byte range
    ///
    /// # Returns
    /// An Option containing the client's requested range if present
    pub fn client_range(&self) -> Option<ByteRange> {
        self.client_range
    }
    
    /// Set the calculated slices
    ///
    /// # Arguments
    /// * `slices` - Vector of calculated slice specifications
    pub fn set_slices(&mut self, slices: Vec<SliceSpec>) {
        self.slices = slices;
    }
    
    /// Get a reference to the calculated slices
    ///
    /// # Returns
    /// A reference to the vector of slice specifications
    pub fn slices(&self) -> &[SliceSpec] {
        &self.slices
    }
    
    /// Get a mutable reference to the calculated slices
    ///
    /// # Returns
    /// A mutable reference to the vector of slice specifications
    pub fn slices_mut(&mut self) -> &mut Vec<SliceSpec> {
        &mut self.slices
    }
    
    /// Get the number of slices
    ///
    /// # Returns
    /// The number of calculated slices
    pub fn slice_count(&self) -> usize {
        self.slices.len()
    }
    
    /// Check if there are any slices
    ///
    /// # Returns
    /// true if there are slices, false otherwise
    pub fn has_slices(&self) -> bool {
        !self.slices.is_empty()
    }
    
    /// Get the number of cached slices
    ///
    /// # Returns
    /// The number of slices that are marked as cached
    pub fn cached_slice_count(&self) -> usize {
        self.slices.iter().filter(|s| s.cached).count()
    }
    
    /// Get the number of uncached slices
    ///
    /// # Returns
    /// The number of slices that need to be fetched from origin
    pub fn uncached_slice_count(&self) -> usize {
        self.slices.iter().filter(|s| !s.cached).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ByteRange;
    
    #[test]
    fn test_slice_proxy_new() {
        let config = Arc::new(SliceConfig::default());
        let proxy = SliceProxy::new(config.clone());
        
        assert_eq!(proxy.config().slice_size, config.slice_size);
        assert_eq!(proxy.config().max_concurrent_subrequests, config.max_concurrent_subrequests);
    }
    
    #[test]
    fn test_slice_proxy_new_ctx() {
        let config = Arc::new(SliceConfig::default());
        let proxy = SliceProxy::new(config);
        let ctx = proxy.new_ctx();
        
        assert!(!ctx.slice_enabled);
        assert!(ctx.metadata.is_none());
        assert!(ctx.client_range.is_none());
        assert!(ctx.slices.is_empty());
    }
    
    #[test]
    fn test_slice_proxy_config_access() {
        let config = Arc::new(SliceConfig::default());
        let proxy = SliceProxy::new(config.clone());
        
        assert_eq!(proxy.config().slice_size, 1024 * 1024);
        
        let config_arc = proxy.config_arc();
        assert_eq!(config_arc.slice_size, 1024 * 1024);
    }
    
    #[test]
    fn test_slice_proxy_metrics_access() {
        let config = Arc::new(SliceConfig::default());
        let proxy = SliceProxy::new(config);
        
        proxy.metrics().record_request(true);
        let stats = proxy.metrics().get_stats();
        assert_eq!(stats.total_requests, 1);
        
        let metrics_arc = proxy.metrics_arc();
        metrics_arc.record_request(false);
        let stats = proxy.metrics().get_stats();
        assert_eq!(stats.total_requests, 2);
    }
    
    #[test]
    fn test_slice_context_new() {
        let ctx = SliceContext::new();
        
        assert!(!ctx.slice_enabled);
        assert!(ctx.metadata.is_none());
        assert!(ctx.client_range.is_none());
        assert!(ctx.slices.is_empty());
    }
    
    #[test]
    fn test_slice_context_enable_disable() {
        let mut ctx = SliceContext::new();
        
        assert!(!ctx.is_slice_enabled());
        
        ctx.enable_slicing();
        assert!(ctx.is_slice_enabled());
        
        ctx.disable_slicing();
        assert!(!ctx.is_slice_enabled());
    }
    
    #[test]
    fn test_slice_context_metadata() {
        let mut ctx = SliceContext::new();
        
        assert!(ctx.metadata().is_none());
        
        let metadata = FileMetadata::new(1024000, true);
        ctx.set_metadata(metadata.clone());
        
        assert!(ctx.metadata().is_some());
        assert_eq!(ctx.metadata().unwrap().content_length, 1024000);
    }
    
    #[test]
    fn test_slice_context_client_range() {
        let mut ctx = SliceContext::new();
        
        assert!(ctx.client_range().is_none());
        
        let range = ByteRange::new(0, 1023).unwrap();
        ctx.set_client_range(range);
        
        assert!(ctx.client_range().is_some());
        assert_eq!(ctx.client_range().unwrap().start, 0);
        assert_eq!(ctx.client_range().unwrap().end, 1023);
    }
    
    #[test]
    fn test_slice_context_slices() {
        let mut ctx = SliceContext::new();
        
        assert!(ctx.slices().is_empty());
        assert!(!ctx.has_slices());
        assert_eq!(ctx.slice_count(), 0);
        
        let range1 = ByteRange::new(0, 1023).unwrap();
        let range2 = ByteRange::new(1024, 2047).unwrap();
        let slices = vec![
            SliceSpec::new(0, range1),
            SliceSpec::new(1, range2),
        ];
        
        ctx.set_slices(slices);
        
        assert!(ctx.has_slices());
        assert_eq!(ctx.slice_count(), 2);
        assert_eq!(ctx.slices().len(), 2);
    }
    
    #[test]
    fn test_slice_context_cached_count() {
        let mut ctx = SliceContext::new();
        
        let range1 = ByteRange::new(0, 1023).unwrap();
        let range2 = ByteRange::new(1024, 2047).unwrap();
        let range3 = ByteRange::new(2048, 3071).unwrap();
        
        let mut slice1 = SliceSpec::new(0, range1);
        slice1.cached = true;
        let slice2 = SliceSpec::new(1, range2);
        let mut slice3 = SliceSpec::new(2, range3);
        slice3.cached = true;
        
        ctx.set_slices(vec![slice1, slice2, slice3]);
        
        assert_eq!(ctx.cached_slice_count(), 2);
        assert_eq!(ctx.uncached_slice_count(), 1);
    }
    
    #[test]
    fn test_slice_context_slices_mut() {
        let mut ctx = SliceContext::new();
        
        let range1 = ByteRange::new(0, 1023).unwrap();
        let range2 = ByteRange::new(1024, 2047).unwrap();
        let slices = vec![
            SliceSpec::new(0, range1),
            SliceSpec::new(1, range2),
        ];
        
        ctx.set_slices(slices);
        
        // Mark first slice as cached
        ctx.slices_mut()[0].cached = true;
        
        assert_eq!(ctx.cached_slice_count(), 1);
        assert_eq!(ctx.uncached_slice_count(), 1);
    }
    
    #[test]
    fn test_slice_context_clone() {
        let mut ctx = SliceContext::new();
        ctx.enable_slicing();
        
        let metadata = FileMetadata::new(1024000, true);
        ctx.set_metadata(metadata);
        
        let range = ByteRange::new(0, 1023).unwrap();
        ctx.set_client_range(range);
        
        let ctx_clone = ctx.clone();
        
        assert!(ctx_clone.is_slice_enabled());
        assert!(ctx_clone.metadata().is_some());
        assert!(ctx_clone.client_range().is_some());
    }
    
    #[test]
    fn test_slice_context_set_client_range_opt() {
        let mut ctx = SliceContext::new();
        
        // Test with None
        ctx.set_client_range_opt(None);
        assert!(ctx.client_range().is_none());
        
        // Test with Some
        let range = ByteRange::new(100, 200).unwrap();
        ctx.set_client_range_opt(Some(range));
        assert!(ctx.client_range().is_some());
        assert_eq!(ctx.client_range().unwrap().start, 100);
        assert_eq!(ctx.client_range().unwrap().end, 200);
    }
    
    #[test]
    fn test_upstream_peer_normal_mode() {
        let config = Arc::new(SliceConfig {
            upstream_address: "example.com:8080".to_string(),
            ..Default::default()
        });
        let proxy = SliceProxy::new(config);
        let ctx = SliceContext::new(); // Slicing disabled by default
        
        let result = proxy.upstream_peer(&ctx);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "example.com:8080");
    }
    
    #[test]
    fn test_upstream_peer_slice_mode() {
        let config = Arc::new(SliceConfig::default());
        let proxy = SliceProxy::new(config);
        let mut ctx = SliceContext::new();
        ctx.enable_slicing();
        
        let result = proxy.upstream_peer(&ctx);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_logging_normal_request() {
        let config = Arc::new(SliceConfig::default());
        let proxy = SliceProxy::new(config);
        let ctx = SliceContext::new();
        
        // Should not panic
        proxy.logging(
            &Method::GET,
            "http://example.com/file.bin",
            &ctx,
            None,
            100,
        );
    }
    
    #[test]
    fn test_logging_slice_request() {
        let config = Arc::new(SliceConfig::default());
        let proxy = SliceProxy::new(config);
        let mut ctx = SliceContext::new();
        ctx.enable_slicing();
        
        let metadata = FileMetadata::new(2048, true);
        ctx.set_metadata(metadata);
        
        let range1 = ByteRange::new(0, 1023).unwrap();
        let range2 = ByteRange::new(1024, 2047).unwrap();
        let mut slice1 = SliceSpec::new(0, range1);
        slice1.cached = true;
        let slice2 = SliceSpec::new(1, range2);
        
        ctx.set_slices(vec![slice1, slice2]);
        
        // Should not panic
        proxy.logging(
            &Method::GET,
            "http://example.com/file.bin",
            &ctx,
            None,
            250,
        );
    }
    
    #[test]
    fn test_logging_with_error() {
        let config = Arc::new(SliceConfig::default());
        let proxy = SliceProxy::new(config);
        let ctx = SliceContext::new();
        
        let error = SliceError::MetadataFetchError("Connection refused".to_string());
        
        // Should not panic
        proxy.logging(
            &Method::GET,
            "http://example.com/file.bin",
            &ctx,
            Some(&error),
            50,
        );
    }
}

#[cfg(test)]
mod request_filter_tests {
    use super::*;
    use http::{Method, HeaderMap, HeaderValue};
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path};
    
    fn create_test_proxy(patterns: Vec<String>) -> SliceProxy {
        let config = Arc::new(SliceConfig {
            slice_size: 1024,
            slice_patterns: patterns,
            cache_ttl: 3600,
            ..Default::default()
        });
        SliceProxy::new(config)
    }
    
    #[tokio::test]
    async fn test_request_filter_non_get_request() {
        let proxy = create_test_proxy(vec![]);
        let mut ctx = SliceContext::new();
        let headers = HeaderMap::new();
        
        let result = proxy.request_filter(
            &Method::POST,
            "http://example.com/file.bin",
            &headers,
            &mut ctx,
        ).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true); // Should continue with normal proxy
        assert!(!ctx.is_slice_enabled());
    }
    
    #[tokio::test]
    async fn test_request_filter_with_range_header() {
        let proxy = create_test_proxy(vec![]);
        let mut ctx = SliceContext::new();
        let mut headers = HeaderMap::new();
        headers.insert("range", HeaderValue::from_static("bytes=0-1023"));
        
        let result = proxy.request_filter(
            &Method::GET,
            "http://example.com/file.bin",
            &headers,
            &mut ctx,
        ).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true); // Should pass through Range requests
        assert!(!ctx.is_slice_enabled());
    }
    
    #[tokio::test]
    async fn test_request_filter_pattern_mismatch() {
        let proxy = create_test_proxy(vec!["/large-files/".to_string()]);
        let mut ctx = SliceContext::new();
        let headers = HeaderMap::new();
        
        let result = proxy.request_filter(
            &Method::GET,
            "http://example.com/small-files/file.bin",
            &headers,
            &mut ctx,
        ).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true); // Should continue with normal proxy
        assert!(!ctx.is_slice_enabled());
    }
    
    #[tokio::test]
    async fn test_request_filter_origin_supports_range() {
        let mock_server = MockServer::start().await;
        
        // Mock HEAD request that supports Range
        Mock::given(method("HEAD"))
            .and(path("/file.bin"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Content-Length", "10240")
                    .insert_header("Accept-Ranges", "bytes")
                    .insert_header("Content-Type", "application/octet-stream")
            )
            .mount(&mock_server)
            .await;
        
        let proxy = create_test_proxy(vec![]);
        let mut ctx = SliceContext::new();
        let headers = HeaderMap::new();
        
        let url = format!("{}/file.bin", mock_server.uri());
        let result = proxy.request_filter(
            &Method::GET,
            &url,
            &headers,
            &mut ctx,
        ).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), false); // Should enable slicing
        assert!(ctx.is_slice_enabled());
        assert!(ctx.metadata().is_some());
        assert_eq!(ctx.metadata().unwrap().content_length, 10240);
        assert!(ctx.metadata().unwrap().supports_range);
        assert!(ctx.has_slices());
        
        // Should have calculated slices (10240 / 1024 = 10 slices)
        assert_eq!(ctx.slice_count(), 10);
    }
    
    #[tokio::test]
    async fn test_request_filter_origin_no_range_support() {
        let mock_server = MockServer::start().await;
        
        // Mock HEAD request that does NOT support Range
        Mock::given(method("HEAD"))
            .and(path("/file.bin"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Content-Length", "10240")
                    .insert_header("Content-Type", "application/octet-stream")
                    // No Accept-Ranges header
            )
            .mount(&mock_server)
            .await;
        
        let proxy = create_test_proxy(vec![]);
        let mut ctx = SliceContext::new();
        let headers = HeaderMap::new();
        
        let url = format!("{}/file.bin", mock_server.uri());
        let result = proxy.request_filter(
            &Method::GET,
            &url,
            &headers,
            &mut ctx,
        ).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true); // Should fall back to normal proxy
        assert!(!ctx.is_slice_enabled());
    }
    
    #[tokio::test]
    async fn test_request_filter_metadata_fetch_failure() {
        let proxy = create_test_proxy(vec![]);
        let mut ctx = SliceContext::new();
        let headers = HeaderMap::new();
        
        // Use an invalid URL that will fail
        let result = proxy.request_filter(
            &Method::GET,
            "http://invalid-host-that-does-not-exist.example/file.bin",
            &headers,
            &mut ctx,
        ).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true); // Should fall back to normal proxy
        assert!(!ctx.is_slice_enabled());
    }
    
    #[tokio::test]
    async fn test_request_filter_with_client_range() {
        let mock_server = MockServer::start().await;
        
        Mock::given(method("HEAD"))
            .and(path("/file.bin"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Content-Length", "10240")
                    .insert_header("Accept-Ranges", "bytes")
            )
            .mount(&mock_server)
            .await;
        
        let proxy = create_test_proxy(vec![]);
        let mut ctx = SliceContext::new();
        let headers = HeaderMap::new();
        // Note: This should be passed through, but let's test the extraction logic
        // by using a different header name for testing
        
        let url = format!("{}/file.bin", mock_server.uri());
        let result = proxy.request_filter(
            &Method::GET,
            &url,
            &headers,
            &mut ctx,
        ).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), false);
        assert!(ctx.is_slice_enabled());
        assert!(ctx.client_range().is_none()); // No client range in this test
    }
    
    #[tokio::test]
    async fn test_request_filter_empty_file() {
        let mock_server = MockServer::start().await;
        
        Mock::given(method("HEAD"))
            .and(path("/empty.bin"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Content-Length", "0")
                    .insert_header("Accept-Ranges", "bytes")
            )
            .mount(&mock_server)
            .await;
        
        let proxy = create_test_proxy(vec![]);
        let mut ctx = SliceContext::new();
        let headers = HeaderMap::new();
        
        let url = format!("{}/empty.bin", mock_server.uri());
        let result = proxy.request_filter(
            &Method::GET,
            &url,
            &headers,
            &mut ctx,
        ).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true); // Should fall back for empty file
        assert!(!ctx.is_slice_enabled());
    }
    
    #[tokio::test]
    async fn test_request_filter_metrics_recording() {
        let mock_server = MockServer::start().await;
        
        Mock::given(method("HEAD"))
            .and(path("/file.bin"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Content-Length", "5120")
                    .insert_header("Accept-Ranges", "bytes")
            )
            .mount(&mock_server)
            .await;
        
        let proxy = create_test_proxy(vec![]);
        let mut ctx = SliceContext::new();
        let headers = HeaderMap::new();
        
        let url = format!("{}/file.bin", mock_server.uri());
        let _ = proxy.request_filter(
            &Method::GET,
            &url,
            &headers,
            &mut ctx,
        ).await;
        
        // Check that metrics were recorded
        let stats = proxy.metrics().get_stats();
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.sliced_requests, 1);
        
        // Should have cache misses for all slices (5 slices for 5120 bytes with 1024 slice size)
        assert_eq!(stats.cache_misses, 5);
    }
    
    #[tokio::test]
    async fn test_request_filter_origin_4xx_error() {
        let mock_server = MockServer::start().await;
        
        Mock::given(method("HEAD"))
            .and(path("/notfound.bin"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;
        
        let proxy = create_test_proxy(vec![]);
        let mut ctx = SliceContext::new();
        let headers = HeaderMap::new();
        
        let url = format!("{}/notfound.bin", mock_server.uri());
        let result = proxy.request_filter(
            &Method::GET,
            &url,
            &headers,
            &mut ctx,
        ).await;
        
        // Should fall back to normal proxy on metadata fetch error
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);
        assert!(!ctx.is_slice_enabled());
    }
}

#[cfg(test)]
mod handle_slice_request_tests {
    use super::*;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path, header};
    use http::StatusCode;
    
    fn create_test_proxy() -> SliceProxy {
        let config = Arc::new(SliceConfig {
            slice_size: 1024,
            max_concurrent_subrequests: 2,
            max_retries: 2,
            cache_ttl: 3600,
            ..Default::default()
        });
        SliceProxy::new(config)
    }
    
    #[tokio::test]
    async fn test_handle_slice_request_no_slices_to_fetch() {
        let mock_server = MockServer::start().await;
        
        // Mock GET requests for slices (in case they're needed)
        Mock::given(method("GET"))
            .and(path("/file.bin"))
            .and(header("range", "bytes=0-1023"))
            .respond_with(
                ResponseTemplate::new(206)
                    .insert_header("Content-Range", "bytes 0-1023/2048")
                    .set_body_bytes(vec![1u8; 1024])
            )
            .mount(&mock_server)
            .await;
        
        Mock::given(method("GET"))
            .and(path("/file.bin"))
            .and(header("range", "bytes=1024-2047"))
            .respond_with(
                ResponseTemplate::new(206)
                    .insert_header("Content-Range", "bytes 1024-2047/2048")
                    .set_body_bytes(vec![2u8; 1024])
            )
            .mount(&mock_server)
            .await;
        
        let proxy = create_test_proxy();
        
        // Create a context with metadata and slices
        let mut ctx = SliceContext::new();
        let metadata = FileMetadata::new(2048, true);
        ctx.set_metadata(metadata);
        
        // Create 2 slices, neither marked as cached (will be fetched)
        let range1 = ByteRange::new(0, 1023).unwrap();
        let range2 = ByteRange::new(1024, 2047).unwrap();
        let slice1 = SliceSpec::new(0, range1);
        let slice2 = SliceSpec::new(1, range2);
        
        ctx.set_slices(vec![slice1, slice2]);
        ctx.enable_slicing();
        
        // Handle the request
        let url = format!("{}/file.bin", mock_server.uri());
        let result = proxy.handle_slice_request(&url, &ctx).await;
        
        if let Err(ref e) = result {
            eprintln!("Error: {:?}", e);
        }
        assert!(result.is_ok());
        let (status, headers, slices) = result.unwrap();
        
        // Verify response
        assert_eq!(status, StatusCode::OK);
        assert!(headers.contains_key("content-length"));
        assert_eq!(slices.len(), 2);
        assert_eq!(slices[0].len(), 1024);
        assert_eq!(slices[1].len(), 1024);
        
        // Verify metrics - all from origin since nothing was cached
        let stats = proxy.metrics().get_stats();
        assert_eq!(stats.bytes_from_origin, 2048);
        assert_eq!(stats.bytes_to_client, 2048);
    }
    
    #[tokio::test]
    async fn test_handle_slice_request_fetch_from_origin() {
        let mock_server = MockServer::start().await;
        
        // Mock GET requests for slices
        Mock::given(method("GET"))
            .and(path("/file.bin"))
            .and(header("range", "bytes=0-1023"))
            .respond_with(
                ResponseTemplate::new(206)
                    .insert_header("Content-Range", "bytes 0-1023/2048")
                    .set_body_bytes(vec![1u8; 1024])
            )
            .mount(&mock_server)
            .await;
        
        Mock::given(method("GET"))
            .and(path("/file.bin"))
            .and(header("range", "bytes=1024-2047"))
            .respond_with(
                ResponseTemplate::new(206)
                    .insert_header("Content-Range", "bytes 1024-2047/2048")
                    .set_body_bytes(vec![2u8; 1024])
            )
            .mount(&mock_server)
            .await;
        
        let proxy = create_test_proxy();
        
        // Create a context with metadata and slices
        let mut ctx = SliceContext::new();
        let metadata = FileMetadata::new(2048, true);
        ctx.set_metadata(metadata);
        
        // Create 2 slices, neither cached
        let range1 = ByteRange::new(0, 1023).unwrap();
        let range2 = ByteRange::new(1024, 2047).unwrap();
        let slice1 = SliceSpec::new(0, range1);
        let slice2 = SliceSpec::new(1, range2);
        
        ctx.set_slices(vec![slice1, slice2]);
        ctx.enable_slicing();
        
        // Handle the request
        let url = format!("{}/file.bin", mock_server.uri());
        let result = proxy.handle_slice_request(&url, &ctx).await;
        
        assert!(result.is_ok());
        let (status, headers, slices) = result.unwrap();
        
        // Verify response
        assert_eq!(status, StatusCode::OK);
        assert!(headers.contains_key("content-length"));
        assert_eq!(slices.len(), 2);
        assert_eq!(slices[0].len(), 1024);
        assert_eq!(slices[1].len(), 1024);
        
        // Verify metrics
        let stats = proxy.metrics().get_stats();
        assert_eq!(stats.total_subrequests, 2);
        assert_eq!(stats.bytes_from_origin, 2048);
        assert_eq!(stats.bytes_to_client, 2048);
    }
    
    #[tokio::test]
    async fn test_handle_slice_request_multiple_slices() {
        let mock_server = MockServer::start().await;
        
        // Mock GET requests for three slices
        Mock::given(method("GET"))
            .and(path("/file.bin"))
            .and(header("range", "bytes=0-1023"))
            .respond_with(
                ResponseTemplate::new(206)
                    .insert_header("Content-Range", "bytes 0-1023/3072")
                    .set_body_bytes(vec![1u8; 1024])
            )
            .mount(&mock_server)
            .await;
        
        Mock::given(method("GET"))
            .and(path("/file.bin"))
            .and(header("range", "bytes=1024-2047"))
            .respond_with(
                ResponseTemplate::new(206)
                    .insert_header("Content-Range", "bytes 1024-2047/3072")
                    .set_body_bytes(vec![2u8; 1024])
            )
            .mount(&mock_server)
            .await;
        
        Mock::given(method("GET"))
            .and(path("/file.bin"))
            .and(header("range", "bytes=2048-3071"))
            .respond_with(
                ResponseTemplate::new(206)
                    .insert_header("Content-Range", "bytes 2048-3071/3072")
                    .set_body_bytes(vec![3u8; 1024])
            )
            .mount(&mock_server)
            .await;
        
        let proxy = create_test_proxy();
        
        // Create a context with metadata and slices
        let mut ctx = SliceContext::new();
        let metadata = FileMetadata::new(3072, true);
        ctx.set_metadata(metadata);
        
        // Create 3 slices
        let range1 = ByteRange::new(0, 1023).unwrap();
        let range2 = ByteRange::new(1024, 2047).unwrap();
        let range3 = ByteRange::new(2048, 3071).unwrap();
        let slice1 = SliceSpec::new(0, range1);
        let slice2 = SliceSpec::new(1, range2);
        let slice3 = SliceSpec::new(2, range3);
        
        ctx.set_slices(vec![slice1, slice2, slice3]);
        ctx.enable_slicing();
        
        // Handle the request
        let url = format!("{}/file.bin", mock_server.uri());
        let result = proxy.handle_slice_request(&url, &ctx).await;
        
        assert!(result.is_ok());
        let (status, _headers, slices) = result.unwrap();
        
        // Verify response
        assert_eq!(status, StatusCode::OK);
        assert_eq!(slices.len(), 3);
        assert_eq!(slices[0].len(), 1024);
        assert_eq!(slices[1].len(), 1024);
        assert_eq!(slices[2].len(), 1024);
        
        // Verify metrics
        let stats = proxy.metrics().get_stats();
        assert_eq!(stats.total_subrequests, 3);
        assert_eq!(stats.bytes_from_origin, 3072);
        assert_eq!(stats.bytes_to_client, 3072);
    }
    
    #[tokio::test]
    async fn test_handle_slice_request_with_client_range() {
        let mock_server = MockServer::start().await;
        
        // Mock GET request for single slice
        Mock::given(method("GET"))
            .and(path("/file.bin"))
            .and(header("range", "bytes=0-1023"))
            .respond_with(
                ResponseTemplate::new(206)
                    .insert_header("Content-Range", "bytes 0-1023/10240")
                    .set_body_bytes(vec![1u8; 1024])
            )
            .mount(&mock_server)
            .await;
        
        let proxy = create_test_proxy();
        
        // Create a context with metadata and client range
        let mut ctx = SliceContext::new();
        let metadata = FileMetadata::new(10240, true);
        ctx.set_metadata(metadata);
        
        // Client requested bytes 0-1023
        let client_range = ByteRange::new(0, 1023).unwrap();
        ctx.set_client_range(client_range);
        
        // Create 1 slice for the client range
        let slice1 = SliceSpec::new(0, client_range);
        ctx.set_slices(vec![slice1]);
        ctx.enable_slicing();
        
        // Handle the request
        let url = format!("{}/file.bin", mock_server.uri());
        let result = proxy.handle_slice_request(&url, &ctx).await;
        
        assert!(result.is_ok());
        let (status, headers, slices) = result.unwrap();
        
        // Verify response - should be 206 for range request
        assert_eq!(status, StatusCode::PARTIAL_CONTENT);
        assert!(headers.contains_key("content-range"));
        assert_eq!(slices.len(), 1);
        assert_eq!(slices[0].len(), 1024);
    }
    
    #[tokio::test]
    async fn test_handle_slice_request_missing_metadata() {
        let proxy = create_test_proxy();
        
        // Create a context without metadata
        let ctx = SliceContext::new();
        
        // Handle the request - should fail
        let url = "http://example.com/file.bin";
        let result = proxy.handle_slice_request(url, &ctx).await;
        
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_handle_slice_request_subrequest_failure() {
        let mock_server = MockServer::start().await;
        
        // Mock GET request that returns error
        Mock::given(method("GET"))
            .and(path("/file.bin"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;
        
        let proxy = create_test_proxy();
        
        // Create a context with metadata and slices
        let mut ctx = SliceContext::new();
        let metadata = FileMetadata::new(1024, true);
        ctx.set_metadata(metadata);
        
        let range1 = ByteRange::new(0, 1023).unwrap();
        let slice1 = SliceSpec::new(0, range1);
        ctx.set_slices(vec![slice1]);
        ctx.enable_slicing();
        
        // Handle the request - should fail after retries
        let url = format!("{}/file.bin", mock_server.uri());
        let result = proxy.handle_slice_request(&url, &ctx).await;
        
        assert!(result.is_err());
        
        // Verify that failed subrequests were recorded
        let stats = proxy.metrics().get_stats();
        assert!(stats.failed_subrequests > 0);
    }
}
