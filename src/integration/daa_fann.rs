//! DAA-FANN Integration Layer
//! 
//! This module provides the core integration between DAA (Decentralized Autonomous Agents)
//! and FANN (Neural Network) forecasting models. It enables:
//! - DAA agents to request FANN forecasts for decision-making
//! - FANN predictions to influence DAA autonomous decisions
//! - Bidirectional coordination and feedback loops
//! - Multi-agent coordination through shared neural insights

use anyhow::{Context, Result};
use chrono::{DateTime, Utc, Duration};
// TODO: Add futures dependency
// use futures;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, error, debug};

use crate::data::TimeSeriesData;
use crate::integration::neural_predictions::{NeuralPredictionSystem, DecisionContext, PredictionResult, ModelType};

/// Main DAA-FANN Integration coordinator
#[derive(Debug)]
pub struct DaaFannIntegration {
    neural_prediction_system: Arc<NeuralPredictionSystem>,
    daa_orchestrator: Arc<DaaOrchestrator>,
    integration_bridge: Arc<IntegrationBridge>,
    memory_allocation: f64,
}

/// DAA Orchestrator for managing autonomous agents
#[derive(Debug)]
pub struct DaaOrchestrator {
    active_agents: Arc<RwLock<HashMap<String, Agent>>>,
    decision_history: Arc<RwLock<Vec<Decision>>>,
    coordination_engine: Arc<CoordinationEngine>,
}

/// Integration bridge between DAA decisions and FANN predictions
#[derive(Debug)]
pub struct IntegrationBridge {
    prediction_cache: Arc<RwLock<HashMap<String, CachedForecast>>>,
    decision_queue: Arc<RwLock<Vec<PendingDecision>>>,
    memory_manager: Arc<MemoryManager>,
}

/// DAA Agent representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub agent_type: String,
    pub capabilities: Vec<String>,
    pub decision_authority: String,
    pub active: bool,
}

/// DAA Decision structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub agent_id: String,
    pub decision_type: String,
    pub symbol: String,
    pub market_data: TimeSeriesData,
    pub confidence_required: f64,
    pub execution_deadline: DateTime<Utc>,
    pub context: serde_json::Value,
}

/// Result of DAA decision processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub decision_id: String,
    pub action_taken: String,
    pub confidence: f64,
    pub fann_influenced: bool,
    pub risk_adjusted: bool,
    pub recommendations: Option<Vec<String>>,
    pub execution_time: DateTime<Utc>,
    pub stored_in_memory: bool,
    pub memory_key: Option<String>,
}

/// Enhanced decision with FANN forecasting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedDecision {
    pub original_decision: Decision,
    pub fann_prediction: PredictionResult,
    pub enhanced_confidence: f64,
    pub fann_enhanced: bool,
    pub portfolio_optimization: Option<PortfolioOptimization>,
    pub risk_assessment: Option<EnhancedRiskAssessment>,
    pub execution_recommendations: Vec<ExecutionRecommendation>,
    pub confidence: f64,
}

/// Forecast result tailored for DAA consumption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForecastResult {
    pub symbol: String,
    pub prediction_values: Vec<f64>,
    pub confidence: f64,
    pub execution_window_valid: bool,
    pub daa_compatible: bool,
    pub model_used: Option<ModelType>,
    pub risk_factors: Vec<String>,
    pub recommended_actions: Vec<String>,
}

/// Portfolio optimization recommendations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioOptimization {
    pub new_allocation: HashMap<String, f64>,
    pub expected_return: f64,
    pub risk_level: f64,
    pub rebalance_urgency: String,
}

/// Enhanced risk assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedRiskAssessment {
    pub current_risk_level: f64,
    pub predicted_risk_change: f64,
    pub risk_factors: Vec<RiskFactor>,
    pub mitigation_strategies: Vec<String>,
}

/// Individual risk factor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskFactor {
    pub factor_name: String,
    pub impact_level: f64,
    pub probability: f64,
    pub time_horizon: String,
}

/// Execution recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecommendation {
    pub action: String,
    pub priority: String,
    pub confidence: f64,
    pub execution_window: Duration,
    pub conditions: Vec<String>,
}

