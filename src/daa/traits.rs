//! Core traits for Decentralized Autonomous Agents
//! 
//! This module defines the fundamental traits that all autonomous agents
//! must implement for the DAA trading system.

use async_trait::async_trait;
use anyhow::Result;
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

/// Core trait that all autonomous agents must implement
#[async_trait]
pub trait AutonomousAgent: Send + Sync {
    /// Unique identifier for the agent
    fn id(&self) -> &str;
    
    /// Type of agent (e.g., "ArbitrageHunter", "MomentumTrader")
    fn agent_type(&self) -> &str;
    
    /// Check agent health status
    async fn health_check(&self) -> Result<HealthStatus>;
    
    /// Get current performance metrics
    async fn get_metrics(&self) -> HashMap<String, f64>;
}

/// Enhanced capabilities for advanced autonomous agents
#[async_trait]
pub trait EnhancedAgentCapabilities: AutonomousAgent {
    /// Learn from market data and improve strategies
    async fn learn_from_market(&mut self, market_data: &MarketData) -> LearningOutcome;
    
    /// Adapt internal parameters based on performance
    async fn adapt_parameters(&mut self, performance: &Performance) -> AdaptationResult;
    
    /// Negotiate with peer agents for consensus
    async fn negotiate_with_peers(&self, proposal: &Proposal) -> ConsensusResult;
    
    /// Share discovered insights with other agents
    async fn share_insights(&self, insights: &Insights) -> SharingResult;
    
    /// Evaluate potential trading opportunities
    async fn evaluate_opportunities(&self, context: &MarketContext) -> Vec<Opportunity>;
    
    /// Execute trading decisions autonomously
    async fn execute_autonomously(&mut self, opportunity: &Opportunity) -> ExecutionResult;
    
    /// Self-diagnose performance issues
    async fn diagnose_performance(&self) -> HealthStatus;
    
    /// Optimize internal parameters and strategies
    async fn self_optimize(&mut self) -> OptimizationResult;
}

/// Agent health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Agent is functioning normally
    Healthy,
    /// Agent is operational but with degraded performance
    Degraded(String),
    /// Agent has critical issues requiring intervention
    Critical(String),
    /// Health status cannot be determined
    Unknown,
}

/// Market data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketData {
    pub timestamp: DateTime<Utc>,
    pub symbol: String,
    pub price: f64,
    pub volume: f64,
    pub bid: f64,
    pub ask: f64,
    pub high_24h: f64,
    pub low_24h: f64,
    pub indicators: HashMap<String, f64>,
}

/// Learning outcome from market analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningOutcome {
    /// Number of new patterns discovered
    pub patterns_discovered: usize,
    /// Estimated performance improvement percentage
    pub performance_improvement: f64,
    /// New strategies developed
    pub new_strategies: Vec<String>,
}

/// Result of parameter adaptation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptationResult {
    /// Whether any parameters were changed
    pub parameters_changed: bool,
    /// New parameter values
    pub new_values: HashMap<String, f64>,
}

/// Agent performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Performance {
    /// Win rate (0.0 to 1.0)
    pub success_rate: f64,
    /// Average profit per trade
    pub avg_profit: f64,
    /// Total number of trades
    pub total_trades: u64,
}

/// Proposal for inter-agent negotiation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    /// Type of proposal (e.g., "position_size", "risk_limit")
    pub proposal_type: String,
    /// Proposed value
    pub value: serde_json::Value,
}

/// Result of consensus negotiation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsensusResult {
    /// Agent agrees with the proposal
    Agree,
    /// Agent disagrees with reason
    Disagree(String),
    /// Agent abstains from voting
    Abstain,
}

/// Insights to share with other agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Insights {
    pub timestamp: DateTime<Utc>,
    pub agent_id: String,
    pub insights_type: String,
    pub data: serde_json::Value,
}

/// Result of sharing insights
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharingResult {
    /// Whether insights were successfully shared
    pub insights_shared: bool,
    /// List of recipient agent IDs
    pub recipients: Vec<String>,
}

/// Market context for decision making
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketContext {
    pub timestamp: DateTime<Utc>,
    /// Market volatility (0.0 to 1.0)
    pub volatility: f64,
    /// Market liquidity score
    pub liquidity: f64,
    /// Trend strength (-1.0 to 1.0)
    pub trend: f64,
    /// Additional context data
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Trading opportunity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Opportunity {
    /// Unique opportunity identifier
    pub id: String,
    /// Type of opportunity (e.g., "arbitrage", "momentum")
    pub opportunity_type: String,
    /// Expected value/profit
    pub expected_value: f64,
    /// Confidence level (0.0 to 1.0)
    pub confidence: f64,
    /// Whether this opportunity is time-sensitive
    pub time_sensitive: bool,
    /// Additional opportunity data
    pub data: Option<serde_json::Value>,
}

/// Result of autonomous execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Whether execution was successful
    pub success: bool,
    /// Execution identifier
    pub execution_id: String,
    /// Execution message/details
    pub message: String,
}

