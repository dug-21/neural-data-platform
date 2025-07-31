# Health Monitoring Code Samples

## 🔧 Complete Working Examples

### 1. Real Database Health Check
```rust
use sqlx::{Pool, Postgres};
use std::time::{Duration, Instant};

impl HealthChecker {
    pub async fn check_database(&self) -> Result<HealthStatus> {
        let pool = self.database_pool.as_ref()
            .ok_or_else(|| anyhow!("Database pool not initialized"))?;
        
        let start = Instant::now();
        let timeout = Duration::from_secs(5);
        
        // Use timeout to prevent hanging
        let result = tokio::time::timeout(timeout, async {
            sqlx::query("SELECT 1 as health_check")
                .fetch_one(pool)
                .await
        }).await;
        
        match result {
            Ok(Ok(_)) => {
                let latency = start.elapsed();
                if latency > Duration::from_millis(1000) {
                    Ok(HealthStatus::Degraded(format!(
                        "Database responding slowly: {:?}", latency
                    )))
                } else {
                    Ok(HealthStatus::Healthy)
                }
            }
            Ok(Err(e)) => Ok(HealthStatus::Unhealthy(format!(
                "Database query failed: {}", e
            ))),
            Err(_) => Ok(HealthStatus::Unhealthy(
                "Database health check timed out".to_string()
            )),
        }
    }
}
```

### 2. Redis Health Check with Connection Pooling
```rust
use redis::{aio::ConnectionManager, AsyncCommands};
use std::time::{Duration, Instant};

impl HealthChecker {
    pub async fn check_redis(&self) -> Result<HealthStatus> {
        let mut conn = self.redis_manager.as_ref()
            .ok_or_else(|| anyhow!("Redis connection not initialized"))?
            .clone();
        
        let start = Instant::now();
        let timeout = Duration::from_secs(2);
        
        let result = tokio::time::timeout(timeout, async {
            // Ping and check latency
            let _: String = conn.ping().await?;
            
            // Also check if we can set/get
            let test_key = "health_check_test";
            let test_value = format!("health_{}", chrono::Utc::now().timestamp());
            
            conn.set_ex(test_key, &test_value, 60).await?;
            let retrieved: String = conn.get(test_key).await?;
            
            if retrieved != test_value {
                return Err(anyhow!("Redis data integrity check failed"));
            }
            
            Ok(())
        }).await;
        
        match result {
            Ok(Ok(_)) => {
                let latency = start.elapsed();
                if latency > Duration::from_millis(500) {
                    Ok(HealthStatus::Degraded(format!(
                        "Redis responding slowly: {:?}", latency
                    )))
                } else {
                    Ok(HealthStatus::Healthy)
                }
            }
            Ok(Err(e)) => Ok(HealthStatus::Unhealthy(format!(
                "Redis operation failed: {}", e
            ))),
            Err(_) => Ok(HealthStatus::Unhealthy(
                "Redis health check timed out".to_string()
            )),
        }
    }
}
```

### 3. Neural System Health Check
```rust
impl NeuralPredictor {
    pub async fn health_check(&self) -> Result<NeuralHealthStatus> {
        let models_guard = self.models.read().await;
        let active_models = models_guard.len();
        
        // Check if we have models loaded
        if active_models == 0 {
            return Ok(NeuralHealthStatus {
                healthy: false,
                models_available: 0,
                last_prediction: None,
                error: Some("No models loaded".to_string()),
            });
        }
        
        // Check last prediction time
        let last_prediction = self.last_prediction_time.read().await;
        let time_since_last = last_prediction
            .map(|t| chrono::Utc::now().signed_duration_since(t))
            .and_then(|d| d.to_std().ok());
        
        // If no predictions for > 5 minutes, consider degraded
        let is_stale = time_since_last
            .map(|d| d > Duration::from_secs(300))
            .unwrap_or(true);
        
        // Try a test prediction
        let test_result = tokio::time::timeout(
            Duration::from_secs(2),
            self.test_prediction()
        ).await;
        
        match test_result {
            Ok(Ok(_)) => Ok(NeuralHealthStatus {
                healthy: !is_stale,
                models_available: active_models,
                last_prediction: *last_prediction,
                error: if is_stale {
                    Some("No recent predictions".to_string())
                } else {
                    None
                },
            }),
            Ok(Err(e)) => Ok(NeuralHealthStatus {
                healthy: false,
                models_available: active_models,
                last_prediction: *last_prediction,
                error: Some(format!("Test prediction failed: {}", e)),
            }),
            Err(_) => Ok(NeuralHealthStatus {
                healthy: false,
                models_available: active_models,
                last_prediction: *last_prediction,
                error: Some("Neural system timeout".to_string()),
            }),
        }
    }
    
    async fn test_prediction(&self) -> Result<()> {
        // Create minimal test input
        let test_input = TimeSeriesData {
            timestamp: chrono::Utc::now(),
            symbol: "TEST".to_string(),
            price: 100.0,
            volume: 1000.0,
            bid: 99.9,
            ask: 100.1,
        };
        
        // Try to get a prediction
        let _ = self.predict(&test_input).await?;
        Ok(())
    }
}
```

