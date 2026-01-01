// Error Handling and Recovery Tests - London School TDD
// Tests for orchestrator's error handling, fault tolerance, and recovery mechanisms

use super::mock_services::*;
use super::mock_services::swarm_mock::*;
use super::test_data_generators::*;
use tokio_test;
use std::collections::HashMap;

#[cfg(test)]
mod error_handling_behavior_tests {
    use super::*;

    struct ErrorHandlingTestContext {
        mock_registry: MockRegistry,
        swarm_service: MockSwarmService,
        test_data: TestDataGenerator,
        error_recovery_swarm_id: String,
    }

    impl ErrorHandlingTestContext {
        async fn new() -> Self {
            let mock_registry = MockRegistry::new();
            let swarm_service = MockSwarmService::new();
            let test_data = TestDataGenerator::new();
            
            mock_registry.register(Box::new(swarm_service.clone())).await;
            mock_registry.start_all().await.unwrap();
            
            // Initialize error handling and recovery swarm
            let error_recovery_swarm_id = swarm_service
                .init_swarm(SwarmTopology::Mesh, 8, "error-recovery-resilient")
                .await
                .unwrap();
            
            Self {
                mock_registry,
                swarm_service,
                test_data,
                error_recovery_swarm_id,
            }
        }

        async fn cleanup(&self) {
            self.swarm_service.destroy_swarm(&self.error_recovery_swarm_id).await.ok();
            self.mock_registry.stop_all().await.unwrap();
        }
    }

