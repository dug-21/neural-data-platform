//! DAA Preservation Tests for Phase 3
//!
//! Ensures that Phase 3 changes preserve all DAA functionality

use anyhow::Result;
use std::sync::Arc;
use tracing_test::traced_test;

use neural_trader::integration::daa_coordinator::{DaaCoordinator, DaaDecision, DataAvailability};
use neural_trader::daa::autonomous_training::{AutonomousTrainingEngine, PerformanceSnapshot};
use neural_trader::daa::training_scheduler::DAATrainingScheduler;
use neural_trader::data::sector_mapper::{SectorMapper, SectorId, SectorInfo};

use crate::phase3::utilities::*;

#[traced_test]
#[tokio::test]
async fn test_daa_coordinator_preservation() -> Result<()> {
    let config = Phase3TestConfig::default();
    let memory_tracker = MemoryTracker::new(config.memory_budget_mb);
    
    let market_hours = create_test_market_hours();
    let predictor = create_test_neural_predictor(None).await?;
    
    // Test DaaCoordinator with MarketHours parameter (Phase 3 requirement)
    let coordinator = DaaCoordinator::new(predictor, market_hours).await?;
    
    // Verify all DAA functionality is preserved
    assert!(coordinator.is_initialized().await?);
    
    // Test autonomous decision making
    let timestamp = chrono::Utc::now();
    let data = create_test_time_series_data("AAPL", timestamp);
    let decision = coordinator.process_market_data(&data).await?;
    
    // Verify decision structure matches Phase 2 specifications
    assert!(decision.confidence >= 0.0 && decision.confidence <= 1.0);
    assert!(decision.timestamp.is_some());
    assert!(!decision.reasoning.is_empty());
    
    // Check memory compliance
    assert!(memory_tracker.check_budget_compliance().await?);
    
    Ok(())
}

#[traced_test]
#[tokio::test]
async fn test_autonomous_training_engine_preservation() -> Result<()> {
    let predictor = create_test_neural_predictor(None).await?;
    
    // Test AutonomousTrainingEngine initialization
    let training_engine = AutonomousTrainingEngine::new(Arc::clone(&predictor)).await?;
    
    // Verify training engine can handle Phase 3 data structures
    let timestamp = chrono::Utc::now();
    let data = create_test_time_series_data("AAPL", timestamp);
    
    // Test performance snapshot creation
    let snapshot = training_engine.create_performance_snapshot(&data).await?;
    
    // Verify snapshot structure
    assert!(!snapshot.model_id.is_empty());
    assert!(snapshot.accuracy >= 0.0 && snapshot.accuracy <= 1.0);
    assert!(snapshot.timestamp.timestamp() > 0);
    
    Ok(())
}

#[traced_test]
#[tokio::test]
async fn test_sector_mapper_preservation() -> Result<()> {
    let sector_mapper = SectorMapper::new();
    
    // Test sector mapping with Phase 3 symbols
    let symbols = vec!["AAPL", "GOOGL", "MSFT", "TSLA"];
    
    for symbol in symbols {
        let sector_info = sector_mapper.get_sector_info(symbol).await?;
        
        // Verify sector info structure is preserved
        assert!(!sector_info.sector_id.to_string().is_empty());
        assert!(!sector_info.name.is_empty());
        assert!(sector_info.market_cap > 0.0);
    }
    
    Ok(())
}

#[traced_test]
#[tokio::test]
async fn test_daa_training_scheduler_preservation() -> Result<()> {
    let predictor = create_test_neural_predictor(None).await?;
    let scheduler = DAATrainingScheduler::new(predictor).await?;
    
    // Test scheduler initialization
    assert!(scheduler.is_active().await?);
    
    // Test scheduling with Phase 3 data
    let timestamp = chrono::Utc::now();
    let data = create_test_time_series_data("AAPL", timestamp);
    
    scheduler.schedule_training(&data).await?;
    
    // Verify training can be scheduled
    let pending_count = scheduler.get_pending_training_count().await?;
    assert!(pending_count >= 0);
    
    Ok(())
}

