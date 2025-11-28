# Metrics Endpoint Implementation

## Overview

The metrics endpoint provides an HTTP server that exposes slice module metrics in Prometheus format. This allows monitoring systems like Prometheus, Grafana, or other observability tools to scrape and visualize the metrics.

## Requirements

**Validates: Requirements 9.5**

> WHERE metrics endpoint is configured, THE Proxy Server SHALL expose slice metrics via HTTP endpoint

## Architecture

The metrics endpoint is implemented as a standalone HTTP server that runs on a separate port from the main proxy. It uses Hyper for the HTTP server and exposes metrics in the standard Prometheus text exposition format.

```
┌─────────────────┐
│  Slice Proxy    │
│  (Port 8080)    │
└────────┬────────┘
         │
         │ Shares metrics
         ▼
┌─────────────────┐
│ SliceMetrics    │
│ (Atomic counters)│
└────────┬────────┘
         │
         │ Read by
         ▼
┌─────────────────┐
│ MetricsEndpoint │
│  (Port 9090)    │
└─────────────────┘
         │
         │ HTTP GET /metrics
         ▼
┌─────────────────┐
│  Prometheus     │
│  or other       │
│  monitoring     │
└─────────────────┘
```

## Configuration

The metrics endpoint is configured in the `pingora_slice.yaml` configuration file:

```yaml
# Optional metrics endpoint configuration
metrics_endpoint:
  enabled: true
  address: "127.0.0.1:9090"
```

If `metrics_endpoint` is not specified or `enabled` is false, the metrics endpoint will not start.

## Available Endpoints

### GET /

Index page that lists all available endpoints with links.

**Response:**
- Status: 200 OK
- Content-Type: text/html

### GET /metrics

Prometheus format metrics endpoint.

**Response:**
- Status: 200 OK
- Content-Type: text/plain; version=0.0.4; charset=utf-8

**Example output:**
```
# HELP pingora_slice_requests_total Total number of requests processed
# TYPE pingora_slice_requests_total counter
pingora_slice_requests_total 1234

# HELP pingora_slice_sliced_requests_total Number of requests handled with slicing
# TYPE pingora_slice_sliced_requests_total counter
pingora_slice_sliced_requests_total 890

# HELP pingora_slice_cache_hit_rate Cache hit rate percentage
# TYPE pingora_slice_cache_hit_rate gauge
pingora_slice_cache_hit_rate 75.50
```

### GET /health

Health check endpoint that returns a simple JSON response.

**Response:**
- Status: 200 OK
- Content-Type: application/json

```json
{"status":"healthy"}
```

## Exposed Metrics

### Request Metrics

| Metric Name | Type | Description |
|------------|------|-------------|
| `pingora_slice_requests_total` | counter | Total number of requests processed |
| `pingora_slice_sliced_requests_total` | counter | Number of requests handled with slicing |
| `pingora_slice_passthrough_requests_total` | counter | Number of requests passed through without slicing |

### Cache Metrics

| Metric Name | Type | Description |
|------------|------|-------------|
| `pingora_slice_cache_hits_total` | counter | Number of cache hits |
| `pingora_slice_cache_misses_total` | counter | Number of cache misses |
| `pingora_slice_cache_errors_total` | counter | Number of cache errors |
| `pingora_slice_cache_hit_rate` | gauge | Cache hit rate percentage (0-100) |

### Subrequest Metrics

| Metric Name | Type | Description |
|------------|------|-------------|
| `pingora_slice_subrequests_total` | counter | Total number of subrequests sent |
| `pingora_slice_failed_subrequests_total` | counter | Number of failed subrequests |
| `pingora_slice_retried_subrequests_total` | counter | Number of retried subrequests |
| `pingora_slice_subrequest_failure_rate` | gauge | Subrequest failure rate percentage (0-100) |

### Byte Transfer Metrics

| Metric Name | Type | Description |
|------------|------|-------------|
| `pingora_slice_bytes_from_origin_total` | counter | Total bytes received from origin |
| `pingora_slice_bytes_from_cache_total` | counter | Total bytes received from cache |
| `pingora_slice_bytes_to_client_total` | counter | Total bytes sent to client |

### Latency Metrics

| Metric Name | Type | Description |
|------------|------|-------------|
| `pingora_slice_request_duration_ms_avg` | gauge | Average request duration in milliseconds |
| `pingora_slice_subrequest_duration_ms_avg` | gauge | Average subrequest duration in milliseconds |
| `pingora_slice_assembly_duration_ms_avg` | gauge | Average assembly duration in milliseconds |

