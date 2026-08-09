use anyhow::{Context, Result};
use axum::{
    body::Body,
    extract::State,
    http::{Request, Response, StatusCode, Uri},
    response::IntoResponse,
    routing::any,
    Router,
};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::trace::TraceLayer;

use super::DaemonState;

/// Backend server for reverse proxy
#[derive(Clone, Debug)]
pub struct Backend {
    pub addr: SocketAddr,
    pub healthy: Arc<AtomicBool>,
    pub active_connections: Arc<AtomicU64>,
    pub total_requests: Arc<AtomicU64>,
}

impl Backend {
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            healthy: Arc::new(AtomicBool::new(true)),
            active_connections: Arc::new(AtomicU64::new(0)),
            total_requests: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn is_healthy(&self) -> bool {
        self.healthy.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn mark_unhealthy(&self) {
        self.healthy.store(false, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn mark_healthy(&self) {
        self.healthy.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// HTTP-aware reverse proxy
pub struct ReverseProxy {
    listen_port: u16,
    backends: Vec<Backend>,
    state: Arc<RwLock<DaemonState>>,
    current_index: Arc<AtomicUsize>,
}

impl ReverseProxy {
    pub fn new(listen_port: u16, backends: Vec<Backend>, state: Arc<RwLock<DaemonState>>) -> Self {
        Self {
            listen_port,
            backends,
            state,
            current_index: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Get next healthy backend (round-robin)
    fn next_backend(&self) -> Option<&Backend> {
        let healthy_backends: Vec<&Backend> = self.backends.iter()
            .filter(|b| b.is_healthy())
            .collect();

        if healthy_backends.is_empty() {
            return None;
        }

        let index = self.current_index.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Some(healthy_backends[index % healthy_backends.len()])
    }

    /// Run the reverse proxy
    pub async fn run(self: &Arc<Self>) -> Result<()> {
        let shared_state = self.clone();

        let app = Router::new()
            .fallback(any(Self::proxy_handler))
            .layer(TraceLayer::new_for_http())
            .with_state(shared_state);

        let addr = SocketAddr::from(([0, 0, 0, 0], self.listen_port));
        tracing::info!("Reverse proxy listening on {}", addr);

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app.into_make_service())
            .await
            .context("Failed to start proxy server")?;

        Ok(())
    }

    /// Handle proxy request
    async fn proxy_handler(
        State(state): State<Arc<ReverseProxy>>,
        req: Request<Body>,
    ) -> Result<impl IntoResponse, StatusCode> {
        // Select a healthy backend
        let backend = state.next_backend()
            .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

        // Log request for metrics
        {
            let _state_guard = state.state.read().await;
            tracing::debug!(
                "Proxying {} {} to {}",
                req.method(),
                req.uri(),
                backend.addr
            );
        }

        // Get the original URI
        let uri = req.uri().clone();
        let path = uri.path();
        let query = uri.query().unwrap_or("");

        // Build target URI
        let target_uri = format!(
            "http://{}:{}{}{}",
            backend.addr.ip(),
            backend.addr.port(),
            path,
            if query.is_empty() { String::new() } else { format!("?{}", query) }
        );

        let target_uri: Uri = target_uri.parse().map_err(|_| StatusCode::BAD_REQUEST)?;

        // Create client
        let client: Client<hyper_util::client::legacy::connect::HttpConnector, Body> = 
            Client::builder(TokioExecutor::new()).build_http();

        // Build new request
        let (parts, body) = req.into_parts();
        let mut new_req = Request::builder()
            .uri(target_uri)
            .method(parts.method)
            .version(parts.version);

        // Copy headers
        for (key, value) in parts.headers.iter() {
            new_req = new_req.header(key, value);
        }

        // Add X-Forwarded headers
        new_req = new_req
            .header("X-Forwarded-For", "luci-proxy")
            .header("X-Forwarded-Proto", "http");

        let new_req = new_req
            .body(body)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        // Track connection
        backend.active_connections.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        backend.total_requests.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Forward request
        let response = client
            .request(new_req)
            .await
            .map_err(|e| {
                tracing::error!("Proxy request failed: {}", e);
                backend.mark_unhealthy();
                StatusCode::BAD_GATEWAY
            })?;

        // Track connection end
        backend.active_connections.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);

        // Build response
        let (parts, body) = response.into_parts();
        let response = Response::from_parts(parts, body);

        Ok(response)
    }

    /// Perform health check on all backends
    pub async fn health_check(&self) {
        for backend in &self.backends {
            let client: Client<hyper_util::client::legacy::connect::HttpConnector, Body> = 
                Client::builder(TokioExecutor::new()).build_http();

            let uri = format!("http://{}/health", backend.addr);
            let req = Request::builder()
                .uri(&uri)
                .body(Body::empty())
                .unwrap();

            match tokio::time::timeout(
                std::time::Duration::from_secs(5),
                client.request(req),
            ).await {
                Ok(Ok(response)) => {
                    let healthy = response.status().is_success();
                    if healthy {
                        backend.mark_healthy();
                    } else {
                        backend.mark_unhealthy();
                    }
                }
                _ => {
                    backend.mark_unhealthy();
                }
            }
        }
    }

    /// Get proxy stats
    pub fn stats(&self) -> Vec<(String, bool, u64, u64)> {
        self.backends.iter().map(|b| {
            (
                b.addr.to_string(),
                b.is_healthy(),
                b.active_connections.load(std::sync::atomic::Ordering::Relaxed),
                b.total_requests.load(std::sync::atomic::Ordering::Relaxed),
            )
        }).collect()
    }
}

/// Start reverse proxy as a background task
pub fn start_proxy_task(
    listen_port: u16,
    target_addrs: Vec<String>,
    state: Arc<RwLock<DaemonState>>,
) -> tokio::task::JoinHandle<Result<()>> {
    let backends: Vec<Backend> = target_addrs.iter()
        .filter_map(|addr| addr.parse().ok())
        .map(Backend::new)
        .collect();

    let proxy = ReverseProxy::new(listen_port, backends, state);

    tokio::spawn(async move {
        // Share proxy state between handler and health check
        let proxy = Arc::new(proxy);

        // Start health check task
        let health_proxy = proxy.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            loop {
                interval.tick().await;
                health_proxy.health_check().await;
            }
        });

        // Run proxy
        proxy.run().await
    })
}
