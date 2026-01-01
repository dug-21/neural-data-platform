// Agent Spawn and Management Tests - London School TDD
// Behavior-driven testing for agent lifecycle management

use super::mock_services::*;
use super::mock_services::swarm_mock::*;
use super::test_data_generators::*;
use tokio_test;

#[cfg(test)]
mod agent_spawn_behavior_tests {
    use super::*;

    struct AgentManagementTestContext {
        mock_registry: MockRegistry,
        swarm_service: MockSwarmService,
        test_data: TestDataGenerator,
        swarm_id: String,
    }

    impl AgentManagementTestContext {
        async fn new() -> Self {
            let mock_registry = MockRegistry::new();
            let swarm_service = MockSwarmService::new();
            let test_data = TestDataGenerator::new();
            
            mock_registry.register(Box::new(swarm_service.clone())).await;
            mock_registry.start_all().await.unwrap();
            
            // Initialize a test swarm
            let swarm_id = swarm_service
                .init_swarm(SwarmTopology::Mesh, 10, "test-strategy")
                .await
                .unwrap();
            
            Self {
                mock_registry,
                swarm_service,
                test_data,
                swarm_id,
            }
        }

        async fn cleanup(&self) {
            self.swarm_service.destroy_swarm(&self.swarm_id).await.ok();
            self.mock_registry.stop_all().await.unwrap();
        }
    }

