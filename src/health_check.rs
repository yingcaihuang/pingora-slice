//! Health Check Endpoint
//!
//! This module provides a simple HTTP health check endpoint for monitoring
//! the streaming proxy service.

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tracing::{error, info};

/// Health status of the service
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// Service is healthy and ready to serve requests
    Healthy,
    /// Service is degraded but still operational
    Degraded,
    /// Service is unhealthy and cannot serve requests
    Unhealthy,
}

impl HealthStatus {
    /// Convert health status to HTTP status code
    pub fn to_status_code(&self) -> StatusCode {
        match self {
            HealthStatus::Healthy => StatusCode::OK,
            HealthStatus::Degraded => StatusCode::OK, // Still return 200 for degraded
            HealthStatus::Unhealthy => StatusCode::SERVICE_UNAVAILABLE,
        }
    }

    /// Convert health status to string
    pub fn as_str(&self) -> &'static str {
        match self {
            HealthStatus::Healthy => "healthy",
            HealthStatus::Degraded => "degraded",
            HealthStatus::Unhealthy => "unhealthy",
        }
    }
}

/// Health check service
pub struct HealthCheckService {
    status: Arc<RwLock<HealthStatus>>,
}

impl HealthCheckService {
    /// Create a new health check service
    pub fn new() -> Self {
        Self {
            status: Arc::new(RwLock::new(HealthStatus::Healthy)),
        }
    }

    /// Get the current health status
    pub async fn status(&self) -> HealthStatus {
        *self.status.read().await
    }

    /// Set the health status
    pub async fn set_status(&self, status: HealthStatus) {
        *self.status.write().await = status;
    }

    /// Start the health check HTTP server
    ///
    /// This starts a simple HTTP server that responds to health check requests.
    ///
    /// # Arguments
    /// * `addr` - Address to bind the server to (e.g., "127.0.0.1:8081")
    ///
    /// # Endpoints
    /// - `GET /health` - Returns health status
    /// - `GET /ready` - Returns readiness status (same as health for now)
    /// - `GET /live` - Returns liveness status (always healthy if server is running)
    ///
    /// # Requirements
    /// Validates: Phase 7, Task 7.4 - Implement health check endpoint
    pub async fn start(self: Arc<Self>, addr: impl Into<SocketAddr>) -> Result<(), Box<dyn std::error::Error>> {
        let addr = addr.into();
        
        info!("Starting health check server on http://{}", addr);
        
        let listener = TcpListener::bind(addr).await?;
        
        info!("Health check server listening on http://{}", addr);
        info!("  GET /health - Health status");
        info!("  GET /ready  - Readiness status");
        info!("  GET /live   - Liveness status");

        loop {
            let (stream, _) = listener.accept().await?;
            let io = TokioIo::new(stream);
            let service = self.clone();

            tokio::task::spawn(async move {
                let result = http1::Builder::new()
                    .serve_connection(
                        io,
                        service_fn(move |req| {
                            let service = service.clone();
                            handle_request(service, req)
                        }),
                    )
                    .await;
                
                if let Err(err) = result {
                    error!("Error serving connection: {:?}", err);
                }
            });
        }
    }
}

impl Default for HealthCheckService {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle HTTP requests for health check endpoints
async fn handle_request(
    service: Arc<HealthCheckService>,
    req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, hyper::http::Error> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/health") => {
            let status = service.status().await;
            Response::builder()
                .status(status.to_status_code())
                .header("Content-Type", "application/json")
                .body(Full::new(Bytes::from(format!(
                    r#"{{"status":"{}"}}"#,
                    status.as_str()
                ))))
        }
        (&Method::GET, "/ready") => {
            // Readiness check - same as health for now
            let status = service.status().await;
            Response::builder()
                .status(status.to_status_code())
                .header("Content-Type", "application/json")
                .body(Full::new(Bytes::from(format!(
                    r#"{{"status":"{}"}}"#,
                    status.as_str()
                ))))
        }
        (&Method::GET, "/live") => {
            // Liveness check - always healthy if server is running
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Full::new(Bytes::from(r#"{"status":"healthy"}"#)))
        }
        _ => {
            // 404 for other paths
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header("Content-Type", "application/json")
                .body(Full::new(Bytes::from(r#"{"error":"not found"}"#)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_status() {
        let service = HealthCheckService::new();
        
        // Initially healthy
        assert_eq!(service.status().await, HealthStatus::Healthy);
        
        // Set to degraded
        service.set_status(HealthStatus::Degraded).await;
        assert_eq!(service.status().await, HealthStatus::Degraded);
        
        // Set to unhealthy
        service.set_status(HealthStatus::Unhealthy).await;
        assert_eq!(service.status().await, HealthStatus::Unhealthy);
        
        // Set back to healthy
        service.set_status(HealthStatus::Healthy).await;
        assert_eq!(service.status().await, HealthStatus::Healthy);
    }

    #[test]
    fn test_health_status_to_status_code() {
        assert_eq!(HealthStatus::Healthy.to_status_code(), StatusCode::OK);
        assert_eq!(HealthStatus::Degraded.to_status_code(), StatusCode::OK);
        assert_eq!(HealthStatus::Unhealthy.to_status_code(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn test_health_status_as_str() {
        assert_eq!(HealthStatus::Healthy.as_str(), "healthy");
        assert_eq!(HealthStatus::Degraded.as_str(), "degraded");
        assert_eq!(HealthStatus::Unhealthy.as_str(), "unhealthy");
    }
}