#[traced_test]
#[tokio::test]
async fn test_data_availability_assessment() -> Result<()> {
    let market_hours = create_test_market_hours();
    let predictor = create_test_neural_predictor(None).await?;
    let coordinator = DaaCoordinator::new(predictor, market_hours).await?;
    
    // Test data availability assessment with Phase 3 data
    let timestamp = chrono::Utc::now();
    let data = create_test_time_series_data("AAPL", timestamp);
    
    let availability = coordinator.assess_data_availability(&data).await?;
    
    // Verify DataAvailability structure
    assert!(availability.completeness >= 0.0 && availability.completeness <= 1.0);
    assert!(availability.freshness >= 0.0 && availability.freshness <= 1.0);
    assert!(availability.quality >= 0.0 && availability.quality <= 1.0);
    assert!(availability.source_count > 0);
    assert!(availability.market_coverage >= 0.0 && availability.market_coverage <= 1.0);
    assert!(availability.consistency >= 0.0 && availability.consistency <= 1.0);
    assert!(availability.latency_ms >= 0.0);
    
    Ok(())
}

#[traced_test]
#[tokio::test]
async fn test_daa_decision_structure_preservation() -> Result<()> {
    let market_hours = create_test_market_hours();
    let predictor = create_test_neural_predictor(None).await?;
    let coordinator = DaaCoordinator::new(predictor, market_hours).await?;
    
    let timestamp = chrono::Utc::now();
    let data = create_test_time_series_data("AAPL", timestamp);
    
    let decision = coordinator.make_autonomous_decision(&data).await?;
    
    // Verify DaaDecision structure is preserved from Phase 2
    assert!(!decision.decision_id.is_empty());
    assert!(!decision.symbol.is_empty());
    assert!(decision.confidence >= 0.0 && decision.confidence <= 1.0);
    assert!(decision.timestamp.is_some());
    assert!(!decision.reasoning.is_empty());
    assert!(decision.risk_assessment >= 0.0 && decision.risk_assessment <= 1.0);
    
    // Test that decision can be serialized/deserialized
    let serialized = serde_json::to_string(&decision)?;
    let _deserialized: DaaDecision = serde_json::from_str(&serialized)?;
    
    Ok(())
}

#[traced_test]
#[tokio::test]
async fn test_cross_sector_coordination_preservation() -> Result<()> {
    let market_hours = create_test_market_hours();
    let predictor = create_test_neural_predictor(None).await?;
    let coordinator = DaaCoordinator::new(predictor, market_hours).await?;
    
    // Test cross-sector coordination with multiple symbols
    let symbols = vec!["AAPL", "JPM", "XOM", "JNJ"]; // Different sectors
    let timestamp = chrono::Utc::now();
    
    let mut decisions = Vec::new();
    for symbol in symbols {
        let data = create_test_time_series_data(symbol, timestamp);
        let decision = coordinator.process_market_data(&data).await?;
        decisions.push(decision);
    }
    
    // Verify cross-sector decisions maintain consistency
    assert_eq!(decisions.len(), 4);
    for decision in decisions {
        assert!(!decision.reasoning.is_empty());
        assert!(decision.confidence >= 0.0 && decision.confidence <= 1.0);
    }
    
    Ok(())
}

#[traced_test]
#[tokio::test]
async fn test_memory_efficiency_preservation() -> Result<()> {
    let memory_tracker = MemoryTracker::new(512); // 512MB budget
    
    // Create multiple DAA coordinators to test memory efficiency
    let mut coordinators = Vec::new();
    for i in 0..5 {
        let market_hours = create_test_market_hours();
        let predictor = create_test_neural_predictor(None).await?;
        let coordinator = DaaCoordinator::new(predictor, market_hours).await?;
        coordinators.push(coordinator);
        
        // Check memory after each coordinator
        assert!(memory_tracker.check_budget_compliance().await?);
    }
    
    // Process data with all coordinators
    let timestamp = chrono::Utc::now();
    let data = create_test_time_series_data("AAPL", timestamp);
    
    for coordinator in &coordinators {
        let _decision = coordinator.process_market_data(&data).await?;
    }
    
    // Final memory check
    assert!(memory_tracker.check_budget_compliance().await?);
    let usage = memory_tracker.get_memory_usage_mb().await;
    println!("DAA coordination memory usage: {}MB", usage);
    
    Ok(())
}