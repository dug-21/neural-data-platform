//! System monitoring integration for production observability
//!
//! This module provides real system metrics collection using sysinfo
//! and integrates with the observability metrics system.

use anyhow::Result;
use sysinfo::{Disks, Networks, System};

/// System monitor that collects real system metrics
pub struct SystemMonitor {
    system: System,
}

impl SystemMonitor {
    pub fn new() -> Self {
        let system = System::new_all();

        Self { system }
    }

    /// Collect all system metrics and return summary
    pub async fn collect_metrics(&mut self) -> Result<SystemSummary> {
        // Refresh system information
        self.system.refresh_all();

        let total_memory = self.system.total_memory() * 1024;
        let used_memory = self.system.used_memory() * 1024;
        let memory_usage_percent = (used_memory as f64 / total_memory as f64) * 100.0;

        let global_cpu = self.system.global_cpu_info();
        let cpu_usage_percent = global_cpu.cpu_usage() as f64;

        let load_avg = sysinfo::System::load_average();

        let mut total_disk_space = 0u64;
        let mut available_disk_space = 0u64;
        let mut network_bytes_sent = 0u64;
        let mut network_bytes_received = 0u64;

        // Collect disk metrics
        let disks = Disks::new_with_refreshed_list();
        for disk in disks.list() {
            total_disk_space += disk.total_space();
            available_disk_space += disk.available_space();
        }

        // Collect network metrics
        let networks = Networks::new_with_refreshed_list();
        for (_interface_name, network_data) in networks.iter() {
            network_bytes_received += network_data.total_received();
            network_bytes_sent += network_data.total_transmitted();
        }

        let used_disk_space = total_disk_space - available_disk_space;
        let disk_usage_percent = if total_disk_space > 0 {
            (used_disk_space as f64 / total_disk_space as f64) * 100.0
        } else {
            0.0
        };

        Ok(SystemSummary {
            cpu_usage_percent,
            memory_usage_percent,
            disk_usage_percent,
            load_average_1m: load_avg.one,
            load_average_5m: load_avg.five,
            load_average_15m: load_avg.fifteen,
            total_memory_bytes: total_memory,
            used_memory_bytes: used_memory,
            total_disk_bytes: total_disk_space,
            used_disk_bytes: used_disk_space,
            process_count: self.system.processes().len(),
            uptime_seconds: sysinfo::System::uptime(),
            network_bytes_sent,
            network_bytes_received,
        })
    }

    /// Get current system summary
    pub fn get_system_summary(&mut self) -> SystemSummary {
        self.system.refresh_all();

        let total_memory = self.system.total_memory() * 1024;
        let used_memory = self.system.used_memory() * 1024;
        let memory_usage_percent = (used_memory as f64 / total_memory as f64) * 100.0;

        let global_cpu = self.system.global_cpu_info();
        let cpu_usage_percent = global_cpu.cpu_usage() as f64;

        let load_avg = sysinfo::System::load_average();

        let mut total_disk_space = 0u64;
        let mut available_disk_space = 0u64;
        let mut network_bytes_sent = 0u64;
        let mut network_bytes_received = 0u64;

        let disks = Disks::new_with_refreshed_list();
        for disk in disks.list() {
            total_disk_space += disk.total_space();
            available_disk_space += disk.available_space();
        }

        let networks = Networks::new_with_refreshed_list();
        for (_interface_name, network_data) in networks.iter() {
            network_bytes_received += network_data.total_received();
            network_bytes_sent += network_data.total_transmitted();
        }

        let used_disk_space = total_disk_space - available_disk_space;
        let disk_usage_percent = if total_disk_space > 0 {
            (used_disk_space as f64 / total_disk_space as f64) * 100.0
        } else {
            0.0
        };

        SystemSummary {
            cpu_usage_percent,
            memory_usage_percent,
            disk_usage_percent,
            load_average_1m: load_avg.one,
            load_average_5m: load_avg.five,
            load_average_15m: load_avg.fifteen,
            total_memory_bytes: total_memory,
            used_memory_bytes: used_memory,
            total_disk_bytes: total_disk_space,
            used_disk_bytes: used_disk_space,
            process_count: self.system.processes().len(),
            uptime_seconds: sysinfo::System::uptime(),
            network_bytes_sent,
            network_bytes_received,
        }
    }

    /// Get process-specific metrics for our application
    pub fn get_process_metrics(&mut self, process_name: &str) -> Option<ProcessMetrics> {
        self.system.refresh_processes();

        for (pid, process) in self.system.processes() {
            if process.name().contains(process_name) {
                return Some(ProcessMetrics {
                    pid: pid.as_u32(),
                    name: process.name().to_string(),
                    cpu_usage_percent: process.cpu_usage() as f64,
                    memory_bytes: process.memory() * 1024,
                    virtual_memory_bytes: process.virtual_memory() * 1024,
                    status: format!("{:?}", process.status()),
                    start_time: process.start_time(),
                });
            }
        }

        None
    }
}

