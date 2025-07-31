# Health Monitoring Implementation

This directory contains the implementation of the enhanced health monitoring system for the Neural Trader platform.

## Quick Start

### Running Tests

```bash
# Run all tests
cargo test --test '*' -- --nocapture

# Run specific test suite
cargo test --test mcp_server_panic_fix_test
cargo test --test async_health_monitor_test
cargo test --test health_server_test
cargo test --test component_health_checks_test
cargo test --test integration_test
```

### Integration

To integrate this health monitoring implementation into the main application:

1. Copy the `src/` directory contents to your main src directory
2. Update `Cargo.toml` with required dependencies:
   ```toml
   [dependencies]
   axum = "0.7"
   tokio = { version = "1", features = ["full"] }
   tokio-util = "0.7"
   sqlx = { version = "0.7", features = ["postgres", "runtime-tokio-native-tls"] }
   redis = { version = "0.24", features = ["tokio-comp"] }
   anyhow = "1.0"
   async-trait = "0.1"
   serde = { version = "1.0", features = ["derive"] }
   tracing = "0.1"
   futures = "0.3"
   ```

3. Initialize health monitoring in your main application:
   ```rust
   use health_monitoring::{AsyncHealthMonitor, HealthServer, HealthServerConfig};

   // Start health monitoring
   let mut health_monitor = AsyncHealthMonitor::new(Default::default());
   health_monitor.start().await?;

   // Start health server
   let mut health_server = HealthServer::new(HealthServerConfig::default());
   health_server.start().await?;
   ```

## Architecture

### Core Components

1. **AsyncHealthMonitor** - Non-blocking health monitoring orchestrator
2. **HealthServer** - HTTP server providing health endpoints
3. **Component Checkers** - Real health checks for each system component
4. **Circuit Breaker** - Fault tolerance mechanism

### Health Check Flow

```
AsyncHealthMonitor
    ├── Spawns background monitoring task
    ├── Performs concurrent health checks
    │   ├── DatabaseHealthChecker
    │   ├── RedisHealthChecker
    │   ├── NeuralSystemHealthChecker
    │   └── DAAOrchestratorHealthChecker
    └── Updates system health state

HealthServer (Port 8080)
    ├── /health         → Overall system health
    ├── /health/live    → Liveness probe
    ├── /health/ready   → Readiness probe
    └── /metrics        → Prometheus metrics
```

## Configuration

### Environment Variables

```bash
# Enable/disable health monitoring
export HEALTH_MONITORING_ENABLED=true

# Health server configuration
export HEALTH_SERVER_PORT=8080
export HEALTH_CHECK_INTERVAL_SECONDS=30

# Component-specific timeouts
export HEALTH_DATABASE_TIMEOUT_SECONDS=5
export HEALTH_REDIS_TIMEOUT_SECONDS=3
export HEALTH_NEURAL_TIMEOUT_SECONDS=10

# MCP server configuration
export MCP_ALLOW_DEGRADED_MODE=true
export MCP_REQUIRE_DATABASE=true
export MCP_REQUIRE_REDIS=false
export MCP_REQUIRE_NEURAL=true
```

## Endpoints

### Health Endpoint
```bash
curl http://localhost:8080/health
```

Response:
```json
{
  "status": "healthy",
  "timestamp": "2024-01-01T00:00:00Z",
  "system_uptime": "24h30m15s",
  "components": {
    "database": {
      "status": "healthy",
      "response_time_ms": 5,
      "last_check": "2024-01-01T00:00:00Z"
    },
    "redis": {
      "status": "healthy",
      "response_time_ms": 2,
      "last_check": "2024-01-01T00:00:00Z"
    }
  },
  "metrics": {
    "total_components": 4,
    "healthy_components": 4,
    "degraded_components": 0,
    "unhealthy_components": 0,
    "health_score": 1.0
  }
}
```

### Metrics Endpoint
```bash
curl http://localhost:8080/metrics
```

Returns Prometheus-formatted metrics.

## Monitoring Dashboard

For production monitoring, import the provided Grafana dashboard or use these key metrics:

- `system_health_score` - Overall health (0.0-1.0)
- `component_health_status{component="..."}` - Per-component status
- `health_check_duration_seconds` - Check latency histogram
- `healthy_components_total` - Count of healthy components

## Troubleshooting

### Common Issues

1. **Health check timeouts**
   - Increase component-specific timeout values
   - Check network connectivity to external services

2. **High memory usage**
   - Reduce `history_size` in HealthMonitorConfig
   - Decrease check frequency

3. **Circuit breaker open**
   - Check component logs for errors
   - Verify external service availability
   - Wait for recovery timeout

## Next Steps

1. Connect to real database and Redis instances
2. Integrate with actual neural predictor
3. Configure alerts based on health metrics
4. Deploy to staging environment
5. Create operational runbooks

For more details, see the [Implementation Report](IMPLEMENTATION_REPORT.md).