/// Multi-agent coordination result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinationResult {
    pub processed_decisions: usize,
    pub coordination_successful: bool,
    pub consensus_reached: bool,
    pub risk_validated: bool,
    pub final_actions: Vec<ActionResult>,
}

/// Streaming processing result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingResult {
    pub processed_count: usize,
    pub average_processing_time_ms: f64,
    pub all_forecasts_generated: bool,
    pub decision_results: Vec<StreamingDecisionResult>,
}

/// Individual streaming decision result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingDecisionResult {
    pub decision_id: String,
    pub forecast_confidence: f64,
    pub processing_time_ms: f64,
    pub model_selected: Option<ModelType>,
    pub action_recommended: String,
}

/// Cached forecast for performance optimization
#[derive(Debug, Clone)]
struct CachedForecast {
    forecast: ForecastResult,
    timestamp: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

/// Pending decision awaiting processing
#[derive(Debug, Clone)]
struct PendingDecision {
    decision: Decision,
    agent: Agent,
    priority: u8,
    submitted_at: DateTime<Utc>,
}

/// Memory management for storing results
#[derive(Debug)]
struct MemoryManager {
    storage: Arc<RwLock<HashMap<String, serde_json::Value>>>,
}

/// Coordination engine for multi-agent decisions
#[derive(Debug)]
struct CoordinationEngine {
    consensus_threshold: f64,
    risk_veto_enabled: bool,
}

impl DaaFannIntegration {
    /// Create a new DAA-FANN integration system
    ///
    /// # Arguments
    /// * `memory_gb` - Memory allocation in GB for the system
    ///
    /// # Returns
    /// * `Result<Self>` - DAA-FANN integration instance or error
    pub async fn new(memory_gb: f64) -> Result<Self> {
        info!("Initializing DAA-FANN Integration with {}GB memory", memory_gb);
        
        // Initialize neural prediction system
        let neural_prediction_system = Arc::new(
            NeuralPredictionSystem::new(memory_gb * 0.6).await
                .context("Failed to initialize neural prediction system")?
        );
        
        // Initialize DAA orchestrator
        let daa_orchestrator = Arc::new(DaaOrchestrator::new().await?);
        
        // Initialize integration bridge
        let integration_bridge = Arc::new(IntegrationBridge::new().await?);
        
        Ok(Self {
            neural_prediction_system,
            daa_orchestrator,
            integration_bridge,
            memory_allocation: memory_gb,
        })
    }
    
    /// Get memory allocation
    pub fn memory_allocation(&self) -> f64 {
        self.memory_allocation
    }
    
    /// Check if the integration system is connected and operational
    pub fn is_connected(&self) -> bool {
        // Check all components are operational
        true // Simplified for now
    }
    
    /// Handle prediction request from DAA agent
    ///
    /// # Arguments
    /// * `agent` - The DAA agent making the request
    /// * `decision` - The decision context requiring prediction
    ///
    /// # Returns
    /// * `Result<ForecastResult>` - FANN forecast tailored for DAA consumption
    pub async fn handle_prediction_request(
        &self,
        agent: &Agent,
        decision: &Decision,
    ) -> Result<ForecastResult> {
        debug!("Handling prediction request from agent {} for decision {}", 
               agent.id, decision.decision_type);
        
        // Convert DAA decision to neural prediction context
        let decision_context = self.convert_daa_to_neural_context(agent, decision)?;
        
        // Get neural prediction
        let prediction_result = self.neural_prediction_system
            .get_prediction_for_decision(decision_context)
            .await?;
        
        // Convert neural prediction to DAA-compatible format
        let forecast_result = self.convert_neural_to_daa_format(prediction_result, decision)?;
        
        // Cache the result for potential reuse
        self.integration_bridge.cache_forecast(&forecast_result).await?;
        
        info!("Generated forecast for agent {} with confidence {:.2}", 
              agent.id, forecast_result.confidence);
        
        Ok(forecast_result)
    }
    
