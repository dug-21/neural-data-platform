// Test Data Generators - London School TDD Support
// Comprehensive test data generation for orchestrator testing

use super::mock_services::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

pub struct TestDataGenerator {
    seed: u64,
}

impl TestDataGenerator {
    pub fn new() -> Self {
        Self { seed: 42 }
    }
    
    pub fn with_seed(seed: u64) -> Self {
        Self { seed }
    }
}

// Mock Agent Generation
impl TestDataGenerator {
    pub fn generate_tdd_london_agent(&self) -> MockAgent {
        MockAgent {
            id: format!("tdd-london-{}", Uuid::new_v4()),
            agent_type: AgentType::TddLondon,
            status: AgentStatus::Active,
            capabilities: vec![
                "outside-in-development".to_string(),
                "mock-driven-testing".to_string(),
                "behavior-verification".to_string(),
                "contract-definition".to_string(),
                "interaction-testing".to_string(),
                "red-green-refactor".to_string(),
            ],
            memory_usage: 256,
            cpu_usage: 0.2,
            last_heartbeat: std::time::SystemTime::now(),
        }
    }

    pub fn generate_neural_specialist_agent(&self) -> MockAgent {
        MockAgent {
            id: format!("neural-specialist-{}", Uuid::new_v4()),
            agent_type: AgentType::Analyst,
            status: AgentStatus::Active,
            capabilities: vec![
                "neural-network-analysis".to_string(),
                "fann-integration".to_string(),
                "model-optimization".to_string(),
                "tensor-operations".to_string(),
                "training-validation".to_string(),
            ],
            memory_usage: 1024,
            cpu_usage: 0.8,
            last_heartbeat: std::time::SystemTime::now(),
        }
    }

    pub fn generate_coordinator_agent(&self) -> MockAgent {
        MockAgent {
            id: format!("coordinator-{}", Uuid::new_v4()),
            agent_type: AgentType::Coordinator,
            status: AgentStatus::Active,
            capabilities: vec![
                "task-orchestration".to_string(),
                "agent-coordination".to_string(),
                "resource-allocation".to_string(),
                "workflow-management".to_string(),
            ],
            memory_usage: 512,
            cpu_usage: 0.3,
            last_heartbeat: std::time::SystemTime::now(),
        }
    }

    pub fn generate_agent_team(&self, team_size: usize) -> Vec<MockAgent> {
        let mut team = Vec::new();
        
        // Always include a coordinator for teams > 1
        if team_size > 1 {
            team.push(self.generate_coordinator_agent());
        }
        
        // Add TDD London agent for testing focus
        if team_size > 0 {
            team.push(self.generate_tdd_london_agent());
        }
        
        // Fill remaining slots with specialized agents
        for i in team.len()..team_size {
            let agent_type = match i % 4 {
                0 => self.generate_researcher_agent(),
                1 => self.generate_coder_agent(),
                2 => self.generate_analyst_agent(),
                3 => self.generate_optimizer_agent(),
                _ => self.generate_researcher_agent(),
            };
            team.push(agent_type);
        }
        
        team
    }

    pub fn generate_researcher_agent(&self) -> MockAgent {
        MockAgent {
            id: format!("researcher-{}", Uuid::new_v4()),
            agent_type: AgentType::Researcher,
            status: AgentStatus::Active,
            capabilities: vec![
                "requirements-analysis".to_string(),
                "architecture-research".to_string(),
                "best-practices".to_string(),
                "documentation".to_string(),
            ],
            memory_usage: 256,
            cpu_usage: 0.15,
            last_heartbeat: std::time::SystemTime::now(),
        }
    }

    pub fn generate_coder_agent(&self) -> MockAgent {
        MockAgent {
            id: format!("coder-{}", Uuid::new_v4()),
            agent_type: AgentType::Coder,
            status: AgentStatus::Active,
            capabilities: vec![
                "implementation".to_string(),
                "refactoring".to_string(),
                "code-generation".to_string(),
                "debugging".to_string(),
            ],
            memory_usage: 384,
            cpu_usage: 0.4,
            last_heartbeat: std::time::SystemTime::now(),
        }
    }

