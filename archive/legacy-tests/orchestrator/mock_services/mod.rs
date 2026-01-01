// Mock Services Framework - London School TDD
// Implements comprehensive mocking for RUV-Swarm orchestrator testing

use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::RwLock;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

// Core mock service trait
#[async_trait]
pub trait MockService: Send + Sync {
    fn name(&self) -> &str;
    async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    async fn stop(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    async fn reset(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn is_running(&self) -> bool;
}

// Mock registry for coordinating all services
pub struct MockRegistry {
    services: Arc<RwLock<HashMap<String, Box<dyn MockService>>>>,
    is_initialized: Arc<RwLock<bool>>,
}

impl MockRegistry {
    pub fn new() -> Self {
        Self {
            services: Arc::new(RwLock::new(HashMap::new())),
            is_initialized: Arc::new(RwLock::new(false)),
        }
    }
    
    pub async fn register(&self, service: Box<dyn MockService>) {
        let mut services = self.services.write().await;
        services.insert(service.name().to_string(), service);
    }

    pub async fn start_all(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut is_initialized = self.is_initialized.write().await;
        if *is_initialized {
            return Ok(());
        }
        
        let services = self.services.read().await;
        for service in services.values() {
            service.start().await?;
        }
        
        *is_initialized = true;
        Ok(())
    }

    pub async fn stop_all(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let services = self.services.read().await;
        for service in services.values() {
            service.stop().await?;
        }
        
        let mut is_initialized = self.is_initialized.write().await;
        *is_initialized = false;
        Ok(())
    }

    pub async fn reset_all(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let services = self.services.read().await;
        for service in services.values() {
            service.reset().await?;
        }
        Ok(())
    }

    pub async fn get_service<T>(&self, name: &str) -> Option<T> 
    where 
        T: 'static
    {
        // This would need proper casting in real implementation
        None
    }
}

// Mock RUV-Swarm agent types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockAgent {
    pub id: String,
    pub agent_type: AgentType,
    pub status: AgentStatus,
    pub capabilities: Vec<String>,
    pub memory_usage: u64,
    pub cpu_usage: f32,
    pub last_heartbeat: std::time::SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentType {
    Researcher,
    Coder,
    Analyst,
    Optimizer,
    Coordinator,
    TddLondon,
    Architect,
    Reviewer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentStatus {
    Initializing,
    Active,
    Idle,
    Busy,
    Error,
    Offline,
}

// Mock task orchestration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockTask {
    pub id: String,
    pub description: String,
    pub assigned_agents: Vec<String>,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub created_at: std::time::SystemTime,
    pub started_at: Option<std::time::SystemTime>,
    pub completed_at: Option<std::time::SystemTime>,
    pub result: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskPriority {
    Low,
    Medium,
    High,
    Critical,
}

// Import specific mock services
pub mod swarm_mock;
pub mod agent_mock;
pub mod task_mock;
pub mod neural_mock;
pub mod redis_mock;
pub mod memory_mock;
pub mod coordination_mock;