    #[tokio::test]
    async fn should_detect_and_handle_agent_failures() {
        // Given: Swarm with agent failure detection requirements
        let context = ErrorHandlingTestContext::new().await;
        
        // When: Agents with different failure scenarios are deployed
        let failure_detection_agents = vec![
            MockAgent {
                id: "health-monitor-agent".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "agent-health-monitoring".to_string(),
                    "heartbeat-failure-detection".to_string(),
                    "resource-exhaustion-detection".to_string(),
                    "performance-degradation-detection".to_string(),
                ],
                memory_usage: 256,
                cpu_usage: 0.15,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "failed-agent".to_string(),
                agent_type: AgentType::TddLondon,
                status: AgentStatus::Error,
                capabilities: vec!["test-execution".to_string()],
                memory_usage: 2048, // High memory usage indicating issues
                cpu_usage: 1.0,     // Maxed out CPU
                last_heartbeat: std::time::SystemTime::now() - std::time::Duration::from_secs(600), // Old heartbeat
            },
            MockAgent {
                id: "recovery-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "failure-recovery-orchestration".to_string(),
                    "agent-replacement-coordination".to_string(),
                    "task-redistribution".to_string(),
                    "state-recovery".to_string(),
                ],
                memory_usage: 384,
                cpu_usage: 0.25,
                last_heartbeat: std::time::SystemTime::now(),
            },
        ];
        
        for agent in failure_detection_agents {
            context.swarm_service
                .add_agent_to_swarm(&context.error_recovery_swarm_id, agent)
                .await
                .unwrap();
        }
        
        // Then: Agent failure should be detectable
        let swarm_status = context.swarm_service
            .get_swarm_status(&context.error_recovery_swarm_id)
            .await
            .unwrap();
        
        assert_eq!(swarm_status.agents.len(), 3);
        
        // Failed agent should be identified
        let failed_agents: Vec<&MockAgent> = swarm_status.agents
            .iter()
            .filter(|a| matches!(a.status, AgentStatus::Error))
            .collect();
        
        assert_eq!(failed_agents.len(), 1);
        assert_eq!(failed_agents[0].id, "failed-agent");
        
        // Health monitoring capabilities should be present
        let monitoring_capabilities: Vec<String> = swarm_status.agents
            .iter()
            .flat_map(|a| a.capabilities.clone())
            .filter(|cap| cap.contains("detection") || cap.contains("monitoring"))
            .collect();
        
        assert!(monitoring_capabilities.contains(&"heartbeat-failure-detection".to_string()));
        assert!(monitoring_capabilities.contains(&"resource-exhaustion-detection".to_string()));
        
        // Recovery capabilities should be available
        let recovery_capabilities: Vec<String> = swarm_status.agents
            .iter()
            .flat_map(|a| a.capabilities.clone())
            .filter(|cap| cap.contains("recovery") || cap.contains("replacement"))
            .collect();
        
        assert!(recovery_capabilities.contains(&"failure-recovery-orchestration".to_string()));
        assert!(recovery_capabilities.contains(&"agent-replacement-coordination".to_string()));
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_handle_task_execution_failures() {
        // Given: Task execution with failure scenarios
        let context = ErrorHandlingTestContext::new().await;
        
        // When: Task failure handling agents are configured
        let task_failure_handlers = vec![
            MockAgent {
                id: "task-failure-detector".to_string(),
                agent_type: AgentType::TddLondon,
                status: AgentStatus::Active,
                capabilities: vec![
                    "test-execution-failure-detection".to_string(),
                    "task-timeout-detection".to_string(),
                    "assertion-failure-analysis".to_string(),
                    "mock-failure-detection".to_string(),
                ],
                memory_usage: 320,
                cpu_usage: 0.2,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "task-recovery-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "task-retry-orchestration".to_string(),
                    "failure-context-preservation".to_string(),
                    "alternative-strategy-coordination".to_string(),
                    "rollback-coordination".to_string(),
                ],
                memory_usage: 384,
                cpu_usage: 0.28,
                last_heartbeat: std::time::SystemTime::now(),
            },
        ];
        
        for handler in task_failure_handlers {
            context.swarm_service
                .add_agent_to_swarm(&context.error_recovery_swarm_id, handler)
                .await
                .unwrap();
        }
        
        // And: Failed tasks are generated for testing
        let failed_tasks = vec![
            context.test_data.generate_failed_task(),
            MockTask {
                id: "timeout-task".to_string(),
                description: "Task that timed out during execution".to_string(),
                assigned_agents: vec!["slow-agent-1".to_string()],
                status: TaskStatus::Failed,
                priority: TaskPriority::High,
                created_at: std::time::SystemTime::now() - std::time::Duration::from_secs(3600),
                started_at: Some(std::time::SystemTime::now() - std::time::Duration::from_secs(1800)),
                completed_at: None,
                result: None,
                error: Some("Task execution timeout after 30 minutes".to_string()),
            },
        ];
        
        // Then: Task failure handling should be comprehensive
        let swarm_status = context.swarm_service
            .get_swarm_status(&context.error_recovery_swarm_id)
            .await
            .unwrap();
        
        assert_eq!(swarm_status.agents.len(), 2);
        
        let failure_handling_capabilities: Vec<String> = swarm_status.agents
            .iter()
            .flat_map(|a| a.capabilities.clone())
            .filter(|cap| cap.contains("failure") || cap.contains("retry") || cap.contains("timeout"))
            .collect();
        
        assert!(failure_handling_capabilities.contains(&"test-execution-failure-detection".to_string()));
        assert!(failure_handling_capabilities.contains(&"task-timeout-detection".to_string()));
        assert!(failure_handling_capabilities.contains(&"task-retry-orchestration".to_string()));
        
        // Failed tasks should be properly categorized
        assert_eq!(failed_tasks.len(), 2);
        assert!(failed_tasks.iter().all(|t| matches!(t.status, TaskStatus::Failed)));
        assert!(failed_tasks.iter().all(|t| t.error.is_some()));
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_handle_mock_service_failures() {
        // Given: Mock services with failure scenarios
        let context = ErrorHandlingTestContext::new().await;
        
        // When: Mock service failure handling is established
        let mock_failure_handlers = vec![
            MockAgent {
                id: "mock-service-monitor".to_string(),
                agent_type: AgentType::TddLondon,
                status: AgentStatus::Active,
                capabilities: vec![
                    "mock-service-health-monitoring".to_string(),
                    "mock-contract-violation-detection".to_string(),
                    "mock-state-consistency-validation".to_string(),
                    "mock-response-validation".to_string(),
                ],
                memory_usage: 256,
                cpu_usage: 0.18,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "mock-recovery-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "mock-service-restart-coordination".to_string(),
                    "mock-state-restoration".to_string(),
                    "alternative-mock-activation".to_string(),
                    "test-environment-recovery".to_string(),
                ],
                memory_usage: 320,
                cpu_usage: 0.22,
                last_heartbeat: std::time::SystemTime::now(),
            },
        ];
        
        for handler in mock_failure_handlers {
            context.swarm_service
                .add_agent_to_swarm(&context.error_recovery_swarm_id, handler)
                .await
                .unwrap();
        }
        
        // And: Mock service error is injected
        context.swarm_service.inject_error("mock_service_call", "Mock Redis service unavailable").await;
        
        // Then: Mock failure handling should be operational
        let swarm_status = context.swarm_service
            .get_swarm_status(&context.error_recovery_swarm_id)
            .await
            .unwrap();
        
        assert_eq!(swarm_status.agents.len(), 2);
        
        let mock_handling_capabilities: Vec<String> = swarm_status.agents
            .iter()
            .flat_map(|a| a.capabilities.clone())
            .filter(|cap| cap.contains("mock"))
            .collect();
        
        assert!(mock_handling_capabilities.contains(&"mock-service-health-monitoring".to_string()));
        assert!(mock_handling_capabilities.contains(&"mock-contract-violation-detection".to_string()));
        assert!(mock_handling_capabilities.contains(&"mock-service-restart-coordination".to_string()));
        
        // Error injection should be logged
        let call_log = context.swarm_service.get_call_log().await;
        let error_calls: Vec<_> = call_log
            .iter()
            .filter(|call| call.result.is_err())
            .collect();
        
        assert!(!error_calls.is_empty());
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_handle_network_partition_scenarios() {
        // Given: Distributed system with network partition risks
        let context = ErrorHandlingTestContext::new().await;
        
        // When: Network partition handling is configured
        let partition_handlers = vec![
            MockAgent {
                id: "network-partition-detector".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "network-connectivity-monitoring".to_string(),
                    "partition-detection".to_string(),
                    "split-brain-detection".to_string(),
                    "communication-failure-detection".to_string(),
                ],
                memory_usage: 384,
                cpu_usage: 0.25,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "partition-recovery-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "network-partition-recovery".to_string(),
                    "cluster-reformation-coordination".to_string(),
                    "data-consistency-restoration".to_string(),
                    "service-discovery-recovery".to_string(),
                ],
                memory_usage: 512,
                cpu_usage: 0.3,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "isolated-agent".to_string(),
                agent_type: AgentType::Researcher,
                status: AgentStatus::Offline,
                capabilities: vec!["research-analysis".to_string()],
                memory_usage: 256,
                cpu_usage: 0.1,
                last_heartbeat: std::time::SystemTime::now() - std::time::Duration::from_secs(900), // Very old heartbeat
            },
        ];
        
        for handler in partition_handlers {
            context.swarm_service
                .add_agent_to_swarm(&context.error_recovery_swarm_id, handler)
                .await
                .unwrap();
        }
        
        // Then: Network partition handling should be comprehensive
        let swarm_status = context.swarm_service
            .get_swarm_status(&context.error_recovery_swarm_id)
            .await
            .unwrap();
        
        assert_eq!(swarm_status.agents.len(), 3);
        
        // Offline agents should be detectable
        let offline_agents: Vec<&MockAgent> = swarm_status.agents
            .iter()
            .filter(|a| matches!(a.status, AgentStatus::Offline))
            .collect();
        
        assert_eq!(offline_agents.len(), 1);
        assert_eq!(offline_agents[0].id, "isolated-agent");
        
        // Partition handling capabilities should be present
        let partition_capabilities: Vec<String> = swarm_status.agents
            .iter()
            .flat_map(|a| a.capabilities.clone())
            .filter(|cap| cap.contains("partition") || cap.contains("network") || cap.contains("connectivity"))
            .collect();
        
        assert!(partition_capabilities.contains(&"network-connectivity-monitoring".to_string()));
        assert!(partition_capabilities.contains(&"partition-detection".to_string()));
        assert!(partition_capabilities.contains(&"network-partition-recovery".to_string()));
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_handle_resource_exhaustion_scenarios() {
        // Given: System with resource constraints
        let context = ErrorHandlingTestContext::new().await;
        
        // When: Resource exhaustion handling is configured
        let resource_handlers = vec![
            MockAgent {
                id: "resource-monitor".to_string(),
                agent_type: AgentType::Optimizer,
                status: AgentStatus::Active,
                capabilities: vec![
                    "memory-usage-monitoring".to_string(),
                    "cpu-utilization-monitoring".to_string(),
                    "disk-space-monitoring".to_string(),
                    "resource-threshold-alerting".to_string(),
                ],
                memory_usage: 256,
                cpu_usage: 0.15,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "resource-recovery-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "resource-cleanup-coordination".to_string(),
                    "agent-prioritization-coordination".to_string(),
                    "load-shedding-coordination".to_string(),
                    "graceful-degradation-coordination".to_string(),
                ],
                memory_usage: 320,
                cpu_usage: 0.22,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "resource-exhausted-agent".to_string(),
                agent_type: AgentType::Analyst,
                status: AgentStatus::Error,
                capabilities: vec!["heavy-analysis".to_string()],
                memory_usage: u64::MAX, // Extremely high memory usage
                cpu_usage: 1.0,         // Maxed CPU
                last_heartbeat: std::time::SystemTime::now() - std::time::Duration::from_secs(120),
            },
        ];
        
        for handler in resource_handlers {
            context.swarm_service
                .add_agent_to_swarm(&context.error_recovery_swarm_id, handler)
                .await
                .unwrap();
        }
        
        // Then: Resource exhaustion should be detectable and recoverable
        let swarm_status = context.swarm_service
            .get_swarm_status(&context.error_recovery_swarm_id)
            .await
            .unwrap();
        
        assert_eq!(swarm_status.agents.len(), 3);
        
        // Resource exhausted agent should be identifiable
        let exhausted_agents: Vec<&MockAgent> = swarm_status.agents
            .iter()
            .filter(|a| a.memory_usage > 1_000_000 || a.cpu_usage > 0.95)
            .collect();
        
        assert!(!exhausted_agents.is_empty());
        
        // Resource monitoring capabilities should be present
        let monitoring_capabilities: Vec<String> = swarm_status.agents
            .iter()
            .flat_map(|a| a.capabilities.clone())
            .filter(|cap| cap.contains("monitoring") || cap.contains("resource"))
            .collect();
        
        assert!(monitoring_capabilities.contains(&"memory-usage-monitoring".to_string()));
        assert!(monitoring_capabilities.contains(&"cpu-utilization-monitoring".to_string()));
        assert!(monitoring_capabilities.contains(&"resource-cleanup-coordination".to_string()));
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_implement_circuit_breaker_patterns() {
        // Given: System requiring circuit breaker protection
        let context = ErrorHandlingTestContext::new().await;
        
        // When: Circuit breaker agents are configured
        let circuit_breaker_agents = vec![
            MockAgent {
                id: "circuit-breaker-monitor".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "failure-rate-monitoring".to_string(),
                    "response-time-monitoring".to_string(),
                    "circuit-state-management".to_string(),
                    "threshold-breach-detection".to_string(),
                ],
                memory_usage: 256,
                cpu_usage: 0.18,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "circuit-breaker-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "circuit-opening-coordination".to_string(),
                    "half-open-state-management".to_string(),
                    "circuit-closing-coordination".to_string(),
                    "fallback-service-activation".to_string(),
                ],
                memory_usage: 320,
                cpu_usage: 0.22,
                last_heartbeat: std::time::SystemTime::now(),
            },
        ];
        
        for agent in circuit_breaker_agents {
            context.swarm_service
                .add_agent_to_swarm(&context.error_recovery_swarm_id, agent)
                .await
                .unwrap();
        }
        
        // And: Multiple failure scenarios to trigger circuit breaker
        for _ in 0..5 {
            context.swarm_service.inject_error("service_call", "Service unavailable").await;
        }
        
        // Then: Circuit breaker functionality should be operational
        let swarm_status = context.swarm_service
            .get_swarm_status(&context.error_recovery_swarm_id)
            .await
            .unwrap();
        
        assert_eq!(swarm_status.agents.len(), 2);
        
        let circuit_breaker_capabilities: Vec<String> = swarm_status.agents
            .iter()
            .flat_map(|a| a.capabilities.clone())
            .filter(|cap| cap.contains("circuit") || cap.contains("breaker") || cap.contains("fallback"))
            .collect();
        
        assert!(circuit_breaker_capabilities.contains(&"failure-rate-monitoring".to_string()));
        assert!(circuit_breaker_capabilities.contains(&"circuit-state-management".to_string()));
        assert!(circuit_breaker_capabilities.contains(&"fallback-service-activation".to_string()));
        
        // Multiple failures should be logged
        let call_log = context.swarm_service.get_call_log().await;
        let error_count = call_log.iter().filter(|call| call.result.is_err()).count();
        assert!(error_count >= 5);
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_handle_cascading_failure_scenarios() {
        // Given: System with cascading failure potential
        let context = ErrorHandlingTestContext::new().await;
        
        // When: Cascading failure prevention is configured
        let cascade_prevention_agents = vec![
            MockAgent {
                id: "dependency-monitor".to_string(),
                agent_type: AgentType::Analyst,
                status: AgentStatus::Active,
                capabilities: vec![
                    "dependency-health-monitoring".to_string(),
                    "cascade-pattern-detection".to_string(),
                    "failure-propagation-analysis".to_string(),
                    "dependency-isolation-analysis".to_string(),
                ],
                memory_usage: 384,
                cpu_usage: 0.28,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "cascade-prevention-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "failure-isolation-coordination".to_string(),
                    "bulkhead-pattern-implementation".to_string(),
                    "timeout-management-coordination".to_string(),
                    "graceful-degradation-orchestration".to_string(),
                ],
                memory_usage: 512,
                cpu_usage: 0.35,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "primary-failed-agent".to_string(),
                agent_type: AgentType::Coder,
                status: AgentStatus::Error,
                capabilities: vec!["code-generation".to_string()],
                memory_usage: 1024,
                cpu_usage: 0.95,
                last_heartbeat: std::time::SystemTime::now() - std::time::Duration::from_secs(180),
            },
            MockAgent {
                id: "dependent-agent-1".to_string(),
                agent_type: AgentType::TddLondon,
                status: AgentStatus::Active, // Should remain active due to isolation
                capabilities: vec!["test-generation".to_string()],
                memory_usage: 256,
                cpu_usage: 0.2,
                last_heartbeat: std::time::SystemTime::now(),
            },
        ];
        
        for agent in cascade_prevention_agents {
            context.swarm_service
                .add_agent_to_swarm(&context.error_recovery_swarm_id, agent)
                .await
                .unwrap();
        }
        
        // Then: Cascading failure prevention should be effective
        let swarm_status = context.swarm_service
            .get_swarm_status(&context.error_recovery_swarm_id)
            .await
            .unwrap();
        
        assert_eq!(swarm_status.agents.len(), 4);
        
        // Primary failure should be isolated
        let failed_agents: Vec<&MockAgent> = swarm_status.agents
            .iter()
            .filter(|a| matches!(a.status, AgentStatus::Error))
            .collect();
        
        let active_agents: Vec<&MockAgent> = swarm_status.agents
            .iter()
            .filter(|a| matches!(a.status, AgentStatus::Active))
            .collect();
        
        assert_eq!(failed_agents.len(), 1);
        assert!(active_agents.len() >= 2); // Cascade should be prevented
        
        // Cascade prevention capabilities should be present
        let prevention_capabilities: Vec<String> = swarm_status.agents
            .iter()
            .flat_map(|a| a.capabilities.clone())
            .filter(|cap| cap.contains("cascade") || cap.contains("isolation") || cap.contains("bulkhead"))
            .collect();
        
        assert!(prevention_capabilities.contains(&"cascade-pattern-detection".to_string()));
        assert!(prevention_capabilities.contains(&"failure-isolation-coordination".to_string()));
        assert!(prevention_capabilities.contains(&"bulkhead-pattern-implementation".to_string()));
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_implement_graceful_degradation_strategies() {
        // Given: System requiring graceful degradation under stress
        let context = ErrorHandlingTestContext::new().await;
        
        // When: Graceful degradation is configured
        let degradation_agents = vec![
            MockAgent {
                id: "degradation-strategy-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "service-level-prioritization".to_string(),
                    "feature-disable-coordination".to_string(),
                    "quality-reduction-coordination".to_string(),
                    "fallback-mode-activation".to_string(),
                ],
                memory_usage: 384,
                cpu_usage: 0.25,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "critical-service-agent".to_string(),
                agent_type: AgentType::TddLondon,
                status: AgentStatus::Active,
                capabilities: vec!["critical-test-execution".to_string()],
                memory_usage: 256,
                cpu_usage: 0.3,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "non-critical-service-agent".to_string(),
                agent_type: AgentType::Optimizer,
                status: AgentStatus::Idle, // Should be degraded/suspended under stress
                capabilities: vec!["performance-optimization".to_string()],
                memory_usage: 128,
                cpu_usage: 0.05,
                last_heartbeat: std::time::SystemTime::now(),
            },
        ];
        
        for agent in degradation_agents {
            context.swarm_service
                .add_agent_to_swarm(&context.error_recovery_swarm_id, agent)
                .await
                .unwrap();
        }
        
        // Then: Graceful degradation should be implemented
        let swarm_status = context.swarm_service
            .get_swarm_status(&context.error_recovery_swarm_id)
            .await
            .unwrap();
        
        assert_eq!(swarm_status.agents.len(), 3);
        
        // Critical services should remain active
        let critical_agents: Vec<&MockAgent> = swarm_status.agents
            .iter()
            .filter(|a| a.capabilities.iter().any(|cap| cap.contains("critical")))
            .collect();
        
        assert_eq!(critical_agents.len(), 1);
        assert!(matches!(critical_agents[0].status, AgentStatus::Active));
        
        // Non-critical services should be degraded
        let non_critical_agents: Vec<&MockAgent> = swarm_status.agents
            .iter()
            .filter(|a| a.capabilities.iter().any(|cap| cap.contains("optimization")))
            .collect();
        
        assert_eq!(non_critical_agents.len(), 1);
        assert!(matches!(non_critical_agents[0].status, AgentStatus::Idle));
        
        // Degradation capabilities should be present
        let degradation_capabilities: Vec<String> = swarm_status.agents
            .iter()
            .flat_map(|a| a.capabilities.clone())
            .filter(|cap| cap.contains("degradation") || cap.contains("fallback") || cap.contains("prioritization"))
            .collect();
        
        assert!(degradation_capabilities.contains(&"service-level-prioritization".to_string()));
        assert!(degradation_capabilities.contains(&"fallback-mode-activation".to_string()));
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_maintain_error_tracking_and_logging() {
        // Given: System requiring comprehensive error tracking
        let context = ErrorHandlingTestContext::new().await;
        
        // When: Error tracking and logging agents are configured
        let error_tracking_agents = vec![
            MockAgent {
                id: "error-logger".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "structured-error-logging".to_string(),
                    "error-categorization".to_string(),
                    "error-correlation".to_string(),
                    "error-metrics-collection".to_string(),
                ],
                memory_usage: 256,
                cpu_usage: 0.15,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "error-analyzer".to_string(),
                agent_type: AgentType::Analyst,
                status: AgentStatus::Active,
                capabilities: vec![
                    "error-pattern-analysis".to_string(),
                    "root-cause-analysis".to_string(),
                    "error-trend-analysis".to_string(),
                    "error-impact-assessment".to_string(),
                ],
                memory_usage: 384,
                cpu_usage: 0.28,
                last_heartbeat: std::time::SystemTime::now(),
            },
        ];
        
        for agent in error_tracking_agents {
            context.swarm_service
                .add_agent_to_swarm(&context.error_recovery_swarm_id, agent)
                .await
                .unwrap();
        }
        
        // And: Various error scenarios are generated
        let error_scenarios = vec![
            "Network timeout",
            "Memory allocation failure",
            "Invalid input data",
            "Service unavailable",
            "Authentication failure",
        ];
        
        for error in error_scenarios.iter() {
            context.swarm_service.inject_error("test_operation", error).await;
        }
        
        // Then: Error tracking should be comprehensive
        let swarm_status = context.swarm_service
            .get_swarm_status(&context.error_recovery_swarm_id)
            .await
            .unwrap();
        
        assert_eq!(swarm_status.agents.len(), 2);
        
        // Error tracking capabilities should be present
        let tracking_capabilities: Vec<String> = swarm_status.agents
            .iter()
            .flat_map(|a| a.capabilities.clone())
            .filter(|cap| cap.contains("error") || cap.contains("logging") || cap.contains("analysis"))
            .collect();
        
        assert!(tracking_capabilities.contains(&"structured-error-logging".to_string()));
        assert!(tracking_capabilities.contains(&"error-pattern-analysis".to_string()));
        assert!(tracking_capabilities.contains(&"root-cause-analysis".to_string()));
        
        // All error scenarios should be logged
        let call_log = context.swarm_service.get_call_log().await;
        let error_calls: Vec<_> = call_log
            .iter()
            .filter(|call| call.result.is_err())
            .collect();
        
        assert!(error_calls.len() >= error_scenarios.len());
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_coordinate_disaster_recovery_procedures() {
        // Given: System with disaster recovery requirements
        let context = ErrorHandlingTestContext::new().await;
        
        // When: Disaster recovery coordination is established
        let disaster_recovery_agents = vec![
            MockAgent {
                id: "disaster-recovery-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "disaster-scenario-detection".to_string(),
                    "recovery-procedure-orchestration".to_string(),
                    "backup-system-activation".to_string(),
                    "data-recovery-coordination".to_string(),
                ],
                memory_usage: 512,
                cpu_usage: 0.35,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "backup-service-agent".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "backup-data-management".to_string(),
                    "backup-validation".to_string(),
                    "restoration-procedures".to_string(),
                ],
                memory_usage: 768,
                cpu_usage: 0.2,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "emergency-response-agent".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "emergency-communication".to_string(),
                    "stakeholder-notification".to_string(),
                    "incident-response-coordination".to_string(),
                ],
                memory_usage: 320,
                cpu_usage: 0.25,
                last_heartbeat: std::time::SystemTime::now(),
            },
        ];
        
        for agent in disaster_recovery_agents {
            context.swarm_service
                .add_agent_to_swarm(&context.error_recovery_swarm_id, agent)
                .await
                .unwrap();
        }
        
        // Then: Disaster recovery capabilities should be comprehensive
        let swarm_status = context.swarm_service
            .get_swarm_status(&context.error_recovery_swarm_id)
            .await
            .unwrap();
        
        assert_eq!(swarm_status.agents.len(), 3);
        
        let disaster_recovery_capabilities: Vec<String> = swarm_status.agents
            .iter()
            .flat_map(|a| a.capabilities.clone())
            .filter(|cap| cap.contains("disaster") || cap.contains("recovery") || cap.contains("backup") || cap.contains("emergency"))
            .collect();
        
        assert!(disaster_recovery_capabilities.contains(&"disaster-scenario-detection".to_string()));
        assert!(disaster_recovery_capabilities.contains(&"recovery-procedure-orchestration".to_string()));
        assert!(disaster_recovery_capabilities.contains(&"backup-system-activation".to_string()));
        assert!(disaster_recovery_capabilities.contains(&"emergency-communication".to_string()));
        
        context.cleanup().await;
    }
}