## Usage Example

### Starting the Metrics Endpoint

```rust
use pingora_slice::{MetricsEndpoint, SliceMetrics};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create shared metrics instance
    let metrics = Arc::new(SliceMetrics::new());
    
    // Create and start the metrics endpoint
    let addr = "127.0.0.1:9090".parse()?;
    let endpoint = MetricsEndpoint::new(metrics, addr);
    
    // Start the endpoint (runs forever)
    endpoint.start().await?;
    
    Ok(())
}
```

### Integrating with Main Server

```rust
use pingora_slice::{SliceConfig, SliceProxy, MetricsEndpoint};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = SliceConfig::from_file("pingora_slice.yaml")?;
    
    // Create proxy with metrics
    let proxy = SliceProxy::new(Arc::new(config.clone()));
    let metrics = proxy.metrics();
    
    // Start metrics endpoint if configured
    if let Some(metrics_config) = &config.metrics_endpoint {
        if metrics_config.enabled {
            let addr = metrics_config.address.parse()?;
            let endpoint = MetricsEndpoint::new(Arc::clone(metrics), addr);
            
            tokio::spawn(async move {
                if let Err(e) = endpoint.start().await {
                    eprintln!("Metrics endpoint error: {}", e);
                }
            });
        }
    }
    
    // Start main proxy server
    // ... proxy server code ...
    
    Ok(())
}
```

## Prometheus Configuration

To scrape metrics from the endpoint, add this to your `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'pingora-slice'
    static_configs:
      - targets: ['localhost:9090']
    metrics_path: '/metrics'
    scrape_interval: 15s
```

## Grafana Dashboard

Example Grafana queries for visualizing metrics:

### Request Rate
```promql
rate(pingora_slice_requests_total[5m])
```

### Cache Hit Rate
```promql
pingora_slice_cache_hit_rate
```

### Subrequest Failure Rate
```promql
pingora_slice_subrequest_failure_rate
```

### Average Request Duration
```promql
pingora_slice_request_duration_ms_avg
```

### Bandwidth Usage
```promql
rate(pingora_slice_bytes_to_client_total[5m])
```

## Testing

Run the example to test the metrics endpoint:

```bash
cargo run --example metrics_endpoint_example
```

Then access the endpoints:

```bash
# View index page
curl http://127.0.0.1:9090/

# View metrics
curl http://127.0.0.1:9090/metrics

# Health check
curl http://127.0.0.1:9090/health
```

## Security Considerations

1. **Bind Address**: By default, the metrics endpoint binds to `127.0.0.1` (localhost only). For production deployments, consider:
   - Keeping it on localhost and using a reverse proxy
   - Using firewall rules to restrict access
   - Implementing authentication if exposing publicly

2. **Information Disclosure**: Metrics may reveal information about traffic patterns and system behavior. Ensure appropriate access controls are in place.

3. **Resource Usage**: The metrics endpoint is lightweight, but in high-traffic scenarios, consider:
   - Rate limiting scrape requests
   - Monitoring the endpoint's own resource usage

## Performance

The metrics endpoint is designed to be lightweight:

- Uses atomic operations for thread-safe metric updates
- No locks or mutexes in the hot path
- Metrics are read-only from the endpoint's perspective
- Minimal memory overhead (< 1KB for all metrics)
- Fast response times (< 1ms for typical metric snapshots)

## Troubleshooting

### Endpoint Not Starting

**Problem**: Metrics endpoint fails to start

**Solutions**:
- Check if the port is already in use
- Verify the address format in configuration
- Check firewall rules
- Review logs for error messages

### Metrics Not Updating

**Problem**: Metrics show zero or stale values

**Solutions**:
- Verify the proxy is actually processing requests
- Check that the same `SliceMetrics` instance is shared between proxy and endpoint
- Ensure metrics recording is not disabled

### High Memory Usage

**Problem**: Metrics endpoint consuming too much memory

**Solutions**:
- This should not happen with the current implementation
- Check for memory leaks in custom code
- Monitor the number of concurrent connections to the endpoint

## Future Enhancements

Potential improvements for future versions:

1. **Histogram Support**: Add proper histogram metrics for latency distributions
2. **Labels**: Support for metric labels (e.g., by upstream, by URL pattern)
3. **Custom Metrics**: Allow users to register custom metrics
4. **Multiple Formats**: Support for other metric formats (JSON, OpenMetrics)
5. **Authentication**: Built-in authentication support
6. **TLS**: HTTPS support for the metrics endpoint
