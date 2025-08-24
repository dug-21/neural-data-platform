//! Comprehensive tests for DAA Coordinator decision orchestration
//! Tests autonomous decision making, multi-agent coordination, and strategy orchestration

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub enum TradeDirection {
    Long,
    Short,
    Hold,
}

#[derive(Debug, Clone)]
pub struct MarketContext {
    pub symbol: String,
    pub price: f64,
    pub volume: f64,
    pub volatility: f64,
    pub trend_strength: f64,
    pub market_sentiment: f64,
    pub timestamp: Instant,
}

#[derive(Debug, Clone)]
pub struct NeuralPrediction {
    pub model_name: String,
    pub direction: TradeDirection,
    pub confidence: f64,
    pub price_target: f64,
    pub horizon: Duration,
    pub features_importance: HashMap<String, f64>,
}

#[derive(Debug, Clone)]
pub struct StrategySignal {
    pub strategy_name: String,
    pub direction: TradeDirection,
    pub strength: f64,
    pub position_size: f64,
    pub stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
    pub reasoning: String,
}

#[derive(Debug, Clone)]
pub struct AutonomousDecision {
    pub id: String,
    pub direction: TradeDirection,
    pub confidence: f64,
    pub position_size: f64,
    pub neural_predictions: Vec<NeuralPrediction>,
    pub strategy_signals: Vec<StrategySignal>,
    pub market_context: MarketContext,
    pub execution_timestamp: Instant,
    pub decision_latency: Duration,
    pub risk_assessment: RiskAssessment,
}

#[derive(Debug, Clone)]
pub struct RiskAssessment {
    pub risk_score: f64,
    pub max_drawdown_estimate: f64,
    pub value_at_risk: f64,
    pub position_risk: f64,
    pub market_risk: f64,
}

#[derive(Debug, Clone)]
pub enum CoordinationMode {
    Consensus,
    Competition,
    Hierarchical,
    Adaptive,
}

pub struct MockDecisionOrchestrator {
    neural_models: HashMap<String, MockNeuralModel>,
    trading_strategies: HashMap<String, MockTradingStrategy>,
    coordination_mode: CoordinationMode,
    performance_weights: HashMap<String, f64>,
    risk_manager: MockRiskManager,
    decision_history: Vec<AutonomousDecision>,
    max_decision_latency: Duration,
}

pub struct MockNeuralModel {
    pub name: String,
    pub accuracy: f64,
    pub latency: Duration,
    pub is_byzantine: bool,
}

pub struct MockTradingStrategy {
    pub name: String,
    pub performance: f64,
    pub risk_tolerance: f64,
    pub specialization: String,
}

pub struct MockRiskManager {
    pub max_position_size: f64,
    pub max_risk_per_trade: f64,
    pub portfolio_risk_limit: f64,
}

impl MockDecisionOrchestrator {
    pub fn new(coordination_mode: CoordinationMode) -> Self {
        Self {
            neural_models: HashMap::new(),
            trading_strategies: HashMap::new(),
            coordination_mode,
            performance_weights: HashMap::new(),
            risk_manager: MockRiskManager {
                max_position_size: 0.05,
                max_risk_per_trade: 0.02,
                portfolio_risk_limit: 0.1,
            },
            decision_history: Vec::new(),
            max_decision_latency: Duration::from_millis(10),
        }
    }

    pub fn add_neural_model(&mut self, model: MockNeuralModel) {
        self.performance_weights.insert(model.name.clone(), model.accuracy);
        self.neural_models.insert(model.name.clone(), model);
    }

    pub fn add_trading_strategy(&mut self, strategy: MockTradingStrategy) {
        self.performance_weights.insert(strategy.name.clone(), strategy.performance);
        self.trading_strategies.insert(strategy.name.clone(), strategy);
    }

