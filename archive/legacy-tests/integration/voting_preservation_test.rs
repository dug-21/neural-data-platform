//! Voting Preservation Tests for Hierarchical DAA
//!
//! Comprehensive tests to validate that the 60/40 voting ratio is preserved
//! across all hierarchical DAA operations and that Byzantine consensus
//! mechanisms work correctly with the voting system.
//!
//! Key Test Areas:
//! - 60/40 voting ratio mathematical correctness
//! - Voting preservation under various confidence distributions
//! - Byzantine fault tolerance with voting mechanisms
//! - Performance impact of voting calculations
//! - Edge cases and boundary conditions
//! - Voting ratio stability across different sector combinations

use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

// Imports for voting preservation testing
use crate::data::sector_mapper::{SectorMapper, SectorMapperConfig, SectorId};
use crate::integration::daa_coordinator::{AutonomousDecision, TradingAction, RiskAssessment};
use crate::tests::unit::sector_daa_test::SectorDAACoordinator;
use crate::tests::integration::hierarchical_daa_test::HierarchicalDAATestEnvironment;

/// Voting ratio analyzer for comprehensive testing
pub struct VotingRatioAnalyzer {
    /// Historical voting ratios for analysis
    voting_history: Vec<VotingRatioSnapshot>,
    
    /// Expected ratio (60% confidence, 40% equal)
    expected_confidence_ratio: f64,
    expected_equal_ratio: f64,
    
    /// Tolerance for ratio validation
    tolerance: f64,
}

/// Snapshot of voting ratio calculation
#[derive(Debug, Clone)]
pub struct VotingRatioSnapshot {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub total_decisions: usize,
    pub confidence_weights: Vec<f64>,
    pub confidence_ratio: f64,
    pub equal_ratio: f64,
    pub aggregate_signal: f64,
    pub aggregate_confidence: f64,
    pub consensus_met: bool,
    pub sectors_involved: Vec<SectorId>,
}

impl VotingRatioAnalyzer {
    pub fn new() -> Self {
        Self {
            voting_history: Vec::new(),
            expected_confidence_ratio: 0.6,
            expected_equal_ratio: 0.4,
            tolerance: 0.05, // 5% tolerance
        }
    }
    
    /// Analyze a set of decisions and record voting ratio
    pub fn analyze_decisions(
        &mut self,
        decisions: &[(SectorId, AutonomousDecision)],
        coordinator: &SectorDAACoordinator,
    ) -> Result<VotingRatioSnapshot> {
        if decisions.is_empty() {
            return Err(anyhow::anyhow!("Cannot analyze empty decision set"));
        }
        
        let total_decisions = decisions.len();
        let confidence_weights: Vec<f64> = decisions.iter()
            .map(|(_, d)| d.confidence)
            .collect();
        
        // Calculate voting ratios using coordinator's method
        let (confidence_ratio, equal_ratio) = coordinator.validate_voting_ratio(decisions);
        
        // Calculate aggregate metrics
        let aggregate_confidence = confidence_weights.iter().sum::<f64>() / total_decisions as f64;
        
        // Calculate aggregate signal
        let mut weighted_signal = 0.0;
        let mut total_weight = 0.0;
        
        for (_, decision) in decisions {
            let signal_value = match &decision.action {
                TradingAction::Buy { .. } => 1.0,
                TradingAction::Sell { .. } => -1.0,
                TradingAction::Hold { .. } => 0.0,
                TradingAction::AdjustPosition { .. } => 0.5,
            };
            
            let confidence_weight = decision.confidence * 0.6;
            let equal_weight = (1.0 / total_decisions as f64) * 0.4;
            let combined_weight = confidence_weight + equal_weight;
            
            weighted_signal += signal_value * combined_weight;
            total_weight += combined_weight;
        }
        
        let aggregate_signal = if total_weight > 0.0 {
            weighted_signal / total_weight
        } else {
            0.0
        };
        
        let consensus_met = aggregate_confidence >= 0.6; // Default threshold
        let sectors_involved: Vec<SectorId> = decisions.iter()
            .map(|(sector, _)| *sector)
            .collect();
        
        let snapshot = VotingRatioSnapshot {
            timestamp: Utc::now(),
            total_decisions,
            confidence_weights,
            confidence_ratio,
            equal_ratio,
            aggregate_signal,
            aggregate_confidence,
            consensus_met,
            sectors_involved,
        };
        
        self.voting_history.push(snapshot.clone());
        Ok(snapshot)
    }
    
