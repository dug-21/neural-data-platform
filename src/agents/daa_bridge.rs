//! DAA (Distributed Autonomous Agent) Bridge Module
//!
//! This module provides integration with the ruv-swarm DAA service,
//! replacing the custom AutonomousAgent implementation with DAA's
//! advanced autonomous capabilities.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use tokio::process::Command;

use crate::agents::{AgentConfig, RiskAssessment, TradingDecision, TradingStrategy};

/// Maps trading strategies to DAA cognitive patterns
fn strategy_to_cognitive_pattern(strategy: &TradingStrategy) -> &'static str {
    match strategy {
        TradingStrategy::Momentum => "fast", // Quick decision-making for momentum
        TradingStrategy::MeanReversion => "analytical", // Analytical for mean reversion calculations
        TradingStrategy::Arbitrage => "critical", // Critical thinking for arbitrage opportunities
        TradingStrategy::Adaptive => "adaptive",  // Pure adaptive learning pattern
        TradingStrategy::Hybrid(_) => "adaptive", // Adaptive for multi-strategy approach
    }
}

/// DAA Agent wrapper that uses the DAA service for all autonomous operations
pub struct DAAAgent {
    config: AgentConfig,
    agent_id: String,
    daa_initialized: bool,
}

impl DAAAgent {
    /// Create a new DAA-powered agent
    pub async fn new(config: AgentConfig) -> Result<Self> {
        let agent_id = format!("trader-{}", config.id);

        // Initialize DAA service if not already done
        Self::ensure_daa_initialized().await?;

        // Create agent in DAA with trading capabilities
        let cognitive_pattern = strategy_to_cognitive_pattern(&config.strategy);

        let create_result = Self::execute_daa_command(
            "createAgent",
            json!({
                "id": agent_id,
                "capabilities": [
                    "decision_making",
                    "risk_assessment",
                    "pattern_recognition",
                    "market_analysis",
                    "self_monitoring",
                    "goal_planning"
                ],
                "cognitivePattern": cognitive_pattern,
                "learningRate": 0.001,
                "enableMemory": true,
                "autonomousMode": true,
                "config": {
                    "strategy": format!("{:?}", config.strategy),
                    "riskTolerance": config.risk_tolerance,
                    "maxPositionSize": config.max_position_size,
                    "decisionThreshold": config.decision_threshold
                }
            }),
        )
        .await?;

        tracing::info!(
            "Created DAA agent: {} with pattern: {}",
            agent_id,
            cognitive_pattern
        );

        Ok(Self {
            config,
            agent_id,
            daa_initialized: true,
        })
    }

    /// Ensure DAA service is initialized
    async fn ensure_daa_initialized() -> Result<()> {
        let status = Self::execute_daa_command("getStatus", json!({})).await?;

        if !status["initialized"].as_bool().unwrap_or(false) {
            tracing::info!("Initializing DAA service...");
            Self::execute_daa_command("initialize", json!({})).await?;
        }

        Ok(())
    }

    /// Execute a DAA command via the integration script
    async fn execute_daa_command(method: &str, params: Value) -> Result<Value> {
        let command_json = json!({
            "method": method,
            "params": params
        });

        let output = Command::new("node")
            .args(&[
                "/workspaces/neural-trader/scripts/daa-integration.js",
                "daa",
                "execute",
                "--json",
                &command_json.to_string(),
            ])
            .output()
            .await
            .context("Failed to execute DAA command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("DAA command failed: {}", stderr);
        }

        let result = String::from_utf8_lossy(&output.stdout);
        serde_json::from_str(&result).context("Failed to parse DAA response")
    }

