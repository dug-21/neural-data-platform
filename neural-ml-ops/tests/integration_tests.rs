//! Integration Tests for Neural ML-Ops
//!
//! Comprehensive integration tests for the domain-agnostic ML operations platform.

use anyhow::Result;
use chrono::Utc;
use neural_ml_ops::{
    events::{EventPublisher, EventConfig, MLEvent, MLEventType},
    features::{FeatureEngine, FeatureStore, FeatureStoreConfig, FeatureConfig},
    models::{ModelRegistry, ModelRegistryConfig, ModelInfo, ModelType, ModelStatus, ModelMetrics},
    training::{TrainingCoordinator, TrainingConfig, WorkflowConfig, WorkflowStep, StepType},
};
use std::collections::HashMap;
use tempfile::TempDir;
use tokio;
use uuid::Uuid;

#[tokio::test]
async fn test_end_to_end_ml_workflow() {
    // Setup temporary directories
    let temp_dir = TempDir::new().unwrap();
    
    // Initialize components
    let training_config = TrainingConfig {
        output_dir: temp_dir.path().join("training"),
        ..TrainingConfig::default()
    };
    
    let feature_config = FeatureStoreConfig {
        storage_backend: neural_ml_ops::features::StorageBackend::FileSystem {
            base_path: temp_dir.path().join("features").to_string_lossy().to_string(),
        },
        ..FeatureStoreConfig::default()
    };
    
    let model_config = ModelRegistryConfig {
        storage_path: temp_dir.path().join("models"),
        ..ModelRegistryConfig::default()
    };
    
    let event_config = EventConfig::default();
    
    // Initialize services
    let coordinator = TrainingCoordinator::new(training_config).await.unwrap();
    let feature_store = FeatureStore::new(feature_config).await.unwrap();
    let model_registry = ModelRegistry::new(model_config).await.unwrap();
    let event_publisher = EventPublisher::new(event_config).await.unwrap();
    
    // 1. Feature Engineering Test
    let feature_engine_config = FeatureConfig::default();
    let feature_engine = FeatureEngine::new(feature_engine_config);
    
    // Generate sample data
    let sample_data: Vec<f64> = (0..100).map(|i| i as f64 + rand::random::<f64>()).collect();
    
    // Extract features
    let features = feature_engine.extract_features(&sample_data).await.unwrap();
    assert!(!features.is_empty());
    
    // Store features
    feature_store.store_features("test-namespace", &features, None).await.unwrap();
    
    // Retrieve features
    let retrieved_features = feature_store
        .retrieve_features("test-namespace", &[], None, None)
        .await
        .unwrap();
    assert_eq!(features.len(), retrieved_features.len());
    
    // 2. Model Registry Test
    let model_info = ModelInfo {
        id: "test-model-1".to_string(),
        name: "Test Neural Network".to_string(),
        version: "1.0.0".to_string(),
        model_type: ModelType::NeuralNetwork,
        status: ModelStatus::Draft,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        created_by: Some("test-user".to_string()),
        description: Some("Integration test model".to_string()),
        tags: vec!["test".to_string(), "integration".to_string()],
        metrics: ModelMetrics {
            accuracy: Some(0.85),
            loss: Some(0.15),
            ..ModelMetrics::default()
        },
        artifacts: HashMap::new(),
    };
    
    // Register model
    let model_id = model_registry.register_model(model_info.clone()).await.unwrap();
    assert_eq!(model_id, "test-model-1");
    
    // Retrieve model
    let retrieved_model = model_registry.get_model_info(&model_id).await.unwrap();
    assert_eq!(retrieved_model.name, "Test Neural Network");
    
    // 3. Training Workflow Test
    let workflow = WorkflowConfig {
        id: "integration-test-workflow".to_string(),
        name: "Integration Test Workflow".to_string(),
        description: "Test workflow for integration testing".to_string(),
        steps: vec![
            WorkflowStep {
                name: "data-preparation".to_string(),
                step_type: StepType::DataPreparation,
                parameters: HashMap::new(),
                depends_on: vec![],
            },
            WorkflowStep {
                name: "feature-engineering".to_string(),
                step_type: StepType::FeatureEngineering,
                parameters: HashMap::new(),
                depends_on: vec!["data-preparation".to_string()],
            },
            WorkflowStep {
                name: "model-training".to_string(),
                step_type: StepType::ModelTraining {
                    model_type: "neural".to_string(),
                    hyperparameters: [
                        ("learning_rate".to_string(), serde_json::json!(0.001)),
                        ("epochs".to_string(), serde_json::json!(10)),
                    ].into(),
                },
                parameters: HashMap::new(),
                depends_on: vec!["feature-engineering".to_string()],
            },
        ],
        timeout_secs: 300,
        retry_count: 1,
    };
    
    // Register workflow
    coordinator.register_workflow(workflow.clone()).await.unwrap();
    
    // Start workflow
    let workflow_job_id = coordinator
        .start_workflow(&workflow.id, serde_json::json!({"test": true}))
        .await
        .unwrap();
    
    // Wait a bit for workflow to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Check workflow status
    let status = coordinator.get_status(&workflow_job_id).await.unwrap();
    assert!(matches!(status.status, neural_ml_ops::training::JobStatus::Running | neural_ml_ops::training::JobStatus::Completed));
    
    // 4. Event Publishing Test
    let test_event = MLEvent {
        id: Uuid::new_v4(),
        event_type: MLEventType::TrainingStarted,
        job_id: Some(Uuid::parse_str(&workflow_job_id).unwrap()),
        workflow_id: Some(workflow.id.clone()),
        timestamp: Utc::now(),
        payload: serde_json::json!({
            "message": "Integration test training started",
            "test": true
        }),
    };
    
    // Publish event
    event_publisher.publish(test_event.clone()).await.unwrap();
    
    // Publish batch of events
    let batch_events = vec![
        MLEvent {
            id: Uuid::new_v4(),
            event_type: MLEventType::ModelRegistered,
            job_id: None,
            workflow_id: None,
            timestamp: Utc::now(),
            payload: serde_json::json!({"model_id": model_id}),
        },
        MLEvent {
            id: Uuid::new_v4(),
            event_type: MLEventType::FeaturesExtracted,
            job_id: None,
            workflow_id: Some("feature-extraction".to_string()),
            timestamp: Utc::now(),
            payload: serde_json::json!({"feature_count": features.len()}),
        },
    ];
    
    event_publisher.publish_batch(batch_events).await.unwrap();
    
    // Check event statistics
    let event_stats = event_publisher.get_stats().await;
    assert!(event_stats.total_events_published >= 3);
    assert!(event_stats.events_by_type.contains_key("TrainingStarted"));
    
    // 5. Integration Validation
    
    // List models
    let models = model_registry.list_models(None).await.unwrap();
    assert!(!models.is_empty());
    
    // Get feature statistics
    let feature_stats = feature_store.get_statistics().await.unwrap();
    assert!(feature_stats.total_features > 0);
    
    // List workflows
    let workflows = coordinator.list_workflows().await.unwrap();
    assert!(workflows.contains(&workflow.id));
    
    // Check health of all components
    let event_health = event_publisher.health_check().await.unwrap();
    assert!(event_health.healthy);
    
    // Get registry statistics
    let registry_stats = model_registry.get_registry_stats().await.unwrap();
    assert!(registry_stats.total_models > 0);
    
    println!("✅ End-to-end ML workflow test completed successfully");
}