    /// Validate that voting ratios are within expected bounds
    pub fn validate_ratio(&self, snapshot: &VotingRatioSnapshot) -> Result<()> {
        let confidence_error = (snapshot.confidence_ratio - self.expected_confidence_ratio).abs();
        let equal_error = (snapshot.equal_ratio - self.expected_equal_ratio).abs();
        
        if confidence_error > self.tolerance {
            return Err(anyhow::anyhow!(
                "Confidence ratio {:.3} deviates from expected {:.3} by {:.3} (tolerance: {:.3})",
                snapshot.confidence_ratio, self.expected_confidence_ratio, confidence_error, self.tolerance
            ));
        }
        
        if equal_error > self.tolerance {
            return Err(anyhow::anyhow!(
                "Equal ratio {:.3} deviates from expected {:.3} by {:.3} (tolerance: {:.3})",
                snapshot.equal_ratio, self.expected_equal_ratio, equal_error, self.tolerance
            ));
        }
        
        // Ratios should sum to approximately 1.0
        let total_ratio = snapshot.confidence_ratio + snapshot.equal_ratio;
        let total_error = (total_ratio - 1.0).abs();
        if total_error > self.tolerance {
            return Err(anyhow::anyhow!(
                "Total ratio {:.3} should sum to 1.0 (error: {:.3})",
                total_ratio, total_error
            ));
        }
        
        Ok(())
    }
    
    /// Get statistics across all recorded snapshots
    pub fn get_statistics(&self) -> VotingStatistics {
        if self.voting_history.is_empty() {
            return VotingStatistics::default();
        }
        
        let count = self.voting_history.len();
        
        let avg_confidence_ratio = self.voting_history.iter()
            .map(|s| s.confidence_ratio)
            .sum::<f64>() / count as f64;
        
        let avg_equal_ratio = self.voting_history.iter()
            .map(|s| s.equal_ratio)
            .sum::<f64>() / count as f64;
        
        let confidence_variance = self.voting_history.iter()
            .map(|s| (s.confidence_ratio - avg_confidence_ratio).powi(2))
            .sum::<f64>() / count as f64;
        
        let equal_variance = self.voting_history.iter()
            .map(|s| (s.equal_ratio - avg_equal_ratio).powi(2))
            .sum::<f64>() / count as f64;
        
        let consensus_rate = self.voting_history.iter()
            .filter(|s| s.consensus_met)
            .count() as f64 / count as f64;
        
        let avg_sectors_involved = self.voting_history.iter()
            .map(|s| s.sectors_involved.len())
            .sum::<usize>() as f64 / count as f64;
        
        VotingStatistics {
            total_samples: count,
            avg_confidence_ratio,
            avg_equal_ratio,
            confidence_ratio_std_dev: confidence_variance.sqrt(),
            equal_ratio_std_dev: equal_variance.sqrt(),
            consensus_rate,
            avg_sectors_involved,
            ratio_within_tolerance: self.voting_history.iter()
                .filter(|s| self.validate_ratio(s).is_ok())
                .count() as f64 / count as f64,
        }
    }
    
