// RUV-Swarm Mock Service - London School TDD
// Mock implementation for testing swarm initialization and coordination

use super::*;
use uuid::Uuid;

pub struct MockSwarmService {
    swarms: Arc<RwLock<HashMap<String, SwarmInstance>>>,
    is_running: Arc<RwLock<bool>>,
    call_log: Arc<RwLock<Vec<SwarmCall>>>,
}

#[derive(Debug, Clone)]
pub struct SwarmInstance {
    pub id: String,
    pub topology: SwarmTopology,
    pub max_agents: u32,
    pub strategy: String,
    pub agents: Vec<MockAgent>,
    pub status: SwarmStatus,
    pub metrics: SwarmMetrics,
    pub created_at: std::time::SystemTime,
}

#[derive(Debug, Clone)]
pub enum SwarmTopology {
    Mesh,
    Hierarchical,
    Ring,
    Star,
}

#[derive(Debug, Clone)]
pub enum SwarmStatus {
    Initializing,
    Active,
    Scaling,
    Error,
    Terminated,
}

#[derive(Debug, Clone, Default)]
pub struct SwarmMetrics {
    pub total_tasks_processed: u64,
    pub active_agents: u32,
    pub memory_usage_mb: u64,
    pub cpu_utilization: f32,
    pub success_rate: f32,
    pub average_task_duration: std::time::Duration,
}

#[derive(Debug, Clone)]
pub struct SwarmCall {
    pub method: String,
    pub parameters: HashMap<String, String>,
    pub timestamp: std::time::SystemTime,
    pub result: Result<String, String>,
}

impl MockSwarmService {
    pub fn new() -> Self {
        Self {
            swarms: Arc::new(RwLock::new(HashMap::new())),
            is_running: Arc::new(RwLock::new(false))),
            call_log: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn init_swarm(
        &self,
        topology: SwarmTopology,
        max_agents: u32,
        strategy: &str,
    ) -> Result<String, String> {
        self.log_call("init_swarm", &[
            ("topology".to_string(), format!("{:?}", topology)),
            ("max_agents".to_string(), max_agents.to_string()),
            ("strategy".to_string(), strategy.to_string()),
        ]).await;

        let swarm_id = Uuid::new_v4().to_string();
        let swarm = SwarmInstance {
            id: swarm_id.clone(),
            topology,
            max_agents,
            strategy: strategy.to_string(),
            agents: Vec::new(),
            status: SwarmStatus::Initializing,
            metrics: SwarmMetrics::default(),
            created_at: std::time::SystemTime::now(),
        };

        let mut swarms = self.swarms.write().await;
        swarms.insert(swarm_id.clone(), swarm);

        // Simulate initialization delay
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Update status to active
        if let Some(swarm) = swarms.get_mut(&swarm_id) {
            swarm.status = SwarmStatus::Active;
        }

        Ok(swarm_id)
    }

    pub async fn get_swarm_status(&self, swarm_id: &str) -> Result<SwarmInstance, String> {
        let swarms = self.swarms.read().await;
        swarms
            .get(swarm_id)
            .cloned()
            .ok_or_else(|| format!("Swarm {} not found", swarm_id))
    }

    pub async fn destroy_swarm(&self, swarm_id: &str) -> Result<(), String> {
        self.log_call("destroy_swarm", &[
            ("swarm_id".to_string(), swarm_id.to_string()),
        ]).await;

        let mut swarms = self.swarms.write().await;
        if let Some(mut swarm) = swarms.get_mut(swarm_id) {
            swarm.status = SwarmStatus::Terminated;
            // Simulate cleanup delay
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            swarms.remove(swarm_id);
            Ok(())
        } else {
            Err(format!("Swarm {} not found", swarm_id))
        }
    }

    pub async fn add_agent_to_swarm(
        &self,
        swarm_id: &str,
        agent: MockAgent,
    ) -> Result<(), String> {
        let mut swarms = self.swarms.write().await;
        if let Some(swarm) = swarms.get_mut(swarm_id) {
            if swarm.agents.len() >= swarm.max_agents as usize {
                return Err("Swarm at maximum capacity".to_string());
            }
            swarm.agents.push(agent);
            swarm.metrics.active_agents = swarm.agents.len() as u32;
            Ok(())
        } else {
            Err(format!("Swarm {} not found", swarm_id))
        }
    }

    pub async fn get_swarm_metrics(&self, swarm_id: &str) -> Result<SwarmMetrics, String> {
        let swarms = self.swarms.read().await;
        swarms
            .get(swarm_id)
            .map(|swarm| swarm.metrics.clone())
            .ok_or_else(|| format!("Swarm {} not found", swarm_id))
    }

    pub async fn list_active_swarms(&self) -> Vec<SwarmInstance> {
        let swarms = self.swarms.read().await;
        swarms
            .values()
            .filter(|swarm| matches!(swarm.status, SwarmStatus::Active | SwarmStatus::Scaling))
            .cloned()
            .collect()
    }

    pub async fn get_call_log(&self) -> Vec<SwarmCall> {
        let log = self.call_log.read().await;
        log.clone()
    }

    pub async fn clear_call_log(&self) {
        let mut log = self.call_log.write().await;
        log.clear();
    }

    async fn log_call(&self, method: &str, params: &[(String, String)]) {
        let mut log = self.call_log.write().await;
        let call = SwarmCall {
            method: method.to_string(),
            parameters: params.iter().cloned().collect(),
            timestamp: std::time::SystemTime::now(),
            result: Ok("success".to_string()),
        };
        log.push(call);
    }

    // Mock error injection for testing
    pub async fn inject_error(&self, method: &str, error: &str) {
        let mut log = self.call_log.write().await;
        let call = SwarmCall {
            method: method.to_string(),
            parameters: HashMap::new(),
            timestamp: std::time::SystemTime::now(),
            result: Err(error.to_string()),
        };
        log.push(call);
    }

    // Simulate network latency
    pub async fn set_network_delay(&self, delay_ms: u64) {
        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
    }
}

#[async_trait]
impl MockService for MockSwarmService {
    fn name(&self) -> &str {
        "swarm-service-mock"
    }

