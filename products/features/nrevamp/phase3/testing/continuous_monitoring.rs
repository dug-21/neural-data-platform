//! Phase 3 Continuous Monitoring System
//! 
//! Implements continuous validation that runs hourly to ensure Phase 3 
//! extensions don't introduce regression in DAA autonomous trading capabilities.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, Mutex};
use tokio::time::{interval, sleep};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use crate::daa::coordinator::DAACoordinator;
use crate::daa::autonomous_training::AutonomousTrainingEngine;
use crate::neural::vendor_predictor::VendorPredictor;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinuousMonitoringResult {
    pub timestamp: DateTime<Utc>,
    pub test_suite: String,
    pub passed: bool,
    pub metrics: MonitoringMetrics,
    pub violations: Vec<ThresholdViolation>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringMetrics {
    pub memory_usage_mb: f64,
    pub prediction_latency_ms: u64,
    pub accuracy: f64,
    pub neural_voting_weight: f64,
    pub strategy_voting_weight: f64,
    pub consensus_percentage: f64,
    pub consecutive_failures: u32,
    pub error_rate: f64,
    pub system_health_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdViolation {
    pub metric: String,
    pub current_value: f64,
    pub threshold: f64,
    pub severity: ViolationSeverity,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ViolationSeverity {
    Warning,
    Critical,
    Blocker,
}

pub struct ContinuousMonitor {
    coordinator: Arc<RwLock<DAACoordinator>>,
    training_engine: Arc<RwLock<AutonomousTrainingEngine>>,
    predictor: Arc<RwLock<VendorPredictor>>,
    results_history: Arc<Mutex<VecDeque<ContinuousMonitoringResult>>>,
    alerting_enabled: bool,
    monitoring_interval: Duration,
}

impl ContinuousMonitor {
    pub fn new(
        coordinator: Arc<RwLock<DAACoordinator>>,
        training_engine: Arc<RwLock<AutonomousTrainingEngine>>,
        predictor: Arc<RwLock<VendorPredictor>>,
    ) -> Self {
        Self {
            coordinator,
            training_engine,
            predictor,
            results_history: Arc::new(Mutex::new(VecDeque::new())),
            alerting_enabled: true,
            monitoring_interval: Duration::from_secs(3600), // 1 hour
        }
    }

    /// Start continuous monitoring - runs forever
    pub async fn start_monitoring(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("Starting Phase 3 continuous monitoring system...");
        
        let mut interval_timer = interval(self.monitoring_interval);
        
        loop {
            interval_timer.tick().await;
            
            println!("Running hourly Phase 3 validation checks...");
            
            // Run all monitoring tests
            let results = self.run_all_monitoring_tests().await?;
            
            // Store results
            self.store_results(results.clone()).await;
            
            // Check for violations and alert if necessary
            if self.alerting_enabled {
                self.process_violations(&results).await;
            }
            
            // Log summary
            self.log_monitoring_summary(&results).await;
            
            // Cleanup old results (keep last 168 hours = 1 week)
            self.cleanup_old_results(168).await;
        }
    }

    /// Run all monitoring test suites
    async fn run_all_monitoring_tests(&self) -> Result<Vec<ContinuousMonitoringResult>, Box<dyn std::error::Error + Send + Sync>> {
        let mut results = Vec::new();
        
        // Test Suite 1: DAA Preservation Monitoring
        results.push(self.monitor_daa_preservation().await?);
        
        // Test Suite 2: Performance Thresholds Monitoring
        results.push(self.monitor_performance_thresholds().await?);
        
        // Test Suite 3: Memory Usage Monitoring
        results.push(self.monitor_memory_usage().await?);
        
        // Test Suite 4: Latency Monitoring
        results.push(self.monitor_prediction_latency().await?);
        
        // Test Suite 5: System Health Monitoring
        results.push(self.monitor_system_health().await?);
        
        Ok(results)
    }

    /// Monitor DAA autonomous trading preservation
    async fn monitor_daa_preservation(&self) -> Result<ContinuousMonitoringResult, Box<dyn std::error::Error + Send + Sync>> {
        let start_time = Instant::now();
        let mut violations = Vec::new();
        let mut passed = true;

        // Test 1: Verify 60/40 voting weights preserved
        let coordinator = self.coordinator.read().await;
        let test_context = create_monitoring_market_context();
        
        let decision = coordinator.make_autonomous_decision(&test_context).await?;
        
        // Check neural voting weight
        if (decision.voting_weights.neural_weight - 0.6).abs() > f64::EPSILON {
            violations.push(ThresholdViolation {
                metric: "neural_voting_weight".to_string(),
                current_value: decision.voting_weights.neural_weight,
                threshold: 0.6,
                severity: ViolationSeverity::Blocker,
                description: "Neural voting weight must be exactly 60%".to_string(),
            });
            passed = false;
        }
        
        // Check strategy voting weight
        if (decision.voting_weights.strategy_weight - 0.4).abs() > f64::EPSILON {
            violations.push(ThresholdViolation {
                metric: "strategy_voting_weight".to_string(),
                current_value: decision.voting_weights.strategy_weight,
                threshold: 0.4,
                severity: ViolationSeverity::Blocker,
                description: "Strategy voting weight must be exactly 40%".to_string(),
            });
            passed = false;
        }
        
        // Check Byzantine consensus threshold
        if decision.consensus_threshold != 0.7 {
            violations.push(ThresholdViolation {
                metric: "consensus_threshold".to_string(),
                current_value: decision.consensus_threshold,
                threshold: 0.7,
                severity: ViolationSeverity::Blocker,
                description: "Byzantine consensus threshold must be exactly 70%".to_string(),
            });
            passed = false;
        }

        let metrics = MonitoringMetrics {
            memory_usage_mb: self.get_current_memory_usage_mb().await,
            prediction_latency_ms: start_time.elapsed().as_millis() as u64,
            accuracy: decision.accuracy_estimate,
            neural_voting_weight: decision.voting_weights.neural_weight,
            strategy_voting_weight: decision.voting_weights.strategy_weight,
            consensus_percentage: decision.consensus_percentage,
            consecutive_failures: 0, // Will be updated in performance monitoring
            error_rate: 0.0,         // Will be updated in performance monitoring
            system_health_score: if passed { 1.0 } else { 0.0 },
        };
        
        let recommendations = if !passed {
            vec![
                "CRITICAL: DAA voting structure has been compromised".to_string(),
                "Immediately revert to last known good configuration".to_string(),
                "Phase 3 extensions may have introduced breaking changes".to_string(),
            ]
        } else {
            vec!["DAA autonomous trading structure preserved correctly".to_string()]
        };

        Ok(ContinuousMonitoringResult {
            timestamp: Utc::now(),
            test_suite: "daa_preservation".to_string(),
            passed,
            metrics,
            violations,
            recommendations,
        })
    }

    /// Monitor performance thresholds (accuracy, error rate, failures)
    async fn monitor_performance_thresholds(&self) -> Result<ContinuousMonitoringResult, Box<dyn std::error::Error + Send + Sync>> {
        let mut violations = Vec::new();
        let mut passed = true;

        let training_engine = self.training_engine.read().await;
        let performance_snapshot = training_engine.get_current_performance_snapshot().await?;
        
        // Check accuracy threshold (≥0.8)
        if performance_snapshot.accuracy < 0.8 {
            violations.push(ThresholdViolation {
                metric: "accuracy".to_string(),
                current_value: performance_snapshot.accuracy,
                threshold: 0.8,
                severity: ViolationSeverity::Critical,
                description: "Model accuracy below required 80% threshold".to_string(),
            });
            passed = false;
        }
        
        // Check error rate threshold (≤0.1)
        if performance_snapshot.error_rate > 0.1 {
            violations.push(ThresholdViolation {
                metric: "error_rate".to_string(),  
                current_value: performance_snapshot.error_rate,
                threshold: 0.1,
                severity: ViolationSeverity::Critical,
                description: "Model error rate above maximum 10% threshold".to_string(),
            });
            passed = false;
        }
        
        // Check consecutive failures threshold (≤5)
        if performance_snapshot.consecutive_failures > 5 {
            violations.push(ThresholdViolation {
                metric: "consecutive_failures".to_string(),
                current_value: performance_snapshot.consecutive_failures as f64,
                threshold: 5.0,
                severity: ViolationSeverity::Critical,
                description: "Too many consecutive failures - model may need retraining".to_string(),
            });
            passed = false;
        }

        let metrics = MonitoringMetrics {
            memory_usage_mb: self.get_current_memory_usage_mb().await,
            prediction_latency_ms: 0, // Not measured in this test
            accuracy: performance_snapshot.accuracy,
            neural_voting_weight: 0.6, // Assumed from DAA preservation
            strategy_voting_weight: 0.4,
            consensus_percentage: 0.7,
            consecutive_failures: performance_snapshot.consecutive_failures,
            error_rate: performance_snapshot.error_rate,
            system_health_score: if passed { 1.0 } else { 0.5 },
        };
        
        let recommendations = if !passed {
            let mut recs = Vec::new();
            if performance_snapshot.accuracy < 0.8 {
                recs.push("Consider triggering model retraining due to low accuracy".to_string());
            }
            if performance_snapshot.error_rate > 0.1 {
                recs.push("Investigate data quality issues causing high error rate".to_string());
            }
            if performance_snapshot.consecutive_failures > 5 {
                recs.push("Consider model rollback due to consecutive failures".to_string());
            }
            recs
        } else {
            vec!["All performance thresholds within acceptable ranges".to_string()]
        };

        Ok(ContinuousMonitoringResult {
            timestamp: Utc::now(),
            test_suite: "performance_thresholds".to_string(),
            passed,
            metrics,
            violations,
            recommendations,
        })
    }

    /// Monitor memory usage (<525MB limit)
    async fn monitor_memory_usage(&self) -> Result<ContinuousMonitoringResult, Box<dyn std::error::Error + Send + Sync>> {
        let mut violations = Vec::new();
        let mut passed = true;

        let current_memory_mb = self.get_current_memory_usage_mb().await;
        
        // Check memory limit (525MB)
        if current_memory_mb > 525.0 {
            violations.push(ThresholdViolation {
                metric: "memory_usage_mb".to_string(),
                current_value: current_memory_mb,
                threshold: 525.0,
                severity: ViolationSeverity::Blocker,
                description: "System memory usage exceeds 525MB limit".to_string(),
            });
            passed = false;
        }
        
        // Warning at 90% of limit (472.5MB)
        if current_memory_mb > 472.5 {
            violations.push(ThresholdViolation {
                metric: "memory_usage_mb".to_string(),
                current_value: current_memory_mb,
                threshold: 472.5,
                severity: ViolationSeverity::Warning,
                description: "System memory usage approaching 525MB limit".to_string(),
            });
        }

        // Check for memory leaks by comparing with recent history
        let memory_leak_detected = self.detect_memory_leak(current_memory_mb).await;
        if memory_leak_detected {
            violations.push(ThresholdViolation {
                metric: "memory_leak".to_string(),
                current_value: current_memory_mb,
                threshold: 0.0, // No acceptable leak rate
                severity: ViolationSeverity::Critical,
                description: "Potential memory leak detected - usage trending upward".to_string(),
            });
            passed = false;
        }

        let metrics = MonitoringMetrics {
            memory_usage_mb: current_memory_mb,
            prediction_latency_ms: 0,
            accuracy: 0.0,
            neural_voting_weight: 0.6,
            strategy_voting_weight: 0.4,
            consensus_percentage: 0.7,
            consecutive_failures: 0,
            error_rate: 0.0,
            system_health_score: if passed { 1.0 } else { 0.3 },
        };
        
        let recommendations = if !passed {
            let mut recs = vec![
                "Monitor memory usage closely for continued growth".to_string(),
                "Consider garbage collection or memory optimization".to_string(),
            ];
            if current_memory_mb > 525.0 {
                recs.push("CRITICAL: Immediate memory reduction required".to_string());
                recs.push("Consider disabling non-essential Phase 3 features".to_string());
            }
            recs
        } else {
            vec!["Memory usage within acceptable limits".to_string()]
        };

        Ok(ContinuousMonitoringResult {
            timestamp: Utc::now(),
            test_suite: "memory_usage".to_string(),
            passed,
            metrics,
            violations,
            recommendations,
        })
    }

    /// Monitor prediction latency (<100ms target)
    async fn monitor_prediction_latency(&self) -> Result<ContinuousMonitoringResult, Box<dyn std::error::Error + Send + Sync>> {
        let mut violations = Vec::new();
        let mut passed = true;
        let mut latency_samples = Vec::new();

        let predictor = self.predictor.read().await;
        
        // Test prediction latency across multiple samples
        for _ in 0..10 {
            let features = create_monitoring_features();
            
            let start_time = Instant::now();
            let _prediction = predictor.predict("AAPL", &features).await?;
            let latency = start_time.elapsed();
            
            latency_samples.push(latency.as_millis() as u64);
        }
        
        let avg_latency = latency_samples.iter().sum::<u64>() / latency_samples.len() as u64;
        let max_latency = *latency_samples.iter().max().unwrap();
        
        // Check average latency (≤100ms)
        if avg_latency > 100 {
            violations.push(ThresholdViolation {
                metric: "avg_prediction_latency_ms".to_string(),
                current_value: avg_latency as f64,
                threshold: 100.0,
                severity: ViolationSeverity::Critical,
                description: "Average prediction latency exceeds 100ms target".to_string(),
            });
            passed = false;
        }
        
        // Check maximum latency (≤200ms)
        if max_latency > 200 {
            violations.push(ThresholdViolation {
                metric: "max_prediction_latency_ms".to_string(),
                current_value: max_latency as f64,
                threshold: 200.0,
                severity: ViolationSeverity::Critical,
                description: "Maximum prediction latency exceeds 200ms limit".to_string(),
            });
            passed = false;
        }

        let metrics = MonitoringMetrics {
            memory_usage_mb: self.get_current_memory_usage_mb().await,
            prediction_latency_ms: avg_latency,
            accuracy: 0.0,
            neural_voting_weight: 0.6,
            strategy_voting_weight: 0.4,
            consensus_percentage: 0.7,
            consecutive_failures: 0,
            error_rate: 0.0,
            system_health_score: if passed { 1.0 } else { 0.6 },
        };
        
        let recommendations = if !passed {
            vec![
                "Consider optimizing prediction pipeline for better performance".to_string(),
                "Review Phase 3 extensions that may be impacting latency".to_string(),
                "Monitor system load and resource utilization".to_string(),
            ]
        } else {
            vec!["Prediction latency within acceptable limits".to_string()]
        };

        Ok(ContinuousMonitoringResult {
            timestamp: Utc::now(),
            test_suite: "prediction_latency".to_string(),
            passed,
            metrics,
            violations,
            recommendations,
        })
    }

    /// Monitor overall system health
    async fn monitor_system_health(&self) -> Result<ContinuousMonitoringResult, Box<dyn std::error::Error + Send + Sync>> {
        let mut violations = Vec::new();
        let mut passed = true;
        let mut health_score = 1.0;

        // Test system responsiveness
        let coordinator = self.coordinator.read().await;
        let training_engine = self.training_engine.read().await;
        let predictor = self.predictor.read().await;
        
        // Test 1: Coordinator responsiveness
        let coordinator_start = Instant::now();
        let test_context = create_monitoring_market_context();
        match coordinator.make_autonomous_decision(&test_context).await {
            Ok(_) => {
                let coordinator_latency = coordinator_start.elapsed();
                if coordinator_latency > Duration::from_millis(500) {
                    health_score -= 0.2;
                    violations.push(ThresholdViolation {
                        metric: "coordinator_responsiveness_ms".to_string(),
                        current_value: coordinator_latency.as_millis() as f64,
                        threshold: 500.0,
                        severity: ViolationSeverity::Warning,
                        description: "DAA coordinator responding slowly".to_string(),
                    });
                }
            }
            Err(_) => {
                health_score -= 0.4;
                passed = false;
                violations.push(ThresholdViolation {
                    metric: "coordinator_availability".to_string(),
                    current_value: 0.0,
                    threshold: 1.0,
                    severity: ViolationSeverity::Blocker,
                    description: "DAA coordinator not responding".to_string(),
                });
            }
        }
        
        // Test 2: Training engine responsiveness
        let training_start = Instant::now();
        match training_engine.get_current_performance_snapshot().await {
            Ok(_) => {
                let training_latency = training_start.elapsed();
                if training_latency > Duration::from_millis(1000) {
                    health_score -= 0.1;
                    violations.push(ThresholdViolation {
                        metric: "training_engine_responsiveness_ms".to_string(),
                        current_value: training_latency.as_millis() as f64,
                        threshold: 1000.0,
                        severity: ViolationSeverity::Warning,
                        description: "Training engine responding slowly".to_string(),
                    });
                }
            }
            Err(_) => {
                health_score -= 0.3;
                passed = false;
                violations.push(ThresholdViolation {
                    metric: "training_engine_availability".to_string(),
                    current_value: 0.0,
                    threshold: 1.0,
                    severity: ViolationSeverity::Critical,
                    description: "Training engine not responding".to_string(),
                });
            }
        }
        
        // Test 3: Predictor responsiveness
        let predictor_start = Instant::now();
        let test_features = create_monitoring_features();
        match predictor.predict("AAPL", &test_features).await {
            Ok(_) => {
                let predictor_latency = predictor_start.elapsed();
                if predictor_latency > Duration::from_millis(100) {
                    health_score -= 0.1;
                    violations.push(ThresholdViolation {
                        metric: "predictor_responsiveness_ms".to_string(),
                        current_value: predictor_latency.as_millis() as f64,
                        threshold: 100.0,
                        severity: ViolationSeverity::Warning,
                        description: "Neural predictor responding slowly".to_string(),
                    });
                }
            }
            Err(_) => {
                health_score -= 0.3;
                passed = false;
                violations.push(ThresholdViolation {
                    metric: "predictor_availability".to_string(),
                    current_value: 0.0,
                    threshold: 1.0,
                    severity: ViolationSeverity::Critical,
                    description: "Neural predictor not responding".to_string(),
                });
            }
        }

        if health_score < 0.8 {
            passed = false;
        }

        let metrics = MonitoringMetrics {
            memory_usage_mb: self.get_current_memory_usage_mb().await,
            prediction_latency_ms: 0,
            accuracy: 0.0,
            neural_voting_weight: 0.6,
            strategy_voting_weight: 0.4,
            consensus_percentage: 0.7,
            consecutive_failures: 0,
            error_rate: 0.0,
            system_health_score: health_score,
        };
        
        let recommendations = if !passed {
            vec![
                "System health degraded - investigate component responsiveness".to_string(),
                "Check system resources and load".to_string(),
                "Consider restarting unresponsive components".to_string(),
            ]
        } else {
            vec!["System health excellent - all components responsive".to_string()]
        };

        Ok(ContinuousMonitoringResult {
            timestamp: Utc::now(),
            test_suite: "system_health".to_string(),
            passed,
            metrics,
            violations,
            recommendations,
        })
    }

    /// Store monitoring results in history
    async fn store_results(&self, results: Vec<ContinuousMonitoringResult>) {
        let mut history = self.results_history.lock().await;
        
        for result in results {
            history.push_back(result);
        }
    }

    /// Process violations and send alerts if necessary
    async fn process_violations(&self, results: &[ContinuousMonitoringResult]) {
        for result in results {
            if !result.passed {
                for violation in &result.violations {
                    match violation.severity {
                        ViolationSeverity::Blocker => {
                            self.send_critical_alert(&result.test_suite, violation).await;
                        }
                        ViolationSeverity::Critical => {
                            self.send_warning_alert(&result.test_suite, violation).await;
                        }
                        ViolationSeverity::Warning => {
                            println!("WARNING: {} - {}", violation.metric, violation.description);
                        }
                    }
                }
            }
        }
    }

    /// Send critical alert for blocker violations
    async fn send_critical_alert(&self, test_suite: &str, violation: &ThresholdViolation) {
        let alert_message = format!(
            "🚨 CRITICAL ALERT: Phase 3 Validation Failure\n\
             Test Suite: {}\n\
             Metric: {}\n\
             Current Value: {}\n\
             Threshold: {}\n\
             Description: {}\n\
             Action Required: Immediate investigation",
            test_suite, violation.metric, violation.current_value, 
            violation.threshold, violation.description
        );
        
        // In a real system, this would send alerts via email, Slack, PagerDuty, etc.
        eprintln!("{}", alert_message);
        
        // Log to monitoring system
        self.log_alert("CRITICAL", &alert_message).await;
    }

    /// Send warning alert for critical violations
    async fn send_warning_alert(&self, test_suite: &str, violation: &ThresholdViolation) {
        let alert_message = format!(
            "⚠️  WARNING: Phase 3 Performance Degradation\n\
             Test Suite: {}\n\
             Metric: {}\n\
             Current Value: {}\n\
             Threshold: {}\n\
             Description: {}",
            test_suite, violation.metric, violation.current_value,
            violation.threshold, violation.description
        );
        
        println!("{}", alert_message);
        self.log_alert("WARNING", &alert_message).await;
    }

    /// Log monitoring summary
    async fn log_monitoring_summary(&self, results: &[ContinuousMonitoringResult]) {
        let total_tests = results.len();
        let passed_tests = results.iter().filter(|r| r.passed).count();
        let failed_tests = total_tests - passed_tests;
        
        let total_violations = results.iter()
            .map(|r| r.violations.len())
            .sum::<usize>();
        
        println!("\n📊 Phase 3 Continuous Monitoring Summary");
        println!("  Timestamp: {}", Utc::now().format("%Y-%m-%d %H:%M:%S UTC"));
        println!("  Total Tests: {}", total_tests);
        println!("  Passed: {} ✅", passed_tests);
        println!("  Failed: {} ❌", failed_tests);
        println!("  Total Violations: {}", total_violations);
        
        for result in results {
            let status = if result.passed { "✅ PASS" } else { "❌ FAIL" };
            println!("    {}: {} ({} violations)", result.test_suite, status, result.violations.len());
        }
        
        if failed_tests > 0 {
            println!("\n🔍 Violations Summary:");
            for result in results.iter().filter(|r| !r.passed) {
                for violation in &result.violations {
                    let severity_emoji = match violation.severity {
                        ViolationSeverity::Blocker => "🚫",
                        ViolationSeverity::Critical => "⚠️",
                        ViolationSeverity::Warning => "⚡",
                    };
                    println!("    {} {}: {} (current: {}, threshold: {})", 
                            severity_emoji, violation.metric, violation.description,
                            violation.current_value, violation.threshold);
                }
            }
        }
        
        println!("");
    }

    /// Cleanup old monitoring results
    async fn cleanup_old_results(&self, max_hours: usize) {
        let mut history = self.results_history.lock().await;
        let cutoff_time = Utc::now() - chrono::Duration::hours(max_hours as i64);
        
        while let Some(front) = history.front() {
            if front.timestamp < cutoff_time {
                history.pop_front();
            } else {
                break;
            }
        }
    }

    /// Detect memory leaks by analyzing trend
    async fn detect_memory_leak(&self, current_memory_mb: f64) -> bool {
        let history = self.results_history.lock().await;
        
        // Get last 24 hours of memory usage data
        let recent_results: Vec<&ContinuousMonitoringResult> = history
            .iter()
            .filter(|r| r.test_suite == "memory_usage" && 
                   r.timestamp > Utc::now() - chrono::Duration::hours(24))
            .collect();
        
        if recent_results.len() < 5 {
            return false; // Not enough data
        }
        
        // Calculate memory usage trend
        let memory_values: Vec<f64> = recent_results
            .iter()
            .map(|r| r.metrics.memory_usage_mb)
            .collect();
        
        // Simple linear regression to detect upward trend
        let trend_slope = calculate_trend_slope(&memory_values);
        
        // If memory is growing by more than 1MB per hour, consider it a leak
        trend_slope > 1.0
    }

    /// Get current system memory usage in MB
    async fn get_current_memory_usage_mb(&self) -> f64 {
        // In a real implementation, this would query actual system memory usage
        // For now, simulate with realistic values
        400.0 // Simulated current usage
    }

    /// Log alert to monitoring system
    async fn log_alert(&self, level: &str, message: &str) {
        // In a real system, this would log to a centralized monitoring system
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
        println!("[{}] {}: {}", timestamp, level, message);
    }
}

// Helper functions
fn create_monitoring_market_context() -> MarketContext {
    MarketContext {
        symbol: "AAPL".to_string(),
        timestamp: Utc::now(),
        price: 150.0,
        volume: 1000000.0,
        volatility: 0.2,
        ..Default::default()
    }
}

fn create_monitoring_features() -> Features {
    Features {
        symbol: "AAPL".to_string(),
        price_features: vec![150.0, 149.5, 151.0],
        volume_features: vec![1000000.0],
        technical_indicators: vec![0.5, 0.3, 0.8],
        ..Default::default()
    }
}

fn calculate_trend_slope(values: &[f64]) -> f64 {
    let n = values.len() as f64;
    let x_sum: f64 = (0..values.len()).map(|i| i as f64).sum();
    let y_sum: f64 = values.iter().sum();
    let xy_sum: f64 = values.iter().enumerate()
        .map(|(i, &y)| i as f64 * y)
        .sum();
    let x_squared_sum: f64 = (0..values.len())
        .map(|i| (i as f64).powi(2))
        .sum();
    
    // Linear regression slope formula
    (n * xy_sum - x_sum * y_sum) / (n * x_squared_sum - x_sum * x_sum)
}

#[cfg(test)]
mod continuous_monitoring_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_daa_preservation_monitoring() {
        // Test the continuous monitoring system itself
        let coordinator = Arc::new(RwLock::new(DAACoordinator::new()));
        let training_engine = Arc::new(RwLock::new(AutonomousTrainingEngine::new()));
        let predictor = Arc::new(RwLock::new(VendorPredictor::new()));
        
        let monitor = ContinuousMonitor::new(coordinator, training_engine, predictor);
        
        let result = monitor.monitor_daa_preservation().await
            .expect("DAA preservation monitoring should work");
        
        assert!(result.passed, "DAA preservation should pass in healthy system");
        assert_eq!(result.metrics.neural_voting_weight, 0.6);
        assert_eq!(result.metrics.strategy_voting_weight, 0.4);
        assert_eq!(result.metrics.consensus_percentage, 0.7);
    }
    
    #[test]
    fn test_trend_slope_calculation() {
        // Test upward trend
        let upward_values = vec![100.0, 102.0, 104.0, 106.0, 108.0];
        let upward_slope = calculate_trend_slope(&upward_values);
        assert!(upward_slope > 1.0, "Should detect upward trend");
        
        // Test stable values
        let stable_values = vec![100.0, 100.1, 99.9, 100.0, 100.2];
        let stable_slope = calculate_trend_slope(&stable_values);
        assert!(stable_slope.abs() < 0.1, "Should detect stable trend");
    }
}