    /// Test voting ratio under extreme confidence distributions
    pub fn test_extreme_distributions(&mut self, coordinator: &SectorDAACoordinator) -> Result<Vec<VotingRatioSnapshot>> {
        let mut snapshots = Vec::new();
        
        // Test case 1: All high confidence decisions
        let high_confidence_decisions = vec![
            (SectorId::Technology, self.create_test_decision(0.95, TradingAction::Buy {
                symbol: "TEST".to_string(),
                size: 0.02,
                stop_loss: None,
                take_profit: None,
            })),
            (SectorId::Financial, self.create_test_decision(0.98, TradingAction::Buy {
                symbol: "TEST".to_string(),
                size: 0.02,
                stop_loss: None,
                take_profit: None,
            })),
            (SectorId::Healthcare, self.create_test_decision(0.92, TradingAction::Buy {
                symbol: "TEST".to_string(),
                size: 0.02,
                stop_loss: None,
                take_profit: None,
            })),
        ];
        
        let snapshot1 = self.analyze_decisions(&high_confidence_decisions, coordinator)?;
        snapshots.push(snapshot1);
        
        // Test case 2: All low confidence decisions
        let low_confidence_decisions = vec![
            (SectorId::Technology, self.create_test_decision(0.15, TradingAction::Hold {
                reason: "Low confidence tech".to_string(),
            })),
            (SectorId::Financial, self.create_test_decision(0.12, TradingAction::Hold {
                reason: "Low confidence finance".to_string(),
            })),
            (SectorId::Healthcare, self.create_test_decision(0.18, TradingAction::Hold {
                reason: "Low confidence health".to_string(),
            })),
        ];
        
        let snapshot2 = self.analyze_decisions(&low_confidence_decisions, coordinator)?;
        snapshots.push(snapshot2);
        
        // Test case 3: Mixed confidence with wide distribution
        let mixed_confidence_decisions = vec![
            (SectorId::Technology, self.create_test_decision(0.95, TradingAction::Buy {
                symbol: "TEST".to_string(),
                size: 0.02,
                stop_loss: None,
                take_profit: None,
            })),
            (SectorId::Financial, self.create_test_decision(0.05, TradingAction::Sell {
                symbol: "TEST".to_string(),
                size: 0.01,
                reason: "Very low confidence".to_string(),
            })),
            (SectorId::Healthcare, self.create_test_decision(0.50, TradingAction::Hold {
                reason: "Neutral confidence".to_string(),
            })),
            (SectorId::Energy, self.create_test_decision(0.85, TradingAction::Buy {
                symbol: "TEST".to_string(),
                size: 0.03,
                stop_loss: None,
                take_profit: None,
            })),
            (SectorId::Materials, self.create_test_decision(0.25, TradingAction::Sell {
                symbol: "TEST".to_string(),
                size: 0.015,
                reason: "Low confidence materials".to_string(),
            })),
        ];
        
        let snapshot3 = self.analyze_decisions(&mixed_confidence_decisions, coordinator)?;
        snapshots.push(snapshot3);
        
        Ok(snapshots)
    }
    