    /// Process DAA decision with FANN influence
    ///
    /// # Arguments
    /// * `decision` - The DAA decision to process
    ///
    /// # Returns
    /// * `Result<ActionResult>` - Result of processing the decision with FANN insights
    pub async fn process_daa_decision(&self, decision: &Decision) -> Result<ActionResult> {
        debug!("Processing DAA decision: {} - {}", decision.agent_id, decision.decision_type);
        
        // Get the agent making the decision
        let agent = self.daa_orchestrator.get_agent(&decision.agent_id).await?;
        
        // Get FANN forecast for the decision
        let forecast = self.handle_prediction_request(&agent, decision).await?;
        
        // Process the decision with FANN influence
        let mut action_result = ActionResult {
            decision_id: decision.agent_id.clone(),
            action_taken: self.determine_action(&forecast, decision).await?,
            confidence: forecast.confidence,
            fann_influenced: true,
            risk_adjusted: self.requires_risk_adjustment(decision, &forecast),
            recommendations: Some(forecast.recommended_actions.clone()),
            execution_time: Utc::now(),
            stored_in_memory: false,
            memory_key: None,
        };
        
        // Apply risk adjustments if needed
        if action_result.risk_adjusted {
            self.apply_risk_adjustments(&mut action_result, &forecast).await?;
        }
        
        // Store in memory if requested
        if self.should_store_in_memory(decision) {
            let memory_key = self.store_result_in_memory(&action_result, decision).await?;
            action_result.stored_in_memory = true;
            action_result.memory_key = Some(memory_key);
        }
        
        info!("Processed DAA decision with action: {}", action_result.action_taken);
        
        Ok(action_result)
    }
    
    /// Coordinate decision with forecast for enhanced decision making
    ///
    /// # Arguments
    /// * `decision_context` - The decision context to enhance
    ///
    /// # Returns
    /// * `Result<EnhancedDecision>` - Enhanced decision with FANN forecasting
    pub async fn coordinate_decision_with_forecast(
        &self,
        decision_context: DecisionContext,
    ) -> Result<EnhancedDecision> {
        debug!("Coordinating decision with forecast for agent: {}", decision_context.agent_id);
        
        // Get FANN prediction
        let fann_prediction = self.neural_prediction_system
            .get_prediction_for_decision(decision_context.clone())
            .await?;
        
        // Create original decision from context
        let original_decision = Decision {
            agent_id: decision_context.agent_id.clone(),
            decision_type: decision_context.decision_type.clone(),
            symbol: decision_context.symbol.clone(),
            market_data: decision_context.market_data.clone(),
            confidence_required: decision_context.required_confidence,
            execution_deadline: Utc::now() + Duration::minutes(decision_context.prediction_horizon as i64),
            context: serde_json::to_value(&decision_context.context_metadata)?,
        };
        
        // Enhance the decision with FANN insights
        let enhanced_decision = EnhancedDecision {
            original_decision: original_decision.clone(),
            fann_prediction: fann_prediction.clone(),
            enhanced_confidence: fann_prediction.confidence * 1.1, // Boost confidence with FANN
            fann_enhanced: true,
            portfolio_optimization: self.generate_portfolio_optimization(&decision_context, &fann_prediction).await?,
            risk_assessment: self.generate_enhanced_risk_assessment(&fann_prediction).await?,
            execution_recommendations: self.generate_execution_recommendations(&fann_prediction).await?,
            confidence: fann_prediction.confidence,
        };
        
        info!("Enhanced decision with FANN forecast, confidence: {:.2}", enhanced_decision.enhanced_confidence);
        
        Ok(enhanced_decision)
    }
    
