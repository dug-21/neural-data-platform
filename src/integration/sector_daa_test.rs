//! Tests for SectorDAACoordinator extension
//!
//! This test verifies that the SectorDAACoordinator properly extends
//! the existing DaaCoordinator without breaking existing functionality.

#[cfg(test)]
mod tests {
    use super::super::daa_coordinator::*;
    use crate::data::sector_mapper::{SectorId, SectorMapper, SectorMapperConfig};
    use crate::config::NeuralConfig;
    use crate::neural::NeuralPredictor;
    use crate::strategies::{MarketContext, Position, PositionSide};
    use crate::data::TimeSeriesData;
    use crate::utils::market_hours::MarketHours;
    use std::sync::Arc;
    use std::collections::HashMap;
    use tokio::sync::mpsc;
    use chrono::Utc;

    /// Helper function to create test market context
    fn create_test_market_context() -> MarketContext {
        MarketContext {
            symbol: "AAPL".to_string(),
            current_price: 150.0,
            bid: 149.95,
            ask: 150.05,
            volume_24h: 1000000.0,
            volatility: 0.02,
            timestamp: Utc::now().timestamp(),
        }
    }

    /// Helper function to create test time series data
    fn create_test_time_series_data() -> Vec<TimeSeriesData> {
        vec![
            TimeSeriesData {
                symbol: "AAPL".to_string(),
                timestamp: Utc::now(),
                open: 149.0,
                high: 151.0,
                low: 148.5,
                close: 150.0,
                volume: vec![100.0],
                indicators: HashMap::new(),
                source: Some("test".to_string()),
                entity: Some("AAPL".to_string()),
                value: Some(150.0),
                metadata: None,
            },
        ]
    }

