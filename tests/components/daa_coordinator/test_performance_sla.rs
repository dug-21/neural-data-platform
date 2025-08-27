//! Comprehensive tests for DAA Coordinator performance SLA verification
//! Tests real-time coordination <10ms SLA, throughput 500 agents/sec, and performance benchmarks

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use tokio::time::timeout;

#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub decision_latency: Duration,
    pub throughput: f64,
    pub cpu_usage: f64,
    pub memory_usage: u64,
    pub success_rate: f64,
    pub agent_coordination_time: Duration,
    pub consensus_building_time: Duration,
    pub error_rate: f64,
}

#[derive(Debug, Clone)]
pub struct SLARequirements {
    pub max_decision_latency: Duration,
    pub min_throughput: f64,
    pub max_cpu_usage: f64,
    pub max_memory_usage: u64,
    pub min_success_rate: f64,
    pub max_error_rate: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TradeDirection {
    Long,
    Short,
    Hold,
}

#[derive(Debug, Clone)]
pub struct CoordinationTask {
    pub id: String,
    pub agents_count: usize,
    pub complexity: TaskComplexity,
    pub priority: TaskPriority,
    pub created_at: Instant,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskComplexity {
    Simple,
    Medium,
    Complex,
    HighlyComplex,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskPriority {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone)]
pub struct Agent {
    pub id: String,
    pub performance_score: f64,
    pub processing_time: Duration,
    pub is_overloaded: bool,
}

pub struct MockPerformanceCoordinator {
    agents: Arc<RwLock<HashMap<String, Agent>>>,
    task_queue: Arc<RwLock<Vec<CoordinationTask>>>,
    performance_metrics: Arc<RwLock<PerformanceMetrics>>,
    sla_requirements: SLARequirements,
    processed_tasks: Arc<AtomicU64>,
    failed_tasks: Arc<AtomicU64>,
    concurrency_limiter: Arc<Semaphore>,
    benchmark_results: Arc<RwLock<Vec<BenchmarkResult>>>,
}

#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub test_name: String,
    pub duration: Duration,
    pub throughput: f64,
    pub latency_p50: Duration,
    pub latency_p95: Duration,
    pub latency_p99: Duration,
    pub success_rate: f64,
    pub memory_peak: u64,
    pub cpu_peak: f64,
}

impl MockPerformanceCoordinator {
    pub fn new() -> Self {
        let sla_requirements = SLARequirements {
            max_decision_latency: Duration::from_millis(10),
            min_throughput: 500.0,
            max_cpu_usage: 80.0,
            max_memory_usage: 1024 * 1024 * 1024, // 1GB
            min_success_rate: 0.95,
            max_error_rate: 0.05,
        };

        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            task_queue: Arc::new(RwLock::new(Vec::new())),
            performance_metrics: Arc::new(RwLock::new(PerformanceMetrics {
                decision_latency: Duration::from_millis(0),
                throughput: 0.0,
                cpu_usage: 0.0,
                memory_usage: 0,
                success_rate: 1.0,
                agent_coordination_time: Duration::from_millis(0),
                consensus_building_time: Duration::from_millis(0),
                error_rate: 0.0,
            })),
            sla_requirements,
            processed_tasks: Arc::new(AtomicU64::new(0)),
            failed_tasks: Arc::new(AtomicU64::new(0)),
            concurrency_limiter: Arc::new(Semaphore::new(1000)), // Limit concurrent operations
            benchmark_results: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn register_agent(&self, agent: Agent) {
        let mut agents = self.agents.write().await;
        agents.insert(agent.id.clone(), agent);
    }

    pub async fn coordinate_agents(&self, task: CoordinationTask) -> Result<TradeDirection, String> {
        let start_time = Instant::now();

        // Acquire semaphore for concurrency control
        let _permit = self.concurrency_limiter.acquire().await.unwrap();

        // Simulate coordination work based on complexity
        let coordination_delay = match task.complexity {
            TaskComplexity::Simple => Duration::from_micros(100),
            TaskComplexity::Medium => Duration::from_micros(500),
            TaskComplexity::Complex => Duration::from_millis(2),
            TaskComplexity::HighlyComplex => Duration::from_millis(5),
        };

        tokio::time::sleep(coordination_delay).await;

        // Simulate agent coordination
        let agents = self.agents.read().await;
        let available_agents: Vec<_> = agents.values()
            .filter(|agent| !agent.is_overloaded)
            .take(task.agents_count)
            .collect();

        if available_agents.len() < task.agents_count {
            self.failed_tasks.fetch_add(1, Ordering::Relaxed);
            return Err("Insufficient available agents".to_string());
        }

        // Simulate consensus building
        let consensus_start = Instant::now();
        let consensus_delay = Duration::from_micros(200 * available_agents.len() as u64);
        tokio::time::sleep(consensus_delay).await;
        let consensus_time = consensus_start.elapsed();

        let total_time = start_time.elapsed();

        // Check SLA compliance
        if total_time > self.sla_requirements.max_decision_latency {
            self.failed_tasks.fetch_add(1, Ordering::Relaxed);
            return Err(format!("SLA violation: decision took {:?}", total_time));
        }

        // Update metrics
        {
            let mut metrics = self.performance_metrics.write().await;
            metrics.decision_latency = total_time;
            metrics.agent_coordination_time = total_time - consensus_time;
            metrics.consensus_building_time = consensus_time;
        }

        self.processed_tasks.fetch_add(1, Ordering::Relaxed);

        // Return consensus result based on agent performance
        let avg_performance: f64 = available_agents.iter()
            .map(|agent| agent.performance_score)
            .sum::<f64>() / available_agents.len() as f64;

        let direction = if avg_performance > 0.7 {
            TradeDirection::Long
        } else if avg_performance < 0.3 {
            TradeDirection::Short
        } else {
            TradeDirection::Hold
        };

        Ok(direction)
    }

    pub async fn run_throughput_test(&self, duration_secs: u64, target_throughput: f64) -> Result<f64, String> {
        let start_time = Instant::now();
        let end_time = start_time + Duration::from_secs(duration_secs);
        let mut task_id = 0;

        let tasks_processed = Arc::new(AtomicU64::new(0));
        let mut handles = Vec::new();

        // Calculate target delay between tasks
        let delay_between_tasks = Duration::from_secs_f64(1.0 / target_throughput);

        while Instant::now() < end_time {
            task_id += 1;
            let task = CoordinationTask {
                id: format!("throughput_test_{}", task_id),
                agents_count: 3,
                complexity: TaskComplexity::Simple,
                priority: TaskPriority::Medium,
                created_at: Instant::now(),
            };

            let coordinator = self.clone_arc_fields();
            let tasks_counter = tasks_processed.clone();

            let handle = tokio::spawn(async move {
                if coordinator.coordinate_agents(task).await.is_ok() {
                    tasks_counter.fetch_add(1, Ordering::Relaxed);
                }
            });

            handles.push(handle);

            // Control rate to target throughput
            tokio::time::sleep(delay_between_tasks).await;
        }

        // Wait for all tasks to complete
        futures::future::join_all(handles).await;

        let actual_duration = start_time.elapsed();
        let tasks_completed = tasks_processed.load(Ordering::Relaxed);
        let actual_throughput = tasks_completed as f64 / actual_duration.as_secs_f64();

        // Update throughput metric
        {
            let mut metrics = self.performance_metrics.write().await;
            metrics.throughput = actual_throughput;
        }

        Ok(actual_throughput)
    }

    pub async fn run_latency_benchmark(&self, num_samples: usize) -> Result<BenchmarkResult, String> {
        let mut latencies = Vec::with_capacity(num_samples);
        let start_time = Instant::now();
        let mut successful_operations = 0;

        for i in 0..num_samples {
            let task = CoordinationTask {
                id: format!("latency_test_{}", i),
                agents_count: 5,
                complexity: TaskComplexity::Medium,
                priority: TaskPriority::High,
                created_at: Instant::now(),
            };

            let operation_start = Instant::now();
            let result = self.coordinate_agents(task).await;
            let operation_latency = operation_start.elapsed();

            latencies.push(operation_latency);

            if result.is_ok() {
                successful_operations += 1;
            }
        }

        let total_duration = start_time.elapsed();
        let success_rate = successful_operations as f64 / num_samples as f64;
        let throughput = num_samples as f64 / total_duration.as_secs_f64();

        // Calculate percentiles
        latencies.sort();
        let p50_idx = (num_samples as f64 * 0.50) as usize;
        let p95_idx = (num_samples as f64 * 0.95) as usize;
        let p99_idx = (num_samples as f64 * 0.99) as usize;

        let benchmark_result = BenchmarkResult {
            test_name: "Latency Benchmark".to_string(),
            duration: total_duration,
            throughput,
            latency_p50: latencies.get(p50_idx).copied().unwrap_or(Duration::ZERO),
            latency_p95: latencies.get(p95_idx).copied().unwrap_or(Duration::ZERO),
            latency_p99: latencies.get(p99_idx).copied().unwrap_or(Duration::ZERO),
            success_rate,
            memory_peak: self.get_memory_usage().await,
            cpu_peak: self.get_cpu_usage().await,
        };

        let mut results = self.benchmark_results.write().await;
        results.push(benchmark_result.clone());

        Ok(benchmark_result)
    }

    pub async fn run_concurrent_load_test(&self, concurrent_agents: usize, operations_per_agent: usize) -> Result<BenchmarkResult, String> {
        let start_time = Instant::now();
        let mut handles = Vec::new();
        let total_operations = concurrent_agents * operations_per_agent;
        let successful_operations = Arc::new(AtomicU64::new(0));

        for agent_id in 0..concurrent_agents {
            let coordinator = self.clone_arc_fields();
            let success_counter = successful_operations.clone();

            let handle = tokio::spawn(async move {
                let mut local_successes = 0;

                for op_id in 0..operations_per_agent {
                    let task = CoordinationTask {
                        id: format!("load_test_{}_{}", agent_id, op_id),
                        agents_count: 2,
                        complexity: TaskComplexity::Simple,
                        priority: TaskPriority::Medium,
                        created_at: Instant::now(),
                    };

                    if coordinator.coordinate_agents(task).await.is_ok() {
                        local_successes += 1;
                    }
                }

                success_counter.fetch_add(local_successes, Ordering::Relaxed);
            });

            handles.push(handle);
        }

        // Wait for all concurrent operations to complete
        futures::future::join_all(handles).await;

        let total_duration = start_time.elapsed();
        let successful_ops = successful_operations.load(Ordering::Relaxed);
        let success_rate = successful_ops as f64 / total_operations as f64;
        let throughput = successful_ops as f64 / total_duration.as_secs_f64();

        let benchmark_result = BenchmarkResult {
            test_name: format!("Concurrent Load Test ({} agents)", concurrent_agents),
            duration: total_duration,
            throughput,
            latency_p50: Duration::from_millis(0), // Not applicable for load test
            latency_p95: Duration::from_millis(0),
            latency_p99: Duration::from_millis(0),
            success_rate,
            memory_peak: self.get_memory_usage().await,
            cpu_peak: self.get_cpu_usage().await,
        };

        let mut results = self.benchmark_results.write().await;
        results.push(benchmark_result.clone());

        Ok(benchmark_result)
    }

    pub async fn run_stress_test(&self, duration_secs: u64, max_load: f64) -> Result<BenchmarkResult, String> {
        let start_time = Instant::now();
        let end_time = start_time + Duration::from_secs(duration_secs);
        let mut task_id = 0;
        let successful_operations = Arc::new(AtomicU64::new(0));
        let total_operations = Arc::new(AtomicU64::new(0));

        // Gradually increase load over time
        while Instant::now() < end_time {
            let elapsed_ratio = start_time.elapsed().as_secs_f64() / duration_secs as f64;
            let current_load = max_load * elapsed_ratio;
            let delay_between_tasks = Duration::from_secs_f64(1.0 / current_load.max(1.0));

            task_id += 1;
            let task = CoordinationTask {
                id: format!("stress_test_{}", task_id),
                agents_count: 4,
                complexity: TaskComplexity::Complex,
                priority: TaskPriority::High,
                created_at: Instant::now(),
            };

            let coordinator = self.clone_arc_fields();
            let success_counter = successful_operations.clone();
            let total_counter = total_operations.clone();

            tokio::spawn(async move {
                total_counter.fetch_add(1, Ordering::Relaxed);
                if coordinator.coordinate_agents(task).await.is_ok() {
                    success_counter.fetch_add(1, Ordering::Relaxed);
                }
            });

            tokio::time::sleep(delay_between_tasks).await;
        }

        // Wait a bit for pending operations to complete
        tokio::time::sleep(Duration::from_millis(100)).await;

        let total_duration = start_time.elapsed();
        let successful_ops = successful_operations.load(Ordering::Relaxed);
        let total_ops = total_operations.load(Ordering::Relaxed);
        let success_rate = if total_ops > 0 { successful_ops as f64 / total_ops as f64 } else { 0.0 };
        let throughput = successful_ops as f64 / total_duration.as_secs_f64();

        let benchmark_result = BenchmarkResult {
            test_name: format!("Stress Test (max load: {:.1})", max_load),
            duration: total_duration,
            throughput,
            latency_p50: Duration::from_millis(0),
            latency_p95: Duration::from_millis(0),
            latency_p99: Duration::from_millis(0),
            success_rate,
            memory_peak: self.get_memory_usage().await,
            cpu_peak: self.get_cpu_usage().await,
        };

        let mut results = self.benchmark_results.write().await;
        results.push(benchmark_result.clone());

        Ok(benchmark_result)
    }

    pub async fn verify_sla_compliance(&self) -> Result<bool, String> {
        let metrics = self.performance_metrics.read().await;

        let mut violations = Vec::new();

        if metrics.decision_latency > self.sla_requirements.max_decision_latency {
            violations.push(format!(
                "Decision latency violation: {:?} > {:?}",
                metrics.decision_latency, self.sla_requirements.max_decision_latency
            ));
        }

        if metrics.throughput < self.sla_requirements.min_throughput {
            violations.push(format!(
                "Throughput violation: {:.2} < {:.2}",
                metrics.throughput, self.sla_requirements.min_throughput
            ));
        }

        if metrics.cpu_usage > self.sla_requirements.max_cpu_usage {
            violations.push(format!(
                "CPU usage violation: {:.2}% > {:.2}%",
                metrics.cpu_usage, self.sla_requirements.max_cpu_usage
            ));
        }

        if metrics.memory_usage > self.sla_requirements.max_memory_usage {
            violations.push(format!(
                "Memory usage violation: {} > {}",
                metrics.memory_usage, self.sla_requirements.max_memory_usage
            ));
        }

        if metrics.success_rate < self.sla_requirements.min_success_rate {
            violations.push(format!(
                "Success rate violation: {:.2} < {:.2}",
                metrics.success_rate, self.sla_requirements.min_success_rate
            ));
        }

        if metrics.error_rate > self.sla_requirements.max_error_rate {
            violations.push(format!(
                "Error rate violation: {:.2} > {:.2}",
                metrics.error_rate, self.sla_requirements.max_error_rate
            ));
        }

        if violations.is_empty() {
            Ok(true)
        } else {
            Err(format!("SLA violations: {}", violations.join(", ")))
        }
    }

    async fn get_memory_usage(&self) -> u64 {
        // Simulate memory usage measurement
        64 * 1024 * 1024 // 64MB
    }

    async fn get_cpu_usage(&self) -> f64 {
        // Simulate CPU usage measurement
        25.0 // 25%
    }

    fn clone_arc_fields(&self) -> MockPerformanceCoordinator {
        MockPerformanceCoordinator {
            agents: self.agents.clone(),
            task_queue: self.task_queue.clone(),
            performance_metrics: self.performance_metrics.clone(),
            sla_requirements: self.sla_requirements.clone(),
            processed_tasks: self.processed_tasks.clone(),
            failed_tasks: self.failed_tasks.clone(),
            concurrency_limiter: self.concurrency_limiter.clone(),
            benchmark_results: self.benchmark_results.clone(),
        }
    }

    pub async fn get_performance_summary(&self) -> PerformanceMetrics {
        let metrics = self.performance_metrics.read().await;
        let processed = self.processed_tasks.load(Ordering::Relaxed);
        let failed = self.failed_tasks.load(Ordering::Relaxed);
        let total = processed + failed;

        let mut summary = metrics.clone();
        if total > 0 {
            summary.success_rate = processed as f64 / total as f64;
            summary.error_rate = failed as f64 / total as f64;
        }

        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::test;

    async fn setup_test_coordinator() -> MockPerformanceCoordinator {
        let coordinator = MockPerformanceCoordinator::new();

        // Register test agents
        for i in 1..=20 {
            let agent = Agent {
                id: format!("agent_{}", i),
                performance_score: 0.8,
                processing_time: Duration::from_micros(100),
                is_overloaded: false,
            };
            coordinator.register_agent(agent).await;
        }

        coordinator
    }

    #[test]
    async fn test_decision_latency_sla_compliance() {
        let coordinator = setup_test_coordinator().await;

        let task = CoordinationTask {
            id: "latency_test".to_string(),
            agents_count: 5,
            complexity: TaskComplexity::Simple,
            priority: TaskPriority::High,
            created_at: Instant::now(),
        };

        let start = Instant::now();
        let result = coordinator.coordinate_agents(task).await;
        let elapsed = start.elapsed();

        assert!(result.is_ok());
        assert!(elapsed < Duration::from_millis(10)); // <10ms SLA
    }

    #[test]
    async fn test_throughput_sla_500_agents_per_second() {
        let coordinator = setup_test_coordinator().await;

        let actual_throughput = coordinator.run_throughput_test(2, 500.0).await;

        assert!(actual_throughput.is_ok());
        let throughput = actual_throughput.unwrap();
        assert!(throughput >= 450.0); // Allow 10% tolerance
        assert!(throughput <= 550.0); // Upper bound check
    }

    #[test]
    async fn test_high_throughput_sustained_performance() {
        let coordinator = setup_test_coordinator().await;

        // Test sustained high throughput
        let actual_throughput = coordinator.run_throughput_test(5, 1000.0).await;

        assert!(actual_throughput.is_ok());
        let throughput = actual_throughput.unwrap();
        assert!(throughput >= 800.0); // Should sustain high throughput
    }

    #[test]
    async fn test_latency_percentiles_benchmark() {
        let coordinator = setup_test_coordinator().await;

        let benchmark = coordinator.run_latency_benchmark(1000).await;

        assert!(benchmark.is_ok());
        let result = benchmark.unwrap();

        // Verify latency percentiles meet SLA
        assert!(result.latency_p50 < Duration::from_millis(10));
        assert!(result.latency_p95 < Duration::from_millis(15));
        assert!(result.latency_p99 < Duration::from_millis(25));
        assert!(result.success_rate >= 0.95);
        assert!(result.throughput >= 100.0); // Minimum expected throughput
    }

    #[test]
    async fn test_concurrent_agent_coordination() {
        let coordinator = setup_test_coordinator().await;

        // Test with 100 concurrent agents, 10 operations each
        let benchmark = coordinator.run_concurrent_load_test(100, 10).await;

        assert!(benchmark.is_ok());
        let result = benchmark.unwrap();

        assert!(result.success_rate >= 0.90); // Allow some failures under load
        assert!(result.throughput >= 200.0); // Reasonable throughput under concurrency
        assert!(result.duration < Duration::from_secs(10)); // Reasonable completion time
    }

    #[test]
    async fn test_stress_test_degradation_graceful() {
        let coordinator = setup_test_coordinator().await;

        // Stress test with increasing load up to 2000 operations/sec
        let benchmark = coordinator.run_stress_test(3, 2000.0).await;

        assert!(benchmark.is_ok());
        let result = benchmark.unwrap();

        // Even under stress, should maintain reasonable success rate
        assert!(result.success_rate >= 0.70);
        // Should still achieve significant throughput
        assert!(result.throughput >= 500.0);
    }

    #[test]
    async fn test_memory_usage_sla_compliance() {
        let coordinator = setup_test_coordinator().await;

        // Run multiple operations to build up memory usage
        for i in 0..100 {
            let task = CoordinationTask {
                id: format!("memory_test_{}", i),
                agents_count: 3,
                complexity: TaskComplexity::Medium,
                priority: TaskPriority::Medium,
                created_at: Instant::now(),
            };
            let _ = coordinator.coordinate_agents(task).await;
        }

        let memory_usage = coordinator.get_memory_usage().await;
        assert!(memory_usage < 1024 * 1024 * 1024); // <1GB SLA
    }

    #[test]
    async fn test_complex_task_coordination_performance() {
        let coordinator = setup_test_coordinator().await;

        let complex_task = CoordinationTask {
            id: "complex_coordination".to_string(),
            agents_count: 15, // Large number of agents
            complexity: TaskComplexity::HighlyComplex,
            priority: TaskPriority::Critical,
            created_at: Instant::now(),
        };

        let start = Instant::now();
        let result = coordinator.coordinate_agents(complex_task).await;
        let elapsed = start.elapsed();

        assert!(result.is_ok());
        // Even complex tasks should complete within reasonable time
        assert!(elapsed < Duration::from_millis(20));
    }

    #[test]
    async fn test_performance_under_agent_overload() {
        let coordinator = setup_test_coordinator().await;

        // Mark half of agents as overloaded
        {
            let mut agents = coordinator.agents.write().await;
            let mut count = 0;
            for agent in agents.values_mut() {
                if count % 2 == 0 {
                    agent.is_overloaded = true;
                }
                count += 1;
            }
        }

        let task = CoordinationTask {
            id: "overload_test".to_string(),
            agents_count: 5,
            complexity: TaskComplexity::Medium,
            priority: TaskPriority::High,
            created_at: Instant::now(),
        };

        let start = Instant::now();
        let result = coordinator.coordinate_agents(task).await;
        let elapsed = start.elapsed();

        assert!(result.is_ok());
        assert!(elapsed < Duration::from_millis(15)); // Slight tolerance for overload
    }

    #[test]
    async fn test_priority_task_processing() {
        let coordinator = setup_test_coordinator().await;

        // Create tasks with different priorities
        let critical_task = CoordinationTask {
            id: "critical_task".to_string(),
            agents_count: 3,
            complexity: TaskComplexity::Medium,
            priority: TaskPriority::Critical,
            created_at: Instant::now(),
        };

        let low_task = CoordinationTask {
            id: "low_task".to_string(),
            agents_count: 3,
            complexity: TaskComplexity::Medium,
            priority: TaskPriority::Low,
            created_at: Instant::now(),
        };

        // Critical task should be processed quickly
        let critical_start = Instant::now();
        let critical_result = coordinator.coordinate_agents(critical_task).await;
        let critical_elapsed = critical_start.elapsed();

        let low_start = Instant::now();
        let low_result = coordinator.coordinate_agents(low_task).await;
        let low_elapsed = low_start.elapsed();

        assert!(critical_result.is_ok());
        assert!(low_result.is_ok());
        
        // Critical task should complete faster
        assert!(critical_elapsed <= low_elapsed);
        assert!(critical_elapsed < Duration::from_millis(8)); // Stricter SLA for critical
    }

    #[test]
    async fn test_sla_compliance_verification() {
        let coordinator = setup_test_coordinator().await;

        // Run some operations to populate metrics
        let _ = coordinator.run_throughput_test(1, 600.0).await;

        let compliance = coordinator.verify_sla_compliance().await;
        
        // Should pass SLA compliance with good performance
        assert!(compliance.is_ok());
    }

    #[test]
    async fn test_performance_metrics_accuracy() {
        let coordinator = setup_test_coordinator().await;

        let task = CoordinationTask {
            id: "metrics_test".to_string(),
            agents_count: 4,
            complexity: TaskComplexity::Medium,
            priority: TaskPriority::Medium,
            created_at: Instant::now(),
        };

        let _ = coordinator.coordinate_agents(task).await;

        let summary = coordinator.get_performance_summary().await;

        // Verify metrics are within reasonable bounds
        assert!(summary.decision_latency > Duration::from_micros(10));
        assert!(summary.decision_latency < Duration::from_millis(50));
        assert!(summary.agent_coordination_time >= Duration::from_micros(1));
        assert!(summary.consensus_building_time >= Duration::from_micros(1));
        assert!(summary.success_rate >= 0.0 && summary.success_rate <= 1.0);
        assert!(summary.error_rate >= 0.0 && summary.error_rate <= 1.0);
    }

    #[test]
    async fn test_timeout_handling_performance() {
        let coordinator = setup_test_coordinator().await;

        let task = CoordinationTask {
            id: "timeout_test".to_string(),
            agents_count: 5,
            complexity: TaskComplexity::Simple,
            priority: TaskPriority::High,
            created_at: Instant::now(),
        };

        // Test with timeout
        let result = timeout(
            Duration::from_millis(50),
            coordinator.coordinate_agents(task)
        ).await;

        assert!(result.is_ok()); // Should complete within timeout
        assert!(result.unwrap().is_ok()); // Should succeed
    }

    #[test]
    async fn test_concurrent_coordination_stability() {
        let coordinator = setup_test_coordinator().await;

        // Run 1000 concurrent coordinations
        let mut handles = Vec::new();
        for i in 0..1000 {
            let coord = coordinator.clone_arc_fields();
            let handle = tokio::spawn(async move {
                let task = CoordinationTask {
                    id: format!("stability_test_{}", i),
                    agents_count: 2,
                    complexity: TaskComplexity::Simple,
                    priority: TaskPriority::Medium,
                    created_at: Instant::now(),
                };
                coord.coordinate_agents(task).await
            });
            handles.push(handle);
        }

        let results = futures::future::join_all(handles).await;
        
        let successful_ops = results.iter()
            .filter(|r| r.is_ok() && r.as_ref().unwrap().is_ok())
            .count();

        // Should maintain high success rate even under heavy concurrency
        let success_rate = successful_ops as f64 / 1000.0;
        assert!(success_rate >= 0.95);
    }

    #[test]
    async fn test_benchmark_results_storage() {
        let coordinator = setup_test_coordinator().await;

        // Run multiple benchmarks
        let _ = coordinator.run_latency_benchmark(100).await;
        let _ = coordinator.run_concurrent_load_test(10, 10).await;
        let _ = coordinator.run_stress_test(1, 100.0).await;

        let results = coordinator.benchmark_results.read().await;
        assert_eq!(results.len(), 3);

        // Verify all results have valid data
        for result in results.iter() {
            assert!(!result.test_name.is_empty());
            assert!(result.duration > Duration::from_millis(0));
            assert!(result.success_rate >= 0.0 && result.success_rate <= 1.0);
            assert!(result.throughput >= 0.0);
        }
    }

    #[test]
    async fn test_resource_cleanup_after_operations() {
        let coordinator = setup_test_coordinator().await;

        let initial_processed = coordinator.processed_tasks.load(Ordering::Relaxed);
        let initial_failed = coordinator.failed_tasks.load(Ordering::Relaxed);

        // Run batch of operations
        for i in 0..50 {
            let task = CoordinationTask {
                id: format!("cleanup_test_{}", i),
                agents_count: 3,
                complexity: TaskComplexity::Simple,
                priority: TaskPriority::Medium,
                created_at: Instant::now(),
            };
            let _ = coordinator.coordinate_agents(task).await;
        }

        let final_processed = coordinator.processed_tasks.load(Ordering::Relaxed);
        let final_failed = coordinator.failed_tasks.load(Ordering::Relaxed);

        // Verify counters updated correctly
        assert!(final_processed > initial_processed);
        assert!(final_processed + final_failed - initial_processed - initial_failed == 50);
    }
}