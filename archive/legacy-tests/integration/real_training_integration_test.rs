//! Real Training System Integration Tests
//! 
//! Comprehensive tests validating the complete training pipeline from
//! TimescaleDB data ingestion through model training to persistent storage.

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{Duration, Utc};
use uuid::Uuid;

use autonomous_platform::{
    realtraining::{
        TrainingPipeline, TrainingConfig, ModelStorage,
        DataSelector, SelectionStrategy, FeatureEngine,
        MarketHoursMonitor, TrainingScheduler, ScheduleStrategy,
        EmergencyOverrideSystem, ModelType, Priority,
    },
    storage::{TimescaleDBStorage, RedisCache},
    common::test_utils::*,
};

/// Test helper to create a complete training system
async fn setup_training_system() -> Result<TestTrainingSystem> {
    // Setup test database
    let db_config = test_db_config();
    let db = TimescaleDBStorage::new(db_config).await?;
    
    // Setup Redis cache
    let cache_config = test_redis_config();
    let cache = RedisCache::new(cache_config).await?;
    
    // Create training pipeline
    let pipeline = TrainingPipeline::builder()
        .with_storage(Arc::new(db))
        .with_cache(Arc::new(cache))
        .with_model_storage(Arc::new(ModelStorage::new("/tmp/test_models")))
        .build()?;
    
    Ok(TestTrainingSystem {
        pipeline: Arc::new(pipeline),
        monitor: Arc::new(MarketHoursMonitor::new()),
        scheduler: Arc::new(RwLock::new(TrainingScheduler::new(
            ScheduleStrategy::PostMarketClose { delay_minutes: 30 }
        ))),
    })
}

struct TestTrainingSystem {
    pipeline: Arc<TrainingPipeline>,
    monitor: Arc<MarketHoursMonitor>,
    scheduler: Arc<RwLock<TrainingScheduler>>,
}

#[tokio::test]
async fn test_timescale_to_training_data_flow() -> Result<()> {
    let system = setup_training_system().await?;
    
    // Insert test market data
    insert_test_market_data(&system.pipeline.storage()).await?;
    
    // Create data selector
    let selector = DataSelector::new(system.pipeline.data_access());
    
    // Test recent data selection
    let data = selector.select_data(
        SelectionStrategy::RecencyBased { days: 7 }
    ).await?;
    
    assert!(!data.is_empty(), "Should retrieve market data");
    assert!(data.len() > 1000, "Should have sufficient data points");
    
    // Verify data quality
    for record in data.iter().take(10) {
        assert!(record.price > 0.0, "Price should be positive");
        assert!(record.volume > 0, "Volume should be positive");
        assert!(record.timestamp < Utc::now(), "Timestamp should be in past");
    }
    
    Ok(())
}

#[tokio::test]
async fn test_real_model_training_improvement() -> Result<()> {
    let system = setup_training_system().await?;
    
    // Load real market data
    let data = load_production_sample_data().await?;
    
    // Configure training
    let config = TrainingConfig {
        model_type: ModelType::MLP,
        epochs: 20,
        batch_size: 64,
        learning_rate: 0.001,
        early_stopping: true,
        validation_split: 0.2,
    };
    
    // Execute training
    let result = system.pipeline.execute_training(config, data).await?;
    
    // Validate real improvement
    assert!(
        result.validation.final_loss < result.validation.initial_loss * 0.9,
        "Model should improve by at least 10%"
    );
    
    assert!(
        result.validation.accuracy > 0.6,
        "Model accuracy should exceed 60%"
    );
    
    // Test model predictions
    let test_features = create_test_feature_vector();
    let prediction = result.model.predict(&test_features).await?;
    
    assert!(prediction.confidence > 0.0 && prediction.confidence <= 1.0);
    assert!(prediction.value.is_finite());
    
    Ok(())
}

#[tokio::test]
async fn test_market_hours_scheduling_compliance() -> Result<()> {
    let system = setup_training_system().await?;
    
    // Test scheduling during market hours
    let market_time = next_market_open_time();
    tokio::time::sleep_until(market_time.into()).await;
    
    let job = TrainingJob {
        id: Uuid::new_v4(),
        model_type: ModelType::LSTM,
        priority: Priority::Normal,
        constraints: JobConstraints {
            market_hours_only: false,
            max_duration: Duration::hours(2),
        },
    };
    
    let scheduled = system.scheduler.write().await
        .schedule_training(job.clone()).await?;
    
    // Verify scheduled after market close
    let next_close = system.monitor.next_close_time("NYSE")?;
    assert!(
        scheduled.execution_time > next_close,
        "Training should be scheduled after market close"
    );
    
    // Test emergency override
    let emergency_job = TrainingJob {
        id: Uuid::new_v4(),
        model_type: ModelType::MLP,
        priority: Priority::Emergency,
        constraints: JobConstraints::default(),
    };
    
    let emergency_scheduled = system.scheduler.write().await
        .schedule_training(emergency_job).await?;
    
    assert!(
        emergency_scheduled.execution_time <= Utc::now() + Duration::minutes(1),
        "Emergency jobs should execute immediately"
    );
    
    Ok(())
}

