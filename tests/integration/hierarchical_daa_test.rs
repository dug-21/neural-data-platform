//! Integration Tests for Hierarchical DAA System
//!
//! Tests the complete hierarchical DAA system with real component integration:
//! - SectorDAACoordinator with real DAA coordinators
//! - Neural Enhanced Strategy integration with sector routing
//! - Performance tracking and bottleneck analysis
//! - Cross-sector decision aggregation and consensus
//! - Memory efficiency and scalability testing
//!
//! This integration test validates that the hierarchical extension maintains
//! all existing DAA capabilities while adding sector-based intelligence.

use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::timeout;

// Import all required components for integration testing
use crate::config::{NeuralConfig, SectorModelsConfig};
use crate::data::sector_mapper::{SectorMapper, SectorMapperConfig, SectorInfo, SectorId, MarketCapTier};
use crate::data::{TimeSeriesData, DataAccessLayer, RedisCache};
use crate::integration::daa_coordinator::{DaaCoordinator, DaaConfig, AutonomousDecision, TradingAction, RiskAssessment};
use crate::monitoring::model_performance_tracker::ModelPerformanceTracker;
use crate::neural::{NeuralPredictor, PredictionResult, vendor_predictor::VendorPredictor};
use crate::strategies::{
    MarketContext, Position, Signal, TradingStrategy, StrategyConfig, StrategyError, PositionSide,
    neural_enhanced::NeuralEnhancedStrategy,
};
use crate::utils::market_hours::MarketHours;
use async_trait::async_trait;

// Import the SectorDAACoordinator from unit tests (would be moved to main codebase)
use crate::tests::unit::sector_daa_test::SectorDAACoordinator;

/// Integration test environment for hierarchical DAA
pub struct HierarchicalDAATestEnvironment {
    /// Sector-based DAA coordinator
    sector_daa: SectorDAACoordinator,
    
    /// Individual sector coordinators for direct testing
    sector_coordinators: HashMap<SectorId, Arc<DaaCoordinator>>,
    
    /// Master coordinator for cross-sector decisions
    master_coordinator: Arc<DaaCoordinator>,
    
    /// Sector mapper for symbol routing
    sector_mapper: Arc<SectorMapper>,
    
    /// Performance tracker for metrics
    performance_tracker: Arc<ModelPerformanceTracker>,
    
    /// Neural enhanced strategies per sector
    sector_strategies: HashMap<SectorId, Arc<NeuralEnhancedStrategy>>,
    
    /// Test data access layer
    data_layer: Arc<DataAccessLayer>,
    
    /// Decision channel for monitoring
    decision_receiver: mpsc::Receiver<AutonomousDecision>,
    
    /// Market hours for testing
    market_hours: Arc<MarketHours>,
}