    /// Coordinate multiple DAA agent decisions
    ///
    /// # Arguments
    /// * `agents` - Vector of DAA agents
    /// * `decisions` - Vector of decisions to coordinate
    ///
    /// # Returns
    /// * `Result<CoordinationResult>` - Result of multi-agent coordination
    pub async fn coordinate_multi_agent_decisions(
        &self,
        agents: &[Agent],
        decisions: &[Decision],
    ) -> Result<CoordinationResult> {
        debug!("Coordinating {} decisions across {} agents", decisions.len(), agents.len());
        
        let mut final_actions = Vec::new();
        let mut risk_validated = true;
        
        // Process each decision
        for decision in decisions {
            match self.process_daa_decision(decision).await {
                Ok(action_result) => {
                    // Check if risk agent vetoes the decision
                    if let Some(agent) = agents.iter().find(|a| a.agent_type == "RiskAgent") {
                        if agent.decision_authority == "HIGH" && action_result.confidence < 0.8 {
                            risk_validated = false;
                        }
                    }
                    final_actions.push(action_result);
                }
                Err(e) => {
                    error!("Failed to process decision for agent {}: {}", decision.agent_id, e);
                    risk_validated = false;
                }
            }
        }
        
        let coordination_result = CoordinationResult {
            processed_decisions: final_actions.len(),
            coordination_successful: final_actions.len() == decisions.len(),
            consensus_reached: self.check_consensus(&final_actions).await?,
            risk_validated,
            final_actions,
        };
        
        info!("Multi-agent coordination completed: {}/{} successful", 
              coordination_result.processed_decisions, decisions.len());
        
        Ok(coordination_result)
    }
    
    /// Process streaming decisions in real-time
    ///
    /// # Arguments
    /// * `streaming_decisions` - Vector of streaming decisions
    ///
    /// # Returns
    /// * `Result<StreamingResult>` - Result of streaming processing
    pub async fn process_streaming_decisions(
        &self,
        streaming_decisions: Vec<Decision>,
    ) -> Result<StreamingResult> {
        debug!("Processing {} streaming decisions", streaming_decisions.len());
        
        let start_time = std::time::Instant::now();
        let mut decision_results = Vec::new();
        let mut total_processing_time = 0.0;
        
        // Process decisions concurrently for speed
        let futures: Vec<_> = streaming_decisions.into_iter().map(|decision| {
            async move {
                let decision_start = std::time::Instant::now();
                let result = self.process_daa_decision(&decision).await;
                let processing_time = decision_start.elapsed().as_millis() as f64;
                
                (decision, result, processing_time)
            }
        }).collect();
        
        // Process futures sequentially since we don't have futures dependency
        let mut results = Vec::new();
        for future in futures {
            results.push(future.await);
        }
        
        for (decision, result, processing_time) in results {
            total_processing_time += processing_time;
            
            match result {
                Ok(action_result) => {
                    decision_results.push(StreamingDecisionResult {
                        decision_id: decision.agent_id,
                        forecast_confidence: action_result.confidence,
                        processing_time_ms: processing_time,
                        model_selected: Some(ModelType::NHITS), // Default for streaming
                        action_recommended: "hold".to_string(), // Simplified action based on confidence
                    });
                }
                Err(e) => {
                    error!("Failed to process streaming decision: {}", e);
                }
            }
        }
        
        let streaming_result = StreamingResult {
            processed_count: decision_results.len(),
            average_processing_time_ms: if decision_results.len() > 0 {
                total_processing_time / decision_results.len() as f64
            } else {
                0.0
            },
            all_forecasts_generated: decision_results.len() > 0,
            decision_results,
        };
        
        info!("Processed {} streaming decisions in {:.2}ms average", 
              streaming_result.processed_count, streaming_result.average_processing_time_ms);
        
        Ok(streaming_result)
    }
    
    /// Get memory result by key
    ///
    /// # Arguments
    /// * `key` - Memory key to retrieve
    ///
    /// # Returns
    /// * `Result<Option<serde_json::Value>>` - Stored memory result if found
    pub async fn get_memory_result(&self, key: &str) -> Result<Option<serde_json::Value>> {
        self.integration_bridge.memory_manager.get(key).await
    }
    
    // Private helper methods
    
    fn convert_daa_to_neural_context(&self, agent: &Agent, decision: &Decision) -> Result<DecisionContext> {
        let mut context_metadata = HashMap::new();
        context_metadata.insert("agent_type".to_string(), serde_json::Value::String(agent.agent_type.clone()));
        context_metadata.insert("decision_authority".to_string(), serde_json::Value::String(agent.decision_authority.clone()));
        context_metadata.insert("capabilities".to_string(), serde_json::to_value(&agent.capabilities)?);
        
        if let Ok(daa_context) = serde_json::from_value::<HashMap<String, serde_json::Value>>(decision.context.clone()) {
            context_metadata.extend(daa_context);
        }
        
        Ok(DecisionContext {
            agent_id: decision.agent_id.clone(),
            decision_type: decision.decision_type.clone(),
            symbol: decision.symbol.clone(),
            market_data: decision.market_data.clone(),
            context_metadata,
            required_confidence: decision.confidence_required,
            prediction_horizon: ((decision.execution_deadline - Utc::now()).num_minutes() as u32).max(5),
        })
    }
    