#[tokio::test]
async fn test_model_persistence_and_recovery() -> Result<()> {
    let system = setup_training_system().await?;
    
    // Train a model
    let training_result = train_test_model(&system).await?;
    let model_id = training_result.model.id.clone();
    
    // Save model
    let storage_result = system.pipeline.model_storage()
        .save_model(&training_result.model, training_result.metadata)
        .await?;
    
    assert!(storage_result.path.exists(), "Model file should exist");
    assert!(storage_result.size > 0, "Model file should have content");
    
    // Simulate system restart
    drop(system);
    let new_system = setup_training_system().await?;
    
    // Load model
    let loaded_model = new_system.pipeline.model_storage()
        .load_model(&model_id, &storage_result.version)
        .await?;
    
    // Verify model integrity
    let test_input = create_test_feature_vector();
    let original_pred = training_result.model.predict(&test_input).await?;
    let loaded_pred = loaded_model.predict(&test_input).await?;
    
    assert!(
        (original_pred.value - loaded_pred.value).abs() < 1e-6,
        "Loaded model should produce same predictions"
    );
    
    Ok(())
}

#[tokio::test]
async fn test_emergency_override_activation() -> Result<()> {
    let system = setup_training_system().await?;
    
    // Setup emergency override system
    let override_system = EmergencyOverrideSystem::builder()
        .add_trigger(Box::new(VolatilitySpikeTrigger {
            threshold: 0.05, // 5% volatility
            window: Duration::minutes(15),
        }))
        .add_trigger(Box::new(ModelDivergenceTrigger {
            divergence_threshold: 0.1,
            model_pairs: vec![("mlp".to_string(), "lstm".to_string())],
        }))
        .build();
    
    // Simulate high volatility market data
    insert_volatile_market_data(&system.pipeline.storage()).await?;
    
    // Check if emergency triggered
    let context = get_current_market_context().await?;
    let triggers = override_system.check_triggers(&context).await?;
    
    assert!(
        triggers.iter().any(|t| matches!(t, TriggerType::VolatilitySpike)),
        "High volatility should trigger emergency training"
    );
    
    // Verify emergency job created
    let emergency_jobs = system.scheduler.read().await
        .get_jobs_by_priority(Priority::Emergency)
        .await?;
    
    assert!(!emergency_jobs.is_empty(), "Emergency jobs should be created");
    
    Ok(())
}

#[tokio::test]
async fn test_concurrent_training_isolation() -> Result<()> {
    let system = setup_training_system().await?;
    
    // Submit multiple training jobs concurrently
    let mut handles = vec![];
    
    for i in 0..3 {
        let pipeline = system.pipeline.clone();
        let handle = tokio::spawn(async move {
            let config = TrainingConfig {
                model_type: match i % 3 {
                    0 => ModelType::MLP,
                    1 => ModelType::LSTM,
                    _ => ModelType::Ensemble,
                },
                epochs: 10,
                batch_size: 32,
                learning_rate: 0.001,
                early_stopping: true,
                validation_split: 0.2,
            };
            
            pipeline.execute_training(config, load_test_data()).await
        });
        handles.push(handle);
    }
    
    // Wait for all training jobs
    let results: Vec<_> = futures::future::join_all(handles).await;
    
    // Verify all succeeded independently
    for (i, result) in results.iter().enumerate() {
        assert!(
            result.is_ok(),
            "Training job {} should complete successfully", i
        );
        
        let training_result = result.as_ref().unwrap().as_ref().unwrap();
        assert!(
            training_result.validation.final_loss < training_result.validation.initial_loss,
            "Model {} should improve", i
        );
    }
    
    Ok(())
}

#[tokio::test]
async fn test_data_quality_validation() -> Result<()> {
    let system = setup_training_system().await?;
    
    // Insert data with quality issues
    insert_corrupted_market_data(&system.pipeline.storage()).await?;
    
    // Attempt to select data
    let selector = DataSelector::new(system.pipeline.data_access());
    let result = selector.select_data(
        SelectionStrategy::RecencyBased { days: 1 }
    ).await;
    
    // Should handle corrupted data gracefully
    assert!(result.is_ok(), "Should handle corrupted data");
    
    let data = result.unwrap();
    // Verify corrupted data was filtered
    for record in &data {
        assert!(record.price > 0.0, "Invalid prices should be filtered");
        assert!(!record.price.is_nan(), "NaN values should be filtered");
        assert!(record.volume >= 0, "Negative volumes should be filtered");
    }
    
    Ok(())
}