/// Result of self-optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationResult {
    /// Whether optimization was performed
    pub optimized: bool,
    /// List of improvements made
    pub improvements: Vec<String>,
    /// Estimated performance gain
    pub performance_gain: f64,
}

/// Trait for agents that can spawn and manage other agents
#[async_trait]
pub trait AgentSpawner: Send + Sync {
    /// Spawn a new agent of specified type
    async fn spawn_agent(&self, agent_type: &str, config: serde_json::Value) -> Result<Box<dyn AutonomousAgent>>;
    
    /// Terminate an agent
    async fn terminate_agent(&self, agent_id: &str) -> Result<()>;
    
    /// Get list of active agents
    async fn list_agents(&self) -> Vec<String>;
}

/// Trait for agents that can coordinate multiple agents
#[async_trait]
pub trait SwarmCoordinator: Send + Sync {
    /// Coordinate agents for a specific task
    async fn coordinate_task(&self, task: &Task, agents: Vec<&dyn AutonomousAgent>) -> Result<TaskResult>;
    
    /// Optimize agent allocation
    async fn optimize_allocation(&self, agents: Vec<&dyn AutonomousAgent>) -> AllocationPlan;
    
    /// Monitor swarm performance
    async fn monitor_swarm(&self) -> SwarmMetrics;
}

/// Task for swarm coordination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub task_type: String,
    pub priority: u8,
    pub requirements: HashMap<String, serde_json::Value>,
}

/// Result of task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: String,
    pub success: bool,
    pub outcome: serde_json::Value,
    pub agents_involved: Vec<String>,
}

/// Agent allocation plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationPlan {
    pub allocations: HashMap<String, Vec<String>>, // task_id -> agent_ids
    pub efficiency_score: f64,
}

/// Swarm performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmMetrics {
    pub total_agents: usize,
    pub active_agents: usize,
    pub tasks_completed: u64,
    pub average_task_time: f64,
    pub collective_performance: f64,
}

/// Trait for resilient agents with self-healing capabilities
#[async_trait]
pub trait ResilientAgent: AutonomousAgent {
    /// Attempt to recover from failure
    async fn recover_from_failure(&mut self, failure: &FailureInfo) -> Result<RecoveryStatus>;
    
    /// Create backup of current state
    async fn create_backup(&self) -> Result<AgentBackup>;
    
    /// Restore from backup
    async fn restore_from_backup(&mut self, backup: &AgentBackup) -> Result<()>;
}

/// Information about agent failure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureInfo {
    pub timestamp: DateTime<Utc>,
    pub failure_type: String,
    pub severity: FailureSeverity,
    pub details: String,
}

/// Failure severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FailureSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Agent recovery status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryStatus {
    /// Successfully recovered
    Recovered,
    /// Partially recovered with limitations
    PartiallyRecovered(Vec<String>), // limitations
    /// Failed to recover
    Failed(String), // reason
}

/// Agent state backup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentBackup {
    pub agent_id: String,
    pub timestamp: DateTime<Utc>,
    pub state_data: Vec<u8>,
    pub version: String,
}

/// Trait for agents with meta-learning capabilities
#[async_trait]
pub trait MetaLearningAgent: EnhancedAgentCapabilities {
    /// Learn from experiences across different market conditions
    async fn meta_learn(&mut self, experiences: Vec<Experience>) -> MetaLearningResult;
    
    /// Transfer learning from one domain to another
    async fn transfer_learning(&mut self, source_domain: &str, target_domain: &str) -> TransferResult;
    
    /// Adapt to new market regime
    async fn adapt_to_regime(&mut self, regime: &MarketRegime) -> Result<()>;
}

/// Trading experience for learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Experience {
    pub timestamp: DateTime<Utc>,
    pub state: HashMap<String, f64>,
    pub action: String,
    pub outcome: f64,
    pub market_regime: String,
}

/// Result of meta-learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaLearningResult {
    pub strategies_improved: usize,
    pub cross_domain_patterns: Vec<String>,
    pub adaptability_score: f64,
}

/// Result of transfer learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferResult {
    pub success: bool,
    pub transferred_knowledge: Vec<String>,
    pub adaptation_required: bool,
}

/// Market regime information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketRegime {
    pub regime_type: String,
    pub characteristics: HashMap<String, f64>,
    pub duration_estimate: Option<i64>, // seconds
}

/// Order book structure
#[derive(Debug, Clone)]
pub struct OrderBook {
    pub bids: Vec<OrderLevel>,
    pub asks: Vec<OrderLevel>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct OrderLevel {
    pub price: f64,
    pub size: f64,
}

impl OrderBook {
    pub fn best_bid(&self) -> Option<&OrderLevel> {
        self.bids.first()
    }
    
    pub fn best_ask(&self) -> Option<&OrderLevel> {
        self.asks.first()
    }
}