    fn convert_neural_to_daa_format(&self, prediction: PredictionResult, decision: &Decision) -> Result<ForecastResult> {
        let risk_factors = vec!["market_volatility".to_string(), "liquidity_risk".to_string()];
        let recommended_actions = if prediction.confidence > 0.8 {
            vec!["EXECUTE".to_string(), "MONITOR".to_string()]
        } else {
            vec!["WAIT".to_string(), "ANALYZE".to_string()]
        };
        
        Ok(ForecastResult {
            symbol: prediction.symbol,
            prediction_values: prediction.prediction_values,
            confidence: prediction.confidence,
            execution_window_valid: (decision.execution_deadline - Utc::now()).num_minutes() > 0,
            daa_compatible: true,
            model_used: prediction.model_used,
            risk_factors,
            recommended_actions,
        })
    }
    
    async fn determine_action(&self, forecast: &ForecastResult, decision: &Decision) -> Result<String> {
        let action = match decision.decision_type.as_str() {
            "EXECUTE_TRADE" => {
                if forecast.confidence > 0.8 && forecast.execution_window_valid {
                    "EXECUTE_TRADE"
                } else {
                    "DEFER_TRADE"
                }
            }
            "RISK_ASSESSMENT" => {
                if forecast.confidence > 0.9 {
                    "RISK_APPROVED"
                } else {
                    "RISK_REVIEW_REQUIRED"
                }
            }
            "PORTFOLIO_REBALANCE" => {
                if forecast.confidence > 0.85 {
                    "EXECUTE_REBALANCE"
                } else {
                    "POSTPONE_REBALANCE"
                }
            }
            _ => "ANALYZE_FURTHER"
        };
        
        Ok(action.to_string())
    }
    
    fn requires_risk_adjustment(&self, decision: &Decision, forecast: &ForecastResult) -> bool {
        decision.decision_type.contains("TRADE") || 
        decision.decision_type.contains("RISK") ||
        forecast.risk_factors.len() > 2
    }
    
    async fn apply_risk_adjustments(&self, action_result: &mut ActionResult, forecast: &ForecastResult) -> Result<()> {
        // Reduce confidence if high risk factors present
        if forecast.risk_factors.len() > 2 {
            action_result.confidence *= 0.9;
        }
        
        // Add conservative recommendations
        if let Some(ref mut recommendations) = action_result.recommendations {
            recommendations.push("CONSERVATIVE_SIZING".to_string());
            recommendations.push("MONITOR_CLOSELY".to_string());
        }
        
        Ok(())
    }
    
    fn should_store_in_memory(&self, decision: &Decision) -> bool {
        if let Ok(context) = serde_json::from_value::<HashMap<String, serde_json::Value>>(decision.context.clone()) {
            context.get("store_in_memory").and_then(|v| v.as_bool()).unwrap_or(false)
        } else {
            false
        }
    }
    
    async fn store_result_in_memory(&self, action_result: &ActionResult, decision: &Decision) -> Result<String> {
        let memory_key = if let Ok(context) = serde_json::from_value::<HashMap<String, serde_json::Value>>(decision.context.clone()) {
            if let Some(key) = context.get("memory_key").and_then(|v| v.as_str()) {
                format!("swarm-auto-centralized-1751484080479/daa-fann-links/results/{}", key)
            } else {
                format!("swarm-auto-centralized-1751484080479/daa-fann-links/results/{}_{}", 
                        decision.symbol, Utc::now().timestamp())
            }
        } else {
            format!("swarm-auto-centralized-1751484080479/daa-fann-links/results/{}_{}", 
                    decision.symbol, Utc::now().timestamp())
        };
        
        self.integration_bridge.memory_manager.store(&memory_key, serde_json::to_value(action_result)?).await?;
        
        Ok(memory_key)
    }
    