    pub fn generate_analyst_agent(&self) -> MockAgent {
        MockAgent {
            id: format!("analyst-{}", Uuid::new_v4()),
            agent_type: AgentType::Analyst,
            status: AgentStatus::Active,
            capabilities: vec![
                "code-analysis".to_string(),
                "performance-analysis".to_string(),
                "security-analysis".to_string(),
                "quality-metrics".to_string(),
            ],
            memory_usage: 512,
            cpu_usage: 0.3,
            last_heartbeat: std::time::SystemTime::now(),
        }
    }

    pub fn generate_optimizer_agent(&self) -> MockAgent {
        MockAgent {
            id: format!("optimizer-{}", Uuid::new_v4()),
            agent_type: AgentType::Optimizer,
            status: AgentStatus::Active,
            capabilities: vec![
                "performance-optimization".to_string(),
                "resource-optimization".to_string(),
                "algorithm-optimization".to_string(),
                "bottleneck-analysis".to_string(),
            ],
            memory_usage: 256,
            cpu_usage: 0.6,
            last_heartbeat: std::time::SystemTime::now(),
        }
    }

    pub fn generate_error_agent(&self) -> MockAgent {
        MockAgent {
            id: format!("error-agent-{}", Uuid::new_v4()),
            agent_type: AgentType::Researcher,
            status: AgentStatus::Error,
            capabilities: vec!["faulty-capability".to_string()],
            memory_usage: 2048, // High memory indicating issues
            cpu_usage: 1.0,     // Max CPU usage
            last_heartbeat: std::time::SystemTime::now() - std::time::Duration::from_secs(600), // Old heartbeat
        }
    }
}

// Mock Task Generation
impl TestDataGenerator {
    pub fn generate_tdd_task(&self, description: &str) -> MockTask {
        MockTask {
            id: format!("tdd-task-{}", Uuid::new_v4()),
            description: description.to_string(),
            assigned_agents: vec![],
            status: TaskStatus::Pending,
            priority: TaskPriority::High,
            created_at: std::time::SystemTime::now(),
            started_at: None,
            completed_at: None,
            result: None,
            error: None,
        }
    }

    pub fn generate_neural_testing_task(&self) -> MockTask {
        MockTask {
            id: format!("neural-test-{}", Uuid::new_v4()),
            description: "Create comprehensive test suite for neural model integration".to_string(),
            assigned_agents: vec!["tdd-london-1".to_string(), "neural-specialist-1".to_string()],
            status: TaskStatus::Pending,
            priority: TaskPriority::Critical,
            created_at: std::time::SystemTime::now(),
            started_at: None,
            completed_at: None,
            result: None,
            error: None,
        }
    }

    pub fn generate_orchestrator_task(&self) -> MockTask {
        MockTask {
            id: format!("orchestrator-task-{}", Uuid::new_v4()),
            description: "Implement RUV-Swarm orchestrator initialization".to_string(),
            assigned_agents: vec!["coordinator-1".to_string(), "coder-1".to_string()],
            status: TaskStatus::InProgress,
            priority: TaskPriority::High,
            created_at: std::time::SystemTime::now(),
            started_at: Some(std::time::SystemTime::now() - std::time::Duration::from_secs(300)),
            completed_at: None,
            result: None,
            error: None,
        }
    }

    pub fn generate_completed_task(&self) -> MockTask {
        let completion_time = std::time::SystemTime::now();
        MockTask {
            id: format!("completed-task-{}", Uuid::new_v4()),
            description: "Successfully completed task".to_string(),
            assigned_agents: vec!["agent-1".to_string()],
            status: TaskStatus::Completed,
            priority: TaskPriority::Medium,
            created_at: completion_time - std::time::Duration::from_secs(3600),
            started_at: Some(completion_time - std::time::Duration::from_secs(3000)),
            completed_at: Some(completion_time),
            result: Some("Task completed successfully with all requirements met".to_string()),
            error: None,
        }
    }

    pub fn generate_failed_task(&self) -> MockTask {
        let failure_time = std::time::SystemTime::now();
        MockTask {
            id: format!("failed-task-{}", Uuid::new_v4()),
            description: "Task that failed during execution".to_string(),
            assigned_agents: vec!["error-agent-1".to_string()],
            status: TaskStatus::Failed,
            priority: TaskPriority::High,
            created_at: failure_time - std::time::Duration::from_secs(1800),
            started_at: Some(failure_time - std::time::Duration::from_secs(900)),
            completed_at: None,
            result: None,
            error: Some("Agent encountered critical error during task execution".to_string()),
        }
    }