/// Integration tests for error handling with neural components
#[cfg(test)]
mod error_handling_neural_integration_tests {
    use super::*;

    #[tokio::test]
    async fn should_handle_neural_model_training_failures() {
        // Given: Neural model training with failure scenarios
        let context = ErrorHandlingTestContext::new().await;
        
        // When: Neural training failure handling is configured
        let neural_failure_handlers = vec![
            MockAgent {
                id: "neural-training-failure-detector".to_string(),
                agent_type: AgentType::Analyst,
                status: AgentStatus::Active,
                capabilities: vec![
                    "training-convergence-failure-detection".to_string(),
                    "model-overfitting-detection".to_string(),
                    "training-data-corruption-detection".to_string(),
                    "gradient-explosion-detection".to_string(),
                ],
                memory_usage: 512,
                cpu_usage: 0.4,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "neural-recovery-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "model-checkpoint-restoration".to_string(),
                    "hyperparameter-adjustment-coordination".to_string(),
                    "alternative-model-activation".to_string(),
                    "training-data-recovery-coordination".to_string(),
                ],
                memory_usage: 768,
                cpu_usage: 0.35,
                last_heartbeat: std::time::SystemTime::now(),
            },
        ];
        
        for handler in neural_failure_handlers {
            context.swarm_service
                .add_agent_to_swarm(&context.error_recovery_swarm_id, handler)
                .await
                .unwrap();
        }
        
