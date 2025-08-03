//! Phase 3 Test Utilities
//!
//! Comprehensive utilities for Phase 3 testing including mock objects,
//! test configurations, and helper functions for async operations.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use autonomous_platform::config::NeuralConfig;
use autonomous_platform::data::TimeSeriesData;
use autonomous_platform::integration::{OrderSide, OrderType, TradeOrder, TradeResult, OrderStatus};
use autonomous_platform::neural::{NeuralPredictor, NeuralPredictorTrait};
use autonomous_platform::strategies::{Position, PositionSide, MarketContext};
use autonomous_platform::utils::market_hours::MarketHours;

/// Test configuration for Phase 3 tests
#[derive(Debug, Clone)]
pub struct Phase3TestConfig {
    pub memory_budget_mb: u64,
    pub max_test_duration_secs: u64,
    pub enable_real_models: bool,
    pub enable_monitoring: bool,
}

impl Default for Phase3TestConfig {
    fn default() -> Self {
        Self {
            memory_budget_mb: 512, // 512MB default budget
            max_test_duration_secs: 120, // 2 minutes max test time
            enable_real_models: false, // Use mocks for tests
            enable_monitoring: false, // Disable monitoring for cleaner tests
        }
    }
}

/// Memory usage tracker for budget compliance testing
pub struct MemoryTracker {
    initial_memory: u64,
    budget_mb: u64,
}

impl MemoryTracker {
    pub fn new(budget_mb: u64) -> Self {
        Self {
            initial_memory: Self::get_current_memory_usage(),
            budget_mb,
        }
    }

    fn get_current_memory_usage() -> u64 {
        // Simplified memory tracking - would use proper system calls in production
        #[cfg(target_os = "linux")]
        {
            std::fs::read_to_string("/proc/self/status")
                .ok()
                .and_then(|contents| {
                    contents
                        .lines()
                        .find(|line| line.starts_with("VmRSS:"))
                        .and_then(|line| {
                            line.split_whitespace()
                                .nth(1)
                                .and_then(|kb| kb.parse::<u64>().ok())
                                .map(|kb| kb / 1024) // Convert to MB
                        })
                })
                .unwrap_or(0)
        }
        #[cfg(not(target_os = "linux"))]
        {
            // Fallback for other platforms - use process stats
            std::process::Command::new("ps")
                .args(["-o", "rss=", "-p"])
                .arg(std::process::id().to_string())
                .output()
                .ok()
                .and_then(|output| {
                    String::from_utf8(output.stdout)
                        .ok()?
                        .trim()
                        .parse::<u64>()
                        .ok()
                        .map(|kb| kb / 1024) // Convert KB to MB
                })
                .unwrap_or(64) // Default 64MB if unable to measure
        }
    }

    pub async fn check_budget_compliance(&self) -> Result<bool> {
        let current_usage = Self::get_current_memory_usage();
        let memory_increase = current_usage.saturating_sub(self.initial_memory);
        Ok(memory_increase <= self.budget_mb)
    }

    pub async fn get_memory_usage_mb(&self) -> u64 {
        Self::get_current_memory_usage()
    }
}

/// Agent type enumeration for DAA testing
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AgentType {
    MarketAnalysis,
    RiskManagement,
    SignalGeneration,
    Portfolio,
    Execution,
}

/// Autonomous Decision System for integration testing
pub struct AutonomousDecisionSystem {
    agents: HashMap<AgentType, bool>,
    decision_count: usize,
}

impl AutonomousDecisionSystem {
    pub async fn new() -> Result<Self> {
        Ok(Self {
            agents: HashMap::new(),
            decision_count: 0,
        })
    }

    pub async fn spawn_trading_agents(&mut self) -> Result<()> {
        self.agents.insert(AgentType::MarketAnalysis, true);
        self.agents.insert(AgentType::RiskManagement, true);
        self.agents.insert(AgentType::SignalGeneration, true);
        self.agents.insert(AgentType::Portfolio, true);
        self.agents.insert(AgentType::Execution, true);
        Ok(())
    }

    pub fn get_agent_count(&self) -> usize {
        self.agents.len()
    }

    pub fn has_agent(&self, agent_type: &AgentType) -> bool {
        self.agents.contains_key(agent_type)
    }