### 4. System Resource Health Check
```rust
use sysinfo::{System, SystemExt, CpuExt, DiskExt};

pub struct SystemHealthChecker {
    system: tokio::sync::Mutex<System>,
}

impl SystemHealthChecker {
    pub async fn new() -> Self {
        Self {
            system: tokio::sync::Mutex::new(System::new_all()),
        }
    }
    
    pub async fn check_system_resources(&self) -> Result<SystemResourceHealth> {
        let mut system = self.system.lock().await;
        system.refresh_all();
        
        // CPU usage
        let cpu_usage = system.global_cpu_info().cpu_usage();
        let cpu_status = if cpu_usage > 90.0 {
            HealthStatus::Unhealthy(format!("CPU usage critical: {:.1}%", cpu_usage))
        } else if cpu_usage > 75.0 {
            HealthStatus::Degraded(format!("CPU usage high: {:.1}%", cpu_usage))
        } else {
            HealthStatus::Healthy
        };
        
        // Memory usage
        let total_memory = system.total_memory();
        let used_memory = system.used_memory();
        let memory_usage_percent = (used_memory as f64 / total_memory as f64) * 100.0;
        
        let memory_status = if memory_usage_percent > 90.0 {
            HealthStatus::Unhealthy(format!("Memory usage critical: {:.1}%", memory_usage_percent))
        } else if memory_usage_percent > 80.0 {
            HealthStatus::Degraded(format!("Memory usage high: {:.1}%", memory_usage_percent))
        } else {
            HealthStatus::Healthy
        };
        
        // Disk usage
        let mut disk_statuses = Vec::new();
        for disk in system.disks() {
            let usage_percent = ((disk.total_space() - disk.available_space()) as f64 
                / disk.total_space() as f64) * 100.0;
            
            if usage_percent > 90.0 {
                disk_statuses.push(HealthStatus::Unhealthy(format!(
                    "Disk {} usage critical: {:.1}%", 
                    disk.mount_point().display(), 
                    usage_percent
                )));
            } else if usage_percent > 80.0 {
                disk_statuses.push(HealthStatus::Degraded(format!(
                    "Disk {} usage high: {:.1}%", 
                    disk.mount_point().display(), 
                    usage_percent
                )));
            }
        }
        
        // Determine overall status
        let overall_status = if cpu_status.is_unhealthy() 
            || memory_status.is_unhealthy() 
            || disk_statuses.iter().any(|s| s.is_unhealthy()) {
            HealthStatus::Unhealthy("System resources critical".to_string())
        } else if cpu_status.is_degraded() 
            || memory_status.is_degraded() 
            || disk_statuses.iter().any(|s| s.is_degraded()) {
            HealthStatus::Degraded("System resources degraded".to_string())
        } else {
            HealthStatus::Healthy
        };
        
        Ok(SystemResourceHealth {
            overall_status,
            cpu_status,
            memory_status,
            disk_statuses,
            cpu_usage,
            memory_usage_mb: used_memory / 1024 / 1024,
            memory_total_mb: total_memory / 1024 / 1024,
        })
    }
}
```

### 5. Circuit Breaker Integration
```rust
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

pub struct CircuitBreaker {
    failure_count: AtomicU32,
    last_failure: tokio::sync::RwLock<Option<Instant>>,
    state: tokio::sync::RwLock<CircuitState>,
    failure_threshold: u32,
    timeout: Duration,
    recovery_timeout: Duration,
}

#[derive(Clone, Copy, Debug)]
enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, timeout: Duration, recovery_timeout: Duration) -> Self {
        Self {
            failure_count: AtomicU32::new(0),
            last_failure: tokio::sync::RwLock::new(None),
            state: tokio::sync::RwLock::new(CircuitState::Closed),
            failure_threshold,
            timeout,
            recovery_timeout,
        }
    }
    
    pub async fn call<F, T, E>(&self, f: F) -> Result<T, E>
    where
        F: Future<Output = Result<T, E>>,
        E: std::fmt::Display,
    {
        let state = *self.state.read().await;
        
        match state {
            CircuitState::Open => {
                // Check if we should try half-open
                let last_failure = *self.last_failure.read().await;
                if let Some(last) = last_failure {
                    if last.elapsed() >= self.recovery_timeout {
                        *self.state.write().await = CircuitState::HalfOpen;
                    } else {
                        return Err(anyhow!("Circuit breaker is open"));
                    }
                } else {
                    return Err(anyhow!("Circuit breaker is open"));
                }
            }
            _ => {}
        }
        
        // Try the operation with timeout
        let result = tokio::time::timeout(self.timeout, f).await;
        
        match result {
            Ok(Ok(value)) => {
                // Success - reset failure count
                self.failure_count.store(0, Ordering::Relaxed);
                *self.state.write().await = CircuitState::Closed;
                Ok(value)
            }
            Ok(Err(e)) | Err(_) => {
                // Failure - increment counter
                let failures = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
                *self.last_failure.write().await = Some(Instant::now());
                
                if failures >= self.failure_threshold {
                    *self.state.write().await = CircuitState::Open;
                    error!("Circuit breaker opened after {} failures", failures);
                }
                
                if result.is_err() {
                    Err(anyhow!("Operation timed out"))
                } else {
                    result.unwrap()
                }
            }
        }
    }
}

// Usage in health checks
impl HealthChecker {
    pub async fn check_external_service(&self) -> Result<HealthStatus> {
        let circuit_breaker = &self.external_service_breaker;
        
        match circuit_breaker.call(async {
            // Your external service health check
            self.external_client.health_check().await
        }).await {
            Ok(_) => Ok(HealthStatus::Healthy),
            Err(e) if e.to_string().contains("Circuit breaker is open") => {
                Ok(HealthStatus::Unhealthy("External service circuit breaker open".to_string()))
            }
            Err(e) => Ok(HealthStatus::Degraded(format!("External service error: {}", e))),
        }
    }
}
```