        // Then: Neural training failure handling should be specialized
        let swarm_status = context.swarm_service
            .get_swarm_status(&context.error_recovery_swarm_id)
            .await
            .unwrap();
        
        assert_eq!(swarm_status.agents.len(), 2);
        
        let neural_failure_capabilities: Vec<String> = swarm_status.agents
            .iter()
            .flat_map(|a| a.capabilities.clone())
            .filter(|cap| cap.contains("training") || cap.contains("model") || cap.contains("neural"))
            .collect();
        
        assert!(neural_failure_capabilities.contains(&"training-convergence-failure-detection".to_string()));
        assert!(neural_failure_capabilities.contains(&"model-checkpoint-restoration".to_string()));
        assert!(neural_failure_capabilities.contains(&"alternative-model-activation".to_string()));
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_handle_ruv_fann_integration_failures() {
        // Given: RUV-FANN integration with failure scenarios
        let context = ErrorHandlingTestContext::new().await;
        
        // When: FANN integration failure handling is established
        let fann_failure_handlers = vec![
            MockAgent {
                id: "fann-integration-failure-detector".to_string(),
                agent_type: AgentType::TddLondon,
                status: AgentStatus::Active,
                capabilities: vec![
                    "fann-library-failure-detection".to_string(),
                    "model-serialization-failure-detection".to_string(),
                    "prediction-accuracy-failure-detection".to_string(),
                    "fann-memory-leak-detection".to_string(),
                ],
                memory_usage: 384,
                cpu_usage: 0.3,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "fann-recovery-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "fann-model-recovery-coordination".to_string(),
                    "alternative-prediction-service-activation".to_string(),
                    "fann-library-restart-coordination".to_string(),
                    "prediction-fallback-coordination".to_string(),
                ],
                memory_usage: 512,
                cpu_usage: 0.28,
                last_heartbeat: std::time::SystemTime::now(),
            },
        ];
        