    pub async fn make_autonomous_decision(&mut self, context: MarketContext) -> Result<TradingDecision> {
        self.decision_count += 1;
        
        // Simple decision logic for testing
        let action = if context.current_price > 50000.0 {
            TradingAction::Buy
        } else if context.current_price < 30000.0 {
            TradingAction::Sell
        } else {
            TradingAction::Hold
        };

        Ok(TradingDecision {
            symbol: context.symbol,
            action,
            confidence: 0.75,
            risk_score: 0.3,
            position_size: 0.1,
            reasoning: vec!["Test decision".to_string()],
            timestamp: Utc::now(),
        })
    }

    pub async fn coordinate_multi_agent_decision(&mut self, scenario: TradingScenario) -> Result<ConsensusDecision> {
        let mut agent_votes = HashMap::new();
        agent_votes.insert(AgentType::MarketAnalysis, 0.8);
        agent_votes.insert(AgentType::RiskManagement, 0.6);
        agent_votes.insert(AgentType::SignalGeneration, 0.9);
        agent_votes.insert(AgentType::Portfolio, 0.7);
        agent_votes.insert(AgentType::Execution, 0.8);

        Ok(ConsensusDecision {
            symbol: scenario.symbol,
            agent_votes,
            consensus_strength: 0.76,
            final_action: TradingAction::Buy,
            timestamp: Utc::now(),
        })
    }

    pub fn set_portfolio_state(&mut self, _state: PortfolioState) {
        // Mock implementation for testing
    }

    pub async fn generate_trade_order(&self, decision: &TradingDecision) -> Result<TradeOrder> {
        Ok(TradeOrder {
            symbol: decision.symbol.clone(),
            side: match decision.action {
                TradingAction::Buy => OrderSide::Buy,
                TradingAction::Sell => OrderSide::Sell,
                _ => OrderSide::Buy, // Default for testing
            },
            quantity: decision.position_size,
            order_type: OrderType::Market,
            price: None,
        })
    }
}

/// Trading decision structure
#[derive(Debug, Clone)]
pub struct TradingDecision {
    pub symbol: String,
    pub action: TradingAction,
    pub confidence: f64,
    pub risk_score: f64,
    pub position_size: f64,
    pub reasoning: Vec<String>,
    pub timestamp: DateTime<Utc>,
}

/// Trading action enumeration
#[derive(Debug, Clone, PartialEq)]
pub enum TradingAction {
    Buy,
    Sell,
    Hold,
}

/// Consensus decision from multi-agent coordination
#[derive(Debug, Clone)]
pub struct ConsensusDecision {
    pub symbol: String,
    pub agent_votes: HashMap<AgentType, f64>,
    pub consensus_strength: f64,
    pub final_action: TradingAction,
    pub timestamp: DateTime<Utc>,
}

/// Trading scenario for multi-agent testing
#[derive(Debug, Clone)]
pub struct TradingScenario {
    pub symbol: String,
    pub scenario_type: ScenarioType,
    pub market_conditions: HashMap<String, f64>,
    pub time_horizon: TimeHorizon,
    pub timestamp: DateTime<Utc>,
}

/// Scenario type enumeration
#[derive(Debug, Clone)]
pub enum ScenarioType {
    EarningsAnnouncement,
    MarketCrash,
    BullRun,
    Consolidation,
}

/// Time horizon enumeration
#[derive(Debug, Clone)]
pub enum TimeHorizon {
    ShortTerm,
    MediumTerm,
    LongTerm,
}

/// Portfolio state for testing
#[derive(Debug, Clone)]
pub struct PortfolioState {
    positions: HashMap<String, f64>,
    cash_allocation: f64,
}

impl PortfolioState {
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
            cash_allocation: 1.0,
        }
    }

    pub fn add_position(&mut self, symbol: &str, allocation: f64) {
        self.positions.insert(symbol.to_string(), allocation);
    }

    pub fn set_cash_allocation(&mut self, allocation: f64) {
        self.cash_allocation = allocation;
    }
}

/// Create a test neural predictor with proper async initialization
pub async fn create_test_neural_predictor(config: Option<NeuralConfig>) -> Result<Arc<NeuralPredictor>> {
    let neural_config = config.unwrap_or_else(|| NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 5,
        enable_model_monitoring: false,
        accuracy_threshold: 0.7,
        use_real_models: false, // Always use mock models in tests
        input_size: 10,
        output_size: 1,
        hidden_layers: vec![20, 10],
        learning_rate: 0.01,
        prediction_horizon: None,
        normalization_method: None,
        ..Default::default()
    });

    let predictor = NeuralPredictor::new(neural_config).await?;
    Ok(Arc::new(predictor))
}