    pub fn generate_task_batch(&self, count: usize) -> Vec<MockTask> {
        let mut tasks = Vec::new();
        
        for i in 0..count {
            let task = match i % 5 {
                0 => self.generate_tdd_task(&format!("TDD Task {}", i)),
                1 => self.generate_neural_testing_task(),
                2 => self.generate_orchestrator_task(),
                3 => self.generate_completed_task(),
                4 => self.generate_failed_task(),
                _ => self.generate_tdd_task(&format!("Generic Task {}", i)),
            };
            tasks.push(task);
        }
        
        tasks
    }
}

// Performance Test Data Generation
impl TestDataGenerator {
    pub fn generate_high_load_agents(&self, count: usize) -> Vec<MockAgent> {
        (0..count).map(|i| MockAgent {
            id: format!("load-agent-{}", i),
            agent_type: match i % 5 {
                0 => AgentType::TddLondon,
                1 => AgentType::Coordinator,
                2 => AgentType::Analyst,
                3 => AgentType::Coder,
                4 => AgentType::Optimizer,
                _ => AgentType::Researcher,
            },
            status: AgentStatus::Active,
            capabilities: vec![format!("capability-{}", i)],
            memory_usage: 128 + (i as u64 * 64), // Varying memory usage
            cpu_usage: 0.1 + (i as f32 * 0.01),  // Varying CPU usage
            last_heartbeat: std::time::SystemTime::now(),
        }).collect()
    }

    pub fn generate_stress_test_tasks(&self, count: usize) -> Vec<MockTask> {
        (0..count).map(|i| MockTask {
            id: format!("stress-task-{}", i),
            description: format!("Stress test task number {}", i),
            assigned_agents: vec![format!("load-agent-{}", i % 10)],
            status: if i % 3 == 0 { TaskStatus::Completed } else { TaskStatus::InProgress },
            priority: match i % 4 {
                0 => TaskPriority::Low,
                1 => TaskPriority::Medium,
                2 => TaskPriority::High,
                3 => TaskPriority::Critical,
                _ => TaskPriority::Medium,
            },
            created_at: std::time::SystemTime::now() - std::time::Duration::from_secs(i as u64),
            started_at: Some(std::time::SystemTime::now() - std::time::Duration::from_secs((i as u64) / 2)),
            completed_at: if i % 3 == 0 { 
                Some(std::time::SystemTime::now() - std::time::Duration::from_secs((i as u64) / 4))
            } else { 
                None 
            },
            result: if i % 3 == 0 { 
                Some(format!("Stress test {} completed", i))
            } else { 
                None 
            },
            error: None,
        }).collect()
    }
}

// Neural Model Test Data
impl TestDataGenerator {
    pub fn generate_neural_model_metadata(&self) -> HashMap<String, String> {
        let mut metadata = HashMap::new();
        metadata.insert("model_type".to_string(), "FANN_MLP".to_string());
        metadata.insert("input_size".to_string(), "10".to_string());
        metadata.insert("hidden_layers".to_string(), "2".to_string());
        metadata.insert("output_size".to_string(), "3".to_string());
        metadata.insert("activation_function".to_string(), "sigmoid".to_string());
        metadata.insert("training_algorithm".to_string(), "backpropagation".to_string());
        metadata
    }

    pub fn generate_test_training_data(&self, size: usize) -> Vec<(Vec<f32>, Vec<f32>)> {
        (0..size).map(|i| {
            let input = (0..10).map(|j| (i + j) as f32 * 0.1).collect();
            let output = (0..3).map(|k| ((i + k) % 2) as f32).collect();
            (input, output)
        }).collect()
    }

    pub fn generate_mock_predictions(&self, count: usize) -> Vec<(Vec<f32>, Vec<f32>, f32)> {
        (0..count).map(|i| {
            let input = (0..10).map(|j| (i + j) as f32 * 0.01).collect();
            let output = (0..3).map(|k| ((i + k) % 100) as f32 * 0.01).collect();
            let confidence = 0.5 + (i as f32 * 0.001) % 0.5;
            (input, output, confidence)
        }).collect()
    }
}