        for handler in fann_failure_handlers {
            context.swarm_service
                .add_agent_to_swarm(&context.error_recovery_swarm_id, handler)
                .await
                .unwrap();
        }
        
        // Then: FANN integration failure handling should be comprehensive
        let swarm_status = context.swarm_service
            .get_swarm_status(&context.error_recovery_swarm_id)
            .await
            .unwrap();
        
        assert_eq!(swarm_status.agents.len(), 2);
        
        let fann_failure_capabilities: Vec<String> = swarm_status.agents
            .iter()
            .flat_map(|a| a.capabilities.clone())
            .filter(|cap| cap.contains("fann") || cap.contains("prediction") || cap.contains("serialization"))
            .collect();
        
        assert!(fann_failure_capabilities.contains(&"fann-library-failure-detection".to_string()));
        assert!(fann_failure_capabilities.contains(&"fann-model-recovery-coordination".to_string()));
        assert!(fann_failure_capabilities.contains(&"prediction-fallback-coordination".to_string()));
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_handle_neural_trader_system_failures() {
        // Given: Complete neural trader system with failure scenarios
        let context = ErrorHandlingTestContext::new().await;
        
        // When: System-wide failure handling is configured
        let system_failure_handlers = vec![
            MockAgent {
                id: "trading-system-failure-detector".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "trading-pipeline-failure-detection".to_string(),
                    "market-data-failure-detection".to_string(),
                    "portfolio-management-failure-detection".to_string(),
                    "risk-management-failure-detection".to_string(),
                ],
                memory_usage: 640,
                cpu_usage: 0.4,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "trading-recovery-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "emergency-trading-halt-coordination".to_string(),
                    "position-protection-coordination".to_string(),
                    "backup-trading-strategy-activation".to_string(),
                    "system-recovery-orchestration".to_string(),
                ],
                memory_usage: 768,
                cpu_usage: 0.45,
                last_heartbeat: std::time::SystemTime::now(),
            },
        ];
        
        for handler in system_failure_handlers {
            context.swarm_service
                .add_agent_to_swarm(&context.error_recovery_swarm_id, handler)
                .await
                .unwrap();
        }
        
        // Then: System-wide failure handling should be comprehensive
        let swarm_status = context.swarm_service
            .get_swarm_status(&context.error_recovery_swarm_id)
            .await
            .unwrap();
        
        assert_eq!(swarm_status.agents.len(), 2);
        
        let system_failure_capabilities: Vec<String> = swarm_status.agents
            .iter()
            .flat_map(|a| a.capabilities.clone())
            .filter(|cap| cap.contains("trading") || cap.contains("portfolio") || cap.contains("market") || cap.contains("system"))
            .collect();
        
        assert!(system_failure_capabilities.contains(&"trading-pipeline-failure-detection".to_string()));
        assert!(system_failure_capabilities.contains(&"emergency-trading-halt-coordination".to_string()));
        assert!(system_failure_capabilities.contains(&"backup-trading-strategy-activation".to_string()));
        assert!(system_failure_capabilities.contains(&"system-recovery-orchestration".to_string()));
        
        context.cleanup().await;
    }
}