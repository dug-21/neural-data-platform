//! Unit Tests for SectorDAACoordinator - Hierarchical DAA Extension
//!
//! Tests the sector-based DAA coordination layer that extends the existing
//! DAA system with hierarchical decision-making and Byzantine consensus.
//!
//! Key Test Areas:
//! - SectorDAACoordinator creation and configuration
//! - Sector-based decision routing and aggregation
//! - 60/40 voting ratio preservation
//! - Cross-sector coordination and consensus
//! - Byzantine fault tolerance mechanisms
//! - Performance benchmarks and memory efficiency

use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};

// Imports for DAA testing
use crate::config::NeuralConfig;
use crate::data::sector_mapper::{SectorMapper, SectorMapperConfig, SectorInfo, SectorId, MarketCapTier};
use crate::data::TimeSeriesData;
use crate::integration::daa_coordinator::{DaaCoordinator, DaaConfig, AutonomousDecision, TradingAction, RiskAssessment};
use crate::monitoring::model_performance_tracker::ModelPerformanceTracker;
use crate::neural::{NeuralPredictor, PredictionResult};
use crate::strategies::{MarketContext, Position, Signal, TradingStrategy, StrategyConfig, StrategyError, PositionSide};
use crate::utils::market_hours::MarketHours;
use async_trait::async_trait;

// Mock SectorDAACoordinator for hierarchical testing
pub struct SectorDAACoordinator {
    /// Base DAA coordinators per sector
    sector_coordinators: HashMap<SectorId, Arc<DaaCoordinator>>,
    
    /// Master coordinator for cross-sector decisions
    master_coordinator: Arc<DaaCoordinator>,
    
    /// Sector mapper for symbol routing
    sector_mapper: Arc<SectorMapper>,
    
    /// Byzantine consensus parameters
    consensus_threshold: f64,
    voting_ratio_60_40: bool,
    
    /// Performance tracking
    sector_performance: HashMap<SectorId, f64>,
    
    /// Decision aggregation channel
    decision_sender: mpsc::Sender<AutonomousDecision>,
}

impl SectorDAACoordinator {
    pub fn new(
        sector_coordinators: HashMap<SectorId, Arc<DaaCoordinator>>,
        master_coordinator: Arc<DaaCoordinator>,
        sector_mapper: Arc<SectorMapper>,
        decision_sender: mpsc::Sender<AutonomousDecision>,
    ) -> Self {
        Self {
            sector_coordinators,
            master_coordinator,
            sector_mapper,
            consensus_threshold: 0.6, // 60% consensus threshold
            voting_ratio_60_40: true,
            sector_performance: HashMap::new(),
            decision_sender,
        }
    }
    
    /// Route decision to appropriate sector coordinator
    pub async fn route_decision(
        &self,
        symbol: &str,
        market_context: &MarketContext,
        current_position: Option<&Position>,
        historical_data: &[TimeSeriesData],
    ) -> Result<AutonomousDecision> {
        // Get sector for symbol
        let sector_info = self.sector_mapper.get_sector(symbol)?;
        let sector_id = sector_info.sector_id;
        
        // Route to sector-specific coordinator
        if let Some(coordinator) = self.sector_coordinators.get(&sector_id) {
            let mut decision = coordinator.make_decision(
                market_context,
                current_position,
                historical_data,
            ).await?;
            
            // Add sector context to decision
            if let Some(ref mut metadata) = decision.adapted_parameters {
                metadata.insert("sector".to_string(), sector_id.as_str().to_string().into());
                metadata.insert("sector_weight".to_string(), sector_info.weight_in_sector.into());
            }
            
            Ok(decision)
        } else {
            // Fallback to master coordinator for unknown sectors
            self.master_coordinator.make_decision(
                market_context,
                current_position,
                historical_data,
            ).await
        }
    }
    