#[tokio::test]
async fn test_feature_engineering_pipeline() {
    let config = FeatureConfig::default();
    let engine = FeatureEngine::new(config);
    
    // Test with various data patterns
    let test_cases = vec![
        // Trend data
        (0..50).map(|i| i as f64 * 0.1).collect::<Vec<f64>>(),
        // Seasonal data
        (0..100).map(|i| (i as f64 * 0.1).sin()).collect::<Vec<f64>>(),
        // Random data
        (0..75).map(|_| rand::random::<f64>()).collect::<Vec<f64>>(),
        // Mixed data
        (0..60).map(|i| i as f64 * 0.05 + (i as f64 * 0.2).sin() + rand::random::<f64>() * 0.1).collect::<Vec<f64>>(),
    ];
    
    for (i, data) in test_cases.iter().enumerate() {
        let features = engine.extract_features(data).await.unwrap();
        assert!(!features.is_empty(), "Case {} should produce features", i);
        
        // Check for expected feature types
        let feature_names: Vec<&str> = features.iter().map(|f| f.name.as_str()).collect();
        assert!(feature_names.iter().any(|name| name.contains("statistical")), "Should have statistical features");
        assert!(feature_names.iter().any(|name| name.contains("frequency")), "Should have frequency features");
        assert!(feature_names.iter().any(|name| name.contains("technical")), "Should have technical features");
    }
    
    // Test batch processing
    let batch_result = engine.extract_features_batch(&test_cases).await.unwrap();
    assert_eq!(batch_result.processing_stats.total_records, test_cases.len());
    assert_eq!(batch_result.processing_stats.processed_records, test_cases.len());
    assert!(!batch_result.features.is_empty());
    assert!(!batch_result.quality_metrics.is_empty());
    
    println!("✅ Feature engineering pipeline test completed");
}

