// Test Fixtures - London School TDD Support
// Shared test fixtures and setup data for orchestrator testing

use super::mock_services::*;
use super::test_data_generators::*;
use std::collections::HashMap;

/// Common test fixtures for orchestrator testing
pub struct OrchestratorTestFixtures {
    test_data_generator: TestDataGenerator,
}

impl OrchestratorTestFixtures {
    pub fn new() -> Self {
        Self {
            test_data_generator: TestDataGenerator::new(),
        }
    }
    
    /// Create a standard TDD London School test swarm
    pub async fn create_tdd_swarm(
        &self,
        swarm_service: &MockSwarmService,
    ) -> Result<String, String> {
        let swarm_id = swarm_service
            .init_swarm(SwarmTopology::Hierarchical, 6, "tdd-london-swarm")
            .await?;
        
        let tdd_team = vec![
            self.test_data_generator.generate_coordinator_agent(),
            self.test_data_generator.generate_tdd_london_agent(),
            self.test_data_generator.generate_coder_agent(),
            self.test_data_generator.generate_researcher_agent(),
        ];
        
        for agent in tdd_team {
            swarm_service.add_agent_to_swarm(&swarm_id, agent).await?;
        }
        
        Ok(swarm_id)
    }
    
    /// Create a neural-focused test swarm
    pub async fn create_neural_swarm(
        &self,
        swarm_service: &MockSwarmService,
    ) -> Result<String, String> {
        let swarm_id = swarm_service
            .init_swarm(SwarmTopology::Star, 8, "neural-specialist-swarm")
            .await?;
        
        let neural_team = vec![
            self.test_data_generator.generate_coordinator_agent(),
            self.test_data_generator.generate_neural_specialist_agent(),
            self.test_data_generator.generate_analyst_agent(),
            self.test_data_generator.generate_tdd_london_agent(),
            self.test_data_generator.generate_optimizer_agent(),
        ];
        
        for agent in neural_team {
            swarm_service.add_agent_to_swarm(&swarm_id, agent).await?;
        }
        
        Ok(swarm_id)
    }
    
    /// Create a performance testing swarm
    pub async fn create_performance_swarm(
        &self,
        swarm_service: &MockSwarmService,
        agent_count: usize,
    ) -> Result<String, String> {
        let swarm_id = swarm_service
            .init_swarm(SwarmTopology::Mesh, agent_count as u32, "performance-test-swarm")
            .await?;
        
        let performance_agents = self.test_data_generator.generate_high_load_agents(agent_count);
        
        for agent in performance_agents {
            swarm_service.add_agent_to_swarm(&swarm_id, agent).await?;
        }
        
        Ok(swarm_id)
    }
    
    /// Create a failure scenario swarm
    pub async fn create_failure_scenario_swarm(
        &self,
        swarm_service: &MockSwarmService,
    ) -> Result<String, String> {
        let swarm_id = swarm_service
            .init_swarm(SwarmTopology::Ring, 8, "failure-scenario-swarm")
            .await?;
        
        let failure_agents = vec![
            self.test_data_generator.generate_coordinator_agent(),
            self.test_data_generator.generate_error_agent(),
            self.test_data_generator.generate_tdd_london_agent(),
        ];
        
        for agent in failure_agents {
            swarm_service.add_agent_to_swarm(&swarm_id, agent).await.ok(); // Allow failures
        }
        
        Ok(swarm_id)
    }
}

/// Mock contract fixtures for common interaction patterns
pub struct MockContractFixtures;

impl MockContractFixtures {
    /// Standard swarm initialization contract
    pub fn swarm_initialization_contract() -> MockContract {
        MockContract::new()
            .require_method("init_swarm")
            .expect_parameters(
                "init_swarm",
                [
                    ("topology".to_string(), "mesh".to_string()),
                    ("max_agents".to_string(), "8".to_string()),
                ]
                .iter()
                .cloned()
                .collect(),
            )
            .expect_call_count("init_swarm", 1)
    }
    
    /// Agent management contract
    pub fn agent_management_contract() -> MockContract {
        MockContract::new()
            .require_method("add_agent_to_swarm")
            .require_method("get_swarm_status")
            .forbid_method("destroy_swarm")
    }
    
    /// TDD workflow contract
    pub fn tdd_workflow_contract() -> MockContract {
        MockContract::new()
            .require_method("init_swarm")
            .require_method("add_agent_to_swarm")
            .require_method("get_swarm_status")
            .expect_call_count("add_agent_to_swarm", 3) // Coordinator, TDD, Coder agents
    }
    
    /// Neural processing contract
    pub fn neural_processing_contract() -> MockContract {
        MockContract::new()
            .require_method("init_swarm")
            .require_method("add_agent_to_swarm")
            .expect_parameters(
                "init_swarm",
                [("strategy".to_string(), "neural-optimized".to_string())]
                    .iter()
                    .cloned()
                    .collect(),
            )
    }
    
    /// Error handling contract
    pub fn error_handling_contract() -> MockContract {
        MockContract::new()
            .require_method("inject_error")
            .require_method("get_call_log")
            .forbid_method("reset") // Should not reset during error scenarios
    }
}

