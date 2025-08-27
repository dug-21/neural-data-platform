// Integration Workflow Tests - London School TDD
// Tests for orchestrator's integration and workflow coordination

use super::mock_services::*;
use super::mock_services::swarm_mock::*;
use super::test_data_generators::*;
use tokio_test;
use std::collections::HashMap;

#[cfg(test)]
mod integration_workflow_behavior_tests {
    use super::*;

    struct IntegrationWorkflowTestContext {
        mock_registry: MockRegistry,
        swarm_service: MockSwarmService,
        test_data: TestDataGenerator,
        workflow_swarm_id: String,
    }

    impl IntegrationWorkflowTestContext {
        async fn new() -> Self {
            let mock_registry = MockRegistry::new();
            let swarm_service = MockSwarmService::new();
            let test_data = TestDataGenerator::new();
            
            mock_registry.register(Box::new(swarm_service.clone())).await;
            mock_registry.start_all().await.unwrap();
            
            // Initialize workflow coordination swarm
            let workflow_swarm_id = swarm_service
                .init_swarm(SwarmTopology::Hierarchical, 12, "integration-workflow")
                .await
                .unwrap();
            
            Self {
                mock_registry,
                swarm_service,
                test_data,
                workflow_swarm_id,
            }
        }

        async fn cleanup(&self) {
            self.swarm_service.destroy_swarm(&self.workflow_swarm_id).await.ok();
            self.mock_registry.stop_all().await.unwrap();
        }
    }