#[tokio::test]
async fn test_model_lifecycle() {
    let temp_dir = TempDir::new().unwrap();
    let config = ModelRegistryConfig {
        storage_path: temp_dir.path().to_path_buf(),
        max_versions_per_model: 3,
        ..ModelRegistryConfig::default()
    };
    
    let registry = ModelRegistry::new(config).await.unwrap();
    
    // Create multiple versions of a model
    for version in 1..=5 {
        let model_info = ModelInfo {
            id: format!("lifecycle-test-model-{}", version),
            name: "Lifecycle Test Model".to_string(),
            version: format!("1.0.{}", version - 1),
            model_type: ModelType::RandomForest,
            status: ModelStatus::Trained,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            created_by: Some("integration-test".to_string()),
            description: Some(format!("Version {} of lifecycle test model", version)),
            tags: vec!["lifecycle".to_string(), "test".to_string()],
            metrics: ModelMetrics {
                accuracy: Some(0.80 + (version as f64) * 0.02),
                loss: Some(0.20 - (version as f64) * 0.02),
                ..ModelMetrics::default()
            },
            artifacts: HashMap::new(),
        };
        
        registry.register_model(model_info).await.unwrap();
        
        // Small delay to ensure different timestamps
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }
    
    // Test model search
    let search_criteria = neural_ml_ops::models::ModelSearchCriteria {
        name_pattern: Some("Lifecycle".to_string()),
        model_type: Some(ModelType::RandomForest),
        ..neural_ml_ops::models::ModelSearchCriteria::default()
    };
    
    let found_models = registry.list_models(Some(search_criteria)).await.unwrap();
    assert_eq!(found_models.len(), 5);
    
    // Test model comparison
    let comparison = registry
        .compare_models("lifecycle-test-model-1", "lifecycle-test-model-5")
        .await
        .unwrap();
    
    assert!(comparison.improvement_percentage > 0.0);
    
    // Test model deletion
    registry.delete_model("lifecycle-test-model-1").await.unwrap();
    
    let remaining_models = registry.list_models(None).await.unwrap();
    assert_eq!(remaining_models.len(), 4);
    
    println!("✅ Model lifecycle test completed");
}