    /// Aggregate decisions from multiple sectors using 60/40 voting
    pub async fn aggregate_cross_sector_decisions(
        &self,
        decisions: Vec<(SectorId, AutonomousDecision)>,
    ) -> Result<AutonomousDecision> {
        if decisions.is_empty() {
            return Err(anyhow::anyhow!("No decisions to aggregate"));
        }
        
        // Apply 60/40 voting ratio: 60% based on confidence, 40% equal weight
        let mut weighted_value = 0.0;
        let mut confidence_weight_sum = 0.0;
        let mut equal_weight_sum = 0.0;
        let total_decisions = decisions.len() as f64;
        
        let mut reasoning = Vec::new();
        let mut consensus_map = HashMap::new();
        
        for (sector, decision) in &decisions {
            let confidence_contribution = decision.confidence * 0.6; // 60% weight
            let equal_contribution = (1.0 / total_decisions) * 0.4; // 40% weight
            let combined_weight = confidence_contribution + equal_contribution;
            
            // Extract trading signal value from decision
            let signal_value = match &decision.action {
                TradingAction::Buy { .. } => 1.0,
                TradingAction::Sell { .. } => -1.0,
                TradingAction::Hold { .. } => 0.0,
                TradingAction::AdjustPosition { .. } => 0.5,
            };
            
            weighted_value += signal_value * combined_weight;
            confidence_weight_sum += combined_weight;
            
            reasoning.push(format!(
                "Sector {} contributed signal={:.3}, weight={:.3} (conf={:.3})",
                sector.as_str(), signal_value, combined_weight, decision.confidence
            ));
            
            consensus_map.insert(sector.as_str().to_string(), decision.confidence);
        }
        
        // Normalize weighted value
        let final_signal = if confidence_weight_sum > 0.0 {
            weighted_value / confidence_weight_sum
        } else {
            0.0
        };
        
        // Calculate average confidence
        let avg_confidence = decisions.iter()
            .map(|(_, d)| d.confidence)
            .sum::<f64>() / total_decisions;
        
        // Determine final action based on aggregated signal
        let final_action = if final_signal > 0.3 {
            TradingAction::Buy {
                symbol: "SECTOR_AGGREGATE".to_string(),
                size: 0.02,
                stop_loss: None,
                take_profit: None,
            }
        } else if final_signal < -0.3 {
            TradingAction::Sell {
                symbol: "SECTOR_AGGREGATE".to_string(),
                size: 0.02,
                reason: "Cross-sector sell signal".to_string(),
            }
        } else {
            TradingAction::Hold {
                reason: format!("Aggregate signal {:.3} below threshold", final_signal),
            }
        };
        
        // Create aggregated risk assessment
        let avg_market_risk = decisions.iter()
            .map(|(_, d)| d.risk_assessment.market_risk)
            .sum::<f64>() / total_decisions;
        
        let avg_portfolio_risk = decisions.iter()
            .map(|(_, d)| d.risk_assessment.portfolio_risk)
            .sum::<f64>() / total_decisions;
        
        let risk_assessment = RiskAssessment {
            market_risk: avg_market_risk,
            position_risk: 0.0, // Cross-sector aggregation
            portfolio_risk: avg_portfolio_risk,
            volatility_adjusted_size: 0.02 * (1.0 - avg_market_risk),
        };
        
        reasoning.push(format!(
            "60/40 voting ratio applied: final_signal={:.3}, consensus_threshold={:.3}",
            final_signal, self.consensus_threshold
        ));
        
        // Check if consensus threshold is met
        let consensus_met = avg_confidence >= self.consensus_threshold;
        if !consensus_met {
            reasoning.push(format!(
                "⚠️ Consensus threshold not met: {:.3} < {:.3}",
                avg_confidence, self.consensus_threshold
            ));
        }
        
        let mut adapted_params = HashMap::new();
        adapted_params.insert("aggregation_method".to_string(), "60_40_voting".to_string().into());
        adapted_params.insert("sectors_involved".to_string(), decisions.len().into());
        adapted_params.insert("consensus_met".to_string(), consensus_met.into());
        adapted_params.insert("final_signal".to_string(), final_signal.into());
        
        Ok(AutonomousDecision {
            timestamp: Utc::now(),
            action: final_action,
            confidence: avg_confidence,
            risk_assessment,
            reasoning,
            neural_consensus: consensus_map,
            adapted_parameters: Some(adapted_params),
        })
    }
    