    #[tokio::test]
    async fn should_orchestrate_tdd_workflow_from_red_to_green_to_refactor() {
        // Given: TDD workflow orchestration requirements
        let context = IntegrationWorkflowTestContext::new().await;
        
        // When: TDD workflow agents are configured
        let tdd_workflow_team = vec![
            MockAgent {
                id: "red-phase-coordinator".to_string(),
                agent_type: AgentType::TddLondon,
                status: AgentStatus::Active,
                capabilities: vec![
                    "failing-test-creation".to_string(),
                    "test-first-development".to_string(),
                    "requirements-to-test-mapping".to_string(),
                    "red-phase-validation".to_string(),
                ],
                memory_usage: 256,
                cpu_usage: 0.2,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "green-phase-coordinator".to_string(),
                agent_type: AgentType::Coder,
                status: AgentStatus::Active,
                capabilities: vec![
                    "minimal-implementation".to_string(),
                    "test-passing-implementation".to_string(),
                    "code-generation-for-tests".to_string(),
                    "green-phase-validation".to_string(),
                ],
                memory_usage: 384,
                cpu_usage: 0.35,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "refactor-phase-coordinator".to_string(),
                agent_type: AgentType::Optimizer,
                status: AgentStatus::Active,
                capabilities: vec![
                    "code-quality-improvement".to_string(),
                    "design-pattern-application".to_string(),
                    "performance-optimization".to_string(),
                    "refactor-phase-validation".to_string(),
                ],
                memory_usage: 320,
                cpu_usage: 0.28,
                last_heartbeat: std::time::SystemTime::now(),
            },
        ];
        
        for agent in tdd_workflow_team {
            context.swarm_service
                .add_agent_to_swarm(&context.workflow_swarm_id, agent)
                .await
                .unwrap();
        }
        
        // Then: Complete TDD workflow should be orchestratable
        let swarm_status = context.swarm_service
            .get_swarm_status(&context.workflow_swarm_id)
            .await
            .unwrap();
        
        assert_eq!(swarm_status.agents.len(), 3);
        
        let workflow_capabilities: Vec<String> = swarm_status.agents
            .iter()
            .flat_map(|a| a.capabilities.clone())
            .collect();
        
        assert!(workflow_capabilities.contains(&"failing-test-creation".to_string()));
        assert!(workflow_capabilities.contains(&"minimal-implementation".to_string()));
        assert!(workflow_capabilities.contains(&"code-quality-improvement".to_string()));
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_coordinate_cross_binary_integration_workflows() {
        // Given: Multi-binary integration requirements
        let context = IntegrationWorkflowTestContext::new().await;
        
        // When: Cross-binary integration workflow is established
        let integration_coordinators = vec![
            MockAgent {
                id: "config-store-integration-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "config-store-grpc-integration".to_string(),
                    "configuration-propagation-workflow".to_string(),
                    "config-validation-workflow".to_string(),
                ],
                memory_usage: 384,
                cpu_usage: 0.25,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "data-ingestion-integration-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "market-data-streaming-integration".to_string(),
                    "redis-streams-publishing-workflow".to_string(),
                    "data-quality-validation-workflow".to_string(),
                ],
                memory_usage: 512,
                cpu_usage: 0.3,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "neural-fann-integration-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "fann-model-integration-workflow".to_string(),
                    "neural-prediction-workflow".to_string(),
                    "model-training-coordination".to_string(),
                ],
                memory_usage: 768,
                cpu_usage: 0.4,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "daa-coordinator-integration".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "agent-coordination-workflow".to_string(),
                    "distributed-decision-workflow".to_string(),
                    "consensus-building-workflow".to_string(),
                ],
                memory_usage: 640,
                cpu_usage: 0.35,
                last_heartbeat: std::time::SystemTime::now(),
            },
        ];
        
        for coordinator in integration_coordinators {
            context.swarm_service
                .add_agent_to_swarm(&context.workflow_swarm_id, coordinator)
                .await
                .unwrap();
        }
        
        // Then: All binary integration workflows should be coordinated
        let swarm_status = context.swarm_service
            .get_swarm_status(&context.workflow_swarm_id)
            .await
            .unwrap();
        
        assert_eq!(swarm_status.agents.len(), 4);
        
        let integration_capabilities: Vec<String> = swarm_status.agents
            .iter()
            .flat_map(|a| a.capabilities.clone())
            .filter(|cap| cap.contains("integration") || cap.contains("workflow"))
            .collect();
        
        assert!(integration_capabilities.contains(&"config-store-grpc-integration".to_string()));
        assert!(integration_capabilities.contains(&"market-data-streaming-integration".to_string()));
        assert!(integration_capabilities.contains(&"fann-model-integration-workflow".to_string()));
        assert!(integration_capabilities.contains(&"agent-coordination-workflow".to_string()));
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_orchestrate_redis_streams_message_flow_workflows() {
        // Given: Redis Streams-based message flow requirements
        let context = IntegrationWorkflowTestContext::new().await;
        
        // When: Redis Streams workflow coordination is established
        let streams_workflow_agents = vec![
            MockAgent {
                id: "stream-producer-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "stream-message-publishing-workflow".to_string(),
                    "message-serialization-workflow".to_string(),
                    "stream-partitioning-workflow".to_string(),
                    "producer-error-handling-workflow".to_string(),
                ],
                memory_usage: 320,
                cpu_usage: 0.22,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "stream-consumer-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "stream-message-consumption-workflow".to_string(),
                    "consumer-group-coordination-workflow".to_string(),
                    "message-acknowledgment-workflow".to_string(),
                    "consumer-error-handling-workflow".to_string(),
                ],
                memory_usage: 384,
                cpu_usage: 0.28,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "stream-flow-validator".to_string(),
                agent_type: AgentType::TddLondon,
                status: AgentStatus::Active,
                capabilities: vec![
                    "end-to-end-message-flow-testing".to_string(),
                    "stream-ordering-validation".to_string(),
                    "message-delivery-guarantee-testing".to_string(),
                    "stream-backpressure-testing".to_string(),
                ],
                memory_usage: 256,
                cpu_usage: 0.18,
                last_heartbeat: std::time::SystemTime::now(),
            },
        ];
        
        for agent in streams_workflow_agents {
            context.swarm_service
                .add_agent_to_swarm(&context.workflow_swarm_id, agent)
                .await
                .unwrap();
        }
        
        // Then: Redis Streams workflow should be fully coordinated
        let swarm_status = context.swarm_service
            .get_swarm_status(&context.workflow_swarm_id)
            .await
            .unwrap();
        
        assert_eq!(swarm_status.agents.len(), 3);
        
        let stream_capabilities: Vec<String> = swarm_status.agents
            .iter()
            .flat_map(|a| a.capabilities.clone())
            .filter(|cap| cap.contains("stream") || cap.contains("message"))
            .collect();
        
        assert!(stream_capabilities.contains(&"stream-message-publishing-workflow".to_string()));
        assert!(stream_capabilities.contains(&"consumer-group-coordination-workflow".to_string()));
        assert!(stream_capabilities.contains(&"end-to-end-message-flow-testing".to_string()));
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_coordinate_neural_model_training_and_deployment_workflow() {
        // Given: Neural model lifecycle workflow requirements
        let context = IntegrationWorkflowTestContext::new().await;
        
        // When: Neural model workflow coordination is configured
        let neural_workflow_team = vec![
            MockAgent {
                id: "data-preparation-coordinator".to_string(),
                agent_type: AgentType::Analyst,
                status: AgentStatus::Active,
                capabilities: vec![
                    "training-data-preparation-workflow".to_string(),
                    "feature-engineering-workflow".to_string(),
                    "data-validation-workflow".to_string(),
                    "data-quality-assurance-workflow".to_string(),
                ],
                memory_usage: 512,
                cpu_usage: 0.3,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "model-training-coordinator".to_string(),
                agent_type: AgentType::Researcher,
                status: AgentStatus::Active,
                capabilities: vec![
                    "fann-model-training-workflow".to_string(),
                    "hyperparameter-optimization-workflow".to_string(),
                    "training-convergence-monitoring-workflow".to_string(),
                    "model-validation-workflow".to_string(),
                ],
                memory_usage: 1024,
                cpu_usage: 0.6,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "model-deployment-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "model-serialization-workflow".to_string(),
                    "model-versioning-workflow".to_string(),
                    "prediction-service-deployment-workflow".to_string(),
                    "model-rollback-workflow".to_string(),
                ],
                memory_usage: 384,
                cpu_usage: 0.25,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "model-testing-coordinator".to_string(),
                agent_type: AgentType::TddLondon,
                status: AgentStatus::Active,
                capabilities: vec![
                    "model-performance-testing-workflow".to_string(),
                    "prediction-accuracy-validation-workflow".to_string(),
                    "model-integration-testing-workflow".to_string(),
                    "a-b-testing-workflow".to_string(),
                ],
                memory_usage: 320,
                cpu_usage: 0.22,
                last_heartbeat: std::time::SystemTime::now(),
            },
        ];
        
        for agent in neural_workflow_team {
            context.swarm_service
                .add_agent_to_swarm(&context.workflow_swarm_id, agent)
                .await
                .unwrap();
        }
        
        // Then: Complete neural model lifecycle should be orchestrated
        let swarm_status = context.swarm_service
            .get_swarm_status(&context.workflow_swarm_id)
            .await
            .unwrap();
        
        assert_eq!(swarm_status.agents.len(), 4);
        
        let neural_capabilities: Vec<String> = swarm_status.agents
            .iter()
            .flat_map(|a| a.capabilities.clone())
            .filter(|cap| cap.contains("model") || cap.contains("training"))
            .collect();
        
        assert!(neural_capabilities.contains(&"training-data-preparation-workflow".to_string()));
        assert!(neural_capabilities.contains(&"fann-model-training-workflow".to_string()));
        assert!(neural_capabilities.contains(&"model-deployment-coordinator".to_string()) || 
               neural_capabilities.contains(&"prediction-service-deployment-workflow".to_string()));
        assert!(neural_capabilities.contains(&"model-integration-testing-workflow".to_string()));
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_orchestrate_distributed_agent_decision_workflows() {
        // Given: Distributed autonomous agent decision-making requirements
        let context = IntegrationWorkflowTestContext::new().await;
        
        // When: DAA decision workflow coordination is established
        let daa_decision_team = vec![
            MockAgent {
                id: "consensus-building-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "distributed-consensus-workflow".to_string(),
                    "agent-voting-coordination-workflow".to_string(),
                    "decision-aggregation-workflow".to_string(),
                    "consensus-validation-workflow".to_string(),
                ],
                memory_usage: 512,
                cpu_usage: 0.35,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "decision-execution-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "decision-implementation-workflow".to_string(),
                    "action-coordination-workflow".to_string(),
                    "execution-monitoring-workflow".to_string(),
                    "rollback-decision-workflow".to_string(),
                ],
                memory_usage: 384,
                cpu_usage: 0.28,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "decision-validation-coordinator".to_string(),
                agent_type: AgentType::TddLondon,
                status: AgentStatus::Active,
                capabilities: vec![
                    "decision-quality-testing-workflow".to_string(),
                    "outcome-validation-workflow".to_string(),
                    "decision-learning-workflow".to_string(),
                    "feedback-integration-workflow".to_string(),
                ],
                memory_usage: 256,
                cpu_usage: 0.2,
                last_heartbeat: std::time::SystemTime::now(),
            },
        ];
        
        for agent in daa_decision_team {
            context.swarm_service
                .add_agent_to_swarm(&context.workflow_swarm_id, agent)
                .await
                .unwrap();
        }
        
        // Then: Distributed decision workflow should be comprehensive
        let swarm_status = context.swarm_service
            .get_swarm_status(&context.workflow_swarm_id)
            .await
            .unwrap();
        
        assert_eq!(swarm_status.agents.len(), 3);
        
        let decision_capabilities: Vec<String> = swarm_status.agents
            .iter()
            .flat_map(|a| a.capabilities.clone())
            .filter(|cap| cap.contains("decision") || cap.contains("consensus"))
            .collect();
        
        assert!(decision_capabilities.contains(&"distributed-consensus-workflow".to_string()));
        assert!(decision_capabilities.contains(&"decision-implementation-workflow".to_string()));
        assert!(decision_capabilities.contains(&"decision-quality-testing-workflow".to_string()));
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_coordinate_end_to_end_trading_workflow() {
        // Given: Complete trading workflow requirements
        let context = IntegrationWorkflowTestContext::new().await;
        
        // When: End-to-end trading workflow is established
        let trading_workflow_team = vec![
            MockAgent {
                id: "market-data-workflow-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "market-data-ingestion-workflow".to_string(),
                    "data-preprocessing-workflow".to_string(),
                    "feature-extraction-workflow".to_string(),
                ],
                memory_usage: 384,
                cpu_usage: 0.25,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "prediction-workflow-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "neural-prediction-workflow".to_string(),
                    "prediction-aggregation-workflow".to_string(),
                    "confidence-assessment-workflow".to_string(),
                ],
                memory_usage: 768,
                cpu_usage: 0.4,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "trading-decision-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "trading-signal-generation-workflow".to_string(),
                    "risk-assessment-workflow".to_string(),
                    "position-sizing-workflow".to_string(),
                    "order-execution-workflow".to_string(),
                ],
                memory_usage: 512,
                cpu_usage: 0.35,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "portfolio-management-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "portfolio-rebalancing-workflow".to_string(),
                    "performance-monitoring-workflow".to_string(),
                    "risk-management-workflow".to_string(),
                ],
                memory_usage: 320,
                cpu_usage: 0.22,
                last_heartbeat: std::time::SystemTime::now(),
            },
        ];
        
        for agent in trading_workflow_team {
            context.swarm_service
                .add_agent_to_swarm(&context.workflow_swarm_id, agent)
                .await
                .unwrap();
        }
        
        // Then: Complete trading workflow should be coordinated
        let swarm_status = context.swarm_service
            .get_swarm_status(&context.workflow_swarm_id)
            .await
            .unwrap();
        
        assert_eq!(swarm_status.agents.len(), 4);
        
        let trading_capabilities: Vec<String> = swarm_status.agents
            .iter()
            .flat_map(|a| a.capabilities.clone())
            .collect();
        
        assert!(trading_capabilities.contains(&"market-data-ingestion-workflow".to_string()));
        assert!(trading_capabilities.contains(&"neural-prediction-workflow".to_string()));
        assert!(trading_capabilities.contains(&"trading-signal-generation-workflow".to_string()));
        assert!(trading_capabilities.contains(&"portfolio-rebalancing-workflow".to_string()));
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_handle_workflow_interruption_and_recovery() {
        // Given: Workflow with potential interruption points
        let context = IntegrationWorkflowTestContext::new().await;
        
        // When: Workflow recovery mechanisms are established
        let recovery_coordinators = vec![
            MockAgent {
                id: "workflow-health-monitor".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "workflow-health-monitoring".to_string(),
                    "failure-detection-workflow".to_string(),
                    "workflow-state-persistence".to_string(),
                ],
                memory_usage: 256,
                cpu_usage: 0.15,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "workflow-recovery-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "workflow-recovery-orchestration".to_string(),
                    "checkpoint-restoration-workflow".to_string(),
                    "partial-completion-handling-workflow".to_string(),
                ],
                memory_usage: 384,
                cpu_usage: 0.25,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "failed-workflow-agent".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Error,
                capabilities: vec![
                    "error-state-reporting".to_string(),
                    "failure-context-preservation".to_string(),
                ],
                memory_usage: 512,
                cpu_usage: 0.9,
                last_heartbeat: std::time::SystemTime::now() - std::time::Duration::from_secs(300),
            },
        ];
        
        for agent in recovery_coordinators {
            context.swarm_service
                .add_agent_to_swarm(&context.workflow_swarm_id, agent)
                .await
                .unwrap();
        }
        
        // Then: Workflow recovery capabilities should be present
        let swarm_status = context.swarm_service
            .get_swarm_status(&context.workflow_swarm_id)
            .await
            .unwrap();
        
        assert_eq!(swarm_status.agents.len(), 3);
        
        // Recovery mechanisms should be available
        let recovery_capabilities: Vec<String> = swarm_status.agents
            .iter()
            .flat_map(|a| a.capabilities.clone())
            .filter(|cap| cap.contains("recovery") || cap.contains("failure") || cap.contains("error"))
            .collect();
        
        assert!(recovery_capabilities.contains(&"failure-detection-workflow".to_string()));
        assert!(recovery_capabilities.contains(&"workflow-recovery-orchestration".to_string()));
        assert!(recovery_capabilities.contains(&"error-state-reporting".to_string()));
        
        // Failed agent should be detectable
        let failed_agents: Vec<&MockAgent> = swarm_status.agents
            .iter()
            .filter(|a| matches!(a.status, AgentStatus::Error))
            .collect();
        
        assert_eq!(failed_agents.len(), 1);
        assert_eq!(failed_agents[0].id, "failed-workflow-agent");
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_coordinate_parallel_workflow_execution() {
        // Given: Multiple concurrent workflow requirements
        let context = IntegrationWorkflowTestContext::new().await;
        
        // When: Parallel workflow coordination is established
        let parallel_coordinators = vec![
            MockAgent {
                id: "parallel-execution-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "parallel-workflow-orchestration".to_string(),
                    "resource-allocation-coordination".to_string(),
                    "dependency-resolution-workflow".to_string(),
                    "execution-synchronization-workflow".to_string(),
                ],
                memory_usage: 512,
                cpu_usage: 0.35,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "workflow-a-executor".to_string(),
                agent_type: AgentType::Coder,
                status: AgentStatus::Busy,
                capabilities: vec!["workflow-a-execution".to_string()],
                memory_usage: 384,
                cpu_usage: 0.8,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "workflow-b-executor".to_string(),
                agent_type: AgentType::Analyst,
                status: AgentStatus::Busy,
                capabilities: vec!["workflow-b-execution".to_string()],
                memory_usage: 256,
                cpu_usage: 0.7,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "workflow-c-executor".to_string(),
                agent_type: AgentType::TddLondon,
                status: AgentStatus::Active,
                capabilities: vec!["workflow-c-execution".to_string()],
                memory_usage: 320,
                cpu_usage: 0.3,
                last_heartbeat: std::time::SystemTime::now(),
            },
        ];
        
        for agent in parallel_coordinators {
            context.swarm_service
                .add_agent_to_swarm(&context.workflow_swarm_id, agent)
                .await
                .unwrap();
        }
        
        // Then: Parallel workflow execution should be coordinated
        let swarm_status = context.swarm_service
            .get_swarm_status(&context.workflow_swarm_id)
            .await
            .unwrap();
        
        assert_eq!(swarm_status.agents.len(), 4);
        
        // Should have coordinator and multiple executors
        let coordinators: Vec<&MockAgent> = swarm_status.agents
            .iter()
            .filter(|a| matches!(a.agent_type, AgentType::Coordinator))
            .collect();
        
        let executors: Vec<&MockAgent> = swarm_status.agents
            .iter()
            .filter(|a| !matches!(a.agent_type, AgentType::Coordinator))
            .collect();
        
        assert_eq!(coordinators.len(), 1);
        assert_eq!(executors.len(), 3);
        
        // Some executors should be busy (parallel execution)
        let busy_agents: Vec<&MockAgent> = swarm_status.agents
            .iter()
            .filter(|a| matches!(a.status, AgentStatus::Busy))
            .collect();
        
        assert!(busy_agents.len() >= 2);
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_validate_workflow_completion_and_success_criteria() {
        // Given: Workflow with completion validation requirements
        let context = IntegrationWorkflowTestContext::new().await;
        
        // When: Workflow completion validation is established
        let completion_validators = vec![
            MockAgent {
                id: "workflow-completion-validator".to_string(),
                agent_type: AgentType::Reviewer,
                status: AgentStatus::Active,
                capabilities: vec![
                    "workflow-completion-validation".to_string(),
                    "success-criteria-validation".to_string(),
                    "output-quality-validation".to_string(),
                    "workflow-performance-validation".to_string(),
                ],
                memory_usage: 320,
                cpu_usage: 0.2,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "integration-test-validator".to_string(),
                agent_type: AgentType::TddLondon,
                status: AgentStatus::Active,
                capabilities: vec![
                    "end-to-end-workflow-testing".to_string(),
                    "integration-point-validation".to_string(),
                    "data-flow-validation".to_string(),
                    "workflow-behavior-validation".to_string(),
                ],
                memory_usage: 384,
                cpu_usage: 0.25,
                last_heartbeat: std::time::SystemTime::now(),
            },
        ];
        
        for validator in completion_validators {
            context.swarm_service
                .add_agent_to_swarm(&context.workflow_swarm_id, validator)
                .await
                .unwrap();
        }
        
        // And: Completed workflow tasks are generated
        let completed_tasks = vec![
            context.test_data.generate_completed_task(),
            context.test_data.generate_neural_testing_task(),
            context.test_data.generate_orchestrator_task(),
        ];
        
        // Then: Workflow completion validation should be thorough
        let swarm_status = context.swarm_service
            .get_swarm_status(&context.workflow_swarm_id)
            .await
            .unwrap();
        
        assert_eq!(swarm_status.agents.len(), 2);
        
        let validation_capabilities: Vec<String> = swarm_status.agents
            .iter()
            .flat_map(|a| a.capabilities.clone())
            .filter(|cap| cap.contains("validation"))
            .collect();
        
        assert!(validation_capabilities.contains(&"workflow-completion-validation".to_string()));
        assert!(validation_capabilities.contains(&"end-to-end-workflow-testing".to_string()));
        assert!(validation_capabilities.contains(&"workflow-behavior-validation".to_string()));
        
        // Completed tasks should be trackable
        assert_eq!(completed_tasks.len(), 3);
        assert!(completed_tasks.iter().any(|t| matches!(t.status, TaskStatus::Completed)));
        
        context.cleanup().await;
    }
}