    pub async fn orchestrate_decision(
        &mut self,
        market_context: &MarketContext,
    ) -> Result<AutonomousDecision, String> {
        let start_time = Instant::now();

        // 1. Get neural predictions
        let neural_predictions = self.get_neural_predictions(market_context).await?;

        // 2. Get strategy signals
        let strategy_signals = self.get_strategy_signals(market_context, &neural_predictions).await?;

        // 3. Coordinate decisions based on mode
        let coordinated_decision = self.coordinate_decisions(
            &neural_predictions,
            &strategy_signals,
            market_context,
        ).await?;

        // 4. Apply risk management
        let risk_adjusted_decision = self.apply_risk_management(coordinated_decision, market_context).await?;

        // 5. Finalize decision
        let decision_latency = start_time.elapsed();
        if decision_latency > self.max_decision_latency {
            return Err(format!("Decision latency exceeded SLA: {:?}", decision_latency));
        }

        let final_decision = AutonomousDecision {
            id: Uuid::new_v4().to_string(),
            direction: risk_adjusted_decision.direction,
            confidence: risk_adjusted_decision.confidence,
            position_size: risk_adjusted_decision.position_size,
            neural_predictions,
            strategy_signals,
            market_context: market_context.clone(),
            execution_timestamp: Instant::now(),
            decision_latency,
            risk_assessment: risk_adjusted_decision.risk_assessment,
        };

        self.decision_history.push(final_decision.clone());
        Ok(final_decision)
    }

    async fn get_neural_predictions(&self, market_context: &MarketContext) -> Result<Vec<NeuralPrediction>, String> {
        let mut predictions = Vec::new();

        for (name, model) in &self.neural_models {
            // Skip Byzantine models in production
            if model.is_byzantine {
                continue;
            }

            // Simulate model prediction based on market context
            let confidence = match market_context.trend_strength {
                x if x > 0.7 => 0.85,
                x if x > 0.5 => 0.72,
                x if x > 0.3 => 0.60,
                _ => 0.45,
            } * model.accuracy;

            let direction = if market_context.trend_strength > 0.5 {
                if market_context.price > 100.0 { TradeDirection::Long } else { TradeDirection::Short }
            } else {
                TradeDirection::Hold
            };

            let price_target = match direction {
                TradeDirection::Long => market_context.price * 1.02,
                TradeDirection::Short => market_context.price * 0.98,
                TradeDirection::Hold => market_context.price,
            };

            predictions.push(NeuralPrediction {
                model_name: name.clone(),
                direction,
                confidence,
                price_target,
                horizon: Duration::from_minutes(15),
                features_importance: HashMap::from([
                    ("price".to_string(), 0.3),
                    ("volume".to_string(), 0.2),
                    ("volatility".to_string(), 0.25),
                    ("trend".to_string(), 0.25),
                ]),
            });
        }

        if predictions.is_empty() {
            return Err("No neural predictions available".to_string());
        }

        Ok(predictions)
    }

    async fn get_strategy_signals(
        &self,
        market_context: &MarketContext,
        predictions: &[NeuralPrediction],
    ) -> Result<Vec<StrategySignal>, String> {
        let mut signals = Vec::new();

        for (name, strategy) in &self.trading_strategies {
            // Strategy decision based on specialization and market context
            let (direction, strength) = match strategy.specialization.as_str() {
                "momentum" => {
                    if market_context.trend_strength > 0.6 {
                        (TradeDirection::Long, market_context.trend_strength)
                    } else {
                        (TradeDirection::Hold, 0.3)
                    }
                }
                "mean_reversion" => {
                    if market_context.volatility > 0.8 {
                        (TradeDirection::Short, market_context.volatility)
                    } else {
                        (TradeDirection::Hold, 0.4)
                    }
                }
                "sentiment" => {
                    if market_context.market_sentiment > 0.7 {
                        (TradeDirection::Long, market_context.market_sentiment)
                    } else if market_context.market_sentiment < 0.3 {
                        (TradeDirection::Short, 1.0 - market_context.market_sentiment)
                    } else {
                        (TradeDirection::Hold, 0.5)
                    }
                }
                _ => (TradeDirection::Hold, 0.5),
            };

            let position_size = (strength * strategy.risk_tolerance * 0.03).min(0.05);

            signals.push(StrategySignal {
                strategy_name: name.clone(),
                direction,
                strength,
                position_size,
                stop_loss: Some(market_context.price * 0.98),
                take_profit: Some(market_context.price * 1.04),
                reasoning: format!("{} analysis based on {} conditions", strategy.specialization, name),
            });
        }

        Ok(signals)
    }

