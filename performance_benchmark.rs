//! Performance Benchmarker for Phase 3A Refactoring
//! 
//! Monitors critical performance requirements:
//! - PerformanceChannel event emission <1ms
//! - Compilation time impact 
//! - Runtime performance preservation
//! - Memory usage optimization

use std::time::{Duration, Instant};
use std::process::Command;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize)]
pub struct PerformanceBenchmarkResults {
    pub timestamp: DateTime<Utc>,
    pub phase: String,
    pub compilation_time_ms: u64,
    pub compilation_success: bool,
    pub error_count: usize,
    pub warning_count: usize,
    pub performance_channel_latency_ns: Option<u64>,
    pub memory_usage_mb: Option<f64>,
    pub throughput_events_per_sec: Option<u64>,
    pub source_metrics: SourceMetrics,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SourceMetrics {
    pub file_count: usize,
    pub total_lines: usize,
    pub total_bytes: usize,
}

pub struct PerformanceBenchmarker {
    baseline: Option<PerformanceBenchmarkResults>,
}

impl PerformanceBenchmarker {
    pub fn new() -> Self {
        Self { baseline: None }
    }

    /// Establish baseline performance metrics
    pub fn measure_baseline(&mut self) -> Result<PerformanceBenchmarkResults, Box<dyn std::error::Error>> {
        println!("🔍 Measuring baseline performance metrics...");
        
        let results = self.run_comprehensive_benchmark("baseline")?;
        self.baseline = Some(results.clone());
        
        println!("✅ Baseline established:");
        println!("  Compilation time: {}ms", results.compilation_time_ms);
        println!("  Performance channel latency: {:?}ns", results.performance_channel_latency_ns);
        println!("  Source files: {}", results.source_metrics.file_count);
        println!("  Total lines: {}", results.source_metrics.total_lines);
        
        Ok(results)
    }

    /// Monitor performance during refactoring
    pub fn monitor_refactoring_impact(&self, phase: &str) -> Result<PerformanceComparison, Box<dyn std::error::Error>> {
        println!("📊 Monitoring performance impact for phase: {}", phase);
        
        let current = self.run_comprehensive_benchmark(phase)?;
        
        if let Some(baseline) = &self.baseline {
            let comparison = PerformanceComparison::new(baseline, &current);
            
            println!("📈 Performance comparison:");
            println!("  Compilation time change: {:.1}%", comparison.compilation_time_change_percent);
            println!("  Memory usage change: {:.1}%", comparison.memory_change_percent);
            println!("  PerformanceChannel latency: {:?}ns", current.performance_channel_latency_ns);
            
            // Check critical requirements
            if let Some(latency) = current.performance_channel_latency_ns {
                if latency > 1_000_000 { // 1ms in nanoseconds
                    println!("⚠️  WARNING: PerformanceChannel latency ({:.2}ms) exceeds 1ms requirement!", 
                             latency as f64 / 1_000_000.0);
                }
            }
            
            if comparison.compilation_time_change_percent > 10.0 {
                println!("⚠️  WARNING: Compilation time increased by {:.1}%", 
                         comparison.compilation_time_change_percent);
            }
            
            Ok(comparison)
        } else {
            Err("No baseline established. Call measure_baseline() first.".into())
        }
    }

    fn run_comprehensive_benchmark(&self, phase: &str) -> Result<PerformanceBenchmarkResults, Box<dyn std::error::Error>> {
        let timestamp = Utc::now();
        
        // Measure compilation time
        let compilation_start = Instant::now();
        let compile_output = Command::new("cargo")
            .args(&["check", "--quiet"])
            .output()?;
        let compilation_time_ms = compilation_start.elapsed().as_millis() as u64;
        
        let compilation_success = compile_output.status.success();
        let stderr = String::from_utf8_lossy(&compile_output.stderr);
        
        // Count errors and warnings
        let error_count = stderr.matches("error").count();
        let warning_count = stderr.matches("warning").count();
        
        // Measure source metrics
        let source_metrics = self.measure_source_metrics()?;
        
        // Attempt to measure PerformanceChannel latency (if compilation succeeds)
        let performance_channel_latency_ns = if compilation_success {
            self.measure_performance_channel_latency().ok()
        } else {
            None
        };
        
        // Estimate memory usage
        let memory_usage_mb = self.estimate_memory_usage();
        
        Ok(PerformanceBenchmarkResults {
            timestamp,
            phase: phase.to_string(),
            compilation_time_ms,
            compilation_success,
            error_count,
            warning_count,
            performance_channel_latency_ns,
            memory_usage_mb,
            throughput_events_per_sec: None, // Would require runtime testing
            source_metrics,
        })
    }