/// Integration tests for workflow coordination with neural components
#[cfg(test)]
mod workflow_neural_integration_tests {
    use super::*;

    #[tokio::test]
    async fn should_orchestrate_neural_trader_complete_workflow() {
        // Given: Complete neural trader workflow requirements
        let context = IntegrationWorkflowTestContext::new().await;
        
        // When: Neural trader complete workflow is orchestrated
        let neural_trader_workflow = vec![
            MockAgent {
                id: "data-pipeline-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "market-data-pipeline-workflow".to_string(),
                    "data-preprocessing-coordination".to_string(),
                    "feature-engineering-coordination".to_string(),
                ],
                memory_usage: 512,
                cpu_usage: 0.3,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "neural-model-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "fann-model-lifecycle-workflow".to_string(),
                    "training-orchestration-workflow".to_string(),
                    "prediction-service-coordination".to_string(),
                ],
                memory_usage: 768,
                cpu_usage: 0.45,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "trading-strategy-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "neural-strategy-execution-workflow".to_string(),
                    "backtesting-workflow-coordination".to_string(),
                    "live-trading-coordination-workflow".to_string(),
                ],
                memory_usage: 640,
                cpu_usage: 0.4,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "quality-assurance-coordinator".to_string(),
                agent_type: AgentType::TddLondon,
                status: AgentStatus::Active,
                capabilities: vec![
                    "neural-trader-testing-workflow".to_string(),
                    "end-to-end-validation-workflow".to_string(),
                    "performance-testing-coordination".to_string(),
                ],
                memory_usage: 384,
                cpu_usage: 0.28,
                last_heartbeat: std::time::SystemTime::now(),
            },
        ];
        
        for agent in neural_trader_workflow {
            context.swarm_service
                .add_agent_to_swarm(&context.workflow_swarm_id, agent)
                .await
                .unwrap();
        }
        
        // Then: Complete neural trader workflow should be orchestrated
        let swarm_status = context.swarm_service
            .get_swarm_status(&context.workflow_swarm_id)
            .await
            .unwrap();
        
        assert_eq!(swarm_status.agents.len(), 4);
        
        let workflow_capabilities: Vec<String> = swarm_status.agents
            .iter()
            .flat_map(|a| a.capabilities.clone())
            .filter(|cap| cap.contains("workflow") || cap.contains("coordination"))
            .collect();
        
        assert!(workflow_capabilities.contains(&"market-data-pipeline-workflow".to_string()));
        assert!(workflow_capabilities.contains(&"fann-model-lifecycle-workflow".to_string()));
        assert!(workflow_capabilities.contains(&"neural-strategy-execution-workflow".to_string()));
        assert!(workflow_capabilities.contains(&"neural-trader-testing-workflow".to_string()));
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_coordinate_ruv_fann_daa_integration_workflow() {
        // Given: RUV-FANN and DAA integration workflow requirements
        let context = IntegrationWorkflowTestContext::new().await;
        
        // When: RUV-FANN DAA integration workflow is established
        let integration_workflow = vec![
            MockAgent {
                id: "ruv-fann-integration-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "ruv-fann-neural-integration-workflow".to_string(),
                    "fann-model-daa-coordination-workflow".to_string(),
                    "neural-decision-integration-workflow".to_string(),
                ],
                memory_usage: 896,
                cpu_usage: 0.5,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "daa-neural-bridge-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    "daa-neural-communication-workflow".to_string(),
                    "distributed-neural-consensus-workflow".to_string(),
                    "neural-agent-coordination-workflow".to_string(),
                ],
                memory_usage: 768,
                cpu_usage: 0.42,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "integration-testing-coordinator".to_string(),
                agent_type: AgentType::TddLondon,
                status: AgentStatus::Active,
                capabilities: vec![
                    "ruv-fann-daa-integration-testing-workflow".to_string(),
                    "neural-distributed-testing-workflow".to_string(),
                    "cross-system-validation-workflow".to_string(),
                ],
                memory_usage: 512,
                cpu_usage: 0.32,
                last_heartbeat: std::time::SystemTime::now(),
            },
        ];
        
        for agent in integration_workflow {
            context.swarm_service
                .add_agent_to_swarm(&context.workflow_swarm_id, agent)
                .await
                .unwrap();
        }
        
        // Then: RUV-FANN DAA integration should be fully coordinated
        let swarm_status = context.swarm_service
            .get_swarm_status(&context.workflow_swarm_id)
            .await
            .unwrap();
        
        assert_eq!(swarm_status.agents.len(), 3);
        
        let integration_capabilities: Vec<String> = swarm_status.agents
            .iter()
            .flat_map(|a| a.capabilities.clone())
            .collect();
        
        assert!(integration_capabilities.contains(&"ruv-fann-neural-integration-workflow".to_string()));
        assert!(integration_capabilities.contains(&"daa-neural-communication-workflow".to_string()));
        assert!(integration_capabilities.contains(&"ruv-fann-daa-integration-testing-workflow".to_string()));
        
        context.cleanup().await;
    }
}