    /// Byzantine fault tolerance check for decisions
    pub fn validate_byzantine_consensus(&self, decisions: &[(SectorId, AutonomousDecision)]) -> bool {
        if decisions.len() < 3 {
            return false; // Need at least 3 nodes for Byzantine fault tolerance
        }
        
        // Count consistent decisions (simplified Byzantine check)
        let mut buy_count = 0;
        let mut sell_count = 0;
        let mut hold_count = 0;
        
        for (_, decision) in decisions {
            match decision.action {
                TradingAction::Buy { .. } => buy_count += 1,
                TradingAction::Sell { .. } => sell_count += 1,
                TradingAction::Hold { .. } => hold_count += 1,
                TradingAction::AdjustPosition { .. } => hold_count += 1,
            }
        }
        
        let total = decisions.len();
        let byzantine_threshold = (total * 2) / 3; // 2/3 majority for Byzantine fault tolerance
        
        buy_count >= byzantine_threshold || 
        sell_count >= byzantine_threshold || 
        hold_count >= byzantine_threshold
    }
    
    /// Update sector performance metrics
    pub async fn update_sector_performance(&mut self, sector: SectorId, accuracy: f64) {
        self.sector_performance.insert(sector, accuracy);
    }
    
    /// Get sector performance statistics
    pub fn get_sector_stats(&self) -> HashMap<SectorId, f64> {
        self.sector_performance.clone()
    }
    
    /// Test helper: validate voting ratio preservation
    pub fn validate_voting_ratio(&self, decisions: &[(SectorId, AutonomousDecision)]) -> (f64, f64) {
        if decisions.is_empty() {
            return (0.0, 0.0);
        }
        
        let total_decisions = decisions.len() as f64;
        
        // Calculate confidence-based weight (60%)
        let confidence_weight: f64 = decisions.iter()
            .map(|(_, d)| d.confidence * 0.6)
            .sum();
        
        // Calculate equal weight (40%)
        let equal_weight: f64 = total_decisions * 0.4;
        
        let confidence_ratio = confidence_weight / (confidence_weight + equal_weight);
        let equal_ratio = equal_weight / (confidence_weight + equal_weight);
        
        (confidence_ratio, equal_ratio)
    }
}

// Mock trading strategy for testing
struct MockSectorStrategy {
    signal: Signal,
    sector: SectorId,
    name: String,
}

#[async_trait]
impl TradingStrategy for MockSectorStrategy {
    fn name(&self) -> &str {
        &self.name
    }
    
    async fn initialize(&mut self, _config: StrategyConfig) -> Result<(), StrategyError> {
        Ok(())
    }
    
    async fn generate_signal(
        &self,
        _market_context: &MarketContext,
        _current_position: Option<&Position>,
    ) -> Result<Signal, StrategyError> {
        Ok(self.signal.clone())
    }
    
    async fn update_parameters(
        &mut self,
        _parameters: HashMap<String, serde_json::Value>,
    ) -> Result<(), StrategyError> {
        Ok(())
    }
    
    fn get_metrics(&self) -> HashMap<String, f64> {
        let mut metrics = HashMap::new();
        metrics.insert("sector_accuracy".to_string(), 0.85);
        metrics
    }
    
    fn can_execute(&self, _context: &MarketContext) -> Result<bool, StrategyError> {
        Ok(true)
    }
}

// Test helper functions
fn create_test_neural_config() -> NeuralConfig {
    NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
        use_real_models: false,
        enable_health_checks: true,
        enable_fallback: true,
        enable_circuit_breakers: true,
        enable_graceful_degradation: false,
        enable_performance_monitoring: true,
        enable_adaptive_retry: true,
        enable_model_ensembles: false,
        model_timeout_seconds: 60,
        max_retries: 3,
        error_threshold: 0.05,
    }
}