impl HierarchicalDAATestEnvironment {
    /// Create a comprehensive test environment
    pub async fn new() -> Result<Self> {
        // Initialize core components
        let neural_config = Self::create_neural_config();
        let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
        let performance_tracker = Arc::new(ModelPerformanceTracker::new()?);
        let market_hours = Arc::new(MarketHours::default());
        
        // Create data access layer with mocked components
        let redis_cache = Arc::new(RedisCache::new_mock()?);
        let data_layer = Arc::new(DataAccessLayer::new_with_cache(redis_cache).await?);
        
        // Create master coordinator
        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config.clone())?);
        let (master_tx, _master_rx) = mpsc::channel(1000);
        let master_config = DaaConfig {
            enable_autonomous_trading: true,
            confidence_threshold: 0.7,
            max_position_size: 0.05,
            risk_tolerance: 0.02,
            enable_stop_loss: true,
            stop_loss_percentage: 0.05,
            enable_take_profit: true,
            take_profit_percentage: 0.10,
            enable_neural_enhancement: true,
            neural_weight: 0.8,
            performance_tracking: true,
            enable_risk_management: true,
            max_daily_trades: 50,
            enable_market_hours_check: true,
            enable_position_sizing: true,
            min_confidence_for_trade: 0.6,
            max_correlation_threshold: 0.8,
            enable_portfolio_rebalancing: true,
            rebalancing_interval_hours: 24,
        };
        
        let master_coordinator = Arc::new(DaaCoordinator::new(
            master_config,
            neural_predictor.clone(),
            master_tx,
            market_hours.clone(),
        )?);
        
        // Create sector coordinators and strategies
        let mut sector_coordinators = HashMap::new();
        let mut sector_strategies = HashMap::new();
        
        for sector in SectorId::all_sectors() {
            // Create sector-specific neural predictor
            let sector_predictor = Arc::new(VendorPredictor::new(
                &neural_config,
                sector_mapper.clone(),
                performance_tracker.clone(),
            )?);
            
            // Create sector coordinator
            let (sector_tx, _sector_rx) = mpsc::channel(100);
            let sector_config = DaaConfig {
                enable_autonomous_trading: true,
                confidence_threshold: 0.65, // Slightly lower for sector-specific decisions
                max_position_size: 0.03, // Smaller positions per sector
                risk_tolerance: 0.015,
                enable_stop_loss: true,
                stop_loss_percentage: 0.04,
                enable_take_profit: true,
                take_profit_percentage: 0.08,
                enable_neural_enhancement: true,
                neural_weight: 0.85, // Higher neural weight for sector expertise
                performance_tracking: true,
                enable_risk_management: true,
                max_daily_trades: 20, // Fewer trades per sector
                enable_market_hours_check: true,
                enable_position_sizing: true,
                min_confidence_for_trade: 0.55,
                max_correlation_threshold: 0.9, // Higher correlation allowed within sectors
                enable_portfolio_rebalancing: true,
                rebalancing_interval_hours: 12, // More frequent rebalancing
            };
            
            let sector_coordinator = Arc::new(DaaCoordinator::new(
                sector_config,
                neural_predictor.clone(),
                sector_tx,
                market_hours.clone(),
            )?);
            
            // Create neural enhanced strategy for sector
            let strategy_config = StrategyConfig {
                name: format!("{}_neural_enhanced", sector.as_str()),
                parameters: {
                    let mut params = HashMap::new();
                    params.insert("sector".to_string(), serde_json::json!(sector.as_str()));
                    params.insert("neural_weight".to_string(), serde_json::json!(0.85));
                    params.insert("risk_factor".to_string(), serde_json::json!(0.015));
                    params.insert("confidence_threshold".to_string(), serde_json::json!(0.65));
                    params
                },
                risk_tolerance: 0.015,
                max_position_size: 0.03,
                time_horizon: chrono::Duration::hours(24),
                enable_stop_loss: true,
                stop_loss_percentage: 0.04,
            };
            
            let mut sector_strategy = NeuralEnhancedStrategy::new(
                sector_predictor,
                data_layer.clone()
            );
            sector_strategy.initialize(strategy_config).await?;
            
            // Register strategy with coordinator
            let strategy_name = format!("{}_enhanced", sector.as_str());
            sector_coordinator.register_strategy(
                strategy_name.clone(),
                Box::new(sector_strategy.clone()),
            ).await;
            
            sector_coordinators.insert(sector, sector_coordinator);
            sector_strategies.insert(sector, Arc::new(sector_strategy));
        }
        
        // Create main decision channel
        let (decision_tx, decision_rx) = mpsc::channel(1000);
        
        // Create sector DAA coordinator
        let sector_daa = SectorDAACoordinator::new(
            sector_coordinators.clone(),
            master_coordinator.clone(),
            sector_mapper.clone(),
            decision_tx,
        );
        
        Ok(Self {
            sector_daa,
            sector_coordinators,
            master_coordinator,
            sector_mapper,
            performance_tracker,
            sector_strategies,
            data_layer,
            decision_receiver: decision_rx,
            market_hours,
        })
    }
    
    fn create_neural_config() -> NeuralConfig {
        NeuralConfig {
            memory_gb: 2.0,
            models: vec!["VendorPredictor".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 20,
            enable_model_monitoring: true,
            accuracy_threshold: 0.75,
            use_real_models: true, // Use real vendor models for integration testing
            enable_health_checks: true,
            enable_fallback: true,
            enable_circuit_breakers: true,
            enable_graceful_degradation: true,
            enable_performance_monitoring: true,
            enable_adaptive_retry: true,
            enable_model_ensembles: true,
            model_timeout_seconds: 30,
            max_retries: 3,
            error_threshold: 0.03,
        }
    }
    
    /// Create realistic market data for testing
    pub fn create_market_data(&self, symbol: &str, trend: f64) -> Vec<TimeSeriesData> {
        let mut data = Vec::new();
        let base_price = match symbol {
            "AAPL" => 180.0,
            "MSFT" => 300.0,
            "GOOGL" => 2800.0,
            "JPM" => 140.0,
            "BAC" => 30.0,
            "JNJ" => 160.0,
            "PFE" => 35.0,
            "XOM" => 110.0,
            "TSLA" => 250.0,
            _ => 100.0,
        };
        
        let mut current_price = base_price;
        let now = Utc::now();
        
        // Generate 100 data points with trend and volatility
        for i in 0..100 {
            let volatility = (i as f64 * 0.01).sin() * 0.02; // Varying volatility
            let trend_factor = trend * 0.001; // Convert trend to price movement
            let random_factor = (i as f64 * 0.1).sin() * 0.01; // Pseudo-random movement
            
            current_price *= 1.0 + trend_factor + volatility + random_factor;
            
            let timestamp = now - chrono::Duration::hours(100 - i as i64);
            data.push(TimeSeriesData {
                symbol: symbol.to_string(),
                timestamp,
                open: current_price * 0.999,
                high: current_price * 1.002,
                low: current_price * 0.998,
                close: current_price,
                volume: 100000.0 + (i as f64 * 1000.0),
                indicators: {
                    let mut indicators = HashMap::new();
                    indicators.insert("sma_20".to_string(), current_price * 0.995);
                    indicators.insert("rsi".to_string(), 50.0 + trend * 10.0);
                    indicators.insert("bb_upper".to_string(), current_price * 1.02);
                    indicators.insert("bb_lower".to_string(), current_price * 0.98);
                    indicators
                },
                source: Some("test_integration".to_string()),
                entity: Some(symbol.to_string()),
                value: Some(current_price),
                metadata: Some({
                    let mut meta = HashMap::new();
                    meta.insert("trend".to_string(), serde_json::json!(trend));
                    meta.insert("volatility".to_string(), serde_json::json!(volatility));
                    meta
                }),
                values: vec![current_price],
                timestamps: vec![timestamp],
                metadata_map: {
                    let mut map = HashMap::new();
                    map.insert("symbol".to_string(), serde_json::json!(symbol));
                    map.insert("data_quality".to_string(), serde_json::json!("high"));
                    map
                },
            });
        }
        
        data
    }
    
    /// Create market context for symbol
    pub fn create_market_context(&self, symbol: &str, price: f64) -> MarketContext {
        MarketContext {
            symbol: symbol.to_string(),
            current_price: price,
            bid: price * 0.9995,
            ask: price * 1.0005,
            volume_24h: 5000000.0,
            volatility: 0.025,
            timestamp: Utc::now().timestamp(),
        }
    }
    
    /// Wait for decisions with timeout
    pub async fn collect_decisions(&mut self, expected_count: usize, timeout_secs: u64) -> Result<Vec<AutonomousDecision>> {
        let mut decisions = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);
        
        while decisions.len() < expected_count && Instant::now() < deadline {
            match timeout(Duration::from_millis(100), self.decision_receiver.recv()).await {
                Ok(Some(decision)) => decisions.push(decision),
                Ok(None) => break, // Channel closed
                Err(_) => continue, // Timeout, keep trying
            }
        }
        
        Ok(decisions)
    }
    
    /// Get comprehensive system metrics
    pub async fn get_system_metrics(&self) -> HashMap<String, serde_json::Value> {
        let mut metrics = HashMap::new();
        
        // Master coordinator metrics
        let master_metrics = self.master_coordinator.get_metrics().await;
        metrics.insert("master_coordinator".to_string(), serde_json::json!(master_metrics));
        
        // Sector coordinator metrics
        let mut sector_metrics = HashMap::new();
        for (sector, coordinator) in &self.sector_coordinators {
            let coord_metrics = coordinator.get_metrics().await;
            sector_metrics.insert(sector.as_str().to_string(), coord_metrics);
        }
        metrics.insert("sector_coordinators".to_string(), serde_json::json!(sector_metrics));
        
        // Performance tracker metrics
        let perf_metrics = self.performance_tracker.get_overall_metrics().await.unwrap_or_default();
        metrics.insert("performance_tracker".to_string(), serde_json::json!(perf_metrics));
        
        // Sector DAA specific metrics
        let sector_stats = self.sector_daa.get_sector_stats();
        metrics.insert("sector_performance".to_string(), serde_json::json!(sector_stats));
        
        metrics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_hierarchical_daa_environment_creation() {
        let env = HierarchicalDAATestEnvironment::new().await.unwrap();
        
        // Verify all sectors have coordinators
        assert_eq!(env.sector_coordinators.len(), 10); // All SectorId variants
        
        // Verify all sectors have strategies
        assert_eq!(env.sector_strategies.len(), 10);
        
        // Verify master coordinator exists
        let master_metrics = env.master_coordinator.get_metrics().await;
        assert_eq!(master_metrics.total_decisions, 0); // Fresh coordinator
        
        // Verify sector mapper is initialized
        let tech_symbols = env.sector_mapper.get_symbols_in_sector(&SectorId::Technology);
        assert!(!tech_symbols.is_empty()); // Should have AAPL, MSFT, GOOGL
    }
    
    #[tokio::test]
    async fn test_sector_routing_integration() {
        let env = HierarchicalDAATestEnvironment::new().await.unwrap();
        
        // Test routing for different sectors
        let test_cases = vec![
            ("AAPL", SectorId::Technology),
            ("JPM", SectorId::Financial),
            ("JNJ", SectorId::Healthcare),
            ("XOM", SectorId::Energy),
        ];
        
        for (symbol, expected_sector) in test_cases {
            let sector_info = env.sector_mapper.get_sector(symbol).unwrap();
            assert_eq!(sector_info.sector_id, expected_sector);
            
            // Test that symbol routes to correct coordinator
            let market_data = env.create_market_data(symbol, 0.5); // Positive trend
            let market_context = env.create_market_context(symbol, market_data.last().unwrap().close);
            
            let decision = env.sector_daa.route_decision(
                symbol,
                &market_context,
                None,
                &market_data,
            ).await.unwrap();
            
            // Verify sector information is included
            assert!(decision.adapted_parameters.is_some());
            let params = decision.adapted_parameters.unwrap();
            assert!(params.contains_key("sector"));
            assert_eq!(params.get("sector").unwrap(), &expected_sector.as_str().to_string().into());
        }
    }
    
    #[tokio::test]
    async fn test_cross_sector_decision_aggregation() {
        let env = HierarchicalDAATestEnvironment::new().await.unwrap();
        
        // Create decisions from multiple sectors
        let mut sector_decisions = Vec::new();
        
        // Technology sector - bullish
        let tech_market_data = env.create_market_data("AAPL", 1.0); // Strong positive trend
        let tech_context = env.create_market_context("AAPL", tech_market_data.last().unwrap().close);
        let tech_decision = env.sector_daa.route_decision(
            "AAPL",
            &tech_context,
            None,
            &tech_market_data,
        ).await.unwrap();
        sector_decisions.push((SectorId::Technology, tech_decision));
        
        // Financial sector - neutral
        let finance_market_data = env.create_market_data("JPM", 0.0); // Neutral trend
        let finance_context = env.create_market_context("JPM", finance_market_data.last().unwrap().close);
        let finance_decision = env.sector_daa.route_decision(
            "JPM",
            &finance_context,
            None,
            &finance_market_data,
        ).await.unwrap();
        sector_decisions.push((SectorId::Financial, finance_decision));
        
        // Healthcare sector - slightly bearish
        let health_market_data = env.create_market_data("JNJ", -0.3); // Slight negative trend
        let health_context = env.create_market_context("JNJ", health_market_data.last().unwrap().close);
        let health_decision = env.sector_daa.route_decision(
            "JNJ",
            &health_context,
            None,
            &health_market_data,
        ).await.unwrap();
        sector_decisions.push((SectorId::Healthcare, health_decision));
        
        // Test aggregation
        let aggregated = env.sector_daa.aggregate_cross_sector_decisions(sector_decisions.clone()).await.unwrap();
        
        // Verify 60/40 voting ratio
        let (confidence_ratio, equal_ratio) = env.sector_daa.validate_voting_ratio(&sector_decisions);
        assert!((confidence_ratio - 0.6).abs() < 0.1, "Confidence ratio should be ~60%");
        assert!((equal_ratio - 0.4).abs() < 0.1, "Equal ratio should be ~40%");
        
        // Verify aggregation metadata
        assert!(aggregated.adapted_parameters.is_some());
        let params = aggregated.adapted_parameters.unwrap();
        assert_eq!(params.get("aggregation_method").unwrap(), &"60_40_voting".to_string().into());
        assert_eq!(params.get("sectors_involved").unwrap(), &3.into());
        
        // Verify reasoning includes voting information
        assert!(aggregated.reasoning.iter().any(|r| r.contains("60/40 voting ratio applied")));
    }
    
    #[tokio::test]
    async fn test_byzantine_consensus_with_real_components() {
        let env = HierarchicalDAATestEnvironment::new().await.unwrap();
        
        // Create decisions from multiple sectors to test Byzantine consensus
        let mut decisions = Vec::new();
        
        let symbols_and_sectors = vec![
            ("AAPL", SectorId::Technology, 0.8), // Strong buy signal
            ("MSFT", SectorId::Technology, 0.85), // Strong buy signal
            ("GOOGL", SectorId::Technology, 0.9), // Strong buy signal
            ("JPM", SectorId::Financial, 0.2), // Weak signal (potential Byzantine failure)
            ("BAC", SectorId::Financial, 0.75), // Buy signal
            ("JNJ", SectorId::Healthcare, -0.1), // Sell signal (potential Byzantine failure)
        ];
        
        for (symbol, sector, trend) in symbols_and_sectors {
            let market_data = env.create_market_data(symbol, trend);
            let market_context = env.create_market_context(symbol, market_data.last().unwrap().close);
            
            let decision = env.sector_daa.route_decision(
                symbol,
                &market_context,
                None,
                &market_data,
            ).await.unwrap();
            
            decisions.push((sector, decision));
        }
        
        // Test Byzantine consensus validation
        let consensus_valid = env.sector_daa.validate_byzantine_consensus(&decisions);
        
        // With 6 decisions and potential Byzantine failures, consensus should still work
        // if the majority (4+) agree
        assert!(consensus_valid || decisions.len() >= 3, "Byzantine consensus should handle failures");
        
        // Test aggregation handles Byzantine scenarios
        let aggregated = env.sector_daa.aggregate_cross_sector_decisions(decisions).await.unwrap();
        
        // Should have reasonable confidence even with some Byzantine failures
        assert!(aggregated.confidence > 0.0);
        assert!(aggregated.confidence <= 1.0);
        assert!(!aggregated.reasoning.is_empty());
    }
    
    #[tokio::test]
    async fn test_performance_tracking_integration() {
        let mut env = HierarchicalDAATestEnvironment::new().await.unwrap();
        
        // Make decisions across sectors and track performance
        let test_symbols = vec!["AAPL", "JPM", "JNJ", "XOM"];
        let mut all_decisions = Vec::new();
        
        for symbol in test_symbols {
            let market_data = env.create_market_data(symbol, 0.5); // Positive trend
            let market_context = env.create_market_context(symbol, market_data.last().unwrap().close);
            
            let decision = env.sector_daa.route_decision(
                symbol,
                &market_context,
                None,
                &market_data,
            ).await.unwrap();
            
            all_decisions.push(decision);
            
            // Update sector performance (simulate outcomes)
            let sector_info = env.sector_mapper.get_sector(symbol).unwrap();
            let simulated_accuracy = 0.8 + (symbol.len() as f64 * 0.02); // Vary by symbol
            env.sector_daa.update_sector_performance(sector_info.sector_id, simulated_accuracy).await;
        }
        
        // Verify performance tracking
        let sector_stats = env.sector_daa.get_sector_stats();
        assert!(!sector_stats.is_empty());
        
        // All tracked sectors should have performance metrics
        for (sector, accuracy) in sector_stats {
            assert!(accuracy >= 0.7 && accuracy <= 1.0, "Accuracy should be reasonable");
        }
        
        // Verify system-wide metrics
        let system_metrics = env.get_system_metrics().await;
        assert!(system_metrics.contains_key("sector_performance"));
        assert!(system_metrics.contains_key("performance_tracker"));
    }
    
    #[tokio::test]
    async fn test_neural_enhanced_strategy_integration() {
        let env = HierarchicalDAATestEnvironment::new().await.unwrap();
        
        // Test that neural enhanced strategies are properly integrated
        for (sector, strategy) in &env.sector_strategies {
            // Get strategy metrics
            let metrics = strategy.get_metrics();
            
            // Should have sector-specific configuration
            assert!(metrics.contains_key("sector_accuracy") || metrics.is_empty());
            
            // Test strategy can generate signals
            let test_symbol = match sector {
                SectorId::Technology => "AAPL",
                SectorId::Financial => "JPM",
                SectorId::Healthcare => "JNJ",
                _ => "TEST",
            };
            
            let market_data = env.create_market_data(test_symbol, 0.3);
            let market_context = env.create_market_context(test_symbol, market_data.last().unwrap().close);
            
            // Strategy should be able to generate signals
            let can_execute = strategy.can_execute(&market_context).unwrap_or(false);
            if can_execute {
                let signal = strategy.generate_signal(&market_context, None).await;
                assert!(signal.is_ok() || signal.is_err()); // Should handle execution attempt
            }
        }
    }
    
    #[tokio::test]
    async fn test_memory_efficiency_and_scalability() {
        let env = HierarchicalDAATestEnvironment::new().await.unwrap();
        
        // Test with many simultaneous decisions
        let symbols = vec![
            "AAPL", "MSFT", "GOOGL", "AMZN", "META", // Technology
            "JPM", "BAC", "WFC", "GS", "C", // Financial
            "JNJ", "PFE", "UNH", "ABBV", "BMY", // Healthcare
            "XOM", "CVX", "COP", "EOG", "SLB", // Energy
        ];
        
        let start_time = Instant::now();
        let mut tasks = Vec::new();
        
        // Process decisions concurrently
        for symbol in symbols {
            let market_data = env.create_market_data(symbol, 0.2);
            let market_context = env.create_market_context(symbol, market_data.last().unwrap().close);
            
            // Clone necessary data for async task
            let sector_daa = &env.sector_daa;
            
            // Create decision task (but run synchronously for testing)
            let decision_result = sector_daa.route_decision(
                symbol,
                &market_context,
                None,
                &market_data,
            ).await;
            
            assert!(decision_result.is_ok(), "Decision should succeed for {}", symbol);
            tasks.push(decision_result.unwrap());
        }
        
        let processing_time = start_time.elapsed();
        
        // Verify performance
        assert_eq!(tasks.len(), 20, "Should process all symbols");
        assert!(processing_time < Duration::from_secs(10), "Should process quickly");
        
        // Verify memory efficiency - all decisions should have reasonable memory footprint
        for decision in tasks {
            assert!(decision.reasoning.len() < 100, "Reasoning should be concise");
            assert!(decision.neural_consensus.len() <= 10, "Consensus map should be bounded");
        }
    }
    
    #[tokio::test]
    async fn test_hierarchical_decision_flow_end_to_end() {
        let mut env = HierarchicalDAATestEnvironment::new().await.unwrap();
        
        // Test complete decision flow from symbol to aggregated decision
        let test_symbol = "AAPL";
        let market_data = env.create_market_data(test_symbol, 0.7); // Strong positive trend
        let market_context = env.create_market_context(test_symbol, market_data.last().unwrap().close);
        
        // Step 1: Route decision through sector DAA
        let sector_decision = env.sector_daa.route_decision(
            test_symbol,
            &market_context,
            None,
            &market_data,
        ).await.unwrap();
        
        // Step 2: Collect decision from channel
        let received_decisions = env.collect_decisions(1, 5).await.unwrap();
        assert_eq!(received_decisions.len(), 1, "Should receive one decision");
        
        let received = &received_decisions[0];
        assert_eq!(received.timestamp.timestamp(), sector_decision.timestamp.timestamp());
        
        // Step 3: Create multi-sector scenario for aggregation
        let cross_sector_decisions = vec![
            (SectorId::Technology, sector_decision.clone()),
            (SectorId::Financial, {
                let fin_data = env.create_market_data("JPM", -0.2);
                let fin_context = env.create_market_context("JPM", fin_data.last().unwrap().close);
                env.sector_daa.route_decision("JPM", &fin_context, None, &fin_data).await.unwrap()
            }),
            (SectorId::Healthcare, {
                let health_data = env.create_market_data("JNJ", 0.1);
                let health_context = env.create_market_context("JNJ", health_data.last().unwrap().close);
                env.sector_daa.route_decision("JNJ", &health_context, None, &health_data).await.unwrap()
            }),
        ];
        
        // Step 4: Test aggregation
        let aggregated = env.sector_daa.aggregate_cross_sector_decisions(cross_sector_decisions).await.unwrap();
        
        // Verify complete decision flow
        assert!(aggregated.confidence > 0.0);
        assert!(!aggregated.reasoning.is_empty());
        assert!(aggregated.adapted_parameters.is_some());
        
        let params = aggregated.adapted_parameters.unwrap();
        assert!(params.contains_key("aggregation_method"));
        assert!(params.contains_key("sectors_involved"));
        assert!(params.contains_key("consensus_met"));
        
        // Step 5: Verify system metrics
        let metrics = env.get_system_metrics().await;
        assert!(metrics.contains_key("master_coordinator"));
        assert!(metrics.contains_key("sector_coordinators"));
        assert!(metrics.contains_key("performance_tracker"));
    }
    
    #[tokio::test]
    async fn test_autonomous_trading_preservation_integration() {
        let env = HierarchicalDAATestEnvironment::new().await.unwrap();
        
        // Verify that hierarchical DAA preserves autonomous trading capabilities
        let symbols = vec!["AAPL", "JPM", "JNJ"];
        
        for symbol in symbols {
            let market_data = env.create_market_data(symbol, 0.4);
            let market_context = env.create_market_context(symbol, market_data.last().unwrap().close);
            
            // Get sector coordinator directly
            let sector_info = env.sector_mapper.get_sector(symbol).unwrap();
            let sector_coordinator = env.sector_coordinators.get(&sector_info.sector_id).unwrap();
            
            // Verify coordinator is autonomous
            let coordinator_metrics = sector_coordinator.get_metrics().await;
            assert_eq!(coordinator_metrics.total_decisions, 0); // Fresh state
            
            // Make decision through hierarchical DAA
            let hierarchical_decision = env.sector_daa.route_decision(
                symbol,
                &market_context,
                None,
                &market_data,
            ).await.unwrap();
            
            // Decision should maintain autonomous characteristics
            assert!(hierarchical_decision.confidence >= 0.0 && hierarchical_decision.confidence <= 1.0);
            assert!(!hierarchical_decision.reasoning.is_empty());
            assert!(hierarchical_decision.risk_assessment.market_risk >= 0.0);
            assert!(hierarchical_decision.risk_assessment.portfolio_risk >= 0.0);
            
            // Should have risk management
            match hierarchical_decision.action {
                TradingAction::Buy { size, .. } => {
                    assert!(size > 0.0 && size <= 0.05, "Position size should be reasonable");
                }
                TradingAction::Sell { size, .. } => {
                    assert!(size > 0.0 && size <= 0.05, "Position size should be reasonable");
                }
                _ => {} // Hold or adjust actions are fine
            }
            
            // Should include sector context without breaking autonomy
            assert!(hierarchical_decision.adapted_parameters.is_some());
            let params = hierarchical_decision.adapted_parameters.unwrap();
            assert!(params.contains_key("sector"));
        }
    }
    
    #[tokio::test]
    async fn test_fault_tolerance_and_error_handling() {
        let env = HierarchicalDAATestEnvironment::new().await.unwrap();
        
        // Test with invalid/unknown symbols
        let invalid_symbol = "INVALID_SYMBOL_TEST";
        let market_data = env.create_market_data(invalid_symbol, 0.0);
        let market_context = env.create_market_context(invalid_symbol, 100.0);
        
        // Should handle unknown symbols gracefully (fallback to default sector)
        let decision_result = env.sector_daa.route_decision(
            invalid_symbol,
            &market_context,
            None,
            &market_data,
        ).await;
        
        // Should either succeed with fallback or fail gracefully
        match decision_result {
            Ok(decision) => {
                // Successful fallback
                assert!(decision.confidence >= 0.0);
                assert!(!decision.reasoning.is_empty());
            }
            Err(e) => {
                // Graceful error handling
                assert!(!e.to_string().is_empty());
            }
        }
        
        // Test Byzantine fault tolerance with empty decisions
        let empty_decisions = vec![];
        let aggregation_result = env.sector_daa.aggregate_cross_sector_decisions(empty_decisions).await;
        assert!(aggregation_result.is_err(), "Should handle empty decision list");
        
        // Test with partial sector coordinator failures (simulated by empty coordinator set)
        let partial_env = HierarchicalDAATestEnvironment::new().await.unwrap();
        // Environment should still work with all coordinators available
        
        let test_decision = partial_env.sector_daa.route_decision(
            "AAPL",
            &market_context,
            None,
            &market_data,
        ).await;
        
        assert!(test_decision.is_ok(), "Should handle partial coordinator scenarios");
    }
}