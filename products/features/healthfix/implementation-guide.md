# Health Monitoring Implementation Guide

## 🚀 Quick Start - Immediate Actions

### 1. Verify Current State
```bash
# Check if application is running with health monitoring disabled
grep -n "enable_health_monitoring" src/neural/predictor.rs
# Should show: enable_health_monitoring: false,  // RUNTIME FIX
```

### 2. Fix MCP Server Panic (Priority 1)
Edit `/src/bin/mcp_server_simple.rs` line 88:

```rust
// OLD - PANICS
panic!("Cannot continue without neural predictor");

// NEW - GRACEFUL ERROR
error!("Neural predictor initialization failed");
return Err(anyhow!("Cannot start MCP server without neural predictor"));
```

### 3. Create Non-Blocking Health Monitor
Create new file `/src/monitoring/health/async_monitor.rs`:

```rust
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use std::time::Duration;
use anyhow::Result;

pub struct AsyncHealthMonitor {
    inner: Arc<HealthMonitor>,
    shutdown: CancellationToken,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl AsyncHealthMonitor {
    pub fn new(monitor: Arc<HealthMonitor>) -> Self {
        Self {
            inner: monitor,
            shutdown: CancellationToken::new(),
            handle: None,
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        let monitor = self.inner.clone();
        let shutdown = self.shutdown.clone();
        
        // Spawn monitoring task WITHOUT awaiting it
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            
            loop {
                tokio::select! {
                    _ = shutdown.cancelled() => {
                        info!("Health monitoring shutting down gracefully");
                        break;
                    }
                    _ = interval.tick() => {
                        if let Err(e) = monitor.check_all_components().await {
                            error!("Health check failed: {}", e);
                        }
                    }
                }
            }
        });
        
        self.handle = Some(handle);
        info!("Async health monitoring started successfully");
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        self.shutdown.cancel();
        
        if let Some(handle) = self.handle.take() {
            handle.await?;
        }
        
        info!("Health monitoring stopped");
        Ok(())
    }
}
```

### 4. Create Standalone Health Server
Create `/src/monitoring/health/server.rs`:

```rust
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use std::sync::Arc;
use tokio::net::TcpListener;
use serde_json::json;

pub struct HealthServer {
    monitor: Arc<HealthMonitor>,
    port: u16,
}

impl HealthServer {
    pub fn new(monitor: Arc<HealthMonitor>, port: u16) -> Self {
        Self { monitor, port }
    }

    pub async fn start(self) -> Result<()> {
        let app = Router::new()
            .route("/health", get(health_check))
            .route("/health/live", get(liveness_check))
            .route("/health/ready", get(readiness_check))
            .route("/metrics", get(prometheus_metrics))
            .with_state(self.monitor);

        let addr = format!("0.0.0.0:{}", self.port);
        let listener = TcpListener::bind(&addr).await?;
        
        info!("Health server listening on {}", addr);
        
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await?;
        
        Ok(())
    }
}

async fn health_check(
    State(monitor): State<Arc<HealthMonitor>>,
) -> impl IntoResponse {
    let health = monitor.get_system_health().await;
    
    let status_code = if health.is_healthy() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    
    (status_code, Json(json!({
        "status": health.overall_status,
        "timestamp": chrono::Utc::now(),
    })))
}

async fn liveness_check() -> impl IntoResponse {
    // Simple liveness - if we can respond, we're alive
    (StatusCode::OK, "alive")
}

async fn readiness_check(
    State(monitor): State<Arc<HealthMonitor>>,
) -> impl IntoResponse {
    let health = monitor.get_system_health().await;
    
    // Ready only if all critical components are healthy
    let critical_healthy = health.components.iter()
        .filter(|(_, h)| h.is_critical)
        .all(|(_, h)| matches!(h.status, HealthStatus::Healthy));
    
    if critical_healthy {
        (StatusCode::OK, "ready")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "not ready")
    }
}

async fn prometheus_metrics(
    State(monitor): State<Arc<HealthMonitor>>,
) -> impl IntoResponse {
    let metrics = monitor.export_prometheus_metrics().await;
    
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        metrics,
    )
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to listen for ctrl+c");
}
```

### 5. Update Main Application
Modify `/src/main.rs` to use the new async health monitoring:

```rust
// Add to imports
use crate::monitoring::health::{AsyncHealthMonitor, HealthServer};

// In main() function, after neural predictor initialization:
// Initialize health monitoring (non-blocking)
let health_monitor = Arc::new(HealthMonitor::new().await?);
let mut async_monitor = AsyncHealthMonitor::new(health_monitor.clone());

// Start monitoring without blocking
async_monitor.start().await?;
info!("Health monitoring started in background");

// Start health server in separate task
let health_server = HealthServer::new(health_monitor.clone(), 8080);
tokio::spawn(async move {
    if let Err(e) = health_server.start().await {
        error!("Health server failed: {}", e);
    }
});

// ... rest of application initialization ...

// At shutdown:
async_monitor.stop().await?;
```

## 📋 Testing the Implementation

### 1. Test Non-Blocking Startup
```rust
#[tokio::test]
async fn test_health_monitor_non_blocking() {
    let start = std::time::Instant::now();
    
    let monitor = Arc::new(HealthMonitor::new().await.unwrap());
    let mut async_monitor = AsyncHealthMonitor::new(monitor);
    
    async_monitor.start().await.unwrap();
    
    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_millis(100), 
            "Health monitor took too long to start: {:?}", elapsed);
    
    async_monitor.stop().await.unwrap();
}
```

### 2. Test Health Endpoints
```bash
# Start the application
cargo run

# In another terminal, test health endpoints
curl http://localhost:8080/health
curl http://localhost:8080/health/live
curl http://localhost:8080/health/ready
curl http://localhost:8080/metrics
```

### 3. Verify Monitoring Works
```bash
# Check logs for health check messages
tail -f logs/neural-trader.log | grep -i health

# Should see periodic health check logs every 30 seconds
```

## 🔄 Gradual Re-enablement Strategy

Once the async health monitoring is working:

1. **Test in Development**
   ```rust
   // Re-enable health monitoring in dev
   enable_health_monitoring: std::env::var("ENABLE_HEALTH_MONITORING")
       .unwrap_or_else(|_| "false".to_string())
       .parse()
       .unwrap_or(false),
   ```

2. **Staged Rollout**
   - Enable in staging environment first
   - Monitor for 24 hours
   - Check performance metrics
   - Enable in production if stable

3. **Feature Flag Control**
   ```toml
   # config.toml
   [monitoring]
   health_enabled = true
   health_check_interval_secs = 30
   health_server_port = 8080
   ```

## 🚨 Monitoring Checklist

Before enabling in production:
- [ ] MCP server panic is fixed
- [ ] Health monitor starts without blocking
- [ ] Health endpoints respond within 100ms
- [ ] No performance degradation observed
- [ ] Graceful shutdown works correctly
- [ ] Metrics are accurately collected
- [ ] Alerts are properly configured

## 📊 Performance Validation

Monitor these metrics after implementation:
1. **Startup Time**: Should remain under 2 seconds
2. **Memory Usage**: Health monitoring should add < 50MB
3. **CPU Usage**: Background monitoring < 1% CPU
4. **Endpoint Latency**: Health checks < 100ms p99

## 🔗 Next Steps

After Phase 1 is complete and validated:
1. Implement real health checks (Phase 2)
2. Add OpenTelemetry integration (Phase 3)
3. Build predictive monitoring (Phase 4)

Each phase builds on the previous one, ensuring stability at every step.