    /// Make autonomous trading decision using DAA
    pub async fn make_decision(
        &self,
        symbol: &str,
        market_data: &crate::mcp::trading_tools::MarketData,
        current_position: f64,
        _position_size: f64,
    ) -> Result<TradingDecision> {
        // Prepare context for DAA decision-making
        let context = json!({
            "type": "trading_decision",
            "symbol": symbol,
            "marketData": {
                "timestamp": market_data.timestamp.to_rfc3339(),
                "open": market_data.open,
                "high": market_data.high,
                "low": market_data.low,
                "close": market_data.close,
                "volume": market_data.volume,
                "priceChange": (market_data.close - market_data.open) / market_data.open,
                "volatility": (market_data.high - market_data.low) / market_data.close
            },
            "position": {
                "current": current_position,
                "isLong": current_position > 0.0,
                "isShort": current_position < 0.0,
                "isFlat": current_position == 0.0
            },
            "agentConfig": {
                "strategy": format!("{:?}", self.config.strategy),
                "riskTolerance": self.config.risk_tolerance,
                "decisionThreshold": self.config.decision_threshold
            },
            "technicalIndicators": self.calculate_indicators(market_data)
        });

        // Get autonomous decision from DAA
        let daa_response = Self::execute_daa_command(
            "makeDecision",
            json!({
                "agentId": self.agent_id,
                "context": context
            }),
        )
        .await?;

        // Parse DAA decision
        let decision_data: Value =
            serde_json::from_str(daa_response.as_str().unwrap_or("{}")).unwrap_or_default();

        // Extract decision components
        let action = decision_data["decision"]
            .as_str()
            .unwrap_or("hold")
            .to_string();

        let confidence = decision_data["confidence"].as_f64().unwrap_or(0.5);

        let reasoning = decision_data["reasoning"]
            .as_str()
            .unwrap_or("DAA autonomous decision")
            .to_string();

        // Calculate risk-adjusted parameters
        let risk_factor = 1.0 - self.config.risk_tolerance;
        let stop_loss = market_data.close * (1.0 - 0.02 * risk_factor);
        let take_profit = market_data.close * (1.0 + 0.03 * risk_factor);

        // Prepare breakdown with DAA insights
        let breakdown = json!({
            "daaAnalysis": decision_data.get("analysis").cloned().unwrap_or_default(),
            "cognitivePattern": decision_data.get("pattern").cloned().unwrap_or_default(),
            "marketConditions": decision_data.get("marketConditions").cloned().unwrap_or_default(),
            "riskFactors": decision_data.get("riskFactors").cloned().unwrap_or_default()
        });

        Ok(TradingDecision {
            action,
            confidence,
            reasoning,
            position_action: if current_position > 0.0 {
                "adjust"
            } else {
                "open"
            }
            .to_string(),
            stop_loss,
            take_profit,
            combined_signal: Some(decision_data.get("signals").cloned().unwrap_or_default()),
            breakdown: Some(breakdown),
        })
    }

    /// Assess risk using DAA's self-monitoring capabilities
    pub async fn assess_risk(
        &self,
        symbol: &str,
        position_size: f64,
        market_data: &crate::mcp::trading_tools::MarketData,
        portfolio_value: Option<f64>,
    ) -> Result<RiskAssessment> {
        // Use DAA's self-monitoring for risk assessment
        let monitoring_result = Self::execute_daa_command(
            "performSelfMonitoring",
            json!({
                "agentId": self.agent_id,
                "context": {
                    "type": "risk_assessment",
                    "symbol": symbol,
                    "positionSize": position_size,
                    "portfolioValue": portfolio_value,
                    "marketVolatility": (market_data.high - market_data.low) / market_data.close,
                    "currentPrice": market_data.close
                }
            }),
        )
        .await?;

        let monitoring_data: Value =
            serde_json::from_str(monitoring_result.as_str().unwrap_or("{}")).unwrap_or_default();

        // Extract risk metrics from DAA monitoring
        let mut factors = HashMap::new();
        let mut warnings = Vec::new();

        // Process DAA risk insights
        if let Some(risks) = monitoring_data["risks"].as_object() {
            for (factor, value) in risks {
                if let Some(v) = value.as_f64() {
                    factors.insert(factor.clone(), v);

                    // Generate warnings based on thresholds
                    if factor == "volatility" && v > 0.15 {
                        warnings.push("High market volatility detected".to_string());
                    } else if factor == "concentration" && v > 0.3 {
                        warnings.push("Position concentration risk".to_string());
                    }
                }
            }
        }

        // Add portfolio-specific risks
        if let Some(portfolio) = portfolio_value {
            let position_ratio = position_size / portfolio;
            factors.insert("position_ratio".to_string(), position_ratio);

            if position_ratio > 0.2 {
                warnings.push(format!(
                    "Position size {:.1}% exceeds recommended 20% of portfolio",
                    position_ratio * 100.0
                ));
            }
        }

        // Get overall risk score from DAA
        let risk_score = monitoring_data["overallRisk"]
            .as_f64()
            .unwrap_or_else(|| {
                // Fallback calculation if DAA doesn't provide score
                factors.values().sum::<f64>() / factors.len().max(1) as f64
            })
            .min(1.0);

        Ok(RiskAssessment {
            risk_score,
            factors,
            max_drawdown: position_size * 0.1 * (1.0 + risk_score),
            value_at_risk: position_size * 0.05 * (1.0 + risk_score),
            warnings,
        })
    }

