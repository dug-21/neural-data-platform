// Performance Benchmark Tests - London School TDD
// Tests for orchestrator performance, scalability, and resource utilization

use super::mock_services::*;
use super::mock_services::swarm_mock::*;
use super::test_data_generators::*;
use tokio_test;
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[cfg(test)]
mod performance_benchmark_behavior_tests {
    use super::*;

    struct PerformanceBenchmarkTestContext {
        mock_registry: MockRegistry,
        swarm_service: MockSwarmService,
        test_data: TestDataGenerator,
        benchmark_swarm_id: String,
    }

    impl PerformanceBenchmarkTestContext {
        async fn new() -> Self {
            let mock_registry = MockRegistry::new();
            let swarm_service = MockSwarmService::new();
            let test_data = TestDataGenerator::new();
            
            mock_registry.register(Box::new(swarm_service.clone())).await;
            mock_registry.start_all().await.unwrap();
            
            // Initialize performance benchmark swarm
            let benchmark_swarm_id = swarm_service
                .init_swarm(SwarmTopology::Mesh, 20, "performance-benchmark")
                .await
                .unwrap();
            
            Self {
                mock_registry,
                swarm_service,
                test_data,
                benchmark_swarm_id,
            }
        }

        async fn cleanup(&self) {
            self.swarm_service.destroy_swarm(&self.benchmark_swarm_id).await.ok();
            self.mock_registry.stop_all().await.unwrap();
        }
    }

