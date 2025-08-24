//! Comprehensive tests for DAA Coordinator fault tolerance mechanisms
//! Tests Byzantine fault tolerance, network partitions, agent failures, and recovery mechanisms

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Mutex};
use tokio::time::timeout;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub enum AgentStatus {
    Active,
    Failed,
    Byzantine,
    NetworkPartitioned,
    Recovering,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TradeDirection {
    Long,
    Short,
    Hold,
}

#[derive(Debug, Clone)]
pub struct Agent {
    pub id: String,
    pub agent_type: String,
    pub status: AgentStatus,
    pub failure_count: u32,
    pub last_heartbeat: Instant,
    pub performance_score: f64,
    pub trust_score: f64,
}

#[derive(Debug, Clone)]
pub struct AgentMessage {
    pub from: String,
    pub to: String,
    pub message_type: MessageType,
    pub payload: String,
    pub timestamp: Instant,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MessageType {
    Heartbeat,
    Vote,
    Consensus,
    Recovery,
    Challenge,
    Response,
}

#[derive(Debug, Clone)]
pub struct FaultToleranceConfig {
    pub max_byzantine_ratio: f64,  // Maximum percentage of Byzantine agents (typically 0.33)
    pub heartbeat_interval: Duration,
    pub failure_detection_timeout: Duration,
    pub recovery_timeout: Duration,
    pub max_retry_attempts: u32,
    pub min_consensus_participants: usize,
}

#[derive(Debug)]
pub enum FaultType {
    AgentFailure(String),
    NetworkPartition(Vec<String>),
    ByzantineAttack(Vec<String>),
    MessageDelay,
    MessageLoss,
    ResourceExhaustion,
}

pub struct MockFaultTolerantCoordinator {
    agents: Arc<RwLock<HashMap<String, Agent>>>,
    network: Arc<RwLock<MockNetwork>>,
    config: FaultToleranceConfig,
    byzantine_detector: ByzantineDetector,
    failure_detector: FailureDetector,
    recovery_manager: RecoveryManager,
    message_log: Arc<Mutex<Vec<AgentMessage>>>,
}

pub struct MockNetwork {
    partitions: Vec<HashSet<String>>,
    message_delay: Duration,
    message_loss_rate: f64,
    is_partitioned: bool,
}

pub struct ByzantineDetector {
    suspicious_agents: HashMap<String, u32>,
    detection_threshold: u32,
    challenge_responses: HashMap<String, String>,
}

pub struct FailureDetector {
    failure_timeouts: HashMap<String, Instant>,
    heartbeat_history: HashMap<String, Vec<Instant>>,
}

pub struct RecoveryManager {
    recovering_agents: HashMap<String, RecoveryState>,
    recovery_strategies: HashMap<String, RecoveryStrategy>,
}

#[derive(Debug, Clone)]
pub struct RecoveryState {
    pub agent_id: String,
    pub recovery_start: Instant,
    pub attempts: u32,
    pub strategy: RecoveryStrategy,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecoveryStrategy {
    Restart,
    StateSync,
    Checkpoint,
    Replacement,
}

impl MockFaultTolerantCoordinator {
    pub fn new(config: FaultToleranceConfig) -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            network: Arc::new(RwLock::new(MockNetwork {
                partitions: Vec::new(),
                message_delay: Duration::from_millis(0),
                message_loss_rate: 0.0,
                is_partitioned: false,
            })),
            config,
            byzantine_detector: ByzantineDetector {
                suspicious_agents: HashMap::new(),
                detection_threshold: 3,
                challenge_responses: HashMap::new(),
            },
            failure_detector: FailureDetector {
                failure_timeouts: HashMap::new(),
                heartbeat_history: HashMap::new(),
            },
            recovery_manager: RecoveryManager {
                recovering_agents: HashMap::new(),
                recovery_strategies: HashMap::new(),
            },
            message_log: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn register_agent(&self, mut agent: Agent) {
        agent.last_heartbeat = Instant::now();
        let mut agents = self.agents.write().await;
        agents.insert(agent.id.clone(), agent);
    }

    pub async fn inject_fault(&self, fault: FaultType) -> Result<(), String> {
        match fault {
            FaultType::AgentFailure(agent_id) => {
                let mut agents = self.agents.write().await;
                if let Some(agent) = agents.get_mut(&agent_id) {
                    agent.status = AgentStatus::Failed;
                    agent.failure_count += 1;
                }
            }
            FaultType::NetworkPartition(partition_agents) => {
                let mut network = self.network.write().await;
                network.is_partitioned = true;
                network.partitions.push(partition_agents.into_iter().collect());
            }
            FaultType::ByzantineAttack(byzantine_agents) => {
                let mut agents = self.agents.write().await;
                for agent_id in byzantine_agents {
                    if let Some(agent) = agents.get_mut(&agent_id) {
                        agent.status = AgentStatus::Byzantine;
                        agent.trust_score *= 0.1; // Significantly reduce trust
                    }
                }
            }
            FaultType::MessageDelay => {
                let mut network = self.network.write().await;
                network.message_delay = Duration::from_millis(100);
            }
            FaultType::MessageLoss => {
                let mut network = self.network.write().await;
                network.message_loss_rate = 0.2; // 20% message loss
            }
            FaultType::ResourceExhaustion => {
                // Simulate resource exhaustion by marking random agents as failed
                let agents_copy = {
                    let agents = self.agents.read().await;
                    agents.keys().cloned().collect::<Vec<_>>()
                };
                
                if !agents_copy.is_empty() {
                    let victim_id = &agents_copy[0];
                    let mut agents = self.agents.write().await;
                    if let Some(agent) = agents.get_mut(victim_id) {
                        agent.status = AgentStatus::Failed;
                        agent.failure_count += 1;
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn send_message(&self, message: AgentMessage) -> Result<(), String> {
        let network = self.network.read().await;
        
        // Simulate message loss
        if network.message_loss_rate > 0.0 {
            use rand::Rng;
            let mut rng = rand::thread_rng();
            if rng.gen::<f64>() < network.message_loss_rate {
                return Err("Message lost due to network issues".to_string());
            }
        }

        // Check for network partitions
        if network.is_partitioned {
            let sender_partition = network.partitions.iter()
                .find(|partition| partition.contains(&message.from));
            let receiver_partition = network.partitions.iter()
                .find(|partition| partition.contains(&message.to));

            if sender_partition != receiver_partition {
                return Err("Message blocked by network partition".to_string());
            }
        }

        // Simulate message delay
        if network.message_delay > Duration::from_millis(0) {
            tokio::time::sleep(network.message_delay).await;
        }

        // Log the message
        let mut message_log = self.message_log.lock().await;
        message_log.push(message);

        Ok(())
    }

    pub async fn detect_failures(&mut self) -> Vec<String> {
        let mut failed_agents = Vec::new();
        let now = Instant::now();
        
        let mut agents = self.agents.write().await;
        for (agent_id, agent) in agents.iter_mut() {
            if agent.status == AgentStatus::Active {
                let time_since_heartbeat = now.duration_since(agent.last_heartbeat);
                
                if time_since_heartbeat > self.config.failure_detection_timeout {
                    agent.status = AgentStatus::Failed;
                    agent.failure_count += 1;
                    failed_agents.push(agent_id.clone());
                }
            }
        }

        // Update failure detector state
        for agent_id in &failed_agents {
            self.failure_detector.failure_timeouts.insert(agent_id.clone(), now);
        }

        failed_agents
    }

    pub async fn detect_byzantine_agents(&mut self) -> Vec<String> {
        let mut byzantine_agents = Vec::new();
        
        // Challenge-response mechanism
        let agents_to_challenge: Vec<String> = {
            let agents = self.agents.read().await;
            agents.iter()
                .filter(|(_, agent)| agent.status == AgentStatus::Active && agent.trust_score < 0.7)
                .map(|(id, _)| id.clone())
                .collect()
        };

        for agent_id in agents_to_challenge {
            let challenge = format!("challenge_{}", Uuid::new_v4());
            let expected_response = format!("response_{}", challenge);
            
            // Send challenge
            let challenge_msg = AgentMessage {
                from: "coordinator".to_string(),
                to: agent_id.clone(),
                message_type: MessageType::Challenge,
                payload: challenge.clone(),
                timestamp: Instant::now(),
                signature: "coordinator_sig".to_string(),
            };

            if self.send_message(challenge_msg).await.is_ok() {
                // Wait for response (simplified)
                tokio::time::sleep(Duration::from_millis(10)).await;
                
                // Check if response was received (in real implementation, this would be event-driven)
                if !self.byzantine_detector.challenge_responses.contains_key(&agent_id) {
                    // No response or invalid response - mark as suspicious
                    let suspicion_count = self.byzantine_detector.suspicious_agents
                        .entry(agent_id.clone())
                        .or_insert(0);
                    *suspicion_count += 1;

                    if *suspicion_count >= self.byzantine_detector.detection_threshold {
                        byzantine_agents.push(agent_id.clone());
                        
                        // Mark agent as Byzantine
                        let mut agents = self.agents.write().await;
                        if let Some(agent) = agents.get_mut(&agent_id) {
                            agent.status = AgentStatus::Byzantine;
                            agent.trust_score = 0.0;
                        }
                    }
                }
            }
        }

        byzantine_agents
    }

    pub async fn attempt_recovery(&mut self, failed_agent_id: &str) -> Result<RecoveryState, String> {
        let recovery_strategy = self.determine_recovery_strategy(failed_agent_id).await;
        
        let recovery_state = RecoveryState {
            agent_id: failed_agent_id.to_string(),
            recovery_start: Instant::now(),
            attempts: 1,
            strategy: recovery_strategy.clone(),
        };

        match recovery_strategy {
            RecoveryStrategy::Restart => {
                self.restart_agent(failed_agent_id).await?;
            }
            RecoveryStrategy::StateSync => {
                self.sync_agent_state(failed_agent_id).await?;
            }
            RecoveryStrategy::Checkpoint => {
                self.restore_from_checkpoint(failed_agent_id).await?;
            }
            RecoveryStrategy::Replacement => {
                self.replace_agent(failed_agent_id).await?;
            }
        }

        self.recovery_manager.recovering_agents.insert(
            failed_agent_id.to_string(),
            recovery_state.clone(),
        );

        Ok(recovery_state)
    }

    async fn determine_recovery_strategy(&self, agent_id: &str) -> RecoveryStrategy {
        let agents = self.agents.read().await;
        if let Some(agent) = agents.get(agent_id) {
            match agent.failure_count {
                1 => RecoveryStrategy::Restart,
                2 => RecoveryStrategy::StateSync,
                3 => RecoveryStrategy::Checkpoint,
                _ => RecoveryStrategy::Replacement,
            }
        } else {
            RecoveryStrategy::Replacement
        }
    }

    async fn restart_agent(&self, agent_id: &str) -> Result<(), String> {
        let mut agents = self.agents.write().await;
        if let Some(agent) = agents.get_mut(agent_id) {
            agent.status = AgentStatus::Recovering;
            agent.last_heartbeat = Instant::now();
            // Simulate restart delay
            tokio::time::sleep(Duration::from_millis(100)).await;
            agent.status = AgentStatus::Active;
        }
        Ok(())
    }

    async fn sync_agent_state(&self, agent_id: &str) -> Result<(), String> {
        // Simulate state synchronization
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        let mut agents = self.agents.write().await;
        if let Some(agent) = agents.get_mut(agent_id) {
            agent.status = AgentStatus::Active;
            agent.last_heartbeat = Instant::now();
        }
        Ok(())
    }

    async fn restore_from_checkpoint(&self, agent_id: &str) -> Result<(), String> {
        // Simulate checkpoint restoration
        tokio::time::sleep(Duration::from_millis(300)).await;
        
        let mut agents = self.agents.write().await;
        if let Some(agent) = agents.get_mut(agent_id) {
            agent.status = AgentStatus::Active;
            agent.last_heartbeat = Instant::now();
            agent.failure_count = 0; // Reset failure count after successful recovery
        }
        Ok(())
    }

    async fn replace_agent(&self, failed_agent_id: &str) -> Result<(), String> {
        let mut agents = self.agents.write().await;
        
        // Remove the failed agent
        agents.remove(failed_agent_id);
        
        // Create replacement agent
        let replacement_agent = Agent {
            id: format!("{}_replacement", failed_agent_id),
            agent_type: "replacement".to_string(),
            status: AgentStatus::Active,
            failure_count: 0,
            last_heartbeat: Instant::now(),
            performance_score: 0.5, // Start with neutral performance
            trust_score: 0.8, // High initial trust for replacement
        };

        agents.insert(replacement_agent.id.clone(), replacement_agent);
        Ok(())
    }

    pub async fn check_byzantine_tolerance(&self) -> Result<bool, String> {
        let agents = self.agents.read().await;
        let total_agents = agents.len();
        let byzantine_agents = agents.values()
            .filter(|agent| agent.status == AgentStatus::Byzantine)
            .count();

        let byzantine_ratio = if total_agents > 0 {
            byzantine_agents as f64 / total_agents as f64
        } else {
            0.0
        };

        Ok(byzantine_ratio <= self.config.max_byzantine_ratio)
    }

    pub async fn get_healthy_agents(&self) -> Vec<String> {
        let agents = self.agents.read().await;
        agents.iter()
            .filter(|(_, agent)| agent.status == AgentStatus::Active && agent.trust_score > 0.5)
            .map(|(id, _)| id.clone())
            .collect()
    }

    pub async fn simulate_heartbeat(&self, agent_id: &str) -> Result<(), String> {
        let mut agents = self.agents.write().await;
        if let Some(agent) = agents.get_mut(agent_id) {
            agent.last_heartbeat = Instant::now();
            
            let heartbeat_msg = AgentMessage {
                from: agent_id.to_string(),
                to: "coordinator".to_string(),
                message_type: MessageType::Heartbeat,
                payload: "alive".to_string(),
                timestamp: Instant::now(),
                signature: format!("{}_sig", agent_id),
            };

            // Don't fail if heartbeat can't be sent due to network issues
            let _ = self.send_message(heartbeat_msg).await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::test;

    fn create_test_config() -> FaultToleranceConfig {
        FaultToleranceConfig {
            max_byzantine_ratio: 0.33,
            heartbeat_interval: Duration::from_millis(100),
            failure_detection_timeout: Duration::from_millis(500),
            recovery_timeout: Duration::from_secs(5),
            max_retry_attempts: 3,
            min_consensus_participants: 2,
        }
    }

    fn create_test_agent(id: &str, agent_type: &str) -> Agent {
        Agent {
            id: id.to_string(),
            agent_type: agent_type.to_string(),
            status: AgentStatus::Active,
            failure_count: 0,
            last_heartbeat: Instant::now(),
            performance_score: 0.8,
            trust_score: 0.9,
        }
    }

    #[test]
    async fn test_agent_failure_detection() {
        let config = create_test_config();
        let mut coordinator = MockFaultTolerantCoordinator::new(config);

        // Register test agents
        let agent1 = create_test_agent("agent_1", "neural_model");
        let agent2 = create_test_agent("agent_2", "strategy");
        
        coordinator.register_agent(agent1).await;
        coordinator.register_agent(agent2).await;

        // Simulate failure of agent_1
        coordinator.inject_fault(FaultType::AgentFailure("agent_1".to_string())).await.unwrap();

        // Wait longer than failure detection timeout
        tokio::time::sleep(Duration::from_millis(600)).await;

        let failed_agents = coordinator.detect_failures().await;
        assert!(failed_agents.contains(&"agent_1".to_string()));
    }

    #[test]
    async fn test_byzantine_agent_detection() {
        let config = create_test_config();
        let mut coordinator = MockFaultTolerantCoordinator::new(config);

        // Register test agents
        let agent1 = create_test_agent("honest_agent", "neural_model");
        let mut byzantine_agent = create_test_agent("byzantine_agent", "strategy");
        byzantine_agent.trust_score = 0.6; // Low trust score

        coordinator.register_agent(agent1).await;
        coordinator.register_agent(byzantine_agent).await;

        // Inject Byzantine attack
        coordinator.inject_fault(FaultType::ByzantineAttack(vec!["byzantine_agent".to_string()])).await.unwrap();

        let byzantine_agents = coordinator.detect_byzantine_agents().await;
        assert!(byzantine_agents.contains(&"byzantine_agent".to_string()));
    }

    #[test]
    async fn test_network_partition_handling() {
        let config = create_test_config();
        let coordinator = MockFaultTolerantCoordinator::new(config);

        // Register agents in different partitions
        coordinator.register_agent(create_test_agent("agent_partition_1", "model")).await;
        coordinator.register_agent(create_test_agent("agent_partition_2", "model")).await;

        // Inject network partition
        coordinator.inject_fault(FaultType::NetworkPartition(vec![
            "agent_partition_1".to_string()
        ])).await.unwrap();

        // Try to send message across partition
        let message = AgentMessage {
            from: "agent_partition_1".to_string(),
            to: "agent_partition_2".to_string(),
            message_type: MessageType::Vote,
            payload: "test".to_string(),
            timestamp: Instant::now(),
            signature: "test_sig".to_string(),
        };

        let result = coordinator.send_message(message).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("network partition"));
    }

    #[test]
    async fn test_message_loss_handling() {
        let config = create_test_config();
        let coordinator = MockFaultTolerantCoordinator::new(config);

        coordinator.register_agent(create_test_agent("sender", "model")).await;
        coordinator.register_agent(create_test_agent("receiver", "model")).await;

        // Inject message loss
        coordinator.inject_fault(FaultType::MessageLoss).await.unwrap();

        let mut successful_sends = 0;
        let mut failed_sends = 0;

        // Send multiple messages to test loss rate
        for i in 0..100 {
            let message = AgentMessage {
                from: "sender".to_string(),
                to: "receiver".to_string(),
                message_type: MessageType::Vote,
                payload: format!("message_{}", i),
                timestamp: Instant::now(),
                signature: "test_sig".to_string(),
            };

            match coordinator.send_message(message).await {
                Ok(_) => successful_sends += 1,
                Err(_) => failed_sends += 1,
            }
        }

        // Should have approximately 20% message loss (80% success)
        assert!(successful_sends > 70);
        assert!(failed_sends > 10);
    }

    #[test]
    async fn test_agent_recovery_restart_strategy() {
        let config = create_test_config();
        let mut coordinator = MockFaultTolerantCoordinator::new(config);

        let agent = create_test_agent("recoverable_agent", "model");
        coordinator.register_agent(agent).await;

        // Inject failure
        coordinator.inject_fault(FaultType::AgentFailure("recoverable_agent".to_string())).await.unwrap();

        // Attempt recovery
        let recovery_result = coordinator.attempt_recovery("recoverable_agent").await;
        assert!(recovery_result.is_ok());

        let recovery_state = recovery_result.unwrap();
        assert_eq!(recovery_state.strategy, RecoveryStrategy::Restart);
        assert_eq!(recovery_state.attempts, 1);
    }

    #[test]
    async fn test_agent_replacement_strategy() {
        let config = create_test_config();
        let mut coordinator = MockFaultTolerantCoordinator::new(config);

        let mut problematic_agent = create_test_agent("problematic_agent", "model");
        problematic_agent.failure_count = 5; // High failure count
        coordinator.register_agent(problematic_agent).await;

        // Attempt recovery - should use replacement strategy
        let recovery_result = coordinator.attempt_recovery("problematic_agent").await;
        assert!(recovery_result.is_ok());

        let recovery_state = recovery_result.unwrap();
        assert_eq!(recovery_state.strategy, RecoveryStrategy::Replacement);

        // Check that replacement agent was created
        let healthy_agents = coordinator.get_healthy_agents().await;
        assert!(healthy_agents.iter().any(|id| id.contains("replacement")));
    }

    #[test]
    async fn test_byzantine_tolerance_threshold() {
        let config = create_test_config();
        let coordinator = MockFaultTolerantCoordinator::new(config);

        // Register 4 agents
        for i in 1..=4 {
            coordinator.register_agent(create_test_agent(&format!("agent_{}", i), "model")).await;
        }

        // Mark 1 agent as Byzantine (25% - within tolerance)
        coordinator.inject_fault(FaultType::ByzantineAttack(vec!["agent_1".to_string()])).await.unwrap();

        let is_tolerant = coordinator.check_byzantine_tolerance().await.unwrap();
        assert!(is_tolerant);

        // Mark another agent as Byzantine (50% - exceeds tolerance)
        coordinator.inject_fault(FaultType::ByzantineAttack(vec!["agent_2".to_string()])).await.unwrap();

        let is_tolerant = coordinator.check_byzantine_tolerance().await.unwrap();
        assert!(!is_tolerant);
    }

    #[test]
    async fn test_heartbeat_mechanism() {
        let config = create_test_config();
        let coordinator = MockFaultTolerantCoordinator::new(config);

        coordinator.register_agent(create_test_agent("heartbeat_agent", "model")).await;

        // Simulate heartbeat
        let result = coordinator.simulate_heartbeat("heartbeat_agent").await;
        assert!(result.is_ok());

        // Check that agent is still active
        let healthy_agents = coordinator.get_healthy_agents().await;
        assert!(healthy_agents.contains(&"heartbeat_agent".to_string()));
    }

    #[test]
    async fn test_concurrent_fault_injection_and_recovery() {
        let config = create_test_config();
        let coordinator = Arc::new(RwLock::new(MockFaultTolerantCoordinator::new(config)));

        // Register multiple agents
        for i in 1..=10 {
            let coord = coordinator.read().await;
            coord.register_agent(create_test_agent(&format!("agent_{}", i), "model")).await;
        }

        // Concurrent fault injection and recovery
        let mut handles = Vec::new();

        for i in 1..=5 {
            let coord = coordinator.clone();
            let handle = tokio::spawn(async move {
                let mut coord = coord.write().await;
                
                // Inject failure
                coord.inject_fault(FaultType::AgentFailure(format!("agent_{}", i))).await.unwrap();
                
                // Attempt recovery
                coord.attempt_recovery(&format!("agent_{}", i)).await
            });
            handles.push(handle);
        }

        let results = futures::future::join_all(handles).await;
        
        // All recovery attempts should succeed
        for result in results {
            assert!(result.is_ok());
            assert!(result.unwrap().is_ok());
        }
    }

    #[test]
    async fn test_fault_tolerance_performance_sla() {
        let config = create_test_config();
        let mut coordinator = MockFaultTolerantCoordinator::new(config);

        // Register agents
        for i in 1..=20 {
            coordinator.register_agent(create_test_agent(&format!("agent_{}", i), "model")).await;
        }

        // Test fault detection performance
        let start = Instant::now();
        
        // Inject multiple failures
        for i in 1..=10 {
            coordinator.inject_fault(FaultType::AgentFailure(format!("agent_{}", i))).await.unwrap();
        }

        let failed_agents = coordinator.detect_failures().await;
        let detection_time = start.elapsed();

        assert_eq!(failed_agents.len(), 10);
        assert!(detection_time < Duration::from_millis(50)); // Fast detection
    }

    #[test]
    async fn test_recovery_timeout_handling() {
        let mut config = create_test_config();
        config.recovery_timeout = Duration::from_millis(100); // Short timeout
        
        let mut coordinator = MockFaultTolerantCoordinator::new(config);
        coordinator.register_agent(create_test_agent("timeout_agent", "model")).await;

        // Inject failure
        coordinator.inject_fault(FaultType::AgentFailure("timeout_agent".to_string())).await.unwrap();

        // Attempt recovery with timeout
        let recovery_start = Instant::now();
        let result = timeout(
            Duration::from_millis(200),
            coordinator.attempt_recovery("timeout_agent")
        ).await;

        assert!(result.is_ok()); // Should complete within timeout
        assert!(recovery_start.elapsed() < Duration::from_millis(200));
    }

    #[test]
    async fn test_cascading_failure_handling() {
        let config = create_test_config();
        let mut coordinator = MockFaultTolerantCoordinator::new(config);

        // Register agents with dependencies
        for i in 1..=5 {
            coordinator.register_agent(create_test_agent(&format!("agent_{}", i), "model")).await;
        }

        // Simulate cascading failures
        for i in 1..=3 {
            coordinator.inject_fault(FaultType::AgentFailure(format!("agent_{}", i))).await.unwrap();
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        // System should still have healthy agents
        let healthy_agents = coordinator.get_healthy_agents().await;
        assert!(healthy_agents.len() >= 2); // At least 2 agents should remain healthy

        // Byzantine tolerance should still be maintained
        let is_tolerant = coordinator.check_byzantine_tolerance().await.unwrap();
        assert!(is_tolerant);
    }

    #[test]
    async fn test_resource_exhaustion_recovery() {
        let config = create_test_config();
        let mut coordinator = MockFaultTolerantCoordinator::new(config);

        // Register agents
        for i in 1..=3 {
            coordinator.register_agent(create_test_agent(&format!("agent_{}", i), "model")).await;
        }

        // Inject resource exhaustion
        coordinator.inject_fault(FaultType::ResourceExhaustion).await.unwrap();

        // Detect failures caused by resource exhaustion
        tokio::time::sleep(Duration::from_millis(600)).await;
        let failed_agents = coordinator.detect_failures().await;

        assert!(!failed_agents.is_empty());

        // Attempt recovery of failed agents
        for failed_agent in &failed_agents {
            let recovery_result = coordinator.attempt_recovery(failed_agent).await;
            assert!(recovery_result.is_ok());
        }

        // Should have healthy agents after recovery
        let healthy_agents = coordinator.get_healthy_agents().await;
        assert!(!healthy_agents.is_empty());
    }

    #[test]
    async fn test_message_delay_tolerance() {
        let config = create_test_config();
        let coordinator = MockFaultTolerantCoordinator::new(config);

        coordinator.register_agent(create_test_agent("sender", "model")).await;
        coordinator.register_agent(create_test_agent("receiver", "model")).await;

        // Inject message delay
        coordinator.inject_fault(FaultType::MessageDelay).await.unwrap();

        let start = Instant::now();
        let message = AgentMessage {
            from: "sender".to_string(),
            to: "receiver".to_string(),
            message_type: MessageType::Vote,
            payload: "delayed_message".to_string(),
            timestamp: Instant::now(),
            signature: "test_sig".to_string(),
        };

        let result = coordinator.send_message(message).await;
        let elapsed = start.elapsed();

        assert!(result.is_ok());
        assert!(elapsed >= Duration::from_millis(100)); // Message was delayed
    }
}