    /// Get strategy signal using DAA's pattern recognition
    pub async fn get_strategy_signal(
        &self,
        strategy: &str,
        symbol: &str,
        market_data: &crate::mcp::trading_tools::MarketData,
    ) -> Result<Value> {
        // Use DAA's cognitive patterns for strategy analysis
        let pattern_result = Self::execute_daa_command(
            "analyzeCognitivePatterns",
            json!({
                "agentId": self.agent_id,
                "context": {
                    "strategy": strategy,
                    "symbol": symbol,
                    "marketData": {
                        "close": market_data.close,
                        "open": market_data.open,
                        "high": market_data.high,
                        "low": market_data.low,
                        "volume": market_data.volume
                    }
                }
            }),
        )
        .await?;

        let pattern_data: Value =
            serde_json::from_str(pattern_result.as_str().unwrap_or("{}")).unwrap_or_default();

        // Extract signal from cognitive pattern analysis
        let signal = pattern_data["recommendation"].as_str().unwrap_or("neutral");

        let strength = pattern_data["confidence"].as_f64().unwrap_or(0.5);

        Ok(json!({
            "signal": signal,
            "strength": strength,
            "indicators": pattern_data.get("indicators").cloned().unwrap_or_default(),
            "patterns": pattern_data.get("patterns").cloned().unwrap_or_default(),
            "insights": pattern_data.get("insights").cloned().unwrap_or_default()
        }))
    }

    /// Calculate technical indicators for decision context
    fn calculate_indicators(&self, market_data: &crate::mcp::trading_tools::MarketData) -> Value {
        let price_change = (market_data.close - market_data.open) / market_data.open;
        let volatility = (market_data.high - market_data.low) / market_data.close;
        let typical_price = (market_data.high + market_data.low + market_data.close) / 3.0;

        json!({
            "priceChange": price_change,
            "volatility": volatility,
            "typicalPrice": typical_price,
            "volumeIndicator": market_data.volume,
            "pricePosition": (market_data.close - market_data.low) / (market_data.high - market_data.low)
        })
    }

    /// Adapt agent based on performance (uses DAA meta-learning)
    pub async fn adapt_performance(&self, performance_data: Value) -> Result<()> {
        Self::execute_daa_command(
            "adaptAgent",
            json!({
                "agentId": self.agent_id,
                "adaptationData": performance_data
            }),
        )
        .await?;

        Ok(())
    }

    /// Share knowledge with other agents (uses DAA knowledge sharing)
    pub async fn share_knowledge(
        &self,
        target_agents: Vec<String>,
        knowledge: Value,
    ) -> Result<()> {
        Self::execute_daa_command(
            "shareKnowledge",
            json!({
                "sourceAgentId": self.agent_id,
                "targetAgentIds": target_agents,
                "knowledgeData": knowledge
            }),
        )
        .await?;

        Ok(())
    }
}

impl Drop for DAAAgent {
    fn drop(&mut self) {
        // Clean up DAA agent on drop
        let agent_id = self.agent_id.clone();
        tokio::spawn(async move {
            let _ = DAAAgent::execute_daa_command("destroyAgent", json!({"id": agent_id})).await;
        });
    }
}