    fn create_test_decision(&self, confidence: f64, action: TradingAction) -> AutonomousDecision {
        AutonomousDecision {
            timestamp: Utc::now(),
            action,
            confidence,
            risk_assessment: RiskAssessment {
                market_risk: 0.02,
                position_risk: 0.0,
                portfolio_risk: 0.01,
                volatility_adjusted_size: 0.02,
            },
            reasoning: vec![format!("Test decision with confidence {:.2}", confidence)],
            neural_consensus: HashMap::new(),
            adapted_parameters: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct VotingStatistics {
    pub total_samples: usize,
    pub avg_confidence_ratio: f64,
    pub avg_equal_ratio: f64,
    pub confidence_ratio_std_dev: f64,
    pub equal_ratio_std_dev: f64,
    pub consensus_rate: f64,
    pub avg_sectors_involved: f64,
    pub ratio_within_tolerance: f64,
}

// Helper functions for test decision creation
async fn create_test_sector_daa() -> Result<SectorDAACoordinator> {
    let env = HierarchicalDAATestEnvironment::new().await?;
    Ok(env.sector_daa)
}

fn create_diverse_confidence_decisions() -> Vec<(SectorId, AutonomousDecision)> {
    vec![
        (SectorId::Technology, AutonomousDecision {
            timestamp: Utc::now(),
            action: TradingAction::Buy {
                symbol: "TECH_TEST".to_string(),
                size: 0.02,
                stop_loss: None,
                take_profit: None,
            },
            confidence: 0.85,
            risk_assessment: RiskAssessment {
                market_risk: 0.02,
                position_risk: 0.0,
                portfolio_risk: 0.01,
                volatility_adjusted_size: 0.02,
            },
            reasoning: vec!["Strong tech momentum".to_string()],
            neural_consensus: HashMap::new(),
            adapted_parameters: None,
        }),
        (SectorId::Financial, AutonomousDecision {
            timestamp: Utc::now(),
            action: TradingAction::Hold {
                reason: "Financial sector neutral".to_string(),
            },
            confidence: 0.45,
            risk_assessment: RiskAssessment {
                market_risk: 0.03,
                position_risk: 0.0,
                portfolio_risk: 0.015,
                volatility_adjusted_size: 0.018,
            },
            reasoning: vec!["Uncertain financial outlook".to_string()],
            neural_consensus: HashMap::new(),
            adapted_parameters: None,
        }),
        (SectorId::Healthcare, AutonomousDecision {
            timestamp: Utc::now(),
            action: TradingAction::Buy {
                symbol: "HEALTH_TEST".to_string(),
                size: 0.015,
                stop_loss: None,
                take_profit: None,
            },
            confidence: 0.70,
            risk_assessment: RiskAssessment {
                market_risk: 0.025,
                position_risk: 0.0,
                portfolio_risk: 0.012,
                volatility_adjusted_size: 0.015,
            },
            reasoning: vec!["Healthcare showing stability".to_string()],
            neural_consensus: HashMap::new(),
            adapted_parameters: None,
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_voting_ratio_analyzer_creation() {
        let analyzer = VotingRatioAnalyzer::new();
        
        assert_eq!(analyzer.expected_confidence_ratio, 0.6);
        assert_eq!(analyzer.expected_equal_ratio, 0.4);
        assert_eq!(analyzer.tolerance, 0.05);
        assert!(analyzer.voting_history.is_empty());
    }
    
    #[tokio::test]
    async fn test_60_40_voting_ratio_mathematical_correctness() {
        let mut analyzer = VotingRatioAnalyzer::new();
        let sector_daa = create_test_sector_daa().await.unwrap();
        
        // Test with known confidence values
        let test_decisions = create_diverse_confidence_decisions();
        
        let snapshot = analyzer.analyze_decisions(&test_decisions, &sector_daa).unwrap();
        
        // Validate the mathematical correctness of 60/40 split
        analyzer.validate_ratio(&snapshot).unwrap();
        
        // Verify specific calculations
        let total_decisions = test_decisions.len() as f64;
        let confidence_weights: f64 = test_decisions.iter()
            .map(|(_, d)| d.confidence * 0.6)
            .sum();
        let equal_weights = total_decisions * 0.4;
        let total_weight = confidence_weights + equal_weights;
        
        let expected_confidence_ratio = confidence_weights / total_weight;
        let expected_equal_ratio = equal_weights / total_weight;
        
        assert!((snapshot.confidence_ratio - expected_confidence_ratio).abs() < 0.001);
        assert!((snapshot.equal_ratio - expected_equal_ratio).abs() < 0.001);
        assert!((expected_confidence_ratio - 0.6).abs() < 0.1); // Should be approximately 60%
        assert!((expected_equal_ratio - 0.4).abs() < 0.1); // Should be approximately 40%
    }
    
    #[tokio::test]
    async fn test_voting_preservation_under_various_confidence_distributions() {
        let mut analyzer = VotingRatioAnalyzer::new();
        let sector_daa = create_test_sector_daa().await.unwrap();
        
        // Test multiple scenarios with different confidence distributions
        let test_scenarios = vec![
            // Scenario 1: Uniform high confidence
            vec![
                (SectorId::Technology, analyzer.create_test_decision(0.9, TradingAction::Buy {
                    symbol: "TEST1".to_string(), size: 0.02, stop_loss: None, take_profit: None,
                })),
                (SectorId::Financial, analyzer.create_test_decision(0.9, TradingAction::Buy {
                    symbol: "TEST2".to_string(), size: 0.02, stop_loss: None, take_profit: None,
                })),
                (SectorId::Healthcare, analyzer.create_test_decision(0.9, TradingAction::Buy {
                    symbol: "TEST3".to_string(), size: 0.02, stop_loss: None, take_profit: None,
                })),
            ],
            // Scenario 2: Uniform low confidence
            vec![
                (SectorId::Technology, analyzer.create_test_decision(0.3, TradingAction::Hold {
                    reason: "Low tech confidence".to_string(),
                })),
                (SectorId::Financial, analyzer.create_test_decision(0.3, TradingAction::Hold {
                    reason: "Low finance confidence".to_string(),
                })),
                (SectorId::Healthcare, analyzer.create_test_decision(0.3, TradingAction::Hold {
                    reason: "Low health confidence".to_string(),
                })),
            ],
            // Scenario 3: Gaussian distribution around 0.6
            vec![
                (SectorId::Technology, analyzer.create_test_decision(0.6, TradingAction::Buy {
                    symbol: "TEST1".to_string(), size: 0.02, stop_loss: None, take_profit: None,
                })),
                (SectorId::Financial, analyzer.create_test_decision(0.65, TradingAction::Buy {
                    symbol: "TEST2".to_string(), size: 0.02, stop_loss: None, take_profit: None,
                })),
                (SectorId::Healthcare, analyzer.create_test_decision(0.55, TradingAction::Hold {
                    reason: "Moderate health confidence".to_string(),
                })),
                (SectorId::Energy, analyzer.create_test_decision(0.62, TradingAction::Buy {
                    symbol: "TEST3".to_string(), size: 0.02, stop_loss: None, take_profit: None,
                })),
                (SectorId::Materials, analyzer.create_test_decision(0.58, TradingAction::Hold {
                    reason: "Moderate materials confidence".to_string(),
                })),
            ],
        ];
        
        for (i, scenario) in test_scenarios.iter().enumerate() {
            let snapshot = analyzer.analyze_decisions(scenario, &sector_daa).unwrap();
            
            // Each scenario should maintain 60/40 ratio within tolerance
            analyzer.validate_ratio(&snapshot).unwrap_or_else(|e| {
                panic!("Scenario {} failed ratio validation: {}", i + 1, e);
            });
            
            // Verify that ratio is preserved regardless of confidence distribution
            assert!(snapshot.confidence_ratio >= 0.55 && snapshot.confidence_ratio <= 0.65,
                "Scenario {}: Confidence ratio {:.3} should be ~60%", i + 1, snapshot.confidence_ratio);
            assert!(snapshot.equal_ratio >= 0.35 && snapshot.equal_ratio <= 0.45,
                "Scenario {}: Equal ratio {:.3} should be ~40%", i + 1, snapshot.equal_ratio);
        }
        
        // Verify statistics across all scenarios
        let stats = analyzer.get_statistics();
        assert_eq!(stats.total_samples, test_scenarios.len());
        assert!(stats.ratio_within_tolerance >= 0.95, "At least 95% of samples should be within tolerance");
    }
    
    #[tokio::test]
    async fn test_extreme_confidence_distributions() {
        let mut analyzer = VotingRatioAnalyzer::new();
        let sector_daa = create_test_sector_daa().await.unwrap();
        
        let extreme_snapshots = analyzer.test_extreme_distributions(&sector_daa).unwrap();
        
        assert_eq!(extreme_snapshots.len(), 3);
        
        // Test high confidence scenario
        let high_conf_snapshot = &extreme_snapshots[0];
        assert!(high_conf_snapshot.aggregate_confidence > 0.9);
        assert!(high_conf_snapshot.consensus_met);
        analyzer.validate_ratio(high_conf_snapshot).unwrap();
        
        // Test low confidence scenario
        let low_conf_snapshot = &extreme_snapshots[1];
        assert!(low_conf_snapshot.aggregate_confidence < 0.2);
        assert!(!low_conf_snapshot.consensus_met);
        analyzer.validate_ratio(low_conf_snapshot).unwrap();
        
        // Test mixed confidence scenario
        let mixed_conf_snapshot = &extreme_snapshots[2];
        assert!(mixed_conf_snapshot.aggregate_confidence > 0.3 && mixed_conf_snapshot.aggregate_confidence < 0.8);
        analyzer.validate_ratio(mixed_conf_snapshot).unwrap();
        
        // All extreme scenarios should preserve 60/40 ratio
        for (i, snapshot) in extreme_snapshots.iter().enumerate() {
            assert!((snapshot.confidence_ratio - 0.6).abs() <= analyzer.tolerance,
                "Extreme scenario {} confidence ratio: {:.3}", i, snapshot.confidence_ratio);
            assert!((snapshot.equal_ratio - 0.4).abs() <= analyzer.tolerance,
                "Extreme scenario {} equal ratio: {:.3}", i, snapshot.equal_ratio);
        }
    }
    
    #[tokio::test]
    async fn test_byzantine_fault_tolerance_with_voting() {
        let mut analyzer = VotingRatioAnalyzer::new();
        let sector_daa = create_test_sector_daa().await.unwrap();
        
        // Create Byzantine scenario: some coordinators give conflicting signals
        let byzantine_decisions = vec![
            // Legitimate decisions (majority)
            (SectorId::Technology, analyzer.create_test_decision(0.85, TradingAction::Buy {
                symbol: "LEGIT1".to_string(), size: 0.02, stop_loss: None, take_profit: None,
            })),
            (SectorId::Financial, analyzer.create_test_decision(0.80, TradingAction::Buy {
                symbol: "LEGIT2".to_string(), size: 0.02, stop_loss: None, take_profit: None,
            })),
            (SectorId::Healthcare, analyzer.create_test_decision(0.75, TradingAction::Buy {
                symbol: "LEGIT3".to_string(), size: 0.02, stop_loss: None, take_profit: None,
            })),
            
            // Byzantine failures (minority)
            (SectorId::Energy, analyzer.create_test_decision(0.15, TradingAction::Sell {
                symbol: "BYZANTINE1".to_string(), size: 0.05, reason: "Byzantine failure".to_string(),
            })),
            (SectorId::Materials, analyzer.create_test_decision(0.05, TradingAction::Sell {
                symbol: "BYZANTINE2".to_string(), size: 0.1, reason: "Byzantine failure".to_string(),
            })),
        ];
        
        // Test Byzantine consensus validation
        let byzantine_consensus = sector_daa.validate_byzantine_consensus(&byzantine_decisions);
        assert!(byzantine_consensus, "Byzantine consensus should detect legitimate majority");
        
        // Test voting ratio preservation despite Byzantine failures
        let snapshot = analyzer.analyze_decisions(&byzantine_decisions, &sector_daa).unwrap();
        analyzer.validate_ratio(&snapshot).unwrap();
        
        // Byzantine failures should not significantly impact voting ratio
        assert!(snapshot.confidence_ratio >= 0.55 && snapshot.confidence_ratio <= 0.65);
        assert!(snapshot.equal_ratio >= 0.35 && snapshot.equal_ratio <= 0.45);
        
        // Aggregate should favor legitimate majority despite Byzantine failures
        assert!(snapshot.aggregate_signal > 0.0, "Should favor buy signal from legitimate majority");
        
        // Test aggregation handles Byzantine scenario correctly
        let aggregated = sector_daa.aggregate_cross_sector_decisions(byzantine_decisions).await.unwrap();
        
        match aggregated.action {
            TradingAction::Buy { .. } => {
                // Expected: legitimate buy signals should dominate
            }
            _ => panic!("Byzantine consensus should result in buy signal from legitimate majority"),
        }
        
        // Aggregation should note the presence of low-confidence decisions
        assert!(aggregated.reasoning.iter().any(|r| r.contains("confidence") || r.contains("consensus")));
    }
    
    #[tokio::test]
    async fn test_voting_ratio_stability_across_sector_combinations() {
        let mut analyzer = VotingRatioAnalyzer::new();
        let sector_daa = create_test_sector_daa().await.unwrap();
        
        // Test different combinations of sectors
        let sector_combinations = vec![
            // 2 sectors
            vec![SectorId::Technology, SectorId::Financial],
            // 3 sectors
            vec![SectorId::Technology, SectorId::Financial, SectorId::Healthcare],
            // 5 sectors
            vec![SectorId::Technology, SectorId::Financial, SectorId::Healthcare, SectorId::Energy, SectorId::Materials],
            // All 10 sectors
            SectorId::all_sectors(),
        ];
        
        for (i, sectors) in sector_combinations.iter().enumerate() {
            let mut decisions = Vec::new();
            
            for (j, &sector) in sectors.iter().enumerate() {
                let confidence = 0.5 + (j as f64 * 0.05); // Vary confidence slightly
                let action = if j % 2 == 0 {
                    TradingAction::Buy {
                        symbol: format!("TEST_{}_{}", i, j),
                        size: 0.02,
                        stop_loss: None,
                        take_profit: None,
                    }
                } else {
                    TradingAction::Hold {
                        reason: format!("Hold for sector {}", sector.as_str()),
                    }
                };
                
                decisions.push((sector, analyzer.create_test_decision(confidence, action)));
            }
            
            let snapshot = analyzer.analyze_decisions(&decisions, &sector_daa).unwrap();
            
            // Voting ratio should be stable regardless of number of sectors
            analyzer.validate_ratio(&snapshot).unwrap_or_else(|e| {
                panic!("Sector combination {} ({} sectors) failed: {}", i, sectors.len(), e);
            });
            
            // Verify consistent ratio across all combinations
            assert!((snapshot.confidence_ratio - 0.6).abs() <= analyzer.tolerance,
                "Combination {}: confidence ratio {:.3} should be ~60%", i, snapshot.confidence_ratio);
            assert!((snapshot.equal_ratio - 0.4).abs() <= analyzer.tolerance,
                "Combination {}: equal ratio {:.3} should be ~40%", i, snapshot.equal_ratio);
        }
        
        // Statistics should show consistent performance
        let stats = analyzer.get_statistics();
        assert_eq!(stats.total_samples, sector_combinations.len());
        assert!(stats.confidence_ratio_std_dev < 0.02, "Confidence ratio should be stable");
        assert!(stats.equal_ratio_std_dev < 0.02, "Equal ratio should be stable");
        assert_eq!(stats.ratio_within_tolerance, 1.0, "All combinations should be within tolerance");
    }
    
    #[tokio::test]
    async fn test_performance_impact_of_voting_calculations() {
        let mut analyzer = VotingRatioAnalyzer::new();
        let sector_daa = create_test_sector_daa().await.unwrap();
        
        // Test with large number of decisions to check performance
        let mut large_decision_set = Vec::new();
        let all_sectors = SectorId::all_sectors();
        
        // Create 100 decisions across all sectors
        for i in 0..100 {
            let sector = all_sectors[i % all_sectors.len()];
            let confidence = 0.3 + ((i as f64 * 0.007) % 0.6); // Vary confidence 0.3-0.9
            
            let action = match i % 3 {
                0 => TradingAction::Buy {
                    symbol: format!("PERF_TEST_{}", i),
                    size: 0.01 + ((i as f64 * 0.0001) % 0.03),
                    stop_loss: None,
                    take_profit: None,
                },
                1 => TradingAction::Sell {
                    symbol: format!("PERF_TEST_{}", i),
                    size: 0.01 + ((i as f64 * 0.0001) % 0.03),
                    reason: format!("Performance test sell {}", i),
                },
                _ => TradingAction::Hold {
                    reason: format!("Performance test hold {}", i),
                },
            };
            
            large_decision_set.push((sector, analyzer.create_test_decision(confidence, action)));
        }
        
        // Time the voting calculation
        let start = std::time::Instant::now();
        let snapshot = analyzer.analyze_decisions(&large_decision_set, &sector_daa).unwrap();
        let duration = start.elapsed();
        
        // Performance should be reasonable (< 100ms for 100 decisions)
        assert!(duration.as_millis() < 100, "Voting calculation should be fast: {:?}", duration);
        
        // Accuracy should be maintained even with large datasets
        analyzer.validate_ratio(&snapshot).unwrap();
        
        // Test aggregation performance
        let start = std::time::Instant::now();
        let aggregated = sector_daa.aggregate_cross_sector_decisions(large_decision_set).await.unwrap();
        let aggregation_duration = start.elapsed();
        
        // Aggregation should also be performant
        assert!(aggregation_duration.as_millis() < 200, "Aggregation should be fast: {:?}", aggregation_duration);
        
        // Verify aggregation preserves voting ratio
        assert!(aggregated.adapted_parameters.is_some());
        let params = aggregated.adapted_parameters.unwrap();
        assert!(params.contains_key("aggregation_method"));
        assert_eq!(params.get("aggregation_method").unwrap(), &"60_40_voting".to_string().into());
    }
    
    #[tokio::test]
    async fn test_edge_cases_and_boundary_conditions() {
        let mut analyzer = VotingRatioAnalyzer::new();
        let sector_daa = create_test_sector_daa().await.unwrap();
        
        // Edge case 1: Single decision
        let single_decision = vec![
            (SectorId::Technology, analyzer.create_test_decision(0.75, TradingAction::Buy {
                symbol: "SINGLE_TEST".to_string(),
                size: 0.02,
                stop_loss: None,
                take_profit: None,
            })),
        ];
        
        let snapshot1 = analyzer.analyze_decisions(&single_decision, &sector_daa).unwrap();
        analyzer.validate_ratio(&snapshot1).unwrap();
        
        // Edge case 2: Zero confidence decisions
        let zero_confidence_decisions = vec![
            (SectorId::Technology, analyzer.create_test_decision(0.0, TradingAction::Hold {
                reason: "Zero confidence".to_string(),
            })),
            (SectorId::Financial, analyzer.create_test_decision(0.0, TradingAction::Hold {
                reason: "Zero confidence".to_string(),
            })),
        ];
        
        let snapshot2 = analyzer.analyze_decisions(&zero_confidence_decisions, &sector_daa).unwrap();
        analyzer.validate_ratio(&snapshot2).unwrap();
        // With zero confidence, equal weighting should dominate
        assert!(snapshot2.equal_ratio > snapshot2.confidence_ratio);
        
        // Edge case 3: Maximum confidence decisions
        let max_confidence_decisions = vec![
            (SectorId::Technology, analyzer.create_test_decision(1.0, TradingAction::Buy {
                symbol: "MAX_TEST1".to_string(),
                size: 0.02,
                stop_loss: None,
                take_profit: None,
            })),
            (SectorId::Financial, analyzer.create_test_decision(1.0, TradingAction::Buy {
                symbol: "MAX_TEST2".to_string(),
                size: 0.02,
                stop_loss: None,
                take_profit: None,
            })),
        ];
        
        let snapshot3 = analyzer.analyze_decisions(&max_confidence_decisions, &sector_daa).unwrap();
        analyzer.validate_ratio(&snapshot3).unwrap();
        // With maximum confidence, confidence weighting should be stronger
        assert!(snapshot3.confidence_ratio > snapshot3.equal_ratio);
        
        // Edge case 4: Empty aggregation should fail gracefully
        let empty_result = sector_daa.aggregate_cross_sector_decisions(vec![]).await;
        assert!(empty_result.is_err(), "Empty decision aggregation should fail");
        
        // Verify all edge cases maintain fundamental voting properties
        let final_stats = analyzer.get_statistics();
        assert!(final_stats.ratio_within_tolerance >= 0.75, "Most edge cases should maintain voting ratio");
    }
    
    #[tokio::test]
    async fn test_voting_statistics_comprehensive_analysis() {
        let mut analyzer = VotingRatioAnalyzer::new();
        let sector_daa = create_test_sector_daa().await.unwrap();
        
        // Generate multiple decision scenarios for comprehensive statistics
        for i in 0..50 {
            let num_sectors = 2 + (i % 8); // 2-9 sectors per decision
            let mut decisions = Vec::new();
            
            for j in 0..num_sectors {
                let sector_idx = (i + j) % SectorId::all_sectors().len();
                let sector = SectorId::all_sectors()[sector_idx];
                
                // Vary confidence in interesting patterns
                let confidence = match i % 5 {
                    0 => 0.9 - (j as f64 * 0.1), // Decreasing confidence
                    1 => 0.1 + (j as f64 * 0.1), // Increasing confidence
                    2 => 0.5 + ((j as f64 * 0.1).sin() * 0.3), // Sinusoidal
                    3 => if j % 2 == 0 { 0.8 } else { 0.2 }, // Alternating
                    _ => 0.6, // Constant
                };
                
                let action = match j % 3 {
                    0 => TradingAction::Buy {
                        symbol: format!("STAT_TEST_{}_{}", i, j),
                        size: 0.02,
                        stop_loss: None,
                        take_profit: None,
                    },
                    1 => TradingAction::Sell {
                        symbol: format!("STAT_TEST_{}_{}", i, j),
                        size: 0.015,
                        reason: format!("Statistical test sell {}", i),
                    },
                    _ => TradingAction::Hold {
                        reason: format!("Statistical test hold {}", i),
                    },
                };
                
                decisions.push((sector, analyzer.create_test_decision(confidence, action)));
            }
            
            analyzer.analyze_decisions(&decisions, &sector_daa).unwrap();
        }
        
        // Analyze comprehensive statistics
        let stats = analyzer.get_statistics();
        
        assert_eq!(stats.total_samples, 50);
        
        // Voting ratios should be stable across diverse scenarios
        assert!((stats.avg_confidence_ratio - 0.6).abs() < 0.05,
            "Average confidence ratio should be ~60%: {:.3}", stats.avg_confidence_ratio);
        assert!((stats.avg_equal_ratio - 0.4).abs() < 0.05,
            "Average equal ratio should be ~40%: {:.3}", stats.avg_equal_ratio);
        
        // Standard deviations should be reasonable (indicating stability)
        assert!(stats.confidence_ratio_std_dev < 0.03,
            "Confidence ratio should be stable: std_dev = {:.4}", stats.confidence_ratio_std_dev);
        assert!(stats.equal_ratio_std_dev < 0.03,
            "Equal ratio should be stable: std_dev = {:.4}", stats.equal_ratio_std_dev);
        
        // Most samples should be within tolerance
        assert!(stats.ratio_within_tolerance >= 0.9,
            "At least 90% should be within tolerance: {:.2}%", stats.ratio_within_tolerance * 100.0);
        
        // Consensus rate should be reasonable
        assert!(stats.consensus_rate >= 0.3 && stats.consensus_rate <= 0.9,
            "Consensus rate should be reasonable: {:.2}%", stats.consensus_rate * 100.0);
        
        // Average sectors involved should reflect our test pattern
        assert!(stats.avg_sectors_involved >= 2.0 && stats.avg_sectors_involved <= 9.0,
            "Average sectors involved: {:.1}", stats.avg_sectors_involved);
    }
}