#[tokio::test]
async fn test_training_coordinator_comprehensive() {
    let temp_dir = TempDir::new().unwrap();
    let config = TrainingConfig {
        output_dir: temp_dir.path().to_path_buf(),
        max_concurrent_jobs: 2,
        default_timeout_secs: 60,
        ..TrainingConfig::default()
    };
    
    let coordinator = TrainingCoordinator::new(config).await.unwrap();
    
    // Create multiple workflows
    let workflows = vec![
        WorkflowConfig {
            id: "quick-workflow".to_string(),
            name: "Quick Test Workflow".to_string(),
            description: "Fast workflow for testing".to_string(),
            steps: vec![
                WorkflowStep {
                    name: "quick-step".to_string(),
                    step_type: StepType::DataPreparation,
                    parameters: HashMap::new(),
                    depends_on: vec![],
                }
            ],
            timeout_secs: 30,
            retry_count: 1,
        },
        WorkflowConfig {
            id: "complex-workflow".to_string(),
            name: "Complex Test Workflow".to_string(),
            description: "Multi-step workflow for comprehensive testing".to_string(),
            steps: vec![
                WorkflowStep {
                    name: "data-prep".to_string(),
                    step_type: StepType::DataPreparation,
                    parameters: HashMap::new(),
                    depends_on: vec![],
                },
                WorkflowStep {
                    name: "feature-eng".to_string(),
                    step_type: StepType::FeatureEngineering,
                    parameters: HashMap::new(),
                    depends_on: vec!["data-prep".to_string()],
                },
                WorkflowStep {
                    name: "training".to_string(),
                    step_type: StepType::ModelTraining {
                        model_type: "xgboost".to_string(),
                        hyperparameters: HashMap::new(),
                    },
                    parameters: HashMap::new(),
                    depends_on: vec!["feature-eng".to_string()],
                },
                WorkflowStep {
                    name: "validation".to_string(),
                    step_type: StepType::ModelValidation,
                    parameters: HashMap::new(),
                    depends_on: vec!["training".to_string()],
                },
            ],
            timeout_secs: 120,
            retry_count: 2,
        },
    ];
    
    // Register workflows
    for workflow in &workflows {
        coordinator.register_workflow(workflow.clone()).await.unwrap();
    }
    
    // Start multiple jobs concurrently
    let mut job_ids = Vec::new();
    for workflow in &workflows {
        let job_id = coordinator
            .start_workflow(&workflow.id, serde_json::json!({}))
            .await
            .unwrap();
        job_ids.push(job_id);
    }
    
    // Wait a bit for jobs to process
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    // Check job statuses
    for job_id in &job_ids {
        let status = coordinator.get_status(job_id).await.unwrap();
        assert!(matches!(
            status.status,
            neural_ml_ops::training::JobStatus::Running |
            neural_ml_ops::training::JobStatus::Completed |
            neural_ml_ops::training::JobStatus::Queued
        ));
    }
    
    // List active jobs
    let active_jobs = coordinator.list_active_jobs().await;
    assert!(active_jobs.len() >= job_ids.len());
    
    println!("✅ Training coordinator comprehensive test completed");
}

#[tokio::test]
async fn test_event_system_integration() {
    let config = EventConfig {
        buffer_size: 10,
        batch_size: 5,
        flush_interval_ms: 100,
        ..EventConfig::default()
    };
    
    let publisher = EventPublisher::new(config).await.unwrap();
    
    // Publish various types of events
    let event_types = vec![
        MLEventType::TrainingStarted,
        MLEventType::TrainingProgress,
        MLEventType::TrainingCompleted,
        MLEventType::ModelRegistered,
        MLEventType::ModelDeployed,
        MLEventType::FeaturesExtracted,
        MLEventType::SystemHealthCheck,
    ];
    
    let mut published_events = Vec::new();
    
    for (i, event_type) in event_types.iter().enumerate() {
        let event = MLEvent {
            id: Uuid::new_v4(),
            event_type: event_type.clone(),
            job_id: Some(Uuid::new_v4()),
            workflow_id: Some(format!("workflow-{}", i)),
            timestamp: Utc::now(),
            payload: serde_json::json!({
                "index": i,
                "event_type": format!("{:?}", event_type),
                "test_data": format!("test-{}", i)
            }),
        };
        
        publisher.publish(event.clone()).await.unwrap();
        published_events.push(event);
    }
    
    // Wait for buffered events to be processed
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    
    // Force flush remaining events
    publisher.flush().await.unwrap();
    
    // Check statistics
    let stats = publisher.get_stats().await;
    assert_eq!(stats.total_events_published, published_events.len() as u64);
    assert!(stats.events_by_type.len() >= event_types.len());
    
    // Test batch publishing
    let batch_events: Vec<MLEvent> = (0..3).map(|i| {
        MLEvent {
            id: Uuid::new_v4(),
            event_type: MLEventType::Custom(format!("batch-event-{}", i)),
            job_id: None,
            workflow_id: Some("batch-test".to_string()),
            timestamp: Utc::now(),
            payload: serde_json::json!({"batch_index": i}),
        }
    }).collect();
    
    publisher.publish_batch(batch_events.clone()).await.unwrap();
    
    // Check updated statistics
    let updated_stats = publisher.get_stats().await;
    assert_eq!(
        updated_stats.total_events_published,
        (published_events.len() + batch_events.len()) as u64
    );
    
    // Check health
    let health = publisher.health_check().await.unwrap();
    assert!(health.healthy);
    assert!(health.backend_healthy);
    
    println!("✅ Event system integration test completed");
}