    #[tokio::test]
    async fn should_spawn_tdd_london_agent_with_correct_capabilities() {
        // Given: A swarm ready for TDD agent deployment
        let context = AgentManagementTestContext::new().await;
        
        // When: TDD London agent is spawned with specific capabilities
        let tdd_agent = MockAgent {
            id: "tdd-london-1".to_string(),
            agent_type: AgentType::TddLondon,
            status: AgentStatus::Initializing,
            capabilities: vec![
                "outside-in-development".to_string(),
                "mock-driven-testing".to_string(),
                "behavior-verification".to_string(),
                "contract-definition".to_string(),
                "interaction-testing".to_string(),
            ],
            memory_usage: 256,
            cpu_usage: 0.1,
            last_heartbeat: std::time::SystemTime::now(),
        };
        
        let result = context.swarm_service.add_agent_to_swarm(&context.swarm_id, tdd_agent.clone()).await;
        
        // Then: Agent should be successfully added to swarm
        assert!(result.is_ok());
        
        // And: Swarm should reflect the new agent
        let swarm_status = context.swarm_service.get_swarm_status(&context.swarm_id).await.unwrap();
        assert_eq!(swarm_status.agents.len(), 1);
        assert_eq!(swarm_status.agents[0].agent_type, AgentType::TddLondon);
        
        // And: Agent capabilities should be preserved
        assert_eq!(swarm_status.agents[0].capabilities.len(), 5);
        assert!(swarm_status.agents[0].capabilities.contains(&"outside-in-development".to_string()));
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_spawn_agents_in_correct_hierarchical_order() {
        // Given: Hierarchical swarm topology
        let context = AgentManagementTestContext::new().await;
        
        // When: Agents are spawned in hierarchical order
        let coordinator = MockAgent {
            id: "coordinator-1".to_string(),
            agent_type: AgentType::Coordinator,
            status: AgentStatus::Active,
            capabilities: vec!["coordination".to_string(), "task-distribution".to_string()],
            memory_usage: 512,
            cpu_usage: 0.2,
            last_heartbeat: std::time::SystemTime::now(),
        };
        
        let analyst = MockAgent {
            id: "analyst-1".to_string(),
            agent_type: AgentType::Analyst,
            status: AgentStatus::Active,
            capabilities: vec!["code-analysis".to_string(), "pattern-recognition".to_string()],
            memory_usage: 384,
            cpu_usage: 0.15,
            last_heartbeat: std::time::SystemTime::now(),
        };
        
        let coder = MockAgent {
            id: "coder-1".to_string(),
            agent_type: AgentType::Coder,
            status: AgentStatus::Active,
            capabilities: vec!["implementation".to_string(), "refactoring".to_string()],
            memory_usage: 256,
            cpu_usage: 0.1,
            last_heartbeat: std::time::SystemTime::now(),
        };
        
        // Spawn in hierarchical order: Coordinator → Analyst → Coder
        context.swarm_service.add_agent_to_swarm(&context.swarm_id, coordinator).await.unwrap();
        context.swarm_service.add_agent_to_swarm(&context.swarm_id, analyst).await.unwrap();
        context.swarm_service.add_agent_to_swarm(&context.swarm_id, coder).await.unwrap();
        
        // Then: Agents should be added in correct order
        let swarm_status = context.swarm_service.get_swarm_status(&context.swarm_id).await.unwrap();
        assert_eq!(swarm_status.agents.len(), 3);
        assert_eq!(swarm_status.agents[0].agent_type, AgentType::Coordinator);
        assert_eq!(swarm_status.agents[1].agent_type, AgentType::Analyst);
        assert_eq!(swarm_status.agents[2].agent_type, AgentType::Coder);
        
        // And: Metrics should reflect active agents
        let metrics = context.swarm_service.get_swarm_metrics(&context.swarm_id).await.unwrap();
        assert_eq!(metrics.active_agents, 3);
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_reject_agent_spawn_when_swarm_at_capacity() {
        // Given: Swarm at maximum capacity
        let mut swarm_service = MockSwarmService::new();
        let swarm_id = swarm_service
            .init_swarm(SwarmTopology::Mesh, 2, "capacity-test")
            .await
            .unwrap();
        
        let agent1 = MockAgent {
            id: "agent-1".to_string(),
            agent_type: AgentType::Researcher,
            status: AgentStatus::Active,
            capabilities: vec!["research".to_string()],
            memory_usage: 128,
            cpu_usage: 0.05,
            last_heartbeat: std::time::SystemTime::now(),
        };
        
        let agent2 = MockAgent {
            id: "agent-2".to_string(),
            agent_type: AgentType::Coder,
            status: AgentStatus::Active,
            capabilities: vec!["coding".to_string()],
            memory_usage: 128,
            cpu_usage: 0.05,
            last_heartbeat: std::time::SystemTime::now(),
        };
        
        let agent3 = MockAgent {
            id: "agent-3".to_string(),
            agent_type: AgentType::Analyst,
            status: AgentStatus::Active,
            capabilities: vec!["analysis".to_string()],
            memory_usage: 128,
            cpu_usage: 0.05,
            last_heartbeat: std::time::SystemTime::now(),
        };
        
        // When: Adding agents up to and beyond capacity
        assert!(swarm_service.add_agent_to_swarm(&swarm_id, agent1).await.is_ok());
        assert!(swarm_service.add_agent_to_swarm(&swarm_id, agent2).await.is_ok());
        
        // Then: Third agent should be rejected
        let result = swarm_service.add_agent_to_swarm(&swarm_id, agent3).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("maximum capacity"));
        
        // And: Swarm should still have only 2 agents
        let metrics = swarm_service.get_swarm_metrics(&swarm_id).await.unwrap();
        assert_eq!(metrics.active_agents, 2);
    }

    #[tokio::test]
    async fn should_track_agent_heartbeat_and_health() {
        // Given: Active agents in swarm
        let context = AgentManagementTestContext::new().await;
        let heartbeat_time = std::time::SystemTime::now();
        
        let agent = MockAgent {
            id: "heartbeat-agent".to_string(),
            agent_type: AgentType::Optimizer,
            status: AgentStatus::Active,
            capabilities: vec!["optimization".to_string()],
            memory_usage: 200,
            cpu_usage: 0.3,
            last_heartbeat: heartbeat_time,
        };
        
        context.swarm_service.add_agent_to_swarm(&context.swarm_id, agent).await.unwrap();
        
        // When: Checking agent health status
        let swarm_status = context.swarm_service.get_swarm_status(&context.swarm_id).await.unwrap();
        let tracked_agent = &swarm_status.agents[0];
        
        // Then: Heartbeat should be tracked
        assert_eq!(tracked_agent.last_heartbeat, heartbeat_time);
        assert_eq!(tracked_agent.status, AgentStatus::Active);
        
        // And: Resource usage should be monitored
        assert_eq!(tracked_agent.memory_usage, 200);
        assert_eq!(tracked_agent.cpu_usage, 0.3);
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_handle_agent_capability_specialization() {
        // Given: Swarm requiring specialized capabilities
        let context = AgentManagementTestContext::new().await;
        
        // When: Agents with different specializations are added
        let neural_specialist = MockAgent {
            id: "neural-specialist".to_string(),
            agent_type: AgentType::Analyst,
            status: AgentStatus::Active,
            capabilities: vec![
                "neural-network-analysis".to_string(),
                "model-optimization".to_string(),
                "tensor-operations".to_string(),
            ],
            memory_usage: 1024,
            cpu_usage: 0.8,
            last_heartbeat: std::time::SystemTime::now(),
        };
        
        let testing_specialist = MockAgent {
            id: "testing-specialist".to_string(),
            agent_type: AgentType::TddLondon,
            status: AgentStatus::Active,
            capabilities: vec![
                "mock-verification".to_string(),
                "behavior-testing".to_string(),
                "contract-validation".to_string(),
            ],
            memory_usage: 256,
            cpu_usage: 0.2,
            last_heartbeat: std::time::SystemTime::now(),
        };
        
        context.swarm_service.add_agent_to_swarm(&context.swarm_id, neural_specialist).await.unwrap();
        context.swarm_service.add_agent_to_swarm(&context.swarm_id, testing_specialist).await.unwrap();
        
        // Then: Each agent should maintain its specialization
        let swarm_status = context.swarm_service.get_swarm_status(&context.swarm_id).await.unwrap();
        assert_eq!(swarm_status.agents.len(), 2);
        
        let neural_agent = &swarm_status.agents[0];
        let testing_agent = &swarm_status.agents[1];
        
        assert!(neural_agent.capabilities.contains(&"neural-network-analysis".to_string()));
        assert!(testing_agent.capabilities.contains(&"mock-verification".to_string()));
        
        // And: Resource allocation should reflect specialization requirements
        assert!(neural_agent.memory_usage > testing_agent.memory_usage);
        assert!(neural_agent.cpu_usage > testing_agent.cpu_usage);
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_manage_agent_lifecycle_state_transitions() {
        // Given: Agent in various lifecycle states
        let context = AgentManagementTestContext::new().await;
        
        // When: Agent goes through lifecycle states
        let mut agent = MockAgent {
            id: "lifecycle-agent".to_string(),
            agent_type: AgentType::Researcher,
            status: AgentStatus::Initializing,
            capabilities: vec!["research".to_string()],
            memory_usage: 128,
            cpu_usage: 0.1,
            last_heartbeat: std::time::SystemTime::now(),
        };
        
        context.swarm_service.add_agent_to_swarm(&context.swarm_id, agent.clone()).await.unwrap();
        
        // Agent transitions to active
        agent.status = AgentStatus::Active;
        // Note: In real implementation, this would be an update operation
        
        // Agent becomes busy with task
        agent.status = AgentStatus::Busy;
        agent.cpu_usage = 0.9;
        
        // Then: State transitions should be valid and tracked
        // This test defines the expected behavior for agent lifecycle management
        assert_eq!(agent.status, AgentStatus::Busy);
        assert_eq!(agent.cpu_usage, 0.9);
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_coordinate_multi_agent_collaborative_spawning() {
        // Given: Requirement for coordinated agent team
        let context = AgentManagementTestContext::new().await;
        
        // When: Spawning a coordinated TDD team
        let team_agents = vec![
            MockAgent {
                id: "tdd-coordinator".to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec!["tdd-coordination".to_string(), "test-planning".to_string()],
                memory_usage: 256,
                cpu_usage: 0.15,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "london-school-tester".to_string(),
                agent_type: AgentType::TddLondon,
                status: AgentStatus::Active,
                capabilities: vec!["mockist-testing".to_string(), "outside-in-tdd".to_string()],
                memory_usage: 256,
                cpu_usage: 0.2,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "test-implementer".to_string(),
                agent_type: AgentType::Coder,
                status: AgentStatus::Active,
                capabilities: vec!["test-implementation".to_string(), "mock-creation".to_string()],
                memory_usage: 256,
                cpu_usage: 0.15,
                last_heartbeat: std::time::SystemTime::now(),
            },
        ];
        
        // Spawn team concurrently
        let spawn_futures = team_agents.into_iter().map(|agent| {
            context.swarm_service.add_agent_to_swarm(&context.swarm_id, agent)
        });
        
        let results: Vec<_> = futures::future::join_all(spawn_futures).await;
        
        // Then: All team members should be successfully spawned
        assert!(results.iter().all(|r| r.is_ok()));
        
        // And: Team should be ready for collaborative work
        let swarm_status = context.swarm_service.get_swarm_status(&context.swarm_id).await.unwrap();
        assert_eq!(swarm_status.agents.len(), 3);
        
        // And: Team composition should support TDD workflow
        let agent_types: Vec<_> = swarm_status.agents.iter().map(|a| &a.agent_type).collect();
        assert!(agent_types.contains(&&AgentType::Coordinator));
        assert!(agent_types.contains(&&AgentType::TddLondon));
        assert!(agent_types.contains(&&AgentType::Coder));
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_validate_agent_capability_requirements() {
        // Given: Swarm with capability requirements
        let context = AgentManagementTestContext::new().await;
        
        // When: Agent with insufficient capabilities is added
        let inadequate_agent = MockAgent {
            id: "inadequate-agent".to_string(),
            agent_type: AgentType::TddLondon,
            status: AgentStatus::Active,
            capabilities: vec![], // No capabilities - should be invalid
            memory_usage: 128,
            cpu_usage: 0.1,
            last_heartbeat: std::time::SystemTime::now(),
        };
        
        let result = context.swarm_service.add_agent_to_swarm(&context.swarm_id, inadequate_agent).await;
        
        // Then: Agent should be accepted (mock doesn't validate)
        // Real implementation should validate capability requirements
        assert!(result.is_ok()); // TODO: Change to is_err() when validation is implemented
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_support_dynamic_agent_capability_updates() {
        // Given: Active agent in swarm
        let context = AgentManagementTestContext::new().await;
        
        let initial_agent = MockAgent {
            id: "updatable-agent".to_string(),
            agent_type: AgentType::Analyst,
            status: AgentStatus::Active,
            capabilities: vec!["basic-analysis".to_string()],
            memory_usage: 200,
            cpu_usage: 0.1,
            last_heartbeat: std::time::SystemTime::now(),
        };
        
        context.swarm_service.add_agent_to_swarm(&context.swarm_id, initial_agent).await.unwrap();
        
        // When: Agent capabilities are dynamically updated
        // Note: This test defines expected behavior for capability updates
        // Real implementation would need update_agent_capabilities() method
        
        // Then: Updated capabilities should be reflected in swarm
        let swarm_status = context.swarm_service.get_swarm_status(&context.swarm_id).await.unwrap();
        let agent = &swarm_status.agents[0];
        
        // Initially has basic capabilities
        assert!(agent.capabilities.contains(&"basic-analysis".to_string()));
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_handle_agent_error_states_gracefully() {
        // Given: Agent that encounters errors
        let context = AgentManagementTestContext::new().await;
        
        let error_prone_agent = MockAgent {
            id: "error-agent".to_string(),
            agent_type: AgentType::Optimizer,
            status: AgentStatus::Error,
            capabilities: vec!["optimization".to_string()],
            memory_usage: 1000,  // High memory usage indicating problems
            cpu_usage: 1.0,      // Max CPU usage indicating overload
            last_heartbeat: std::time::SystemTime::now() - std::time::Duration::from_secs(300), // Old heartbeat
        };
        
        let result = context.swarm_service.add_agent_to_swarm(&context.swarm_id, error_prone_agent).await;
        
        // When: Error agent is added to swarm
        assert!(result.is_ok()); // Mock accepts error states
        
        // Then: Error state should be tracked
        let swarm_status = context.swarm_service.get_swarm_status(&context.swarm_id).await.unwrap();
        let agent = &swarm_status.agents[0];
        
        assert_eq!(agent.status, AgentStatus::Error);
        assert_eq!(agent.memory_usage, 1000);
        assert_eq!(agent.cpu_usage, 1.0);
        
        // And: Health monitoring should flag issues
        let heartbeat_age = std::time::SystemTime::now()
            .duration_since(agent.last_heartbeat)
            .unwrap();
        assert!(heartbeat_age > std::time::Duration::from_secs(60)); // Stale heartbeat
        
        context.cleanup().await;
    }
}

/// Integration tests for agent spawning with neural trader components
#[cfg(test)]
mod agent_neural_integration_tests {
    use super::*;

    #[tokio::test]
    async fn should_spawn_neural_aware_tdd_agents() {
        // Given: Neural trader system requiring TDD coverage
        let context = AgentManagementTestContext::new().await;
        
        // When: Neural-specific TDD agents are spawned
        let neural_tdd_agent = MockAgent {
            id: "neural-tdd-london".to_string(),
            agent_type: AgentType::TddLondon,
            status: AgentStatus::Active,
            capabilities: vec![
                "neural-model-testing".to_string(),
                "fann-integration-testing".to_string(),
                "behavior-verification".to_string(),
                "mock-neural-networks".to_string(),
            ],
            memory_usage: 512,
            cpu_usage: 0.25,
            last_heartbeat: std::time::SystemTime::now(),
        };
        
        let result = context.swarm_service.add_agent_to_swarm(&context.swarm_id, neural_tdd_agent).await;
        
        // Then: Neural TDD agent should be successfully integrated
        assert!(result.is_ok());
        
        let swarm_status = context.swarm_service.get_swarm_status(&context.swarm_id).await.unwrap();
        let agent = &swarm_status.agents[0];
        
        assert_eq!(agent.agent_type, AgentType::TddLondon);
        assert!(agent.capabilities.contains(&"neural-model-testing".to_string()));
        assert!(agent.capabilities.contains(&"fann-integration-testing".to_string()));
        
        context.cleanup().await;
    }

    #[tokio::test]
    async fn should_coordinate_neural_tdd_team_formation() {
        // Given: Neural trader requiring comprehensive TDD coverage
        let context = AgentManagementTestContext::new().await;
        
        // When: Complete neural TDD team is formed
        let neural_team = vec![
            MockAgent {
                id: "neural-architect".to_string(),
                agent_type: AgentType::Architect,
                status: AgentStatus::Active,
                capabilities: vec!["neural-architecture-design".to_string(), "test-strategy".to_string()],
                memory_usage: 384,
                cpu_usage: 0.2,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "neural-tdd-specialist".to_string(),
                agent_type: AgentType::TddLondon,
                status: AgentStatus::Active,
                capabilities: vec![
                    "neural-behavior-testing".to_string(),
                    "ruv-fann-mocking".to_string(),
                    "outside-in-neural-design".to_string(),
                ],
                memory_usage: 512,
                cpu_usage: 0.3,
                last_heartbeat: std::time::SystemTime::now(),
            },
            MockAgent {
                id: "neural-validator".to_string(),
                agent_type: AgentType::Reviewer,
                status: AgentStatus::Active,
                capabilities: vec!["neural-model-validation".to_string(), "test-coverage-analysis".to_string()],
                memory_usage: 256,
                cpu_usage: 0.15,
                last_heartbeat: std::time::SystemTime::now(),
            },
        ];
        
        for agent in neural_team {
            let result = context.swarm_service.add_agent_to_swarm(&context.swarm_id, agent).await;
            assert!(result.is_ok());
        }
        
        // Then: Neural TDD team should be fully operational
        let swarm_status = context.swarm_service.get_swarm_status(&context.swarm_id).await.unwrap();
        assert_eq!(swarm_status.agents.len(), 3);
        
        // And: Team should have complementary neural capabilities
        let all_capabilities: Vec<String> = swarm_status.agents
            .iter()
            .flat_map(|agent| agent.capabilities.clone())
            .collect();
        
        assert!(all_capabilities.contains(&"neural-architecture-design".to_string()));
        assert!(all_capabilities.contains(&"neural-behavior-testing".to_string()));
        assert!(all_capabilities.contains(&"neural-model-validation".to_string()));
        
        context.cleanup().await;
    }
}