#[tokio::test]
async fn test_performance_regression_detection() -> Result<()> {
    let system = setup_training_system().await?;
    
    // Train baseline model
    let baseline = train_baseline_model(&system).await?;
    let baseline_metrics = evaluate_model_performance(&baseline).await?;
    
    // Train new model with potentially degraded config
    let degraded_config = TrainingConfig {
        model_type: ModelType::MLP,
        epochs: 5, // Too few epochs
        batch_size: 256, // Too large batch
        learning_rate: 0.1, // Too high learning rate
        early_stopping: false,
        validation_split: 0.1, // Too small validation
    };
    
    let new_result = system.pipeline.execute_training(
        degraded_config,
        load_test_data()
    ).await?;
    
    let new_metrics = evaluate_model_performance(&new_result).await?;
    
    // Check for performance regression
    let regression = detect_performance_regression(&baseline_metrics, &new_metrics);
    
    assert!(
        regression.is_some(),
        "Should detect performance regression"
    );
    
    if let Some(reg) = regression {
        assert!(reg.severity > 0.1, "Regression should be significant");
        assert!(reg.metrics.contains("accuracy"), "Should identify accuracy drop");
    }
    
    Ok(())
}

// Helper functions

async fn insert_test_market_data(storage: &TimescaleDBStorage) -> Result<()> {
    let data = generate_market_data(
        vec!["AAPL", "GOOGL", "MSFT"],
        30, // 30 days
        60, // 1 minute intervals
    );
    
    storage.insert_batch(data).await?;
    Ok(())
}

async fn insert_volatile_market_data(storage: &TimescaleDBStorage) -> Result<()> {
    let mut data = generate_market_data(vec!["SPY"], 1, 60);
    
    // Add volatility spike
    let spike_start = data.len() / 2;
    for i in spike_start..data.len() {
        if let Some(point) = data.get_mut(i) {
            let volatility = ((i - spike_start) as f64 / 10.0).sin() * 0.1;
            point.price *= 1.0 + volatility;
        }
    }
    
    storage.insert_batch(data).await?;
    Ok(())
}

async fn insert_corrupted_market_data(storage: &TimescaleDBStorage) -> Result<()> {
    let mut data = generate_market_data(vec!["TEST"], 1, 60);
    
    // Corrupt some data points
    for (i, point) in data.iter_mut().enumerate() {
        match i % 10 {
            0 => point.price = f64::NAN,
            1 => point.price = -100.0,
            2 => point.volume = -1000,
            3 => point.price = f64::INFINITY,
            _ => {}
        }
    }
    
    storage.insert_batch(data).await?;
    Ok(())
}

async fn train_test_model(system: &TestTrainingSystem) -> Result<TrainingResult> {
    let config = TrainingConfig {
        model_type: ModelType::MLP,
        epochs: 10,
        batch_size: 32,
        learning_rate: 0.001,
        early_stopping: true,
        validation_split: 0.2,
    };
    
    system.pipeline.execute_training(config, load_test_data()).await
}

async fn train_baseline_model(system: &TestTrainingSystem) -> Result<TrainingResult> {
    let config = TrainingConfig {
        model_type: ModelType::MLP,
        epochs: 50,
        batch_size: 32,
        learning_rate: 0.001,
        early_stopping: true,
        validation_split: 0.2,
    };
    
    system.pipeline.execute_training(config, load_test_data()).await
}

async fn evaluate_model_performance(result: &TrainingResult) -> Result<PerformanceMetrics> {
    Ok(PerformanceMetrics {
        accuracy: result.validation.accuracy,
        loss: result.validation.final_loss,
        inference_time: measure_inference_time(&result.model).await?,
        memory_usage: result.model.memory_footprint(),
    })
}

fn detect_performance_regression(
    baseline: &PerformanceMetrics,
    current: &PerformanceMetrics,
) -> Option<RegressionReport> {
    let accuracy_drop = baseline.accuracy - current.accuracy;
    let loss_increase = current.loss - baseline.loss;
    let speed_decrease = current.inference_time - baseline.inference_time;
    
    if accuracy_drop > 0.05 || loss_increase > 0.1 || speed_decrease > 10.0 {
        Some(RegressionReport {
            severity: accuracy_drop.max(loss_increase).max(speed_decrease / 100.0),
            metrics: vec![
                format!("accuracy: -{:.2}%", accuracy_drop * 100.0),
                format!("loss: +{:.3}", loss_increase),
                format!("speed: +{:.1}ms", speed_decrease),
            ],
        })
    } else {
        None
    }
}

#[derive(Debug)]
struct PerformanceMetrics {
    accuracy: f64,
    loss: f64,
    inference_time: f64, // milliseconds
    memory_usage: usize, // bytes
}

#[derive(Debug)]
struct RegressionReport {
    severity: f64,
    metrics: Vec<String>,
}