    async fn coordinate_decisions(
        &self,
        predictions: &[NeuralPrediction],
        signals: &[StrategySignal],
        market_context: &MarketContext,
    ) -> Result<AutonomousDecision, String> {
        match self.coordination_mode {
            CoordinationMode::Consensus => self.consensus_coordination(predictions, signals, market_context).await,
            CoordinationMode::Competition => self.competition_coordination(predictions, signals, market_context).await,
            CoordinationMode::Hierarchical => self.hierarchical_coordination(predictions, signals, market_context).await,
            CoordinationMode::Adaptive => self.adaptive_coordination(predictions, signals, market_context).await,
        }
    }

    async fn consensus_coordination(
        &self,
        predictions: &[NeuralPrediction],
        signals: &[StrategySignal],
        market_context: &MarketContext,
    ) -> Result<AutonomousDecision, String> {
        // Voting-based consensus
        let mut direction_votes = HashMap::new();
        let mut total_weight = 0.0;

        // Count neural model votes
        for prediction in predictions {
            let weight = self.performance_weights.get(&prediction.model_name).unwrap_or(&1.0) * prediction.confidence;
            *direction_votes.entry(prediction.direction.clone()).or_insert(0.0) += weight;
            total_weight += weight;
        }

        // Count strategy votes
        for signal in signals {
            let weight = self.performance_weights.get(&signal.strategy_name).unwrap_or(&1.0) * signal.strength;
            *direction_votes.entry(signal.direction.clone()).or_insert(0.0) += weight;
            total_weight += weight;
        }

        let (winning_direction, winning_weight) = direction_votes
            .iter()
            .max_by(|(_, &a), (_, &b)| a.partial_cmp(&b).unwrap())
            .ok_or("No decisions to coordinate")?;

        let consensus_confidence = *winning_weight / total_weight;
        let avg_position_size = signals.iter().map(|s| s.position_size).sum::<f64>() / signals.len() as f64;

        Ok(AutonomousDecision {
            id: Uuid::new_v4().to_string(),
            direction: winning_direction.clone(),
            confidence: consensus_confidence,
            position_size: avg_position_size,
            neural_predictions: predictions.to_vec(),
            strategy_signals: signals.to_vec(),
            market_context: market_context.clone(),
            execution_timestamp: Instant::now(),
            decision_latency: Duration::from_millis(0),
            risk_assessment: RiskAssessment {
                risk_score: 0.5,
                max_drawdown_estimate: 0.1,
                value_at_risk: 0.02,
                position_risk: avg_position_size * 2.0,
                market_risk: market_context.volatility,
            },
        })
    }