    #[tokio::test]
    async fn test_sector_daa_coordinator_creation() {
        // Create neural predictor
        let neural_config = NeuralConfig {
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
        };
        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).await.unwrap());

        // Create base DAA coordinator
        let (tx, _rx) = mpsc::channel(100);
        let base_config = DaaConfig::default();
        let market_hours = Arc::new(MarketHours::default());
        let base_coordinator = Arc::new(
            DaaCoordinator::new(base_config, neural_predictor, tx, market_hours).unwrap()
        );

        // Create sector mapper
        let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));

        // Create sector DAA coordinator
        let sector_config = SectorDAAConfig::default();
        let sector_coordinator = SectorDAACoordinator::new(
            SectorId::Technology,
            base_coordinator,
            sector_mapper,
            sector_config,
        ).unwrap();

        // Verify properties
        assert_eq!(sector_coordinator.get_sector_id(), SectorId::Technology);
        
        // Test getting sector metrics
        let metrics = sector_coordinator.get_sector_metrics().await;
        assert_eq!(metrics.avg_sector_signal, 0.0); // Initial value
        assert_eq!(metrics.sector_timing_accuracy, 0.0); // Initial value
    }

    #[tokio::test]
    async fn test_sector_aware_decision_making() {
        // Setup components
        let neural_config = NeuralConfig {
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
        };
        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).await.unwrap());

        let (tx, _rx) = mpsc::channel(100);
        let base_config = DaaConfig::default();
        let market_hours = Arc::new(MarketHours::default());
        let base_coordinator = Arc::new(
            DaaCoordinator::new(base_config, neural_predictor, tx, market_hours).unwrap()
        );

        let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
        let sector_config = SectorDAAConfig::default();
        let sector_coordinator = SectorDAACoordinator::new(
            SectorId::Technology,
            base_coordinator,
            sector_mapper,
            sector_config,
        ).unwrap();

        // Test sector-aware decision making
        let market_context = create_test_market_context();
        let historical_data = create_test_time_series_data();
        
        let sector_decision = sector_coordinator.make_sector_decision(
            &market_context,
            None,
            &historical_data,
            None,
        ).await.unwrap();

        // Verify decision structure
        assert_eq!(sector_decision.sector_context.sector_id, SectorId::Technology);
        assert!(sector_decision.base_decision.confidence >= 0.0);
        assert!(sector_decision.base_decision.confidence <= 1.0);
        assert!(!sector_decision.base_decision.reasoning.is_empty());

        // Verify sector-specific reasoning was added
        let has_sector_reasoning = sector_decision.base_decision.reasoning.iter()
            .any(|r| r.contains("Sector Technology adjustment"));
        assert!(has_sector_reasoning);
    }

    #[tokio::test]
    async fn test_sector_metrics_calculation() {
        // Setup sector coordinator
        let neural_config = NeuralConfig {
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
        };
        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).await.unwrap());

        let (tx, _rx) = mpsc::channel(100);
        let base_config = DaaConfig::default();
        let market_hours = Arc::new(MarketHours::default());
        let base_coordinator = Arc::new(
            DaaCoordinator::new(base_config, neural_predictor, tx, market_hours).unwrap()
        );

        let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
        let sector_config = SectorDAAConfig::default();
        let sector_coordinator = SectorDAACoordinator::new(
            SectorId::Technology,
            base_coordinator,
            sector_mapper,
            sector_config,
        ).unwrap();

        // Test sector metrics calculation
        let market_context = create_test_market_context();
        let sector_metrics = sector_coordinator.calculate_sector_metrics(&market_context).await.unwrap();

        // Verify metrics are reasonable
        assert!(sector_metrics.symbol_count > 0);
        assert!(sector_metrics.volatility > 0.0);
        assert!(sector_metrics.avg_performance.abs() < 1.0); // Reasonable range
        assert!(sector_metrics.momentum.abs() < 1.0); // Reasonable range
        assert!(sector_metrics.relative_strength.abs() < 1.0); // Reasonable range
    }

    #[tokio::test]
    async fn test_cross_sector_correlations() {
        // Setup
        let neural_config = NeuralConfig {
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
        };
        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).await.unwrap());

        let (tx, _rx) = mpsc::channel(100);
        let base_config = DaaConfig::default();
        let market_hours = Arc::new(MarketHours::default());
        let base_coordinator = Arc::new(
            DaaCoordinator::new(base_config, neural_predictor, tx, market_hours).unwrap()
        );

        let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
        let sector_config = SectorDAAConfig::default();
        let sector_coordinator = SectorDAACoordinator::new(
            SectorId::Technology,
            base_coordinator,
            sector_mapper,
            sector_config,
        ).unwrap();

        // Test cross-sector correlations
        let correlations = sector_coordinator.calculate_cross_sector_correlations().await;

        // Should have correlations with other sectors
        assert!(!correlations.is_empty());
        assert_eq!(correlations.len(), 9); // 10 total sectors - 1 (itself)

        // Should not include self-correlation
        assert!(!correlations.contains_key(&SectorId::Technology));

        // Verify correlation values are reasonable
        for (_sector, correlation) in correlations {
            assert!(correlation >= 0.0 && correlation <= 1.0);
        }
    }

    #[tokio::test]
    async fn test_sector_coordinator_interface_compatibility() {
        // Verify that SectorDAACoordinator maintains interface compatibility
        let neural_config = NeuralConfig {
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
        };
        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).await.unwrap());

        let (tx, _rx) = mpsc::channel(100);
        let base_config = DaaConfig::default();
        let market_hours = Arc::new(MarketHours::default());
        let base_coordinator = Arc::new(
            DaaCoordinator::new(base_config, neural_predictor, tx, market_hours).unwrap()
        );

        let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
        let sector_config = SectorDAAConfig::default();
        let sector_coordinator = SectorDAACoordinator::new(
            SectorId::Technology,
            base_coordinator.clone(),
            sector_mapper,
            sector_config,
        ).unwrap();

        // Test that we can access the base coordinator
        let base_ref = sector_coordinator.get_base_coordinator();
        assert!(Arc::ptr_eq(base_ref, &base_coordinator));

        // Test sector retraining interface
        let retraining_result = sector_coordinator.force_sector_retraining().await;
        assert!(retraining_result.is_ok());

        // Test sector history interface
        let history = sector_coordinator.get_sector_decision_history().await;
        assert!(history.is_empty()); // Should be empty initially
    }
}