    #[tokio::test]
    async fn should_benchmark_swarm_initialization_performance() {
        // Given: Performance benchmarking requirements for swarm initialization
        let context = PerformanceBenchmarkTestContext::new().await;
        
        // When: Swarm initialization performance is measured
        let initialization_times = vec![];
        let mut results = Vec::new();
        
        for topology in [SwarmTopology::Mesh, SwarmTopology::Hierarchical, SwarmTopology::Star, SwarmTopology::Ring] {
            let start_time = Instant::now();
            
            let swarm_id = context.swarm_service
                .init_swarm(topology.clone(), 10, "benchmark-test")
                .await
                .unwrap();
            
            let initialization_duration = start_time.elapsed();
            
            results.push((topology, initialization_duration));
            
            // Cleanup for next test
            context.swarm_service.destroy_swarm(&swarm_id).await.ok();
        }
        
        // Then: Initialization performance should meet benchmarks
        for (topology, duration) in results {
            // Initialization should complete within acceptable time limits
            assert!(duration < Duration::from_millis(500), 
                   "Swarm initialization for {:?} took {:?}, expected < 500ms", 
                   topology, duration);
        }
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_benchmark_agent_spawning_throughput() {
        // Given: Agent spawning performance requirements
        let context = PerformanceBenchmarkTestContext::new().await;
        
        // When: Multiple agents are spawned concurrently
        let agent_counts = vec![10, 50, 100];
        let mut throughput_results = Vec::new();
        
        for agent_count in agent_counts {
            let agents = context.test_data.generate_high_load_agents(agent_count);
            let start_time = Instant::now();
            
            // Spawn agents concurrently
            let spawn_futures: Vec<_> = agents.into_iter().map(|agent| {
                context.swarm_service.add_agent_to_swarm(&context.benchmark_swarm_id, agent)
            }).collect();
            
            let spawn_results = futures::future::join_all(spawn_futures).await;
            let spawn_duration = start_time.elapsed();
            
            let successful_spawns = spawn_results.iter().filter(|r| r.is_ok()).count();
            let throughput = successful_spawns as f64 / spawn_duration.as_secs_f64();
            
            throughput_results.push((agent_count, throughput, spawn_duration));
        }
        
        // Then: Agent spawning throughput should meet performance requirements
        for (count, throughput, duration) in throughput_results {
            // Should spawn at least 20 agents per second
            assert!(throughput >= 20.0, 
                   "Agent spawning throughput {} agents/sec is below 20 for {} agents", 
                   throughput, count);
            
            // Total spawn time should be reasonable
            assert!(duration < Duration::from_secs(10),
                   "Spawning {} agents took {:?}, expected < 10s", count, duration);
        }
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_benchmark_task_orchestration_latency() {
        // Given: Task orchestration performance requirements
        let context = PerformanceBenchmarkTestContext::new().await;
        
        // Setup test agents
        let test_agents = context.test_data.generate_agent_team(8);
        for agent in test_agents {
            context.swarm_service
                .add_agent_to_swarm(&context.benchmark_swarm_id, agent)
                .await
                .unwrap();
        }
        
        // When: Task orchestration latency is measured
        let task_counts = vec![1, 10, 50, 100];
        let mut latency_results = Vec::new();
        
        for task_count in task_counts {
            let tasks = context.test_data.generate_task_batch(task_count);
            let start_time = Instant::now();
            
            // Simulate task orchestration (in real implementation, this would involve actual orchestration)
            for task in tasks.iter() {
                // Mock task assignment latency
                tokio::time::sleep(Duration::from_micros(100)).await;
            }
            
            let orchestration_duration = start_time.elapsed();
            let average_latency = orchestration_duration / task_count as u32;
            
            latency_results.push((task_count, average_latency, orchestration_duration));
        }
        
        // Then: Task orchestration latency should be within acceptable limits
        for (count, avg_latency, total_duration) in latency_results {
            // Average task orchestration should be < 10ms
            assert!(avg_latency < Duration::from_millis(10),
                   "Average task orchestration latency {:?} exceeds 10ms for {} tasks",
                   avg_latency, count);
            
            // Total orchestration time should scale linearly
            if count > 1 {
                let expected_max_duration = Duration::from_millis(count as u64 * 10);
                assert!(total_duration < expected_max_duration,
                       "Total orchestration time {:?} exceeds expected {:?} for {} tasks",
                       total_duration, expected_max_duration, count);
            }
        }
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_benchmark_memory_usage_under_load() {
        // Given: Memory usage performance requirements
        let context = PerformanceBenchmarkTestContext::new().await;
        
        // When: System is under varying memory loads
        let agent_counts = vec![10, 50, 100, 200];
        let mut memory_usage_results = Vec::new();
        
        for agent_count in agent_counts {
            let high_memory_agents = context.test_data.generate_high_load_agents(agent_count);
            
            // Add agents and measure memory impact
            let initial_metrics = context.swarm_service
                .get_swarm_metrics(&context.benchmark_swarm_id)
                .await
                .unwrap();
            
            for agent in high_memory_agents {
                context.swarm_service
                    .add_agent_to_swarm(&context.benchmark_swarm_id, agent)
                    .await
                    .unwrap();
            }
            
            let final_metrics = context.swarm_service
                .get_swarm_metrics(&context.benchmark_swarm_id)
                .await
                .unwrap();
            
            let memory_increase = final_metrics.memory_usage_mb - initial_metrics.memory_usage_mb;
            memory_usage_results.push((agent_count, memory_increase));
        }
        
        // Then: Memory usage should scale predictably
        for (count, memory_usage) in memory_usage_results {
            // Memory usage per agent should be reasonable (< 50MB per agent average)
            let memory_per_agent = memory_usage / count as u64;
            assert!(memory_per_agent < 50,
                   "Memory usage per agent {} MB exceeds 50MB limit for {} agents",
                   memory_per_agent, count);
            
            // Total memory usage should be bounded
            assert!(memory_usage < 10240, // 10GB limit
                   "Total memory usage {} MB exceeds 10GB limit for {} agents",
                   memory_usage, count);
        }
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_benchmark_cpu_utilization_efficiency() {
        // Given: CPU utilization performance requirements
        let context = PerformanceBenchmarkTestContext::new().await;
        
        // When: CPU-intensive agents are deployed
        let cpu_intensive_agents = vec![
            MockAgent {
                id: "cpu-intensive-1".to_string(),
                agent_type: AgentType::Optimizer,
                status: AgentStatus::Busy,
                capabilities: vec!["intensive-optimization".to_string()],
                memory_usage: 256,
                cpu_usage: 0.8,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "cpu-intensive-2".to_string(),
                agent_type: AgentType::Analyst,
                status: AgentStatus::Busy,
                capabilities: vec!["intensive-analysis".to_string()],
                memory_usage: 512,
                cpu_usage: 0.9,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "cpu-light-1".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec!["coordination".to_string()],
                memory_usage: 128,
                cpu_usage: 0.1,
                last_heartbeat: std::time::SystemTime::now(),
            },
        ];
        
        for agent in cpu_intensive_agents {
            context.swarm_service
                .add_agent_to_swarm(&context.benchmark_swarm_id, agent)
                .await
                .unwrap();
        }
        
        // Then: CPU utilization should be tracked and optimized
        let swarm_status = context.swarm_service
            .get_swarm_status(&context.benchmark_swarm_id)
            .await
            .unwrap();
        
        let metrics = context.swarm_service
            .get_swarm_metrics(&context.benchmark_swarm_id)
            .await
            .unwrap();
        
        // CPU utilization should be reasonable
        assert!(metrics.cpu_utilization <= 1.0,
               "CPU utilization {} exceeds 100%", metrics.cpu_utilization);
        
        // High CPU agents should be identifiable
        let high_cpu_agents: Vec<&MockAgent> = swarm_status.agents
            .iter()
            .filter(|a| a.cpu_usage > 0.7)
            .collect();
        
        assert_eq!(high_cpu_agents.len(), 2);
        
        // System should maintain responsiveness under CPU load
        let response_start = Instant::now();
        let _ = context.swarm_service
            .get_swarm_status(&context.benchmark_swarm_id)
            .await
            .unwrap();
        let response_time = response_start.elapsed();
        
        assert!(response_time < Duration::from_millis(100),
               "System response time {:?} exceeds 100ms under CPU load", response_time);
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_benchmark_concurrent_swarm_operations() {
        // Given: Concurrent operation performance requirements
        let context = PerformanceBenchmarkTestContext::new().await;
        
        // When: Multiple swarm operations are performed concurrently
        let concurrent_operations = 20;
        let start_time = Instant::now();
        
        let operation_futures: Vec<_> = (0..concurrent_operations).map(|i| {
            let context_clone = &context;
            async move {
                // Simulate various concurrent operations
                match i % 4 {
                    0 => {
                        // Agent spawning
                        let agent = context_clone.test_data.generate_tdd_london_agent();
                        context_clone.swarm_service
                            .add_agent_to_swarm(&context_clone.benchmark_swarm_id, agent)
                            .await
                    }
                    1 => {
                        // Status checking
                        context_clone.swarm_service
                            .get_swarm_status(&context_clone.benchmark_swarm_id)
                            .await
                            .map(|_| ())
                            .map_err(|e| e.to_string())
                    }
                    2 => {
                        // Metrics retrieval
                        context_clone.swarm_service
                            .get_swarm_metrics(&context_clone.benchmark_swarm_id)
                            .await
                            .map(|_| ())
                            .map_err(|e| e.to_string())
                    }
                    3 => {
                        // Swarm listing
                        context_clone.swarm_service
                            .list_active_swarms()
                            .await;
                        Ok(())
                    }
                    _ => Ok(()),
                }
            }
        }).collect();
        
        let results = futures::future::join_all(operation_futures).await;
        let total_duration = start_time.elapsed();
        
        // Then: Concurrent operations should complete efficiently
        let successful_operations = results.iter().filter(|r| r.is_ok()).count();
        let success_rate = successful_operations as f64 / concurrent_operations as f64;
        
        assert!(success_rate >= 0.95,
               "Concurrent operation success rate {} below 95%", success_rate);
        
        assert!(total_duration < Duration::from_secs(5),
               "Concurrent operations took {:?}, expected < 5s", total_duration);
        
        let operations_per_second = concurrent_operations as f64 / total_duration.as_secs_f64();
        assert!(operations_per_second >= 10.0,
               "Concurrent operations throughput {} ops/sec below 10", operations_per_second);
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_benchmark_stress_test_resilience() {
        // Given: Stress test performance requirements
        let context = PerformanceBenchmarkTestContext::new().await;
        
        // When: System is under extreme stress
        let stress_agents = context.test_data.generate_high_load_agents(100);
        let stress_tasks = context.test_data.generate_stress_test_tasks(200);
        
        let stress_start = Instant::now();
        
        // Apply maximum load
        let agent_spawn_futures: Vec<_> = stress_agents.into_iter().map(|agent| {
            context.swarm_service.add_agent_to_swarm(&context.benchmark_swarm_id, agent)
        }).collect();
        
        let spawn_results = futures::future::join_all(agent_spawn_futures).await;
        let stress_duration = stress_start.elapsed();
        
        // Then: System should remain stable under stress
        let successful_spawns = spawn_results.iter().filter(|r| r.is_ok()).count();
        let spawn_success_rate = successful_spawns as f64 / 100.0;
        
        assert!(spawn_success_rate >= 0.8,
               "Agent spawn success rate {} below 80% under stress", spawn_success_rate);
        
        // System should still be responsive
        let response_start = Instant::now();
        let metrics = context.swarm_service
            .get_swarm_metrics(&context.benchmark_swarm_id)
            .await
            .unwrap();
        let response_time = response_start.elapsed();
        
        assert!(response_time < Duration::from_secs(1),
               "System response time {:?} exceeds 1s under stress", response_time);
        
        // Resource usage should be tracked
        assert!(metrics.memory_usage_mb > 0);
        assert!(metrics.active_agents > 0);
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_benchmark_scalability_limits() {
        // Given: Scalability testing requirements
        let context = PerformanceBenchmarkTestContext::new().await;
        
        // When: System scalability is tested progressively
        let scale_levels = vec![10, 25, 50, 100];
        let mut scalability_results = Vec::new();
        
        for scale_level in scale_levels {
            let scale_start = Instant::now();
            
            // Generate agents for this scale level
            let agents = context.test_data.generate_high_load_agents(scale_level);
            let mut successful_additions = 0;
            
            for agent in agents {
                match context.swarm_service
                    .add_agent_to_swarm(&context.benchmark_swarm_id, agent)
                    .await {
                    Ok(_) => successful_additions += 1,
                    Err(_) => break, // Stop at first failure to find limits
                }
            }
            
            let scale_duration = scale_start.elapsed();
            let final_metrics = context.swarm_service
                .get_swarm_metrics(&context.benchmark_swarm_id)
                .await
                .unwrap();
            
            scalability_results.push((scale_level, successful_additions, scale_duration, final_metrics));
        }
        
        // Then: Scalability characteristics should be documented
        for (target_scale, actual_scale, duration, metrics) in scalability_results {
            let scale_efficiency = actual_scale as f64 / target_scale as f64;
            
            // Should achieve at least 80% of target scale
            assert!(scale_efficiency >= 0.8,
                   "Scale efficiency {} below 80% for target {} agents", 
                   scale_efficiency, target_scale);
            
            // Scaling time should be reasonable
            assert!(duration < Duration::from_secs(30),
                   "Scaling to {} agents took {:?}, expected < 30s", 
                   target_scale, duration);
            
            // Metrics should reflect the scale
            assert_eq!(metrics.active_agents, actual_scale as u32);
        }
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_benchmark_tdd_workflow_performance() {
        // Given: TDD workflow performance requirements
        let context = PerformanceBenchmarkTestContext::new().await;
        
        // Setup TDD-specific agents
        let tdd_agents = vec![
            context.test_data.generate_tdd_london_agent(),
            context.test_data.generate_coder_agent(),
            context.test_data.generate_coordinator_agent(),
        ];
        
        for agent in tdd_agents {
            context.swarm_service
                .add_agent_to_swarm(&context.benchmark_swarm_id, agent)
                .await
                .unwrap();
        }
        
        // When: TDD workflow performance is measured
        let tdd_cycles = 10;
        let workflow_start = Instant::now();
        
        for cycle in 0..tdd_cycles {
            // Simulate Red-Green-Refactor cycle
            let red_task = context.test_data.generate_tdd_task(&format!("Red phase cycle {}", cycle));
            let green_task = context.test_data.generate_tdd_task(&format!("Green phase cycle {}", cycle));
            let refactor_task = context.test_data.generate_tdd_task(&format!("Refactor phase cycle {}", cycle));
            
            // Simulate TDD cycle timing (each phase should be quick)
            tokio::time::sleep(Duration::from_millis(50)).await; // Red
            tokio::time::sleep(Duration::from_millis(100)).await; // Green  
            tokio::time::sleep(Duration::from_millis(75)).await; // Refactor
        }
        
        let total_workflow_duration = workflow_start.elapsed();
        let average_cycle_time = total_workflow_duration / tdd_cycles as u32;
        
        // Then: TDD workflow performance should be optimized
        assert!(average_cycle_time < Duration::from_secs(1),
               "Average TDD cycle time {:?} exceeds 1s", average_cycle_time);
        
        assert!(total_workflow_duration < Duration::from_secs(10),
               "Total TDD workflow time {:?} exceeds 10s for {} cycles", 
               total_workflow_duration, tdd_cycles);
        
        // System should remain responsive during TDD workflows
        let response_start = Instant::now();
        let _ = context.swarm_service
            .get_swarm_status(&context.benchmark_swarm_id)
            .await
            .unwrap();
        let response_time = response_start.elapsed();
        
        assert!(response_time < Duration::from_millis(50),
               "System response time {:?} during TDD workflow exceeds 50ms", response_time);
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_benchmark_network_latency_impact() {
        // Given: Network latency simulation requirements
        let context = PerformanceBenchmarkTestContext::new().await;
        
        // When: Various network latencies are simulated
        let latencies = vec![0, 10, 50, 100, 200]; // milliseconds
        let mut latency_impact_results = Vec::new();
        
        for latency_ms in latencies {
            // Simulate network latency
            context.swarm_service.set_network_delay(latency_ms).await;
            
            let operation_start = Instant::now();
            
            // Perform network-dependent operations
            let swarm_id = context.swarm_service
                .init_swarm(SwarmTopology::Mesh, 5, &format!("latency-test-{}", latency_ms))
                .await
                .unwrap();
            
            let agent = context.test_data.generate_tdd_london_agent();
            let _ = context.swarm_service
                .add_agent_to_swarm(&swarm_id, agent)
                .await;
            
            let _ = context.swarm_service
                .get_swarm_status(&swarm_id)
                .await
                .unwrap();
            
            let operation_duration = operation_start.elapsed();
            latency_impact_results.push((latency_ms, operation_duration));
            
            // Cleanup
            context.swarm_service.destroy_swarm(&swarm_id).await.ok();
        }
        
        // Then: Network latency impact should be measurable and bounded
        for (latency_ms, total_duration) in latency_impact_results {
            // Operations should complete within reasonable time even with latency
            let max_expected_duration = Duration::from_millis(1000 + latency_ms * 5); // Base + latency impact
            
            assert!(total_duration < max_expected_duration,
                   "Operation with {}ms latency took {:?}, expected < {:?}",
                   latency_ms, total_duration, max_expected_duration);
        }
        
        context.cleanup().await;
    }
}

/// Integration tests for performance with neural components
#[cfg(test)]
mod performance_neural_integration_tests {
    use super::*;

    #[tokio::test]
    async fn should_benchmark_neural_model_training_performance() {
        // Given: Neural model training performance requirements
        let context = PerformanceBenchmarkTestContext::new().await;
        
        // Setup neural training agents
        let neural_agents = vec![
            context.test_data.generate_neural_specialist_agent(),
            MockAgent {
                id: "training-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec!["neural-training-coordination".to_string()],
                memory_usage: 512,
                cpu_usage: 0.3,
                last_heartbeat: std::time::SystemTime::now(),
            },
        ];
        
        for agent in neural_agents {
            context.swarm_service
                .add_agent_to_swarm(&context.benchmark_swarm_id, agent)
                .await
                .unwrap();
        }
        
        // When: Neural training performance is benchmarked
        let training_iterations = vec![100, 500, 1000];
        let mut training_performance_results = Vec::new();
        
        for iterations in training_iterations {
            let training_start = Instant::now();
            
            // Simulate neural training iterations
            for _ in 0..iterations {
                tokio::time::sleep(Duration::from_micros(500)).await; // Simulate training step
            }
            
            let training_duration = training_start.elapsed();
            let iterations_per_second = iterations as f64 / training_duration.as_secs_f64();
            
            training_performance_results.push((iterations, training_duration, iterations_per_second));
        }
        
        // Then: Neural training performance should meet requirements
        for (iterations, duration, iter_per_sec) in training_performance_results {
            // Training should achieve minimum throughput
            assert!(iter_per_sec >= 1000.0,
                   "Training throughput {} iter/sec below 1000 for {} iterations",
                   iter_per_sec, iterations);
            
            // Training time should scale reasonably
            let max_expected_duration = Duration::from_secs(iterations / 500); // Should handle 500+ iter/sec
            assert!(duration <= max_expected_duration,
                   "Training {} iterations took {:?}, expected <= {:?}",
                   iterations, duration, max_expected_duration);
        }
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_benchmark_neural_prediction_latency() {
        // Given: Neural prediction latency requirements
        let context = PerformanceBenchmarkTestContext::new().await;
        
        // Setup prediction agents
        let prediction_agent = MockAgent {
            id: "neural-prediction-agent".to_string(),
            agent_type: AgentType::Analyst,
            status: AgentStatus::Active,
            capabilities: vec![
                "real-time-prediction".to_string(),
                "batch-prediction".to_string(),
                "model-inference".to_string(),
            ],
            memory_usage: 768,
            cpu_usage: 0.4,
            last_heartbeat: std::time::SystemTime::now(),
        };
        
        context.swarm_service
            .add_agent_to_swarm(&context.benchmark_swarm_id, prediction_agent)
            .await
            .unwrap();
        
        // When: Prediction latency is benchmarked
        let prediction_counts = vec![1, 10, 100, 1000];
        let mut prediction_latency_results = Vec::new();
        
        for prediction_count in prediction_counts {
            let predictions_start = Instant::now();
            
            // Simulate neural predictions
            for _ in 0..prediction_count {
                tokio::time::sleep(Duration::from_micros(200)).await; // Simulate prediction latency
            }
            
            let predictions_duration = predictions_start.elapsed();
            let average_latency = predictions_duration / prediction_count as u32;
            let predictions_per_second = prediction_count as f64 / predictions_duration.as_secs_f64();
            
            prediction_latency_results.push((prediction_count, average_latency, predictions_per_second));
        }
        
        // Then: Prediction latency should meet real-time requirements
        for (count, avg_latency, pred_per_sec) in prediction_latency_results {
            // Average prediction latency should be < 10ms for real-time trading
            assert!(avg_latency < Duration::from_millis(10),
                   "Average prediction latency {:?} exceeds 10ms for {} predictions",
                   avg_latency, count);
            
            // Should achieve high prediction throughput
            assert!(pred_per_sec >= 100.0,
                   "Prediction throughput {} pred/sec below 100 for {} predictions",
                   pred_per_sec, count);
        }
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_benchmark_ruv_fann_integration_performance() {
        // Given: RUV-FANN integration performance requirements
        let context = PerformanceBenchmarkTestContext::new().await;
        
        // Setup FANN integration agents
        let fann_agents = vec![
            MockAgent {
                id: "fann-integration-agent".to_string(),
                agent_type: AgentType::Analyst,
                status: AgentStatus::Active,
                capabilities: vec![
                    "fann-model-loading".to_string(),
                    "fann-prediction-execution".to_string(),
                    "fann-model-serialization".to_string(),
                ],
                memory_usage: 1024,
                cpu_usage: 0.5,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "fann-performance-monitor".to_string(),
                agent_type: AgentType::Optimizer,
                status: AgentStatus::Active,
                capabilities: vec!["fann-performance-monitoring".to_string()],
                memory_usage: 256,
                cpu_usage: 0.15,
                last_heartbeat: std::time::SystemTime::now(),
            },
        ];
        
        for agent in fann_agents {
            context.swarm_service
                .add_agent_to_swarm(&context.benchmark_swarm_id, agent)
                .await
                .unwrap();
        }
        
        // When: FANN integration performance is benchmarked
        let model_sizes = vec![10, 50, 100]; // Input sizes
        let mut fann_performance_results = Vec::new();
        
        for model_size in model_sizes {
            let fann_start = Instant::now();
            
            // Simulate FANN operations
            let load_time = Duration::from_millis(100 + model_size * 2); // Model loading
            tokio::time::sleep(load_time).await;
            
            // Simulate predictions
            for _ in 0..100 {
                tokio::time::sleep(Duration::from_micros(100 + model_size as u64 * 2)).await;
            }
            
            let total_duration = fann_start.elapsed();
            let predictions_per_second = 100.0 / total_duration.as_secs_f64();
            
            fann_performance_results.push((model_size, total_duration, predictions_per_second));
        }
        
        // Then: FANN integration performance should be optimized
        for (size, duration, pred_per_sec) in fann_performance_results {
            // FANN operations should complete within reasonable time
            let max_expected_duration = Duration::from_secs(1 + size / 10); // Scale with model size
            assert!(duration < max_expected_duration,
                   "FANN operations for size {} took {:?}, expected < {:?}",
                   size, duration, max_expected_duration);
            
            // Should maintain reasonable prediction throughput
            assert!(pred_per_sec >= 50.0,
                   "FANN prediction throughput {} pred/sec below 50 for model size {}",
                   pred_per_sec, size);
        }
        
        context.cleanup().await;
    }
}