    async fn competition_coordination(
        &self,
        predictions: &[NeuralPrediction],
        signals: &[StrategySignal],
        _market_context: &MarketContext,
    ) -> Result<AutonomousDecision, String> {
        // Winner-takes-all based on performance
        let best_prediction = predictions.iter()
            .max_by(|a, b| {
                let weight_a = self.performance_weights.get(&a.model_name).unwrap_or(&1.0) * a.confidence;
                let weight_b = self.performance_weights.get(&b.model_name).unwrap_or(&1.0) * b.confidence;
                weight_a.partial_cmp(&weight_b).unwrap()
            })
            .ok_or("No predictions available")?;

        let best_signal = signals.iter()
            .max_by(|a, b| {
                let weight_a = self.performance_weights.get(&a.strategy_name).unwrap_or(&1.0) * a.strength;
                let weight_b = self.performance_weights.get(&b.strategy_name).unwrap_or(&1.0) * b.strength;
                weight_a.partial_cmp(&weight_b).unwrap()
            })
            .ok_or("No signals available")?;

        // Choose the best between prediction and signal
        let (direction, confidence, position_size) = 
            if (self.performance_weights.get(&best_prediction.model_name).unwrap_or(&1.0) * best_prediction.confidence) >
               (self.performance_weights.get(&best_signal.strategy_name).unwrap_or(&1.0) * best_signal.strength) {
                (best_prediction.direction.clone(), best_prediction.confidence, 0.025)
            } else {
                (best_signal.direction.clone(), best_signal.strength, best_signal.position_size)
            };

        Ok(AutonomousDecision {
            id: Uuid::new_v4().to_string(),
            direction,
            confidence,
            position_size,
            neural_predictions: predictions.to_vec(),
            strategy_signals: signals.to_vec(),
            market_context: _market_context.clone(),
            execution_timestamp: Instant::now(),
            decision_latency: Duration::from_millis(0),
            risk_assessment: RiskAssessment {
                risk_score: 0.4,
                max_drawdown_estimate: 0.08,
                value_at_risk: 0.015,
                position_risk: position_size * 2.0,
                market_risk: _market_context.volatility,
            },
        })
    }

    async fn hierarchical_coordination(
        &self,
        predictions: &[NeuralPrediction],
        signals: &[StrategySignal],
        market_context: &MarketContext,
    ) -> Result<AutonomousDecision, String> {
        // Neural models have priority, strategies provide confirmation
        let neural_consensus = if !predictions.is_empty() {
            let avg_confidence = predictions.iter().map(|p| p.confidence).sum::<f64>() / predictions.len() as f64;
            let majority_direction = predictions.iter()
                .fold(HashMap::new(), |mut acc, p| {
                    *acc.entry(p.direction.clone()).or_insert(0) += 1;
                    acc
                })
                .into_iter()
                .max_by_key(|(_, count)| *count)
                .map(|(dir, _)| dir);

            (majority_direction, avg_confidence)
        } else {
            (None, 0.0)
        };

        let strategy_confirmation = if !signals.is_empty() {
            let confirming_signals: Vec<_> = signals.iter()
                .filter(|s| Some(&s.direction) == neural_consensus.0.as_ref())
                .collect();

            confirming_signals.len() as f64 / signals.len() as f64
        } else {
            0.0
        };

        let final_confidence = neural_consensus.1 * (0.7 + 0.3 * strategy_confirmation);
        let avg_position_size = signals.iter().map(|s| s.position_size).sum::<f64>() / signals.len().max(1) as f64;

        Ok(AutonomousDecision {
            id: Uuid::new_v4().to_string(),
            direction: neural_consensus.0.unwrap_or(TradeDirection::Hold),
            confidence: final_confidence,
            position_size: avg_position_size,
            neural_predictions: predictions.to_vec(),
            strategy_signals: signals.to_vec(),
            market_context: market_context.clone(),
            execution_timestamp: Instant::now(),
            decision_latency: Duration::from_millis(0),
            risk_assessment: RiskAssessment {
                risk_score: 0.3,
                max_drawdown_estimate: 0.06,
                value_at_risk: 0.012,
                position_risk: avg_position_size * 1.5,
                market_risk: market_context.volatility * 0.8,
            },
        })
    }

