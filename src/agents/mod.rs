//! Autonomous Trading Agents Module with DAA Integration
//!
//! This module provides autonomous trading agents powered by the ruv-swarm DAA framework.
//! The DAA service provides advanced capabilities including:
//! - Autonomous decision-making with < 1ms latency
//! - Self-monitoring and adaptation
//! - Multi-agent coordination
//! - Persistent memory and learning
//! - Cognitive pattern recognition

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

// Re-export DAA bridge for direct DAA service integration
mod daa_bridge;
pub use daa_bridge::DAAAgent;

use crate::data::MarketContext;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub id: String,
    pub strategy: TradingStrategy,
    pub risk_tolerance: f64,
    pub max_position_size: f64,
    pub decision_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TradingStrategy {
    Momentum,      // Fast decision-making for price momentum
    MeanReversion, // Analytical approach for mean reversion
    Arbitrage,     // Critical thinking for arbitrage opportunities
    Adaptive,      // NEW: Adaptive strategy that learns and evolves (recommended for DAA)
    Hybrid(Vec<TradingStrategy>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingDecision {
    pub action: String,
    pub confidence: f64,
    pub reasoning: String,
    pub position_action: String,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub combined_signal: Option<serde_json::Value>,
    pub breakdown: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAssessment {
    pub risk_score: f64,
    pub factors: HashMap<String, f64>,
    pub max_drawdown: f64,
    pub value_at_risk: f64,
    pub warnings: Vec<String>,
}

/// AutonomousAgent - Now powered by DAA service
/// This implementation delegates all autonomous capabilities to the DAA framework
pub struct AutonomousAgent {
    config: AgentConfig,
    market_context: Option<MarketContext>,
    daa_agent: Option<DAAAgent>,
}

impl AutonomousAgent {
    pub fn new(config: AgentConfig) -> Result<Self> {
        // For synchronous new(), we'll initialize DAA lazily
        Ok(Self {
            config,
            market_context: None,
            daa_agent: None,
        })
    }
    
    /// Initialize the DAA agent asynchronously
    async fn ensure_daa_initialized(&mut self) -> Result<()> {
        if self.daa_agent.is_none() {
            let daa = DAAAgent::new(self.config.clone()).await?;
            self.daa_agent = Some(daa);
        }
        Ok(())
    }
    
    pub async fn update_market_context(&self, context: MarketContext) -> Result<()> {
        // Update internal market context
        Ok(())
    }
    
    pub async fn make_decision(
        &mut self,
        symbol: &str,
        market_data: &crate::mcp::trading_tools::MarketData,
        current_position: f64,
        position_size: f64,
    ) -> Result<TradingDecision> {
        // Ensure DAA is initialized
        self.ensure_daa_initialized().await?;
        
        // Delegate to DAA agent for autonomous decision-making
        self.daa_agent
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("DAA agent not initialized"))?
            .make_decision(symbol, market_data, current_position, position_size)
            .await
    }
    
    pub async fn get_strategy_signal(
        &mut self,
        strategy: &str,
        symbol: &str,
        market_data: &crate::mcp::trading_tools::MarketData,
    ) -> Result<serde_json::Value> {
        // Ensure DAA is initialized
        self.ensure_daa_initialized().await?;
        
        // Use DAA's pattern recognition for strategy signals
        self.daa_agent
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("DAA agent not initialized"))?
            .get_strategy_signal(strategy, symbol, market_data)
            .await
    }
    
    pub async fn assess_risk(
        &mut self,
        symbol: &str,
        position_size: f64,
        market_data: &crate::mcp::trading_tools::MarketData,
        portfolio_value: Option<f64>,
    ) -> Result<RiskAssessment> {
        // Ensure DAA is initialized
        self.ensure_daa_initialized().await?;
        
        // Use DAA's self-monitoring for comprehensive risk assessment
        self.daa_agent
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("DAA agent not initialized"))?
            .assess_risk(symbol, position_size, market_data, portfolio_value)
            .await
    }
}

// Default implementation
impl Default for AutonomousAgent {
    fn default() -> Self {
        let config = AgentConfig {
            id: "default-agent".to_string(),
            strategy: TradingStrategy::Adaptive, // Changed to Adaptive for DAA
            risk_tolerance: 0.5,
            max_position_size: 10000.0,
            decision_threshold: 0.7,
        };
        Self::new(config).unwrap()
    }
}

/// Create a DAA-powered autonomous agent
/// This is the recommended way to create agents with full DAA capabilities
pub async fn create_daa_agent(config: AgentConfig) -> Result<DAAAgent> {
    DAAAgent::new(config).await
}

/// Helper to create a DAA agent with common trading configurations
pub async fn create_trading_agent(
    id: &str,
    strategy: TradingStrategy,
    risk_tolerance: f64,
) -> Result<DAAAgent> {
    let config = AgentConfig {
        id: id.to_string(),
        strategy,
        risk_tolerance,
        max_position_size: 10000.0,
        decision_threshold: 0.7,
    };
    create_daa_agent(config).await
}