/// Create test market hours instance
pub fn create_test_market_hours() -> Arc<MarketHours> {
    Arc::new(MarketHours::default())
}

/// Create test time series data with current Phase 3 structure
pub fn create_test_time_series_data(symbol: &str, timestamp: DateTime<Utc>) -> TimeSeriesData {
    let base_price = 100.0;
    let volatility = 0.02;
    let open = base_price * (1.0 + (timestamp.timestamp() % 100) as f64 * 0.001);
    let close = open * (1.0 + volatility * ((timestamp.timestamp() % 50 - 25) as f64 / 25.0));
    let high = open.max(close) * 1.01;
    let low = open.min(close) * 0.99;
    let volume_value = 10000.0 + (timestamp.timestamp() % 5000) as f64;

    TimeSeriesData {
        symbol: symbol.to_string(),
        timestamp,
        open,
        high,
        low,
        close,
        volume: vec![volume_value],
        volume_value,
        indicators: HashMap::from([
            ("sma_20".to_string(), close * 0.99),
            ("rsi".to_string(), 50.0),
            ("macd".to_string(), 0.0),
        ]),
        source: Some("test".to_string()),
        entity: Some(symbol.to_string()),
        value: Some(close),
        metadata: Some(serde_json::json!({
            "test": true,
            "generator": "phase3_utilities"
        })),
        values: vec![open, high, low, close],
        intervals: vec![0],
        timestamps: vec![timestamp],
        metadata_map: HashMap::new(),
    }
}

/// Timeout wrapper for async operations
pub async fn with_timeout<F, T>(future: F, timeout_secs: u64) -> Result<T>
where
    F: std::future::Future<Output = Result<T>>,
{
    match tokio::time::timeout(Duration::from_secs(timeout_secs), future).await {
        Ok(result) => result,
        Err(_) => Err(anyhow::anyhow!("Operation timed out after {} seconds", timeout_secs)),
    }
}

/// Validate test environment setup
pub async fn validate_test_environment() -> Result<()> {
    // Check basic system requirements
    let memory_tracker = MemoryTracker::new(100);
    if !memory_tracker.check_budget_compliance().await? {
        return Err(anyhow::anyhow!("Insufficient memory for testing"));
    }

    // Validate neural predictor can be created
    let _predictor = create_test_neural_predictor(None).await?;

    // Validate DAA system can be initialized
    let mut system = AutonomousDecisionSystem::new().await?;
    system.spawn_trading_agents().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_phase3_utilities_integration() {
        // Test that all utilities work together
        let config = Phase3TestConfig::default();
        assert_eq!(config.memory_budget_mb, 512);

        let memory_tracker = MemoryTracker::new(config.memory_budget_mb);
        assert!(memory_tracker.check_budget_compliance().await.unwrap());

        let predictor = create_test_neural_predictor(None).await.unwrap();
        assert!(Arc::strong_count(&predictor) >= 1);

        let mut system = AutonomousDecisionSystem::new().await.unwrap();
        system.spawn_trading_agents().await.unwrap();
        assert_eq!(system.get_agent_count(), 5);

        let context = MarketContext {
            symbol: "TEST".to_string(),
            current_price: 45000.0,
            bid: 44990.0,
            ask: 45010.0,
            volume_24h: 1000000.0,
            volatility: 0.02,
            timestamp: Utc::now().timestamp(),
        };

        let decision = system.make_autonomous_decision(context).await.unwrap();
        assert_eq!(decision.symbol, "TEST");
        assert!(decision.confidence > 0.0);
    }

    #[tokio::test]
    async fn test_memory_tracking() {
        let tracker = MemoryTracker::new(1024); // 1GB budget
        assert!(tracker.check_budget_compliance().await.unwrap());
        
        let usage = tracker.get_memory_usage_mb().await;
        assert!(usage > 0); // Should report some memory usage
    }

    #[test]
    fn test_agent_types() {
        assert_eq!(AgentType::MarketAnalysis, AgentType::MarketAnalysis);
        assert_ne!(AgentType::MarketAnalysis, AgentType::RiskManagement);
    }

    #[test]
    fn test_scenario_types() {
        let scenario = TradingScenario {
            symbol: "TEST".to_string(),
            scenario_type: ScenarioType::EarningsAnnouncement,
            market_conditions: HashMap::new(),
            time_horizon: TimeHorizon::ShortTerm,
            timestamp: Utc::now(),
        };
        assert_eq!(scenario.symbol, "TEST");
    }
}