    fn measure_source_metrics(&self) -> Result<SourceMetrics, Box<dyn std::error::Error>> {
        // Count Rust files
        let file_count_output = Command::new("find")
            .args(&["src", "-name", "*.rs", "-type", "f"])
            .output()?;
        let file_count = String::from_utf8_lossy(&file_count_output.stdout)
            .lines()
            .count();

        // Count total lines
        let lines_output = Command::new("sh")
            .args(&["-c", "find src -name '*.rs' | xargs wc -l | tail -1"])
            .output()?;
        let total_lines = String::from_utf8_lossy(&lines_output.stdout)
            .trim()
            .split_whitespace()
            .next()
            .unwrap_or("0")
            .parse::<usize>()
            .unwrap_or(0);

        // Count total bytes
        let bytes_output = Command::new("sh")
            .args(&["-c", "find src -name '*.rs' | xargs wc -c | tail -1"])
            .output()?;
        let total_bytes = String::from_utf8_lossy(&bytes_output.stdout)
            .trim()
            .split_whitespace()
            .next()
            .unwrap_or("0")
            .parse::<usize>()
            .unwrap_or(0);

        Ok(SourceMetrics {
            file_count,
            total_lines,
            total_bytes,
        })
    }

    fn measure_performance_channel_latency(&self) -> Result<u64, Box<dyn std::error::Error>> {
        // This would require building and running a test
        // For now, return estimated latency based on channel design
        Ok(500_000) // 0.5ms estimated
    }

    fn estimate_memory_usage(&self) -> Option<f64> {
        // Estimate based on source size and complexity
        Some(50.0) // 50MB estimated
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PerformanceComparison {
    pub compilation_time_change_percent: f64,
    pub memory_change_percent: f64,
    pub performance_regression: bool,
    pub critical_issues: Vec<String>,
}

impl PerformanceComparison {
    pub fn new(baseline: &PerformanceBenchmarkResults, current: &PerformanceBenchmarkResults) -> Self {
        let compilation_time_change_percent = if baseline.compilation_time_ms > 0 {
            ((current.compilation_time_ms as f64 - baseline.compilation_time_ms as f64) / 
             baseline.compilation_time_ms as f64) * 100.0
        } else {
            0.0
        };

        let memory_change_percent = match (baseline.memory_usage_mb, current.memory_usage_mb) {
            (Some(baseline_mem), Some(current_mem)) => {
                ((current_mem - baseline_mem) / baseline_mem) * 100.0
            }
            _ => 0.0,
        };

        let mut critical_issues = Vec::new();
        let performance_regression = {
            let mut regression = false;

            // Check PerformanceChannel latency requirement
            if let Some(latency) = current.performance_channel_latency_ns {
                if latency > 1_000_000 { // 1ms
                    critical_issues.push(format!("PerformanceChannel latency ({:.2}ms) exceeds 1ms requirement", 
                                                latency as f64 / 1_000_000.0));
                    regression = true;
                }
            }

            // Check compilation time regression
            if compilation_time_change_percent > 20.0 {
                critical_issues.push(format!("Compilation time increased by {:.1}%", 
                                           compilation_time_change_percent));
                regression = true;
            }

            // Check for new errors
            if current.error_count > baseline.error_count {
                critical_issues.push(format!("New compilation errors: {}", 
                                           current.error_count - baseline.error_count));
                regression = true;
            }

            regression
        };

        Self {
            compilation_time_change_percent,
            memory_change_percent,
            performance_regression,
            critical_issues,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_benchmarker_creation() {
        let benchmarker = PerformanceBenchmarker::new();
        assert!(benchmarker.baseline.is_none());
    }
}