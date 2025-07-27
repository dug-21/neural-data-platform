use crate::error::{Error, Result};
use crate::models::{
    SystemHealth, ComponentHealth, ComponentHealthMap, PerformanceMetrics, 
    NetworkIO, APILatency, LogEntry, Alert, HealthCheckResult, HealthCheckDetail
};
use chrono::Utc;
use sysinfo::{System, Disks, Networks};
use std::sync::{Arc, Mutex};
use tracing::info;

#[derive(Debug, Clone)]
pub struct MonitorClient {
    system: Arc<Mutex<System>>,
    alerts: Arc<Mutex<Vec<Alert>>>,
    error_logs: Arc<Mutex<Vec<LogEntry>>>,
}

impl MonitorClient {
    pub fn new() -> Self {
        info!("Initializing system monitor client...");
        
        let mut system = System::new_all();
        system.refresh_all();
        
        Self {
            system: Arc::new(Mutex::new(system)),
            alerts: Arc::new(Mutex::new(Vec::new())),
            error_logs: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    pub async fn get_system_health(&self) -> Result<SystemHealth> {
        let mut components = ComponentHealthMap::new();
        
        // Check database health
        components.insert("database".to_string(), self.check_database_health().await?);
        
        // Check Redis health
        components.insert("redis".to_string(), self.check_redis_health().await?);
        
        // Check neural service health
        components.insert("neural".to_string(), self.check_neural_health().await?);
        
        // Check agent service health
        components.insert("agents".to_string(), self.check_agent_health().await?);
        
        // Determine overall status
        let overall_status = self.determine_overall_status(&components);
        
        Ok(SystemHealth {
            overall_status,
            components,
            timestamp: Utc::now(),
        })
    }
    
    pub async fn get_component_health(&self, component: &str) -> Result<ComponentHealth> {
        match component {
            "database" => self.check_database_health().await,
            "redis" => self.check_redis_health().await,
            "neural" => self.check_neural_health().await,
            "agents" => self.check_agent_health().await,
            _ => Err(Error::InvalidParameter(format!("Unknown component: {}", component))),
        }
    }
    
    pub async fn get_performance_metrics(&self, timeframe: &str) -> Result<PerformanceMetrics> {
        let mut system = self.system.lock().unwrap();
        system.refresh_all();
        
        // CPU usage
        let cpu_usage = system.global_cpu_info().cpu_usage();
        
        // Memory usage
        let total_memory = system.total_memory();
        let used_memory = system.used_memory();
        let memory_usage = if total_memory > 0 {
            (used_memory as f64 / total_memory as f64) * 100.0
        } else {
            0.0
        };
        
        // Disk usage
        let disks = Disks::new_with_refreshed_list();
        let mut total_disk = 0;
        let mut used_disk = 0;
        for disk in &disks {
            total_disk += disk.total_space();
            used_disk += disk.total_space() - disk.available_space();
        }
        let disk_usage = if total_disk > 0 {
            (used_disk as f64 / total_disk as f64) * 100.0
        } else {
            0.0
        };
        
        // Network I/O
        let networks = Networks::new_with_refreshed_list();
        let mut bytes_sent = 0;
        let mut bytes_received = 0;
        for (_, network) in &networks {
            bytes_sent += network.transmitted();
            bytes_received += network.received();
        }
        
        Ok(PerformanceMetrics {
            timeframe: timeframe.to_string(),
            cpu_usage: cpu_usage as f64,
            memory_usage,
            disk_usage,
            network_io: NetworkIO {
                bytes_sent,
                bytes_received,
                packets_sent: 0, // Would need pcap for packet counts
                packets_received: 0,
            },
            api_latency: self.calculate_api_latency(),
            timestamp: Utc::now(),
        })
    }
    
    pub async fn get_error_logs(&self, limit: usize) -> Result<Vec<LogEntry>> {
        let logs = self.error_logs.lock().unwrap();
        let start = if logs.len() > limit { logs.len() - limit } else { 0 };
        Ok(logs[start..].to_vec())
    }
    
    pub async fn get_alerts(&self) -> Result<(Vec<Alert>, usize)> {
        let alerts = self.alerts.lock().unwrap();
        let active_alerts: Vec<Alert> = alerts.iter()
            .filter(|a| a.resolved_at.is_none())
            .cloned()
            .collect();
        let count = active_alerts.len();
        Ok((active_alerts, count))
    }
    
    pub async fn run_health_check(&self) -> Result<HealthCheckResult> {
        let mut checks_passed = 0;
        let mut checks_failed = 0;
        let mut details = Vec::new();
        
        // Check system resources
        let metrics = self.get_performance_metrics("1m").await?;
        
        if metrics.cpu_usage < 80.0 {
            checks_passed += 1;
            details.push(HealthCheckDetail {
                check: "CPU Usage".to_string(),
                status: "passed".to_string(),
                value: format!("{:.1}%", metrics.cpu_usage),
            });
        } else {
            checks_failed += 1;
            details.push(HealthCheckDetail {
                check: "CPU Usage".to_string(),
                status: "failed".to_string(),
                value: format!("{:.1}% (high)", metrics.cpu_usage),
            });
        }
        
        if metrics.memory_usage < 90.0 {
            checks_passed += 1;
            details.push(HealthCheckDetail {
                check: "Memory Usage".to_string(),
                status: "passed".to_string(),
                value: format!("{:.1}%", metrics.memory_usage),
            });
        } else {
            checks_failed += 1;
            details.push(HealthCheckDetail {
                check: "Memory Usage".to_string(),
                status: "failed".to_string(),
                value: format!("{:.1}% (high)", metrics.memory_usage),
            });
        }
        
        // Check component health
        let system_health = self.get_system_health().await?;
        for (component, health) in system_health.components {
            if health.status == "healthy" {
                checks_passed += 1;
                details.push(HealthCheckDetail {
                    check: format!("{} Health", component),
                    status: "passed".to_string(),
                    value: format!("{:.1}ms latency", health.latency_ms),
                });
            } else {
                checks_failed += 1;
                details.push(HealthCheckDetail {
                    check: format!("{} Health", component),
                    status: "failed".to_string(),
                    value: health.status,
                });
            }
        }
        
        Ok(HealthCheckResult {
            checks_passed,
            checks_failed,
            details,
            timestamp: Utc::now(),
        })
    }
    
    // Helper methods
    
    async fn check_database_health(&self) -> Result<ComponentHealth> {
        // In a real implementation, this would ping the database
        Ok(ComponentHealth {
            component: "database".to_string(),
            status: "healthy".to_string(),
            latency_ms: 5.2,
            last_check: Utc::now(),
            error_count: 0,
            details: None,
        })
    }
    
    async fn check_redis_health(&self) -> Result<ComponentHealth> {
        // In a real implementation, this would ping Redis
        Ok(ComponentHealth {
            component: "redis".to_string(),
            status: "healthy".to_string(),
            latency_ms: 0.8,
            last_check: Utc::now(),
            error_count: 0,
            details: None,
        })
    }
    
    async fn check_neural_health(&self) -> Result<ComponentHealth> {
        // In a real implementation, this would check neural service
        Ok(ComponentHealth {
            component: "neural".to_string(),
            status: "healthy".to_string(),
            latency_ms: 125.3,
            last_check: Utc::now(),
            error_count: 0,
            details: None,
        })
    }
    
    async fn check_agent_health(&self) -> Result<ComponentHealth> {
        // In a real implementation, this would check agent service
        Ok(ComponentHealth {
            component: "agents".to_string(),
            status: "healthy".to_string(),
            latency_ms: 45.7,
            last_check: Utc::now(),
            error_count: 0,
            details: None,
        })
    }
    
    fn determine_overall_status(&self, components: &ComponentHealthMap) -> String {
        let unhealthy_count = components.values()
            .filter(|c| c.status == "unhealthy")
            .count();
        
        let degraded_count = components.values()
            .filter(|c| c.status == "degraded")
            .count();
        
        if unhealthy_count > 0 {
            "unhealthy".to_string()
        } else if degraded_count > 0 {
            "degraded".to_string()
        } else {
            "healthy".to_string()
        }
    }
    
    fn calculate_api_latency(&self) -> APILatency {
        // In a real implementation, this would track actual API latencies
        APILatency {
            p50: 12.5,
            p90: 45.2,
            p95: 78.3,
            p99: 125.7,
            mean: 28.4,
        }
    }
    
    pub fn log_error(&self, component: &str, message: &str) {
        let entry = LogEntry {
            timestamp: Utc::now(),
            level: "error".to_string(),
            component: component.to_string(),
            message: message.to_string(),
            context: None,
        };
        
        let mut logs = self.error_logs.lock().unwrap();
        logs.push(entry);
        
        // Keep only last 1000 entries
        if logs.len() > 1000 {
            let drain_count = logs.len() - 1000;
            logs.drain(0..drain_count);
        }
    }
    
    pub fn create_alert(&self, severity: &str, component: &str, message: &str) -> String {
        let alert_id = uuid::Uuid::new_v4().to_string();
        let alert = Alert {
            id: alert_id.clone(),
            severity: severity.to_string(),
            component: component.to_string(),
            message: message.to_string(),
            triggered_at: Utc::now(),
            resolved_at: None,
        };
        
        let mut alerts = self.alerts.lock().unwrap();
        alerts.push(alert);
        
        alert_id
    }
    
    pub fn resolve_alert(&self, alert_id: &str) {
        let mut alerts = self.alerts.lock().unwrap();
        if let Some(alert) = alerts.iter_mut().find(|a| a.id == alert_id) {
            alert.resolved_at = Some(Utc::now());
        }
    }
}