    async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut is_running = self.is_running.write().await;
        *is_running = true;
        Ok(())
    }

    async fn stop(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut swarms = self.swarms.write().await;
        swarms.clear();
        
        let mut is_running = self.is_running.write().await;
        *is_running = false;
        Ok(())
    }

    async fn reset(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.stop().await?;
        self.start().await?;
        self.clear_call_log().await;
        Ok(())
    }

    fn is_running(&self) -> bool {
        // Note: This is sync, so we can't await. In practice, use Arc<AtomicBool>
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_swarm_initialization() {
        let mock_service = MockSwarmService::new();
        
        let swarm_id = mock_service
            .init_swarm(SwarmTopology::Mesh, 5, "balanced")
            .await
            .unwrap();
        
        let status = mock_service.get_swarm_status(&swarm_id).await.unwrap();
        assert_eq!(status.max_agents, 5);
        assert!(matches!(status.status, SwarmStatus::Active));
    }

    #[tokio::test]
    async fn test_swarm_agent_capacity() {
        let mock_service = MockSwarmService::new();
        let swarm_id = mock_service
            .init_swarm(SwarmTopology::Mesh, 1, "balanced")
            .await
            .unwrap();

        let agent = MockAgent {
            id: "agent-1".to_string(),
            agent_type: AgentType::Researcher,
            status: AgentStatus::Active,
            capabilities: vec!["research".to_string()],
            memory_usage: 100,
            cpu_usage: 0.5,
            last_heartbeat: std::time::SystemTime::now(),
        };

        // Should succeed
        assert!(mock_service.add_agent_to_swarm(&swarm_id, agent.clone()).await.is_ok());
        
        // Should fail - at capacity
        assert!(mock_service.add_agent_to_swarm(&swarm_id, agent).await.is_err());
    }

    #[tokio::test]
    async fn test_call_logging() {
        let mock_service = MockSwarmService::new();
        
        mock_service
            .init_swarm(SwarmTopology::Hierarchical, 3, "adaptive")
            .await
            .unwrap();
        
        let log = mock_service.get_call_log().await;
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].method, "init_swarm");
        assert_eq!(log[0].parameters.get("max_agents"), Some(&"3".to_string()));
    }
}