/// Test data fixtures for common scenarios
pub struct TestDataFixtures {
    generator: TestDataGenerator,
}

impl TestDataFixtures {
    pub fn new() -> Self {
        Self {
            generator: TestDataGenerator::new(),
        }
    }
    
    /// Standard TDD task set
    pub fn tdd_task_set(&self) -> Vec<MockTask> {
        vec![
            self.generator.generate_tdd_task("Write failing test for user authentication"),
            self.generator.generate_tdd_task("Implement minimal authentication logic"),
            self.generator.generate_tdd_task("Refactor authentication code for clarity"),
            self.generator.generate_completed_task(),
        ]
    }
    
    /// Neural model training task set
    pub fn neural_training_task_set(&self) -> Vec<MockTask> {
        vec![
            self.generator.generate_neural_testing_task(),
            MockTask {
                id: "data-preparation".to_string(),
                description: "Prepare training data for neural model".to_string(),
                assigned_agents: vec!["data-analyst".to_string()],
                status: TaskStatus::InProgress,
                priority: TaskPriority::High,
                created_at: std::time::SystemTime::now(),
                started_at: Some(std::time::SystemTime::now()),
                completed_at: None,
                result: None,
                error: None,
            },
            MockTask {
                id: "model-validation".to_string(),
                description: "Validate trained neural model performance".to_string(),
                assigned_agents: vec!["neural-validator".to_string()],
                status: TaskStatus::Pending,
                priority: TaskPriority::Medium,
                created_at: std::time::SystemTime::now(),
                started_at: None,
                completed_at: None,
                result: None,
                error: None,
            },
        ]
    }
    
    /// Performance testing scenario
    pub fn performance_test_scenario(&self, scale: usize) -> (Vec<MockAgent>, Vec<MockTask>) {
        let agents = self.generator.generate_high_load_agents(scale);
        let tasks = self.generator.generate_stress_test_tasks(scale * 2);
        (agents, tasks)
    }
    
    /// Error scenario fixtures
    pub fn error_scenario_fixtures(&self) -> (Vec<MockAgent>, Vec<MockTask>) {
        let agents = vec![
            self.generator.generate_coordinator_agent(),
            self.generator.generate_error_agent(),
            MockAgent {
                id: "recovering-agent".to_string(),
                agent_type: AgentType::TddLondon,
                status: AgentStatus::Active,
                capabilities: vec!["error-recovery".to_string()],
                memory_usage: 256,
                cpu_usage: 0.3,
                last_heartbeat: std::time::SystemTime::now(),
            },
        ];
        
        let tasks = vec![
            self.generator.generate_failed_task(),
            MockTask {
                id: "recovery-task".to_string(),
                description: "Recover from previous failure".to_string(),
                assigned_agents: vec!["recovering-agent".to_string()],
                status: TaskStatus::Pending,
                priority: TaskPriority::Critical,
                created_at: std::time::SystemTime::now(),
                started_at: None,
                completed_at: None,
                result: None,
                error: None,
            },
        ];
        
        (agents, tasks)
    }
}

/// Integration test fixtures
pub struct IntegrationTestFixtures;

impl IntegrationTestFixtures {
    /// Complete neural trader integration scenario
    pub async fn neural_trader_integration_scenario(
        swarm_service: &MockSwarmService,
    ) -> Result<NeuralTraderIntegrationFixture, String> {
        let data_swarm = swarm_service
            .init_swarm(SwarmTopology::Ring, 4, "data-processing")
            .await?;
        
        let neural_swarm = swarm_service
            .init_swarm(SwarmTopology::Star, 6, "neural-processing")
            .await?;
        
        let trading_swarm = swarm_service
            .init_swarm(SwarmTopology::Mesh, 8, "trading-coordination")
            .await?;
        
        let test_data = TestDataGenerator::new();
        
        // Setup data processing agents
        let data_agents = vec![
            test_data.generate_coordinator_agent(),
            test_data.generate_analyst_agent(),
        ];
        
        for agent in data_agents {
            swarm_service.add_agent_to_swarm(&data_swarm, agent).await?;
        }
        
        // Setup neural processing agents
        let neural_agents = vec![
            test_data.generate_coordinator_agent(),
            test_data.generate_neural_specialist_agent(),
            test_data.generate_tdd_london_agent(),
        ];
        
        for agent in neural_agents {
            swarm_service.add_agent_to_swarm(&neural_swarm, agent).await?;
        }
        
        // Setup trading agents
        let trading_agents = vec![
            test_data.generate_coordinator_agent(),
            test_data.generate_optimizer_agent(),
            test_data.generate_analyst_agent(),
        ];
        
        for agent in trading_agents {
            swarm_service.add_agent_to_swarm(&trading_swarm, agent).await?;
        }
        
        Ok(NeuralTraderIntegrationFixture {
            data_swarm_id: data_swarm,
            neural_swarm_id: neural_swarm,
            trading_swarm_id: trading_swarm,
        })
    }
    