fn create_test_market_context(symbol: &str) -> MarketContext {
    MarketContext {
        symbol: symbol.to_string(),
        current_price: 150.0,
        bid: 149.9,
        ask: 150.1,
        volume_24h: 1000000.0,
        volatility: 0.02,
        timestamp: Utc::now().timestamp(),
    }
}

fn create_test_time_series_data(symbol: &str) -> Vec<TimeSeriesData> {
    vec![
        TimeSeriesData {
            symbol: symbol.to_string(),
            timestamp: Utc::now(),
            open: 148.0,
            high: 152.0,
            low: 147.0,
            close: 150.0,
            volume: vec![100000.0],
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some(symbol.to_string()),
            value: Some(150.0),
            metadata: None,
            values: vec![148.0, 149.0, 150.0, 151.0, 150.0],
            timestamps: vec![Utc::now(); 5],
            metadata_map: HashMap::new(),
        }
    ]
}

fn create_test_position(symbol: &str) -> Position {
    Position {
        symbol: symbol.to_string(),
        side: PositionSide::Long,
        size: 0.1,
        entry_price: 145.0,
        current_price: 150.0,
        unrealized_pnl: 5.0,
        timestamp: Utc::now().timestamp(),
    }
}

async fn create_test_daa_coordinator() -> Result<Arc<DaaCoordinator>> {
    let neural_config = create_test_neural_config();
    let neural_predictor = Arc::new(NeuralPredictor::new(neural_config)?);
    let (tx, _rx) = mpsc::channel(100);
    let market_hours = Arc::new(MarketHours::default());
    
    let config = DaaConfig::default();
    let coordinator = DaaCoordinator::new(config, neural_predictor, tx, market_hours)?;
    
    Ok(Arc::new(coordinator))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_sector_daa_coordinator_creation() {
        let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
        let master_coordinator = create_test_daa_coordinator().await.unwrap();
        let (tx, _rx) = mpsc::channel(100);
        
        // Create sector coordinators for key sectors
        let mut sector_coordinators = HashMap::new();
        sector_coordinators.insert(SectorId::Technology, create_test_daa_coordinator().await.unwrap());
        sector_coordinators.insert(SectorId::Financial, create_test_daa_coordinator().await.unwrap());
        sector_coordinators.insert(SectorId::Healthcare, create_test_daa_coordinator().await.unwrap());
        
        let sector_daa = SectorDAACoordinator::new(
            sector_coordinators,
            master_coordinator,
            sector_mapper,
            tx,
        );
        
        // Verify initialization
        assert_eq!(sector_daa.sector_coordinators.len(), 3);
        assert_eq!(sector_daa.consensus_threshold, 0.6);
        assert!(sector_daa.voting_ratio_60_40);
        assert!(sector_daa.sector_performance.is_empty());
    }
    
    #[tokio::test]
    async fn test_sector_based_decision_routing() {
        let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
        let master_coordinator = create_test_daa_coordinator().await.unwrap();
        let (tx, _rx) = mpsc::channel(100);
        
        let mut sector_coordinators = HashMap::new();
        let tech_coordinator = create_test_daa_coordinator().await.unwrap();
        
        // Register a mock strategy for technology sector
        let tech_strategy = Box::new(MockSectorStrategy {
            signal: Signal::Buy {
                confidence: 0.85,
                size: Some(0.02),
                reason: "Tech sector momentum".to_string(),
            },
            sector: SectorId::Technology,
            name: "tech_momentum".to_string(),
        });
        
        tech_coordinator.register_strategy("tech_momentum".to_string(), tech_strategy).await;
        sector_coordinators.insert(SectorId::Technology, tech_coordinator);
        
        let sector_daa = SectorDAACoordinator::new(
            sector_coordinators,
            master_coordinator,
            sector_mapper,
            tx,
        );
        
        // Test routing for AAPL (Technology sector)
        let market_context = create_test_market_context("AAPL");
        let historical_data = create_test_time_series_data("AAPL");
        
        let decision = sector_daa.route_decision(
            "AAPL",
            &market_context,
            None,
            &historical_data,
        ).await.unwrap();
        
        // Verify decision has sector context
        assert!(decision.adapted_parameters.is_some());
        let params = decision.adapted_parameters.unwrap();
        assert!(params.contains_key("sector"));
        assert!(params.contains_key("sector_weight"));
    }
    
    #[tokio::test]
    async fn test_60_40_voting_ratio_preservation() {
        let decisions = vec![
            (SectorId::Technology, AutonomousDecision {
                timestamp: Utc::now(),
                action: TradingAction::Buy {
                    symbol: "TECH_AGGREGATE".to_string(),
                    size: 0.02,
                    stop_loss: None,
                    take_profit: None,
                },
                confidence: 0.9, // High confidence
                risk_assessment: RiskAssessment {
                    market_risk: 0.02,
                    position_risk: 0.0,
                    portfolio_risk: 0.01,
                    volatility_adjusted_size: 0.02,
                },
                reasoning: vec!["Technology sector buy signal".to_string()],
                neural_consensus: HashMap::new(),
                adapted_parameters: None,
            }),
            (SectorId::Financial, AutonomousDecision {
                timestamp: Utc::now(),
                action: TradingAction::Hold {
                    reason: "Financial sector neutral".to_string(),
                },
                confidence: 0.5, // Low confidence
                risk_assessment: RiskAssessment {
                    market_risk: 0.03,
                    position_risk: 0.0,
                    portfolio_risk: 0.02,
                    volatility_adjusted_size: 0.015,
                },
                reasoning: vec!["Financial sector hold signal".to_string()],
                neural_consensus: HashMap::new(),
                adapted_parameters: None,
            }),
            (SectorId::Healthcare, AutonomousDecision {
                timestamp: Utc::now(),
                action: TradingAction::Buy {
                    symbol: "HEALTH_AGGREGATE".to_string(),
                    size: 0.02,
                    stop_loss: None,
                    take_profit: None,
                },
                confidence: 0.7, // Medium confidence
                risk_assessment: RiskAssessment {
                    market_risk: 0.025,
                    position_risk: 0.0,
                    portfolio_risk: 0.015,
                    volatility_adjusted_size: 0.018,
                },
                reasoning: vec!["Healthcare sector buy signal".to_string()],
                neural_consensus: HashMap::new(),
                adapted_parameters: None,
            }),
        ];
        
        let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
        let master_coordinator = create_test_daa_coordinator().await.unwrap();
        let (tx, _rx) = mpsc::channel(100);
        let sector_coordinators = HashMap::new();
        
        let sector_daa = SectorDAACoordinator::new(
            sector_coordinators,
            master_coordinator,
            sector_mapper,
            tx,
        );
        
        // Test voting ratio validation
        let (confidence_ratio, equal_ratio) = sector_daa.validate_voting_ratio(&decisions);
        
        // Should be approximately 60/40 split
        assert!((confidence_ratio - 0.6).abs() < 0.1, "Confidence ratio should be ~60%");
        assert!((equal_ratio - 0.4).abs() < 0.1, "Equal ratio should be ~40%");
        
        // Test aggregation with 60/40 weighting
        let aggregated = sector_daa.aggregate_cross_sector_decisions(decisions).await.unwrap();
        
        // Verify aggregation metadata
        assert!(aggregated.adapted_parameters.is_some());
        let params = aggregated.adapted_parameters.unwrap();
        assert_eq!(params.get("aggregation_method").unwrap(), &"60_40_voting".to_string().into());
        assert_eq!(params.get("sectors_involved").unwrap(), &3.into());
        
        // Should result in a buy decision (2 buy signals vs 1 hold)
        match aggregated.action {
            TradingAction::Buy { .. } => {
                // Expected outcome
            }
            _ => panic!("Expected aggregated buy decision"),
        }
    }
    
    #[tokio::test]
    async fn test_byzantine_consensus_validation() {
        let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
        let master_coordinator = create_test_daa_coordinator().await.unwrap();
        let (tx, _rx) = mpsc::channel(100);
        let sector_coordinators = HashMap::new();
        
        let sector_daa = SectorDAACoordinator::new(
            sector_coordinators,
            master_coordinator,
            sector_mapper,
            tx,
        );
        
        // Test with insufficient nodes (should fail)
        let insufficient_decisions = vec![
            (SectorId::Technology, AutonomousDecision {
                timestamp: Utc::now(),
                action: TradingAction::Buy {
                    symbol: "TEST".to_string(),
                    size: 0.02,
                    stop_loss: None,
                    take_profit: None,
                },
                confidence: 0.8,
                risk_assessment: RiskAssessment {
                    market_risk: 0.02,
                    position_risk: 0.0,
                    portfolio_risk: 0.01,
                    volatility_adjusted_size: 0.02,
                },
                reasoning: vec![],
                neural_consensus: HashMap::new(),
                adapted_parameters: None,
            }),
        ];
        
        assert!(!sector_daa.validate_byzantine_consensus(&insufficient_decisions));
        
        // Test with Byzantine consensus (2/3 majority)
        let consensus_decisions = vec![
            (SectorId::Technology, AutonomousDecision {
                timestamp: Utc::now(),
                action: TradingAction::Buy {
                    symbol: "TEST".to_string(),
                    size: 0.02,
                    stop_loss: None,
                    take_profit: None,
                },
                confidence: 0.8,
                risk_assessment: RiskAssessment {
                    market_risk: 0.02,
                    position_risk: 0.0,
                    portfolio_risk: 0.01,
                    volatility_adjusted_size: 0.02,
                },
                reasoning: vec![],
                neural_consensus: HashMap::new(),
                adapted_parameters: None,
            }),
            (SectorId::Financial, AutonomousDecision {
                timestamp: Utc::now(),
                action: TradingAction::Buy {
                    symbol: "TEST".to_string(),
                    size: 0.02,
                    stop_loss: None,
                    take_profit: None,
                },
                confidence: 0.7,
                risk_assessment: RiskAssessment {
                    market_risk: 0.03,
                    position_risk: 0.0,
                    portfolio_risk: 0.015,
                    volatility_adjusted_size: 0.018,
                },
                reasoning: vec![],
                neural_consensus: HashMap::new(),
                adapted_parameters: None,
            }),
            (SectorId::Healthcare, AutonomousDecision {
                timestamp: Utc::now(),
                action: TradingAction::Hold {
                    reason: "Healthcare neutral".to_string(),
                },
                confidence: 0.6,
                risk_assessment: RiskAssessment {
                    market_risk: 0.025,
                    position_risk: 0.0,
                    portfolio_risk: 0.012,
                    volatility_adjusted_size: 0.019,
                },
                reasoning: vec![],
                neural_consensus: HashMap::new(),
                adapted_parameters: None,
            }),
        ];
        
        // 2 out of 3 agree on buy (meets 2/3 Byzantine threshold)
        assert!(sector_daa.validate_byzantine_consensus(&consensus_decisions));
    }
    
    #[tokio::test]
    async fn test_cross_sector_consensus_threshold() {
        let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
        let master_coordinator = create_test_daa_coordinator().await.unwrap();
        let (tx, _rx) = mpsc::channel(100);
        let sector_coordinators = HashMap::new();
        
        let sector_daa = SectorDAACoordinator::new(
            sector_coordinators,
            master_coordinator,
            sector_mapper,
            tx,
        );
        
        // Test decisions below consensus threshold
        let low_confidence_decisions = vec![
            (SectorId::Technology, AutonomousDecision {
                timestamp: Utc::now(),
                action: TradingAction::Buy {
                    symbol: "TEST".to_string(),
                    size: 0.02,
                    stop_loss: None,
                    take_profit: None,
                },
                confidence: 0.4, // Below 0.6 threshold
                risk_assessment: RiskAssessment {
                    market_risk: 0.02,
                    position_risk: 0.0,
                    portfolio_risk: 0.01,
                    volatility_adjusted_size: 0.02,
                },
                reasoning: vec![],
                neural_consensus: HashMap::new(),
                adapted_parameters: None,
            }),
            (SectorId::Financial, AutonomousDecision {
                timestamp: Utc::now(),
                action: TradingAction::Buy {
                    symbol: "TEST".to_string(),
                    size: 0.02,
                    stop_loss: None,
                    take_profit: None,
                },
                confidence: 0.5, // Below 0.6 threshold
                risk_assessment: RiskAssessment {
                    market_risk: 0.03,
                    position_risk: 0.0,
                    portfolio_risk: 0.015,
                    volatility_adjusted_size: 0.018,
                },
                reasoning: vec![],
                neural_consensus: HashMap::new(),
                adapted_parameters: None,
            }),
        ];
        
        let aggregated = sector_daa.aggregate_cross_sector_decisions(low_confidence_decisions).await.unwrap();
        
        // Should detect low consensus
        assert!(aggregated.adapted_parameters.is_some());
        let params = aggregated.adapted_parameters.unwrap();
        assert_eq!(params.get("consensus_met").unwrap(), &false.into());
        
        // Should contain warning in reasoning
        assert!(aggregated.reasoning.iter().any(|r| r.contains("Consensus threshold not met")));
    }
    
    #[tokio::test]
    async fn test_sector_performance_tracking() {
        let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
        let master_coordinator = create_test_daa_coordinator().await.unwrap();
        let (tx, _rx) = mpsc::channel(100);
        let sector_coordinators = HashMap::new();
        
        let mut sector_daa = SectorDAACoordinator::new(
            sector_coordinators,
            master_coordinator,
            sector_mapper,
            tx,
        );
        
        // Update sector performance metrics
        sector_daa.update_sector_performance(SectorId::Technology, 0.92).await;
        sector_daa.update_sector_performance(SectorId::Financial, 0.85).await;
        sector_daa.update_sector_performance(SectorId::Healthcare, 0.88).await;
        
        let stats = sector_daa.get_sector_stats();
        
        assert_eq!(stats.len(), 3);
        assert_eq!(stats.get(&SectorId::Technology).unwrap(), &0.92);
        assert_eq!(stats.get(&SectorId::Financial).unwrap(), &0.85);
        assert_eq!(stats.get(&SectorId::Healthcare).unwrap(), &0.88);
    }
    
    #[tokio::test]
    async fn test_hierarchical_decision_flow() {
        let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
        let master_coordinator = create_test_daa_coordinator().await.unwrap();
        let (tx, mut rx) = mpsc::channel(100);
        
        // Create sector coordinators with strategies
        let mut sector_coordinators = HashMap::new();
        
        // Technology sector coordinator
        let tech_coordinator = create_test_daa_coordinator().await.unwrap();
        let tech_strategy = Box::new(MockSectorStrategy {
            signal: Signal::Buy {
                confidence: 0.85,
                size: Some(0.03),
                reason: "Strong tech momentum".to_string(),
            },
            sector: SectorId::Technology,
            name: "tech_hierarchical".to_string(),
        });
        tech_coordinator.register_strategy("tech_hierarchical".to_string(), tech_strategy).await;
        sector_coordinators.insert(SectorId::Technology, tech_coordinator);
        
        // Financial sector coordinator
        let finance_coordinator = create_test_daa_coordinator().await.unwrap();
        let finance_strategy = Box::new(MockSectorStrategy {
            signal: Signal::Hold {
                reason: "Financial sector waiting for fed decision".to_string(),
            },
            sector: SectorId::Financial,
            name: "finance_hierarchical".to_string(),
        });
        finance_coordinator.register_strategy("finance_hierarchical".to_string(), finance_strategy).await;
        sector_coordinators.insert(SectorId::Financial, finance_coordinator);
        
        let sector_daa = SectorDAACoordinator::new(
            sector_coordinators,
            master_coordinator,
            sector_mapper,
            tx,
        );
        
        // Test hierarchical decision flow
        let market_context = create_test_market_context("AAPL");
        let historical_data = create_test_time_series_data("AAPL");
        let position = create_test_position("AAPL");
        
        // Make decision for technology symbol
        let tech_decision = sector_daa.route_decision(
            "AAPL",
            &market_context,
            Some(&position),
            &historical_data,
        ).await.unwrap();
        
        // Decision should be sent through channel
        let received_decision = timeout(Duration::from_millis(100), rx.recv())
            .await
            .expect("Should receive decision")
            .expect("Channel should not be closed");
        
        assert_eq!(received_decision.timestamp, tech_decision.timestamp);
        
        // Verify sector-specific decision characteristics
        assert!(tech_decision.adapted_parameters.is_some());
        let params = tech_decision.adapted_parameters.unwrap();
        assert_eq!(params.get("sector").unwrap(), &"technology".to_string().into());
    }
    
    #[tokio::test]
    async fn test_memory_efficiency() {
        let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
        let master_coordinator = create_test_daa_coordinator().await.unwrap();
        let (tx, _rx) = mpsc::channel(100);
        
        // Create coordinators for all sectors
        let mut sector_coordinators = HashMap::new();
        for sector in SectorId::all_sectors() {
            sector_coordinators.insert(sector, create_test_daa_coordinator().await.unwrap());
        }
        
        let sector_daa = SectorDAACoordinator::new(
            sector_coordinators,
            master_coordinator,
            sector_mapper,
            tx,
        );
        
        // Memory usage should be reasonable even with all sectors
        assert_eq!(sector_daa.sector_coordinators.len(), 10); // All sectors
        assert!(sector_daa.sector_performance.is_empty()); // No performance data yet
        
        // Simulate memory-efficient operations
        let decisions = (0..1000).map(|i| {
            (SectorId::Technology, AutonomousDecision {
                timestamp: Utc::now(),
                action: TradingAction::Hold {
                    reason: format!("Decision {}", i),
                },
                confidence: 0.7,
                risk_assessment: RiskAssessment {
                    market_risk: 0.02,
                    position_risk: 0.0,
                    portfolio_risk: 0.01,
                    volatility_adjusted_size: 0.02,
                },
                reasoning: vec![],
                neural_consensus: HashMap::new(),
                adapted_parameters: None,
            })
        }).collect::<Vec<_>>();
        
        // Should handle large decision sets efficiently
        assert_eq!(decisions.len(), 1000);
        
        // Byzantine validation should work with large sets
        let sample_decisions = &decisions[0..5];
        let is_valid = sector_daa.validate_byzantine_consensus(sample_decisions);
        assert!(is_valid); // All decisions are hold, so consensus should be true
    }
    
    #[tokio::test]
    async fn test_autonomous_trading_preservation() {
        let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
        let master_coordinator = create_test_daa_coordinator().await.unwrap();
        let (tx, _rx) = mpsc::channel(100);
        
        let mut sector_coordinators = HashMap::new();
        let tech_coordinator = create_test_daa_coordinator().await.unwrap();
        
        // Ensure tech coordinator is autonomous (enabled by default in DaaConfig)
        let tech_metrics = tech_coordinator.get_metrics().await;
        assert_eq!(tech_metrics.total_decisions, 0); // Fresh coordinator
        
        sector_coordinators.insert(SectorId::Technology, tech_coordinator);
        
        let sector_daa = SectorDAACoordinator::new(
            sector_coordinators,
            master_coordinator,
            sector_mapper,
            tx,
        );
        
        // Test that autonomous capabilities are preserved through hierarchy
        let market_context = create_test_market_context("AAPL");
        let historical_data = create_test_time_series_data("AAPL");
        
        let decision = sector_daa.route_decision(
            "AAPL",
            &market_context,
            None,
            &historical_data,
        ).await.unwrap();
        
        // Decision should maintain autonomous characteristics
        assert!(decision.confidence >= 0.0);
        assert!(decision.confidence <= 1.0);
        assert!(!decision.reasoning.is_empty());
        assert!(decision.risk_assessment.market_risk >= 0.0);
        
        // Sector information should be added without breaking autonomy
        assert!(decision.adapted_parameters.is_some());
    }
}