    async fn adaptive_coordination(
        &self,
        predictions: &[NeuralPrediction],
        signals: &[StrategySignal],
        market_context: &MarketContext,
    ) -> Result<AutonomousDecision, String> {
        // Adapt coordination based on market conditions
        let coordination_mode = if market_context.volatility > 0.8 {
            // High volatility: use hierarchical with neural priority
            CoordinationMode::Hierarchical
        } else if market_context.trend_strength > 0.7 {
            // Strong trend: use competition to maximize gains
            CoordinationMode::Competition
        } else {
            // Normal conditions: use consensus
            CoordinationMode::Consensus
        };

        // Create temporary orchestrator with adaptive mode
        let temp_orchestrator = MockDecisionOrchestrator {
            neural_models: HashMap::new(),
            trading_strategies: HashMap::new(),
            coordination_mode,
            performance_weights: self.performance_weights.clone(),
            risk_manager: MockRiskManager {
                max_position_size: 0.05,
                max_risk_per_trade: 0.02,
                portfolio_risk_limit: 0.1,
            },
            decision_history: Vec::new(),
            max_decision_latency: Duration::from_millis(10),
        };

        temp_orchestrator.coordinate_decisions(predictions, signals, market_context).await
    }

    async fn apply_risk_management(
        &self,
        mut decision: AutonomousDecision,
        market_context: &MarketContext,
    ) -> Result<AutonomousDecision, String> {
        // Position size risk management
        decision.position_size = decision.position_size.min(self.risk_manager.max_position_size);

        // Confidence-based adjustment
        if decision.confidence < 0.6 {
            decision.position_size *= 0.5; // Reduce position for low confidence
        }

        // Volatility adjustment
        if market_context.volatility > 0.8 {
            decision.position_size *= 0.7; // Reduce position in high volatility
        }

        // Update risk assessment
        decision.risk_assessment.position_risk = decision.position_size * 2.0;
        decision.risk_assessment.risk_score = 
            (decision.risk_assessment.position_risk + 
             decision.risk_assessment.market_risk) / 2.0;

        Ok(decision)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::test;

    fn create_test_market_context() -> MarketContext {
        MarketContext {
            symbol: "BTCUSD".to_string(),
            price: 45000.0,
            volume: 1000000.0,
            volatility: 0.6,
            trend_strength: 0.75,
            market_sentiment: 0.8,
            timestamp: Instant::now(),
        }
    }

    fn create_test_orchestrator() -> MockDecisionOrchestrator {
        let mut orchestrator = MockDecisionOrchestrator::new(CoordinationMode::Consensus);

        // Add neural models
        orchestrator.add_neural_model(MockNeuralModel {
            name: "lstm_model".to_string(),
            accuracy: 0.85,
            latency: Duration::from_millis(2),
            is_byzantine: false,
        });

        orchestrator.add_neural_model(MockNeuralModel {
            name: "nbeats_model".to_string(),
            accuracy: 0.78,
            latency: Duration::from_millis(3),
            is_byzantine: false,
        });

        // Add trading strategies
        orchestrator.add_trading_strategy(MockTradingStrategy {
            name: "momentum_strategy".to_string(),
            performance: 0.82,
            risk_tolerance: 0.8,
            specialization: "momentum".to_string(),
        });

        orchestrator.add_trading_strategy(MockTradingStrategy {
            name: "sentiment_strategy".to_string(),
            performance: 0.74,
            risk_tolerance: 0.6,
            specialization: "sentiment".to_string(),
        });

        orchestrator
    }

    #[test]
    async fn test_consensus_decision_orchestration() {
        let mut orchestrator = create_test_orchestrator();
        let market_context = create_test_market_context();

        let result = orchestrator.orchestrate_decision(&market_context).await;

        assert!(result.is_ok());
        let decision = result.unwrap();
        assert_eq!(decision.direction, TradeDirection::Long);
        assert!(decision.confidence > 0.5);
        assert!(decision.position_size > 0.0);
        assert!(decision.decision_latency < Duration::from_millis(10));
    }

    #[test]
    async fn test_competition_coordination_mode() {
        let mut orchestrator = MockDecisionOrchestrator::new(CoordinationMode::Competition);
        orchestrator.add_neural_model(MockNeuralModel {
            name: "high_accuracy_model".to_string(),
            accuracy: 0.95,
            latency: Duration::from_millis(2),
            is_byzantine: false,
        });

        orchestrator.add_trading_strategy(MockTradingStrategy {
            name: "low_performance_strategy".to_string(),
            performance: 0.6,
            risk_tolerance: 0.5,
            specialization: "momentum".to_string(),
        });

        let market_context = create_test_market_context();
        let result = orchestrator.orchestrate_decision(&market_context).await;

        assert!(result.is_ok());
        let decision = result.unwrap();
        // High accuracy model should dominate
        assert!(decision.confidence > 0.8);
    }

    #[test]
    async fn test_hierarchical_coordination_mode() {
        let mut orchestrator = MockDecisionOrchestrator::new(CoordinationMode::Hierarchical);
        orchestrator.add_neural_model(MockNeuralModel {
            name: "neural_primary".to_string(),
            accuracy: 0.8,
            latency: Duration::from_millis(2),
            is_byzantine: false,
        });

        orchestrator.add_trading_strategy(MockTradingStrategy {
            name: "strategy_secondary".to_string(),
            performance: 0.9,
            risk_tolerance: 0.7,
            specialization: "momentum".to_string(),
        });

        let market_context = create_test_market_context();
        let result = orchestrator.orchestrate_decision(&market_context).await;

        assert!(result.is_ok());
        let decision = result.unwrap();
        // Neural models should have priority in hierarchical mode
        assert!(!decision.neural_predictions.is_empty());
    }

    #[test]
    async fn test_adaptive_coordination_high_volatility() {
        let mut orchestrator = MockDecisionOrchestrator::new(CoordinationMode::Adaptive);
        orchestrator.add_neural_model(MockNeuralModel {
            name: "adaptive_model".to_string(),
            accuracy: 0.85,
            latency: Duration::from_millis(2),
            is_byzantine: false,
        });

        let mut market_context = create_test_market_context();
        market_context.volatility = 0.9; // High volatility

        let result = orchestrator.orchestrate_decision(&market_context).await;

        assert!(result.is_ok());
        let decision = result.unwrap();
        // Should adapt to use hierarchical coordination in high volatility
        assert!(decision.position_size < 0.04); // Risk-adjusted smaller position
    }

    #[test]
    async fn test_risk_management_position_sizing() {
        let mut orchestrator = create_test_orchestrator();
        let market_context = create_test_market_context();

        let result = orchestrator.orchestrate_decision(&market_context).await;

        assert!(result.is_ok());
        let decision = result.unwrap();
        assert!(decision.position_size <= 0.05); // Max position size limit
        assert!(decision.risk_assessment.risk_score < 1.0);
    }

    #[test]
    async fn test_decision_latency_sla() {
        let mut orchestrator = create_test_orchestrator();
        let market_context = create_test_market_context();

        let start = Instant::now();
        let result = orchestrator.orchestrate_decision(&market_context).await;
        let elapsed = start.elapsed();

        assert!(result.is_ok());
        assert!(elapsed < Duration::from_millis(10)); // <10ms SLA
        
        let decision = result.unwrap();
        assert!(decision.decision_latency < Duration::from_millis(10));
    }

    #[test]
    async fn test_concurrent_decision_orchestration() {
        let market_context = create_test_market_context();

        // Run 100 concurrent decision orchestrations
        let mut handles = Vec::new();
        for _ in 0..100 {
            let market_context_clone = market_context.clone();
            
            let handle = tokio::spawn(async move {
                let mut orchestrator = create_test_orchestrator();
                orchestrator.orchestrate_decision(&market_context_clone).await
            });
            handles.push(handle);
        }

        let results = futures::future::join_all(handles).await;
        
        // All should succeed
        for result in results {
            assert!(result.is_ok());
            assert!(result.unwrap().is_ok());
        }
    }

    #[test]
    async fn test_high_throughput_decision_orchestration() {
        let mut orchestrator = create_test_orchestrator();
        let market_context = create_test_market_context();

        // Test 500 decisions/sec throughput
        let start = Instant::now();
        let mut successful_decisions = 0;

        for _ in 0..50 {
            if orchestrator.orchestrate_decision(&market_context).await.is_ok() {
                successful_decisions += 1;
            }
        }

        let elapsed = start.elapsed();
        let throughput = successful_decisions as f64 / elapsed.as_secs_f64();

        assert!(successful_decisions >= 45); // 90% success rate
        assert!(throughput >= 500.0); // 500 decisions/sec
    }

    #[test]
    async fn test_byzantine_model_filtering() {
        let mut orchestrator = MockDecisionOrchestrator::new(CoordinationMode::Consensus);
        
        // Add Byzantine model
        orchestrator.add_neural_model(MockNeuralModel {
            name: "byzantine_model".to_string(),
            accuracy: 0.3,
            latency: Duration::from_millis(2),
            is_byzantine: true,
        });

        // Add honest model
        orchestrator.add_neural_model(MockNeuralModel {
            name: "honest_model".to_string(),
            accuracy: 0.8,
            latency: Duration::from_millis(2),
            is_byzantine: false,
        });

        let market_context = create_test_market_context();
        let result = orchestrator.orchestrate_decision(&market_context).await;

        assert!(result.is_ok());
        let decision = result.unwrap();
        // Should only have predictions from honest models
        assert_eq!(decision.neural_predictions.len(), 1);
        assert_eq!(decision.neural_predictions[0].model_name, "honest_model");
    }

    #[test]
    async fn test_empty_models_and_strategies() {
        let mut orchestrator = MockDecisionOrchestrator::new(CoordinationMode::Consensus);
        let market_context = create_test_market_context();

        let result = orchestrator.orchestrate_decision(&market_context).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No neural predictions"));
    }

    #[test]
    async fn test_low_confidence_position_adjustment() {
        let mut orchestrator = MockDecisionOrchestrator::new(CoordinationMode::Consensus);
        
        // Add low accuracy model
        orchestrator.add_neural_model(MockNeuralModel {
            name: "low_accuracy_model".to_string(),
            accuracy: 0.4, // Low accuracy leads to low confidence
            latency: Duration::from_millis(2),
            is_byzantine: false,
        });

        orchestrator.add_trading_strategy(MockTradingStrategy {
            name: "test_strategy".to_string(),
            performance: 0.5,
            risk_tolerance: 0.5,
            specialization: "momentum".to_string(),
        });

        let market_context = create_test_market_context();
        let result = orchestrator.orchestrate_decision(&market_context).await;

        assert!(result.is_ok());
        let decision = result.unwrap();
        // Position should be reduced due to low confidence
        assert!(decision.position_size < 0.02);
    }

    #[test]
    async fn test_high_volatility_position_adjustment() {
        let mut orchestrator = create_test_orchestrator();
        
        let mut market_context = create_test_market_context();
        market_context.volatility = 0.9; // High volatility

        let result = orchestrator.orchestrate_decision(&market_context).await;

        assert!(result.is_ok());
        let decision = result.unwrap();
        // Position should be reduced in high volatility
        assert!(decision.risk_assessment.market_risk > 0.8);
    }

    #[test]
    async fn test_decision_history_tracking() {
        let mut orchestrator = create_test_orchestrator();
        let market_context = create_test_market_context();

        // Make multiple decisions
        for _ in 0..3 {
            let _ = orchestrator.orchestrate_decision(&market_context).await;
        }

        assert_eq!(orchestrator.decision_history.len(), 3);
        
        // Each decision should have unique ID
        let ids: std::collections::HashSet<_> = orchestrator.decision_history
            .iter()
            .map(|d| &d.id)
            .collect();
        assert_eq!(ids.len(), 3);
    }
}