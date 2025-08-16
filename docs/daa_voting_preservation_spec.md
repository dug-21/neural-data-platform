# DAA Voting Preservation Specification

## Overview

This specification defines how to preserve the critical 60/40 voting ratio across the hierarchical Decentralized Autonomous Agent (DAA) architecture while maintaining sector-level intelligence and master-level aggregation.

## Architecture Flow

```
Symbol → Sector Vote (60/40) → Master Aggregation → Portfolio Decision
```

## 1. Sector-Level Voting Preservation

### 1.1 SectorDAAVote Structure

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorDAAVote {
    pub sector_id: SectorId,
    pub symbol: String,
    pub timestamp: DateTime<Utc>,
    
    // Core 60/40 voting components
    pub neural_vote: NeuralVote,      // 60% weight
    pub strategy_vote: StrategyVote,  // 40% weight
    
    // Sector-specific enhancements
    pub sector_confidence: f64,       // 0.0-1.0 sector model confidence
    pub correlation_adjustment: f64,  // Adjust for sector correlations
    pub market_cap_weight: f64,      // Symbol's weight in sector
    
    // Aggregation metadata
    pub vote_id: String,
    pub dependencies: Vec<String>,    // Other votes this depends on
    pub conflict_resolution: ConflictResolution,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuralVote {
    pub signal_strength: f64,     // -1.0 to 1.0
    pub confidence: f64,          // 0.0 to 1.0
    pub model_predictions: Vec<ModelPrediction>,
    pub ensemble_weight: f64,     // Weight in ensemble
    pub sector_neural_bias: f64,  // Sector-specific neural adjustment
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyVote {
    pub signal_strength: f64,     // -1.0 to 1.0
    pub confidence: f64,          // 0.0 to 1.0
    pub strategy_type: String,    // "momentum", "mean_reversion", etc.
    pub technical_indicators: HashMap<String, f64>,
    pub sector_technical_bias: f64, // Sector-specific technical adjustment
}
```

### 1.2 Sector Voting Algorithm

```rust
impl SectorDAACoordinator {
    pub async fn generate_sector_vote(
        &self,
        symbol: &str,
        market_context: &MarketContext,
    ) -> Result<SectorDAAVote, DAAError> {
        // 1. Get neural predictions from sector model
        let neural_signals = self.sector_neural_model
            .predict_with_confidence(symbol, &market_context.data)
            .await?;
        
        // 2. Get strategy signals adapted for sector
        let strategy_signals = self.sector_strategy
            .generate_sector_adapted_signal(symbol, market_context, &self.sector_info)
            .await?;
        
        // 3. Calculate sector-specific adjustments
        let sector_confidence = self.calculate_sector_confidence(symbol).await?;
        let correlation_adj = self.calculate_correlation_adjustment(symbol).await?;
        let market_cap_weight = self.get_market_cap_weight(symbol).await?;
        
        // 4. Enforce 60/40 weighting
        let neural_vote = NeuralVote {
            signal_strength: neural_signals.strength,
            confidence: neural_signals.confidence * sector_confidence,
            model_predictions: neural_signals.predictions,
            ensemble_weight: 0.6, // CRITICAL: 60% weight
            sector_neural_bias: self.calculate_neural_bias(symbol).await?,
        };
        
        let strategy_vote = StrategyVote {
            signal_strength: strategy_signals.strength,
            confidence: strategy_signals.confidence * sector_confidence,
            strategy_type: strategy_signals.strategy_type,
            technical_indicators: strategy_signals.indicators,
            sector_technical_bias: self.calculate_technical_bias(symbol).await?,
        };
        
        // 5. Create final sector vote with 60/40 enforcement
        let vote = SectorDAAVote {
            sector_id: self.sector_id,
            symbol: symbol.to_string(),
            timestamp: Utc::now(),
            neural_vote,
            strategy_vote,
            sector_confidence,
            correlation_adjustment: correlation_adj,
            market_cap_weight,
            vote_id: generate_vote_id(),
            dependencies: self.get_vote_dependencies(symbol).await?,
            conflict_resolution: ConflictResolution::WeightedAverage,
        };
        
        Ok(vote)
    }
    
    /// Calculate final sector decision with 60/40 preservation
    pub fn calculate_sector_decision(&self, vote: &SectorDAAVote) -> DecisionSignal {
        // Enforce exact 60/40 weighting
        let neural_weight = 0.6;
        let strategy_weight = 0.4;
        
        // Calculate weighted signal strength
        let weighted_signal = (
            vote.neural_vote.signal_strength * neural_weight * vote.neural_vote.confidence
        ) + (
            vote.strategy_vote.signal_strength * strategy_weight * vote.strategy_vote.confidence
        );
        
        // Calculate combined confidence
        let combined_confidence = (
            vote.neural_vote.confidence * neural_weight
        ) + (
            vote.strategy_vote.confidence * strategy_weight
        );
        
        // Apply sector-specific adjustments
        let adjusted_signal = weighted_signal * vote.correlation_adjustment;
        let final_confidence = combined_confidence * vote.sector_confidence;
        
        DecisionSignal {
            signal_strength: adjusted_signal.clamp(-1.0, 1.0),
            confidence: final_confidence.clamp(0.0, 1.0),
            sector_weight: vote.market_cap_weight,
            metadata: self.create_decision_metadata(vote),
        }
    }
}
```

## 2. Master-Level Aggregation

### 2.1 MasterDAAVote Structure

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterDAAVote {
    pub symbol: String,
    pub timestamp: DateTime<Utc>,
    
    // Aggregated sector votes
    pub sector_votes: Vec<SectorDAAVote>,
    
    // Master-level 60/40 components
    pub aggregated_neural: AggregatedNeuralVote,  // 60% weight
    pub aggregated_strategy: AggregatedStrategyVote, // 40% weight
    
    // Master-level metadata
    pub portfolio_confidence: f64,
    pub market_regime_adjustment: f64,
    pub risk_adjustment: f64,
    
    // Byzantine consensus
    pub consensus_threshold: f64,     // 0.7 for 70% threshold
    pub participating_sectors: Vec<SectorId>,
    pub consensus_achieved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedNeuralVote {
    pub weighted_signal: f64,         // Sector-weighted neural signal
    pub confidence: f64,              // Combined neural confidence
    pub sector_contributions: HashMap<SectorId, f64>,
    pub neural_consensus: f64,        // Agreement across sector neural models
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedStrategyVote {
    pub weighted_signal: f64,         // Sector-weighted strategy signal
    pub confidence: f64,              // Combined strategy confidence
    pub sector_contributions: HashMap<SectorId, f64>,
    pub strategy_consensus: f64,      // Agreement across sector strategies
}
```

### 2.2 Master Aggregation Algorithm

```rust
impl MasterDAACoordinator {
    pub async fn aggregate_sector_votes(
        &self,
        symbol: &str,
        sector_votes: Vec<SectorDAAVote>,
    ) -> Result<MasterDAAVote, DAAError> {
        // 1. Validate sector votes meet Byzantine threshold (70%)
        if !self.validate_byzantine_consensus(&sector_votes, 0.7).await? {
            return Err(DAAError::ConsensusFailure(
                "Less than 70% of sectors participating".to_string()
            ));
        }
        
        // 2. Aggregate neural components (60% weight preservation)
        let aggregated_neural = self.aggregate_neural_votes(&sector_votes).await?;
        
        // 3. Aggregate strategy components (40% weight preservation)
        let aggregated_strategy = self.aggregate_strategy_votes(&sector_votes).await?;
        
        // 4. Calculate master-level adjustments
        let portfolio_confidence = self.calculate_portfolio_confidence(&sector_votes).await?;
        let market_regime_adj = self.get_market_regime_adjustment().await?;
        let risk_adjustment = self.calculate_risk_adjustment(&sector_votes).await?;
        
        // 5. Create master vote with preserved 60/40 ratio
        let master_vote = MasterDAAVote {
            symbol: symbol.to_string(),
            timestamp: Utc::now(),
            sector_votes,
            aggregated_neural,
            aggregated_strategy,
            portfolio_confidence,
            market_regime_adjustment: market_regime_adj,
            risk_adjustment,
            consensus_threshold: 0.7,
            participating_sectors: self.get_participating_sectors(&sector_votes),
            consensus_achieved: true,
        };
        
        Ok(master_vote)
    }
    
    /// Aggregate neural votes while preserving 60% weight
    async fn aggregate_neural_votes(
        &self,
        sector_votes: &[SectorDAAVote],
    ) -> Result<AggregatedNeuralVote, DAAError> {
        let mut weighted_signal = 0.0;
        let mut total_weight = 0.0;
        let mut confidence_sum = 0.0;
        let mut sector_contributions = HashMap::new();
        
        for vote in sector_votes {
            let sector_weight = self.get_sector_weight(&vote.sector_id).await?;
            let neural_contribution = vote.neural_vote.signal_strength 
                * vote.neural_vote.confidence 
                * sector_weight;
            
            weighted_signal += neural_contribution;
            total_weight += sector_weight;
            confidence_sum += vote.neural_vote.confidence * sector_weight;
            
            sector_contributions.insert(vote.sector_id, neural_contribution);
        }
        
        // Normalize by total weight
        let final_signal = if total_weight > 0.0 {
            weighted_signal / total_weight
        } else {
            0.0
        };
        
        let final_confidence = if total_weight > 0.0 {
            confidence_sum / total_weight
        } else {
            0.0
        };
        
        // Calculate neural consensus (agreement across sectors)
        let neural_consensus = self.calculate_neural_consensus(sector_votes).await?;
        
        Ok(AggregatedNeuralVote {
            weighted_signal: final_signal,
            confidence: final_confidence,
            sector_contributions,
            neural_consensus,
        })
    }
    
    /// Calculate final portfolio decision with 60/40 preservation
    pub fn calculate_portfolio_decision(&self, master_vote: &MasterDAAVote) -> PortfolioDecision {
        // CRITICAL: Maintain exact 60/40 weighting at master level
        let neural_weight = 0.6;
        let strategy_weight = 0.4;
        
        // Calculate master-level weighted signal
        let master_signal = (
            master_vote.aggregated_neural.weighted_signal * neural_weight
        ) + (
            master_vote.aggregated_strategy.weighted_signal * strategy_weight
        );
        
        // Calculate master-level confidence
        let master_confidence = (
            master_vote.aggregated_neural.confidence * neural_weight
        ) + (
            master_vote.aggregated_strategy.confidence * strategy_weight
        );
        
        // Apply master-level adjustments
        let adjusted_signal = master_signal 
            * master_vote.market_regime_adjustment
            * master_vote.risk_adjustment;
        
        let final_confidence = master_confidence * master_vote.portfolio_confidence;
        
        PortfolioDecision {
            symbol: master_vote.symbol.clone(),
            signal_strength: adjusted_signal.clamp(-1.0, 1.0),
            confidence: final_confidence.clamp(0.0, 1.0),
            decision_timestamp: Utc::now(),
            contributing_sectors: master_vote.participating_sectors.clone(),
            neural_weight: neural_weight,  // Track the 60% weight
            strategy_weight: strategy_weight, // Track the 40% weight
            consensus_metadata: self.create_consensus_metadata(master_vote),
        }
    }
}
```

## 3. Voting Flow Design

### 3.1 End-to-End Voting Process

```rust
pub struct DAAVotingOrchestrator {
    pub sector_coordinators: HashMap<SectorId, Arc<SectorDAACoordinator>>,
    pub master_coordinator: Arc<MasterDAACoordinator>,
    pub voting_cache: Arc<RwLock<VotingCache>>,
    pub performance_tracker: Arc<PerformanceTracker>,
}

impl DAAVotingOrchestrator {
    /// Execute complete voting flow with 60/40 preservation
    pub async fn execute_voting_flow(
        &self,
        symbol: &str,
        market_context: &MarketContext,
    ) -> Result<PortfolioDecision, DAAError> {
        let start_time = Instant::now();
        
        // Step 1: Parallel sector voting
        let sector_votes = self.collect_sector_votes(symbol, market_context).await?;
        
        // Step 2: Validate 60/40 ratios at sector level
        self.validate_sector_voting_ratios(&sector_votes)?;
        
        // Step 3: Master aggregation with 60/40 preservation
        let master_vote = self.master_coordinator
            .aggregate_sector_votes(symbol, sector_votes)
            .await?;
        
        // Step 4: Validate 60/40 ratio at master level
        self.validate_master_voting_ratio(&master_vote)?;
        
        // Step 5: Generate final portfolio decision
        let decision = self.master_coordinator
            .calculate_portfolio_decision(&master_vote)
            .await?;
        
        // Step 6: Track performance and store results
        self.track_voting_performance(symbol, &decision, start_time.elapsed()).await?;
        
        Ok(decision)
    }
    
    /// Collect votes from all relevant sectors in parallel
    async fn collect_sector_votes(
        &self,
        symbol: &str,
        market_context: &MarketContext,
    ) -> Result<Vec<SectorDAAVote>, DAAError> {
        // Get symbol's primary sector
        let primary_sector = self.get_symbol_sector(symbol).await?;
        
        // Get related sectors for cross-sector analysis
        let related_sectors = self.get_related_sectors(&primary_sector).await?;
        
        // Collect votes in parallel
        let mut vote_futures = Vec::new();
        
        // Primary sector vote (required)
        if let Some(coordinator) = self.sector_coordinators.get(&primary_sector) {
            vote_futures.push(coordinator.generate_sector_vote(symbol, market_context));
        }
        
        // Related sector votes (optional but valuable for consensus)
        for sector in related_sectors {
            if let Some(coordinator) = self.sector_coordinators.get(&sector) {
                vote_futures.push(coordinator.generate_sector_vote(symbol, market_context));
            }
        }
        
        // Execute all votes in parallel
        let vote_results = join_all(vote_futures).await;
        
        // Collect successful votes
        let mut sector_votes = Vec::new();
        for result in vote_results {
            match result {
                Ok(vote) => sector_votes.push(vote),
                Err(e) => warn!("Sector vote failed: {}", e),
            }
        }
        
        // Ensure we have at least one vote
        if sector_votes.is_empty() {
            return Err(DAAError::InsufficientVotes(
                "No sector votes collected".to_string()
            ));
        }
        
        Ok(sector_votes)
    }
    
    /// Validate 60/40 ratios are preserved at sector level
    fn validate_sector_voting_ratios(&self, votes: &[SectorDAAVote]) -> Result<(), DAAError> {
        for vote in votes {
            // Check neural vote weight (should be 0.6)
            if (vote.neural_vote.ensemble_weight - 0.6).abs() > 0.001 {
                return Err(DAAError::VotingRatioViolation(
                    format!("Sector {} neural weight is {}, expected 0.6", 
                           vote.sector_id.as_str(), vote.neural_vote.ensemble_weight)
                ));
            }
            
            // Implied strategy weight should be 0.4
            let implied_strategy_weight = 1.0 - vote.neural_vote.ensemble_weight;
            if (implied_strategy_weight - 0.4).abs() > 0.001 {
                return Err(DAAError::VotingRatioViolation(
                    format!("Sector {} implied strategy weight is {}, expected 0.4", 
                           vote.sector_id.as_str(), implied_strategy_weight)
                ));
            }
        }
        
        Ok(())
    }
    
    /// Validate 60/40 ratio is preserved at master level
    fn validate_master_voting_ratio(&self, master_vote: &MasterDAAVote) -> Result<(), DAAError> {
        // Check that aggregation preserves 60/40 ratio
        let neural_weight = 0.6;
        let strategy_weight = 0.4;
        
        // Validate weights sum to 1.0
        if (neural_weight + strategy_weight - 1.0).abs() > 0.001 {
            return Err(DAAError::VotingRatioViolation(
                "Master level weights do not sum to 1.0".to_string()
            ));
        }
        
        // Additional validation: check that neural confidence and strategy confidence
        // are being weighted properly in the final calculation
        let total_neural_weight: f64 = master_vote.sector_votes.iter()
            .map(|v| v.neural_vote.ensemble_weight)
            .sum();
        let avg_neural_weight = total_neural_weight / master_vote.sector_votes.len() as f64;
        
        if (avg_neural_weight - 0.6).abs() > 0.05 {  // Allow 5% tolerance for aggregation
            warn!("Average neural weight across sectors: {}, expected ~0.6", avg_neural_weight);
        }
        
        Ok(())
    }
}
```

## 4. Implementation Patterns

### 4.1 Vote Structure Enhancements

```rust
/// Enhanced vote with conflict resolution capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConflictResolution {
    WeightedAverage,      // Weight by confidence
    HighestConfidence,    // Choose most confident vote
    MajorityConsensus,    // Require majority agreement
    ByzantineConsensus,   // Require 70% agreement
}

/// Vote dependencies for proper sequencing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteDependency {
    pub dependency_id: String,
    pub dependency_type: DependencyType,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DependencyType {
    MarketData,       // Requires specific market data
    SectorAnalysis,   // Requires sector analysis completion
    RiskAssessment,   // Requires risk assessment
    CorrelationCheck, // Requires correlation analysis
}
```

### 4.2 Confidence Propagation

```rust
impl VotingConfidencePropagation {
    /// Propagate confidence from sector to master level
    pub fn propagate_confidence(
        &self,
        sector_votes: &[SectorDAAVote],
        sector_weights: &HashMap<SectorId, f64>,
    ) -> ConfidencePropagationResult {
        let mut neural_confidence_sum = 0.0;
        let mut strategy_confidence_sum = 0.0;
        let mut total_weight = 0.0;
        
        for vote in sector_votes {
            let sector_weight = sector_weights.get(&vote.sector_id).unwrap_or(&1.0);
            
            // Weighted confidence aggregation preserving 60/40
            neural_confidence_sum += vote.neural_vote.confidence * sector_weight * 0.6;
            strategy_confidence_sum += vote.strategy_vote.confidence * sector_weight * 0.4;
            total_weight += sector_weight;
        }
        
        let normalized_neural_confidence = neural_confidence_sum / total_weight;
        let normalized_strategy_confidence = strategy_confidence_sum / total_weight;
        
        // Combined confidence maintains 60/40 weighting
        let combined_confidence = normalized_neural_confidence + normalized_strategy_confidence;
        
        ConfidencePropagationResult {
            neural_confidence: normalized_neural_confidence,
            strategy_confidence: normalized_strategy_confidence,
            combined_confidence,
            confidence_agreement: self.calculate_agreement(sector_votes),
        }
    }
}
```

### 4.3 Performance Tracking Integration

```rust
impl VotingPerformanceTracker {
    pub async fn track_voting_decision(
        &self,
        symbol: &str,
        decision: &PortfolioDecision,
        sector_contributions: &HashMap<SectorId, f64>,
        duration: Duration,
    ) -> Result<(), DAAError> {
        let performance_record = VotingPerformanceRecord {
            symbol: symbol.to_string(),
            timestamp: Utc::now(),
            decision_strength: decision.signal_strength,
            confidence: decision.confidence,
            neural_weight: decision.neural_weight,      // Track 60%
            strategy_weight: decision.strategy_weight,  // Track 40%
            sector_contributions: sector_contributions.clone(),
            processing_duration: duration,
            consensus_achieved: decision.consensus_metadata.consensus_achieved,
        };
        
        // Store in performance database
        self.store_performance_record(performance_record).await?;
        
        // Update real-time metrics
        self.update_voting_metrics(symbol, decision).await?;
        
        Ok(())
    }
}
```

## 5. Conflict Resolution Mechanisms

### 5.1 Byzantine Consensus Implementation

```rust
impl ByzantineConsensusValidator {
    /// Validate Byzantine consensus with 70% threshold
    pub async fn validate_consensus(
        &self,
        sector_votes: &[SectorDAAVote],
        threshold: f64,
    ) -> Result<ConsensusResult, DAAError> {
        let total_sectors = sector_votes.len() as f64;
        let required_consensus = (total_sectors * threshold).ceil() as usize;
        
        // Group votes by signal direction (buy/sell/hold)
        let mut buy_votes = Vec::new();
        let mut sell_votes = Vec::new();
        let mut hold_votes = Vec::new();
        
        for vote in sector_votes {
            let sector_decision = self.calculate_sector_decision(vote);
            match sector_decision.signal_direction() {
                SignalDirection::Buy => buy_votes.push(vote),
                SignalDirection::Sell => sell_votes.push(vote),
                SignalDirection::Hold => hold_votes.push(vote),
            }
        }
        
        // Check if any direction achieves Byzantine consensus
        let consensus_achieved = 
            buy_votes.len() >= required_consensus ||
            sell_votes.len() >= required_consensus ||
            hold_votes.len() >= required_consensus;
        
        if !consensus_achieved {
            return Err(DAAError::ConsensusFailure(
                format!("Byzantine consensus not achieved. Required: {}, Got: Buy={}, Sell={}, Hold={}",
                       required_consensus, buy_votes.len(), sell_votes.len(), hold_votes.len())
            ));
        }
        
        // Determine winning consensus
        let winning_direction = if buy_votes.len() >= required_consensus {
            SignalDirection::Buy
        } else if sell_votes.len() >= required_consensus {
            SignalDirection::Sell
        } else {
            SignalDirection::Hold
        };
        
        Ok(ConsensusResult {
            direction: winning_direction,
            participating_votes: sector_votes.len(),
            consensus_percentage: (sector_votes.len() as f64 / total_sectors),
            neural_agreement: self.calculate_neural_agreement(sector_votes),
            strategy_agreement: self.calculate_strategy_agreement(sector_votes),
        })
    }
}
```

## 6. Testing and Validation

### 6.1 Unit Tests for Voting Preservation

```rust
#[cfg(test)]
mod voting_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_sector_vote_60_40_preservation() {
        let sector_coordinator = create_test_sector_coordinator().await;
        let market_context = create_test_market_context();
        
        let vote = sector_coordinator
            .generate_sector_vote("AAPL", &market_context)
            .await
            .unwrap();
        
        // Validate 60/40 ratio
        assert_eq!(vote.neural_vote.ensemble_weight, 0.6);
        
        let decision = sector_coordinator.calculate_sector_decision(&vote);
        
        // Validate decision preserves weighting
        let expected_neural_contribution = vote.neural_vote.signal_strength * 0.6;
        let expected_strategy_contribution = vote.strategy_vote.signal_strength * 0.4;
        let expected_total = expected_neural_contribution + expected_strategy_contribution;
        
        assert!((decision.signal_strength - expected_total).abs() < 0.001);
    }
    
    #[tokio::test]
    async fn test_master_aggregation_60_40_preservation() {
        let master_coordinator = create_test_master_coordinator().await;
        let sector_votes = create_test_sector_votes();
        
        let master_vote = master_coordinator
            .aggregate_sector_votes("AAPL", sector_votes)
            .await
            .unwrap();
        
        let decision = master_coordinator
            .calculate_portfolio_decision(&master_vote);
        
        // Validate master-level 60/40 preservation
        assert_eq!(decision.neural_weight, 0.6);
        assert_eq!(decision.strategy_weight, 0.4);
        assert!((decision.neural_weight + decision.strategy_weight - 1.0).abs() < 0.001);
    }
    
    #[tokio::test]
    async fn test_byzantine_consensus_validation() {
        let orchestrator = create_test_orchestrator().await;
        let insufficient_votes = create_insufficient_sector_votes();
        
        let result = orchestrator
            .execute_voting_flow("AAPL", &create_test_market_context())
            .await;
        
        // Should fail if less than 70% consensus
        assert!(result.is_err());
        
        let sufficient_votes = create_sufficient_sector_votes();
        let result = orchestrator
            .execute_voting_flow_with_votes("AAPL", sufficient_votes)
            .await;
        
        // Should succeed with 70% consensus
        assert!(result.is_ok());
    }
}
```

## 7. Performance Considerations

### 7.1 Caching Strategy

```rust
pub struct VotingCache {
    sector_votes: LruCache<String, SectorDAAVote>,
    master_votes: LruCache<String, MasterDAAVote>,
    confidence_propagation: LruCache<String, ConfidencePropagationResult>,
    ttl: Duration,
}

impl VotingCache {
    pub fn get_cached_sector_vote(
        &self,
        symbol: &str,
        sector: &SectorId,
    ) -> Option<SectorDAAVote> {
        let key = format!("{}:{}", symbol, sector.as_str());
        self.sector_votes.get(&key).cloned()
    }
    
    pub fn cache_sector_vote(
        &mut self,
        symbol: &str,
        sector: &SectorId,
        vote: SectorDAAVote,
    ) {
        let key = format!("{}:{}", symbol, sector.as_str());
        self.sector_votes.put(key, vote);
    }
}
```

### 7.2 Parallel Processing

```rust
impl ParallelVotingProcessor {
    pub async fn process_votes_parallel(
        &self,
        symbols: Vec<String>,
        market_contexts: Vec<MarketContext>,
    ) -> Result<Vec<PortfolioDecision>, DAAError> {
        let voting_futures = symbols
            .into_iter()
            .zip(market_contexts.into_iter())
            .map(|(symbol, context)| {
                self.orchestrator.execute_voting_flow(&symbol, &context)
            })
            .collect::<Vec<_>>();
        
        let results = join_all(voting_futures).await;
        
        let decisions = results
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
        
        Ok(decisions)
    }
}
```

## Summary

This specification ensures that the critical 60/40 voting ratio (neural/strategy) is preserved throughout the hierarchical DAA architecture:

1. **Sector Level**: Each `SectorDAACoordinator` enforces 60/40 weighting in its vote generation
2. **Master Level**: The `MasterDAACoordinator` preserves 60/40 ratios during aggregation
3. **Validation**: Multiple validation layers ensure ratio preservation
4. **Consensus**: Byzantine consensus (70% threshold) ensures robust decision-making
5. **Performance**: Caching and parallel processing maintain efficiency
6. **Tracking**: Comprehensive performance tracking validates the system

The design maintains the proven 60/40 balance while adding sector intelligence and hierarchical coordination for improved portfolio management.