    /// Cross-binary integration scenario
    pub async fn cross_binary_integration_scenario(
        swarm_service: &MockSwarmService,
    ) -> Result<CrossBinaryIntegrationFixture, String> {
        let config_swarm = swarm_service
            .init_swarm(SwarmTopology::Star, 3, "config-store-integration")
            .await?;
        
        let data_ingestion_swarm = swarm_service
            .init_swarm(SwarmTopology::Ring, 4, "data-ingestion-integration")
            .await?;
        
        let neural_swarm = swarm_service
            .init_swarm(SwarmTopology::Mesh, 6, "ruv-fann-integration")
            .await?;
        
        let daa_swarm = swarm_service
            .init_swarm(SwarmTopology::Hierarchical, 8, "daa-coordinator-integration")
            .await?;
        
        let test_data = TestDataGenerator::new();
        
        // Setup integration coordinators for each binary
        let integration_agents = vec![
            (config_swarm, "config-store-coordinator"),
            (data_ingestion_swarm, "data-ingestion-coordinator"),
            (neural_swarm, "neural-fann-coordinator"),
            (daa_swarm, "daa-coordinator"),
        ];
        
        for (swarm_id, agent_name) in integration_agents {
            let coordinator = MockAgent {
                id: agent_name.to_string(),
                agent_type: AgentType::Coordinator,
                status: AgentStatus::Active,
                capabilities: vec![
                    format!("{}-integration", agent_name),
                    "cross-binary-communication".to_string(),
                ],
                memory_usage: 384,
                cpu_usage: 0.25,
                last_heartbeat: std::time::SystemTime::now(),
            };
            
            swarm_service.add_agent_to_swarm(&swarm_id, coordinator).await?;
        }
        
        Ok(CrossBinaryIntegrationFixture {
            config_swarm_id: config_swarm,
            data_ingestion_swarm_id: data_ingestion_swarm,
            neural_swarm_id: neural_swarm,
            daa_swarm_id: daa_swarm,
        })
    }
}

/// Fixture data structures
pub struct NeuralTraderIntegrationFixture {
    pub data_swarm_id: String,
    pub neural_swarm_id: String,
    pub trading_swarm_id: String,
}

pub struct CrossBinaryIntegrationFixture {
    pub config_swarm_id: String,
    pub data_ingestion_swarm_id: String,
    pub neural_swarm_id: String,
    pub daa_swarm_id: String,
}

/// Mock service fixtures
pub struct MockServiceFixtures;

impl MockServiceFixtures {
    /// Create a fully configured mock registry
    pub async fn create_configured_registry() -> MockRegistry {
        let registry = MockRegistry::new();
        
        let swarm_service = MockSwarmService::new();
        registry.register(Box::new(swarm_service)).await;
        
        registry.start_all().await.unwrap();
        registry
    }
    
    /// Create a mock registry with error injection
    pub async fn create_error_prone_registry() -> (MockRegistry, MockSwarmService) {
        let registry = MockRegistry::new();
        let swarm_service = MockSwarmService::new();
        
        // Pre-inject some errors
        swarm_service.inject_error("network_call", "Network timeout").await;
        swarm_service.inject_error("resource_allocation", "Out of memory").await;
        
        registry.register(Box::new(swarm_service.clone())).await;
        registry.start_all().await.unwrap();
        
        (registry, swarm_service)
    }
}

#[cfg(test)]
mod fixture_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_orchestrator_fixtures_creation() {
        let fixtures = OrchestratorTestFixtures::new();
        let swarm_service = MockSwarmService::new();
        
        let tdd_swarm = fixtures.create_tdd_swarm(&swarm_service).await;
        assert!(tdd_swarm.is_ok());
        
        let neural_swarm = fixtures.create_neural_swarm(&swarm_service).await;
        assert!(neural_swarm.is_ok());
    }
    
    #[test]
    fn test_mock_contract_fixtures() {
        let init_contract = MockContractFixtures::swarm_initialization_contract();
        assert!(!init_contract.required_methods.is_empty());
        
        let tdd_contract = MockContractFixtures::tdd_workflow_contract();
        assert!(tdd_contract.expected_call_count.contains_key("add_agent_to_swarm"));
    }
    
    #[test]
    fn test_test_data_fixtures() {
        let fixtures = TestDataFixtures::new();
        
        let tdd_tasks = fixtures.tdd_task_set();
        assert_eq!(tdd_tasks.len(), 4);
        assert!(tdd_tasks.iter().any(|t| t.description.contains("failing test")));
        
        let neural_tasks = fixtures.neural_training_task_set();
        assert_eq!(neural_tasks.len(), 3);
        assert!(neural_tasks.iter().any(|t| t.description.contains("neural")));
    }
    
    #[tokio::test]
    async fn test_integration_fixtures() {
        let swarm_service = MockSwarmService::new();
        
        let neural_fixture = IntegrationTestFixtures::neural_trader_integration_scenario(&swarm_service).await;
        assert!(neural_fixture.is_ok());
        
        let cross_binary_fixture = IntegrationTestFixtures::cross_binary_integration_scenario(&swarm_service).await;
        assert!(cross_binary_fixture.is_ok());
    }
}