// Edge Cases and Error Scenarios
impl TestDataGenerator {
    pub fn generate_invalid_agent_configurations(&self) -> Vec<MockAgent> {
        vec![
            // Agent with empty capabilities
            MockAgent {
                id: "invalid-no-capabilities".to_string(),
                agent_type: AgentType::TddLondon,
                status: AgentStatus::Active,
                capabilities: vec![],
                memory_usage: 256,
                cpu_usage: 0.1,
                last_heartbeat: std::time::SystemTime::now(),
            },
            // Agent with excessive resource usage
            MockAgent {
                id: "invalid-high-resources".to_string(),
                agent_type: AgentType::Analyst,
                status: AgentStatus::Active,
                capabilities: vec!["analysis".to_string()],
                memory_usage: u64::MAX,
                cpu_usage: f32::MAX,
                last_heartbeat: std::time::SystemTime::now(),
            },
            // Agent with very old heartbeat
            MockAgent {
                id: "invalid-stale-heartbeat".to_string(),
                agent_type: AgentType::Coder,
                status: AgentStatus::Active,
                capabilities: vec!["coding".to_string()],
                memory_usage: 256,
                cpu_usage: 0.1,
                last_heartbeat: std::time::SystemTime::UNIX_EPOCH,
            },
        ]
    }

    pub fn generate_boundary_condition_tasks(&self) -> Vec<MockTask> {
        vec![
            // Task with extremely long description
            MockTask {
                id: "boundary-long-description".to_string(),
                description: "x".repeat(10000),
                assigned_agents: vec![],
                status: TaskStatus::Pending,
                priority: TaskPriority::Low,
                created_at: std::time::SystemTime::now(),
                started_at: None,
                completed_at: None,
                result: None,
                error: None,
            },
            // Task with maximum number of assigned agents
            MockTask {
                id: "boundary-many-agents".to_string(),
                description: "Task with many agents".to_string(),
                assigned_agents: (0..1000).map(|i| format!("agent-{}", i)).collect(),
                status: TaskStatus::InProgress,
                priority: TaskPriority::Critical,
                created_at: std::time::SystemTime::now(),
                started_at: Some(std::time::SystemTime::now()),
                completed_at: None,
                result: None,
                error: None,
            },
            // Task with future timestamps (invalid)
            MockTask {
                id: "boundary-future-time".to_string(),
                description: "Task with future timestamp".to_string(),
                assigned_agents: vec!["agent-1".to_string()],
                status: TaskStatus::Completed,
                priority: TaskPriority::Medium,
                created_at: std::time::SystemTime::now() + std::time::Duration::from_secs(3600),
                started_at: Some(std::time::SystemTime::now() + std::time::Duration::from_secs(1800)),
                completed_at: Some(std::time::SystemTime::now() + std::time::Duration::from_secs(900)),
                result: Some("Future task completed".to_string()),
                error: None,
            },
        ]
    }
}

#[cfg(test)]
mod test_data_generator_tests {
    use super::*;

    #[test]
    fn test_generate_tdd_london_agent() {
        let generator = TestDataGenerator::new();
        let agent = generator.generate_tdd_london_agent();
        
        assert_eq!(agent.agent_type, AgentType::TddLondon);
        assert!(agent.capabilities.contains(&"outside-in-development".to_string()));
        assert!(agent.capabilities.contains(&"mock-driven-testing".to_string()));
        assert_eq!(agent.status, AgentStatus::Active);
    }

    #[test]
    fn test_generate_agent_team() {
        let generator = TestDataGenerator::new();
        let team = generator.generate_agent_team(5);
        
        assert_eq!(team.len(), 5);
        // First agent should be coordinator
        assert_eq!(team[0].agent_type, AgentType::Coordinator);
        // Second agent should be TDD London
        assert_eq!(team[1].agent_type, AgentType::TddLondon);
    }

    #[test]
    fn test_generate_task_batch() {
        let generator = TestDataGenerator::new();
        let tasks = generator.generate_task_batch(10);
        
        assert_eq!(tasks.len(), 10);
        assert!(tasks.iter().any(|t| matches!(t.status, TaskStatus::Completed)));
        assert!(tasks.iter().any(|t| matches!(t.status, TaskStatus::Failed)));
    }

    #[test]
    fn test_generate_invalid_configurations() {
        let generator = TestDataGenerator::new();
        let invalid_agents = generator.generate_invalid_agent_configurations();
        
        assert!(!invalid_agents.is_empty());
        assert!(invalid_agents.iter().any(|a| a.capabilities.is_empty()));
        assert!(invalid_agents.iter().any(|a| a.memory_usage == u64::MAX));
    }
}