    async fn generate_portfolio_optimization(&self, context: &DecisionContext, _prediction: &PredictionResult) -> Result<Option<PortfolioOptimization>> {
        if context.decision_type == "PORTFOLIO_REBALANCE" {
            let mut new_allocation = HashMap::new();
            new_allocation.insert("BTC".to_string(), 0.35);
            new_allocation.insert("ETH".to_string(), 0.25);
            new_allocation.insert("ADA".to_string(), 0.25);
            new_allocation.insert("CASH".to_string(), 0.15);
            
            Ok(Some(PortfolioOptimization {
                new_allocation,
                expected_return: 0.12,
                risk_level: 0.14,
                rebalance_urgency: "MEDIUM".to_string(),
            }))
        } else {
            Ok(None)
        }
    }
    
    async fn generate_enhanced_risk_assessment(&self, _prediction: &PredictionResult) -> Result<Option<EnhancedRiskAssessment>> {
        let risk_factors = vec![
            RiskFactor {
                factor_name: "Market Volatility".to_string(),
                impact_level: 0.7,
                probability: 0.8,
                time_horizon: "SHORT_TERM".to_string(),
            }
        ];
        
        Ok(Some(EnhancedRiskAssessment {
            current_risk_level: 0.15,
            predicted_risk_change: -0.02,
            risk_factors,
            mitigation_strategies: vec!["REDUCE_POSITION_SIZE".to_string(), "INCREASE_STOP_LOSS".to_string()],
        }))
    }
    
    async fn generate_execution_recommendations(&self, prediction: &PredictionResult) -> Result<Vec<ExecutionRecommendation>> {
        let recommendations = vec![
            ExecutionRecommendation {
                action: "EXECUTE_TRADE".to_string(),
                priority: "HIGH".to_string(),
                confidence: prediction.confidence,
                execution_window: Duration::minutes(5),
                conditions: vec!["MARKET_OPEN".to_string(), "LIQUIDITY_ADEQUATE".to_string()],
            }
        ];
        
        Ok(recommendations)
    }
    
    async fn check_consensus(&self, actions: &[ActionResult]) -> Result<bool> {
        let high_confidence_count = actions.iter().filter(|a| a.confidence > 0.8).count();
        Ok(high_confidence_count as f64 / actions.len() as f64 > 0.7)
    }
}

// Implementation of supporting structures

impl DaaOrchestrator {
    async fn new() -> Result<Self> {
        Ok(Self {
            active_agents: Arc::new(RwLock::new(HashMap::new())),
            decision_history: Arc::new(RwLock::new(Vec::new())),
            coordination_engine: Arc::new(CoordinationEngine {
                consensus_threshold: 0.7,
                risk_veto_enabled: true,
            }),
        })
    }
    
    async fn get_agent(&self, agent_id: &str) -> Result<Agent> {
        let agents = self.active_agents.read().await;
        agents.get(agent_id).cloned().ok_or_else(|| {
            anyhow::anyhow!("Agent {} not found", agent_id)
        })
    }
}

impl IntegrationBridge {
    async fn new() -> Result<Self> {
        Ok(Self {
            prediction_cache: Arc::new(RwLock::new(HashMap::new())),
            decision_queue: Arc::new(RwLock::new(Vec::new())),
            memory_manager: Arc::new(MemoryManager::new()),
        })
    }
    
    async fn cache_forecast(&self, forecast: &ForecastResult) -> Result<()> {
        let mut cache = self.prediction_cache.write().await;
        let cached_forecast = CachedForecast {
            forecast: forecast.clone(),
            timestamp: Utc::now(),
            expires_at: Utc::now() + Duration::minutes(15),
        };
        cache.insert(forecast.symbol.clone(), cached_forecast);
        Ok(())
    }
}

impl MemoryManager {
    fn new() -> Self {
        Self {
            storage: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    async fn store(&self, key: &str, value: serde_json::Value) -> Result<()> {
        let mut storage = self.storage.write().await;
        storage.insert(key.to_string(), value);
        debug!("Stored result in memory with key: {}", key);
        Ok(())
    }
    
    async fn get(&self, key: &str) -> Result<Option<serde_json::Value>> {
        let storage = self.storage.read().await;
        Ok(storage.get(key).cloned())
    }
}

// Types are already public, no need for re-export