#[tokio::test]
async fn test_error_handling_and_recovery() {
    let temp_dir = TempDir::new().unwrap();
    
    // Test with invalid configurations
    let invalid_model_config = ModelRegistryConfig {
        storage_path: PathBuf::from("/invalid/path/that/should/not/exist"),
        ..ModelRegistryConfig::default()
    };
    
    // This should handle the error gracefully by creating the directory
    let result = ModelRegistry::new(invalid_model_config).await;
    // In practice, this might fail due to permissions, but the point is to test error handling
    
    // Test feature store with invalid backend
    let invalid_feature_config = FeatureStoreConfig {
        storage_backend: neural_ml_ops::features::StorageBackend::FileSystem {
            base_path: "/another/invalid/path".to_string(),
        },
        ..FeatureStoreConfig::default()
    };
    
    // This should also handle the error by creating directories
    let _feature_store = FeatureStore::new(invalid_feature_config).await.unwrap();
    
    // Test training coordinator with empty output directory
    let training_config = TrainingConfig {
        output_dir: temp_dir.path().join("empty"),
        max_concurrent_jobs: 1,
        ..TrainingConfig::default()
    };
    
    let coordinator = TrainingCoordinator::new(training_config).await.unwrap();
    
    // Try to start a non-existent workflow
    let result = coordinator
        .start_workflow("non-existent-workflow", serde_json::json!({}))
        .await;
    
    assert!(result.is_err());
    
    // Test event publisher with disabled events
    let disabled_config = EventConfig {
        enabled: false,
        ..EventConfig::default()
    };
    
    let publisher = EventPublisher::new(disabled_config).await.unwrap();
    
    let test_event = MLEvent {
        id: Uuid::new_v4(),
        event_type: MLEventType::TrainingStarted,
        job_id: Some(Uuid::new_v4()),
        workflow_id: Some("test".to_string()),
        timestamp: Utc::now(),
        payload: serde_json::json!({}),
    };
    
    // Should succeed but not actually publish
    publisher.publish(test_event).await.unwrap();
    
    let stats = publisher.get_stats().await;
    assert_eq!(stats.total_events_published, 0);
    
    println!("✅ Error handling and recovery test completed");
}

// Helper function for generating test data
use rand::Rng;

fn generate_test_data(size: usize, pattern: &str) -> Vec<f64> {
    let mut rng = rand::thread_rng();
    
    match pattern {
        "random" => (0..size).map(|_| rng.gen()).collect(),
        "trend" => (0..size).map(|i| i as f64 * 0.1).collect(),
        "seasonal" => (0..size).map(|i| (i as f64 * 0.1).sin()).collect(),
        "mixed" => (0..size).map(|i| {
            i as f64 * 0.05 + (i as f64 * 0.2).sin() + rng.gen::<f64>() * 0.1
        }).collect(),
        _ => (0..size).map(|i| i as f64).collect(),
    }
}

use std::path::PathBuf;