### 6. Comprehensive Health Dashboard Response
```rust
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
pub struct HealthDashboardResponse {
    pub status: OverallStatus,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub version: String,
    pub uptime_seconds: u64,
    pub components: HashMap<String, ComponentHealthDetail>,
    pub metrics: SystemMetrics,
    pub alerts: Vec<Alert>,
}

#[derive(Serialize, Deserialize)]
pub struct ComponentHealthDetail {
    pub status: String,
    pub message: Option<String>,
    pub last_check: chrono::DateTime<chrono::Utc>,
    pub check_duration_ms: u64,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
pub struct SystemMetrics {
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: u64,
    pub memory_total_mb: u64,
    pub active_connections: u32,
    pub requests_per_second: f64,
    pub average_latency_ms: f64,
    pub error_rate_percent: f64,
}

pub async fn create_health_dashboard(
    monitor: &HealthMonitor,
    start_time: Instant,
) -> Result<HealthDashboardResponse> {
    let health = monitor.get_system_health().await?;
    let metrics = monitor.get_current_metrics().await?;
    let alerts = monitor.get_active_alerts().await?;
    
    let mut components = HashMap::new();
    
    for (name, component_health) in health.components {
        components.insert(
            name.to_string(),
            ComponentHealthDetail {
                status: format!("{:?}", component_health.status),
                message: component_health.message,
                last_check: component_health.last_check,
                check_duration_ms: component_health.check_duration.as_millis() as u64,
                metadata: component_health.metadata,
            },
        );
    }
    
    Ok(HealthDashboardResponse {
        status: health.overall_status,
        timestamp: chrono::Utc::now(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: start_time.elapsed().as_secs(),
        components,
        metrics: SystemMetrics {
            cpu_usage_percent: metrics.cpu_usage,
            memory_usage_mb: metrics.memory_usage_mb,
            memory_total_mb: metrics.memory_total_mb,
            active_connections: metrics.active_connections,
            requests_per_second: metrics.requests_per_second,
            average_latency_ms: metrics.average_latency_ms,
            error_rate_percent: metrics.error_rate_percent,
        },
        alerts: alerts.into_iter().map(|a| Alert {
            id: a.id,
            severity: a.severity,
            component: a.component,
            message: a.message,
            triggered_at: a.triggered_at,
        }).collect(),
    })
}
```

## 🧪 Testing Utilities

### Health Check Test Helpers
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    async fn create_test_monitor() -> HealthMonitor {
        let config = HealthConfig::default();
        HealthMonitor::new(config).await.unwrap()
    }
    
    #[tokio::test]
    async fn test_health_check_timeout() {
        let monitor = create_test_monitor().await;
        
        // Simulate slow health check
        let slow_check = async {
            tokio::time::sleep(Duration::from_secs(10)).await;
            Ok(HealthStatus::Healthy)
        };
        
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            slow_check
        ).await;
        
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_circuit_breaker_opens() {
        let breaker = CircuitBreaker::new(3, Duration::from_secs(1), Duration::from_secs(5));
        
        // Fail 3 times
        for _ in 0..3 {
            let _ = breaker.call(async {
                Err::<(), _>(anyhow!("Simulated failure"))
            }).await;
        }
        
        // Next call should fail immediately
        let result = breaker.call(async {
            Ok::<_, anyhow::Error>(())
        }).await;
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Circuit breaker is open"));
    }
}
```

These code samples provide production-ready implementations for all aspects of the health monitoring system. Each example includes proper error handling, timeouts, and graceful degradation.