/// System summary snapshot
#[derive(Debug, Clone)]
pub struct SystemSummary {
    pub cpu_usage_percent: f64,
    pub memory_usage_percent: f64,
    pub disk_usage_percent: f64,
    pub load_average_1m: f64,
    pub load_average_5m: f64,
    pub load_average_15m: f64,
    pub total_memory_bytes: u64,
    pub used_memory_bytes: u64,
    pub total_disk_bytes: u64,
    pub used_disk_bytes: u64,
    pub process_count: usize,
    pub uptime_seconds: u64,
    pub network_bytes_sent: u64,
    pub network_bytes_received: u64,
}

/// Process-specific metrics
#[derive(Debug, Clone)]
pub struct ProcessMetrics {
    pub pid: u32,
    pub name: String,
    pub cpu_usage_percent: f64,
    pub memory_bytes: u64,
    pub virtual_memory_bytes: u64,
    pub status: String,
    pub start_time: u64,
}

impl SystemSummary {
    /// Check if system is under stress
    pub fn is_under_stress(&self) -> bool {
        self.cpu_usage_percent > 80.0
            || self.memory_usage_percent > 85.0
            || self.disk_usage_percent > 90.0
            || self.load_average_1m > 4.0
    }

    /// Get health status based on resource usage
    pub fn get_health_status(&self) -> SystemHealthStatus {
        if self.cpu_usage_percent > 95.0
            || self.memory_usage_percent > 95.0
            || self.disk_usage_percent > 95.0
        {
            SystemHealthStatus::Critical
        } else if self.cpu_usage_percent > 85.0
            || self.memory_usage_percent > 85.0
            || self.disk_usage_percent > 90.0
        {
            SystemHealthStatus::Warning
        } else if self.cpu_usage_percent > 70.0
            || self.memory_usage_percent > 70.0
            || self.disk_usage_percent > 80.0
        {
            SystemHealthStatus::Degraded
        } else {
            SystemHealthStatus::Healthy
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SystemHealthStatus {
    Healthy,
    Degraded,
    Warning,
    Critical,
}

impl ProcessMetrics {
    /// Check if process is consuming excessive resources
    pub fn is_resource_intensive(&self) -> bool {
        self.cpu_usage_percent > 50.0 || self.memory_bytes > 2_000_000_000 // 2GB
    }
}

impl Default for SystemMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observability::metrics::SystemMetrics;

    #[tokio::test]
    async fn test_system_monitor_creation() {
        let mut monitor = SystemMonitor::new();

        // Should be able to collect metrics without error
        let result = monitor.collect_metrics().await;
        assert!(result.is_ok());

        let summary = result.unwrap();
        assert!(summary.cpu_usage_percent >= 0.0);
        assert!(summary.memory_usage_percent >= 0.0);
        assert!(summary.disk_usage_percent >= 0.0);
    }

    #[test]
    fn test_system_summary_health_status() {
        let healthy_summary = SystemSummary {
            cpu_usage_percent: 30.0,
            memory_usage_percent: 50.0,
            disk_usage_percent: 60.0,
            load_average_1m: 1.0,
            load_average_5m: 1.2,
            load_average_15m: 1.1,
            total_memory_bytes: 8_000_000_000,
            used_memory_bytes: 4_000_000_000,
            total_disk_bytes: 1_000_000_000_000,
            used_disk_bytes: 600_000_000_000,
            process_count: 150,
            uptime_seconds: 86400,
            network_bytes_sent: 1_000_000,
            network_bytes_received: 2_000_000,
        };

        assert_eq!(
            healthy_summary.get_health_status(),
            SystemHealthStatus::Healthy
        );
        assert!(!healthy_summary.is_under_stress());

        let critical_summary = SystemSummary {
            cpu_usage_percent: 98.0,
            memory_usage_percent: 97.0,
            disk_usage_percent: 96.0,
            load_average_1m: 8.0,
            load_average_5m: 7.5,
            load_average_15m: 7.0,
            total_memory_bytes: 8_000_000_000,
            used_memory_bytes: 7_800_000_000,
            total_disk_bytes: 1_000_000_000_000,
            used_disk_bytes: 960_000_000_000,
            process_count: 500,
            uptime_seconds: 86400,
            network_bytes_sent: 10_000_000,
            network_bytes_received: 20_000_000,
        };

        assert_eq!(
            critical_summary.get_health_status(),
            SystemHealthStatus::Critical
        );
        assert!(critical_summary.is_under_stress());
    }
}
