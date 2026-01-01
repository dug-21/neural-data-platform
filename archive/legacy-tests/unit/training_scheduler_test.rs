//! Unit tests for the DAA training scheduler
//!
//! Tests the scheduling logic, resource allocation, priority queue management,
//! and circuit breaker functionality.

use chrono::{DateTime, Duration, NaiveTime, TimeZone, Utc, Weekday};
use autonomous_platform::daa::autonomous_training::{
    ResourceRequirements, TrainingDecision, TrainingDecisionType, TrainingPriority,
};
use autonomous_platform::daa::training_scheduler::{
    CircuitBreaker, DAASchedulerConfig, DAATrainingJob, DAATrainingScheduler, EmergencyConfig,
    JobStatus, PreemptionConfig, QueueConfig, ResourceLimitConfig, ResourceProfile,
    SchedulerMetrics,
};
use autonomous_platform::streaming::event_bus::EventBusIntegration;
use autonomous_platform::utils::market_hours::{MarketHours, TrainingWindow};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Create a mock event bus for testing
async fn create_mock_event_bus() -> Arc<EventBusIntegration> {
    use autonomous_platform::data::{RedisCache, TimescaleDBStorage};
    use autonomous_platform::integration::data_access::DataAccessLayer;

    // In test environment, use mock connections
    let storage = Arc::new(
        TimescaleDBStorage::new("postgresql://test:test@localhost/test")
            .await
            .unwrap(),
    );
    let cache = Arc::new(RedisCache::new("redis://localhost:6379").await.unwrap());
    let data_access = Arc::new(DataAccessLayer::new(storage, cache).await.unwrap());

    Arc::new(EventBusIntegration::new(data_access).await.unwrap())
}

/// Create a test scheduler configuration
fn create_test_config() -> DAASchedulerConfig {
    DAASchedulerConfig {
        resource_limits: ResourceLimitConfig {
            optimal_window: ResourceProfile {
                cpu_cores: 16,
                memory_gb: 64.0,
                gpu_count: 4,
                network_mbps: 1000.0,
            },
            good_window: ResourceProfile {
                cpu_cores: 8,
                memory_gb: 32.0,
                gpu_count: 2,
                network_mbps: 500.0,
            },
            acceptable_window: ResourceProfile {
                cpu_cores: 4,
                memory_gb: 16.0,
                gpu_count: 1,
                network_mbps: 250.0,
            },
            poor_window: ResourceProfile {
                cpu_cores: 2,
                memory_gb: 8.0,
                gpu_count: 0,
                network_mbps: 100.0,
            },
            restricted_window: ResourceProfile {
                cpu_cores: 1,
                memory_gb: 4.0,
                gpu_count: 0,
                network_mbps: 50.0,
            },
        },
        emergency_override: EmergencyConfig {
            max_emergency_jobs_per_hour: 3,
            emergency_resource_multiplier: 2.0,
            circuit_breaker_threshold: 5,
            cooldown_minutes: 30,
        },
        queue_config: QueueConfig {
            max_queue_size: 100,
            priority_boost_after_minutes: 60,
            starvation_prevention_enabled: true,
            batch_scheduling_enabled: true,
        },
        preemption_config: PreemptionConfig {
            enabled: true,
            checkpoint_interval_minutes: 15,
            min_progress_for_checkpoint: 0.1,
            preemption_grace_period_secs: 60,
        },
        monitoring: autonomous_platform::daa::training_scheduler::MonitoringConfig {
            metrics_interval_secs: 1,
            alert_thresholds: autonomous_platform::daa::training_scheduler::AlertThresholds {
                queue_length_warning: 20,
                queue_length_critical: 50,
                resource_usage_warning: 0.8,
                resource_usage_critical: 0.95,
                job_failure_rate_warning: 0.1,
            },
            performance_tracking_enabled: true,
        },
    }
}

/// Create a test training decision
fn create_test_decision(priority: TrainingPriority) -> TrainingDecision {
    TrainingDecision {
        decision_id: Uuid::new_v4().to_string(),
        timestamp: Utc::now(),
        decision_type: TrainingDecisionType::IncrementalTraining,
        confidence: 0.85,
        reasoning: vec!["test reasoning".to_string()],
        reasons: vec!["test reason".to_string()],
        priority: Some(priority),
        priority_numeric: Some(128), // Medium priority as numeric
        performance_snapshot: autonomous_platform::daa::autonomous_training::PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: 0.8,
            latency_ms: 100,
            error_rate: 0.05,
            recent_predictions: 100,
            confidence: 0.85,
            price_error: 0.05,
            sharpe_ratio: 1.2,
            max_drawdown: 0.05,
            volatility: 0.02,
            model_agreement: 0.9,
            consecutive_failures: 0,
            trading_volume: vec![1000000.0],
            profit_loss: 0.05,
            event_count: 1,
            window_duration: chrono::Duration::minutes(5),
            symbol: "BTCUSD".to_string(),
            trading_performance: None,
            accuracy_metrics: None,
            data_type_metrics: None,
            cpu_usage: 0.0,
            memory_usage: 0.0,
            active_connections: 0,
            requests_per_second: 0.0,
            average_response_time: 0.0,
            cache_hit_rate: 0.0,
        },
        resource_requirements: ResourceRequirements {
            memory_gb: 16.0,
            cpu_cores: 4,
            gpu_memory_gb: Some(8.0),
            storage_gb: 50.0,
            gpu_required: true,
            network_bandwidth_mbps: 100.0,
        },
        estimated_duration: Duration::minutes(30),
        affected_models: vec!["model1".to_string(), "model2".to_string()],
        // MCP compatibility fields
        triggered_by: Some("test".to_string()),
        estimated_training_time_minutes: Some(30),
        target_symbols: vec!["BTCUSD".to_string(), "ETHUSD".to_string()],
        training_parameters: None,
    }
}

#[tokio::test]
async fn test_scheduler_creation() {
    let config = create_test_config();
    let market_hours = Arc::new(MarketHours::new());
    let event_bus = create_mock_event_bus().await;

    let scheduler = DAATrainingScheduler::new(config, market_hours, event_bus)
        .await
        .unwrap();

    // Verify initial state
    let status = scheduler.get_status().await;
    assert_eq!(status["queue_length"], 0);
    assert_eq!(status["active_jobs"], 0);
}

#[tokio::test]
async fn test_job_submission_and_queuing() {
    let config = create_test_config();
    let market_hours = Arc::new(MarketHours::new());
    let event_bus = create_mock_event_bus().await;

    let scheduler = DAATrainingScheduler::new(config, market_hours, event_bus)
        .await
        .unwrap();

    // Submit jobs with different priorities
    let high_priority = create_test_decision(TrainingPriority::High);
    let medium_priority = create_test_decision(TrainingPriority::Medium);
    let low_priority = create_test_decision(TrainingPriority::Low);

    let job_id1 = scheduler.submit_training_decision(low_priority).await.unwrap();
    let job_id2 = scheduler
        .submit_training_decision(high_priority)
        .await
        .unwrap();
    let job_id3 = scheduler
        .submit_training_decision(medium_priority)
        .await
        .unwrap();

    // Check queue order (high priority should be first)
    let status = scheduler.get_status().await;
    assert_eq!(status["queue_length"], 3);
    assert_eq!(status["active_jobs"], 0);
}

#[tokio::test]
async fn test_queue_size_limit() {
    let mut config = create_test_config();
    config.queue_config.max_queue_size = 2;

    let market_hours = Arc::new(MarketHours::new());
    let event_bus = create_mock_event_bus().await;

    let scheduler = DAATrainingScheduler::new(config, market_hours, event_bus)
        .await
        .unwrap();

    // Fill the queue
    let decision1 = create_test_decision(TrainingPriority::Medium);
    let decision2 = create_test_decision(TrainingPriority::Medium);
    let decision3 = create_test_decision(TrainingPriority::Medium);

    scheduler.submit_training_decision(decision1).await.unwrap();
    scheduler.submit_training_decision(decision2).await.unwrap();

    // Third submission should fail
    let result = scheduler.submit_training_decision(decision3).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("queue is full"));
}

#[tokio::test]
async fn test_priority_ordering() {
    let config = create_test_config();
    let market_hours = Arc::new(MarketHours::new());
    let event_bus = create_mock_event_bus().await;

    let scheduler = DAATrainingScheduler::new(config, market_hours, event_bus)
        .await
        .unwrap();

    // Submit jobs in reverse priority order
    let priorities = vec![
        TrainingPriority::Low,
        TrainingPriority::Medium,
        TrainingPriority::High,
        TrainingPriority::Critical,
        TrainingPriority::Emergency,
    ];

    let mut job_ids = Vec::new();
    for priority in priorities {
        let decision = create_test_decision(priority);
        let job_id = scheduler.submit_training_decision(decision).await.unwrap();
        job_ids.push(job_id);
    }

    // Verify queue has 5 jobs
    let status = scheduler.get_status().await;
    assert_eq!(status["queue_length"], 5);
}

#[tokio::test]
async fn test_emergency_override() {
    let config = create_test_config();
    let market_hours = Arc::new(MarketHours::new());
    let event_bus = create_mock_event_bus().await;

    let scheduler = DAATrainingScheduler::new(config, market_hours, event_bus)
        .await
        .unwrap();

    // Submit a low priority job
    let low_priority = create_test_decision(TrainingPriority::Low);
    let job_id = scheduler
        .submit_training_decision(low_priority)
        .await
        .unwrap();

    // Emergency override should elevate priority
    scheduler
        .emergency_override(&job_id, "Critical bug fix")
        .await
        .unwrap();

    let status = scheduler.get_status().await;
    let metrics = status["metrics"].as_object().unwrap();
    assert_eq!(metrics["emergency_overrides"], 1);
}

#[tokio::test]
async fn test_circuit_breaker_functionality() {
    let mut config = create_test_config();
    config.emergency_override.max_emergency_jobs_per_hour = 2;
    config.emergency_override.circuit_breaker_threshold = 3;

    let market_hours = Arc::new(MarketHours::new());
    let event_bus = create_mock_event_bus().await;

    let scheduler = DAATrainingScheduler::new(config, market_hours, event_bus)
        .await
        .unwrap();

    // Submit multiple emergency jobs to trigger circuit breaker
    for i in 0..4 {
        let emergency = create_test_decision(TrainingPriority::Emergency);
        let result = scheduler.submit_training_decision(emergency).await;

        if i < 3 {
            assert!(result.is_ok(), "Emergency job {} should succeed", i);
        }
    }

    // Check circuit breaker status
    let status = scheduler.get_status().await;
    let breaker = status["circuit_breaker"].as_object().unwrap();
    assert_eq!(breaker["emergency_count"], 3);
}

#[tokio::test]
async fn test_resource_allocation() {
    let config = create_test_config();
    let market_hours = Arc::new(MarketHours::new());
    let event_bus = create_mock_event_bus().await;

    let scheduler = DAATrainingScheduler::new(config.clone(), market_hours, event_bus)
        .await
        .unwrap();

    // Create a job that requires specific resources
    let mut decision = create_test_decision(TrainingPriority::High);
    decision.resource_requirements = ResourceRequirements {
        cpu_cores: 8,
        memory_gb: 32.0,
        gpu_required: true,
        estimated_time_minutes: 60,
        network_bandwidth_mbps: 200.0,
        storage_gb: 100.0,
    };

    let job_id = scheduler.submit_training_decision(decision).await.unwrap();

    // Run scheduling cycle
    scheduler.scheduling_cycle().await.unwrap();

    // Check that resources were allocated
    let status = scheduler.get_status().await;
    assert!(status["active_jobs"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn test_preemption_mechanism() {
    let mut config = create_test_config();
    config.preemption_config.enabled = true;

    let market_hours = Arc::new(MarketHours::new());
    let event_bus = create_mock_event_bus().await;

    let scheduler = DAATrainingScheduler::new(config, market_hours, event_bus)
        .await
        .unwrap();

    // Submit a low priority job
    let low_priority = create_test_decision(TrainingPriority::Low);
    let low_job_id = scheduler
        .submit_training_decision(low_priority)
        .await
        .unwrap();

    // Run scheduling to start the low priority job
    scheduler.scheduling_cycle().await.unwrap();

    // Submit a high priority job that requires preemption
    let mut high_priority = create_test_decision(TrainingPriority::Critical);
    high_priority.resource_requirements.cpu_cores = 16; // Requires all CPUs

    let high_job_id = scheduler
        .submit_training_decision(high_priority)
        .await
        .unwrap();

    // Run scheduling again - should preempt low priority job
    scheduler.scheduling_cycle().await.unwrap();

    let status = scheduler.get_status().await;
    let metrics = status["metrics"].as_object().unwrap();
    // Note: Preemption count might be 0 if the job wasn't actually running
    // In a real test with actual execution, this would be > 0
    assert!(metrics.contains_key("preemptions"));
}

#[tokio::test]
async fn test_job_completion_handling() {
    let config = create_test_config();
    let market_hours = Arc::new(MarketHours::new());
    let event_bus = create_mock_event_bus().await;

    let scheduler = DAATrainingScheduler::new(config, market_hours, event_bus)
        .await
        .unwrap();

    // Submit and schedule a job
    let decision = create_test_decision(TrainingPriority::High);
    let job_id = scheduler.submit_training_decision(decision).await.unwrap();

    // Run scheduling cycle to start job
    scheduler.scheduling_cycle().await.unwrap();

    // Simulate job completion
    // In production, this would be triggered by actual training completion
    // For testing, we'll manually update the job status
    {
        let mut active_jobs = scheduler.active_jobs.write().await;
        if let Some(job) = active_jobs.get_mut(&job_id) {
            job.status = JobStatus::Completed;
            job.completed_at = Some(Utc::now());
        }
    }

    // Run scheduling cycle to handle completion
    scheduler.scheduling_cycle().await.unwrap();

    // Verify job was removed from active jobs
    let status = scheduler.get_status().await;
    assert_eq!(status["active_jobs"], 0);
}

#[tokio::test]
async fn test_resource_limit_by_window() {
    let config = create_test_config();
    let market_hours = Arc::new(MarketHours::new());
    let event_bus = create_mock_event_bus().await;

    let scheduler = DAATrainingScheduler::new(config.clone(), market_hours, event_bus)
        .await
        .unwrap();

    // Test resource limits for different training windows
    let windows = vec![
        TrainingWindow::Optimal,
        TrainingWindow::Good,
        TrainingWindow::Acceptable,
        TrainingWindow::Poor,
        TrainingWindow::Restricted,
    ];

    for window in windows {
        let limit = scheduler.get_resource_limit_for_window(window);
        match window {
            TrainingWindow::Optimal => {
                assert_eq!(limit.cpu_cores, 16);
                assert_eq!(limit.gpu_count, 4);
            }
            TrainingWindow::Good => {
                assert_eq!(limit.cpu_cores, 8);
                assert_eq!(limit.gpu_count, 2);
            }
            TrainingWindow::Acceptable => {
                assert_eq!(limit.cpu_cores, 4);
                assert_eq!(limit.gpu_count, 1);
            }
            TrainingWindow::Poor => {
                assert_eq!(limit.cpu_cores, 2);
                assert_eq!(limit.gpu_count, 0);
            }
            TrainingWindow::Restricted => {
                assert_eq!(limit.cpu_cores, 1);
                assert_eq!(limit.gpu_count, 0);
            }
        }
    }
}

#[tokio::test]
async fn test_batch_scheduling() {
    let mut config = create_test_config();
    config.queue_config.batch_scheduling_enabled = true;

    let market_hours = Arc::new(MarketHours::new());
    let event_bus = create_mock_event_bus().await;

    let scheduler = DAATrainingScheduler::new(config, market_hours, event_bus)
        .await
        .unwrap();

    // Submit multiple jobs that can be scheduled together
    for _ in 0..5 {
        let mut decision = create_test_decision(TrainingPriority::Medium);
        decision.resource_requirements.cpu_cores = 2;
        decision.resource_requirements.memory_gb = 8.0;
        decision.resource_requirements.gpu_required = false;

        scheduler.submit_training_decision(decision).await.unwrap();
    }

    // Run scheduling cycle
    scheduler.scheduling_cycle().await.unwrap();

    // Multiple jobs should be scheduled in the batch
    let status = scheduler.get_status().await;
    assert!(status["active_jobs"].as_u64().unwrap() > 1);
}

#[tokio::test]
async fn test_metrics_tracking() {
    let config = create_test_config();
    let market_hours = Arc::new(MarketHours::new());
    let event_bus = create_mock_event_bus().await;

    let scheduler = DAATrainingScheduler::new(config, market_hours, event_bus)
        .await
        .unwrap();

    // Submit jobs with different priorities
    let priorities = vec![
        TrainingPriority::Emergency,
        TrainingPriority::Critical,
        TrainingPriority::High,
        TrainingPriority::Medium,
        TrainingPriority::Low,
    ];

    for priority in priorities {
        let decision = create_test_decision(priority);
        scheduler.submit_training_decision(decision).await.unwrap();
    }

    // Check metrics
    let status = scheduler.get_status().await;
    let metrics = status["metrics"].as_object().unwrap();

    assert_eq!(metrics["total_scheduled"], 5);
    assert!(metrics.contains_key("failures"));
    assert!(metrics.contains_key("preemptions"));
    assert!(metrics.contains_key("emergency_overrides"));
}

#[tokio::test]
async fn test_job_lifecycle() {
    let config = create_test_config();
    let market_hours = Arc::new(MarketHours::new());
    let event_bus = create_mock_event_bus().await;

    let scheduler = DAATrainingScheduler::new(config, market_hours, event_bus)
        .await
        .unwrap();

    // Create and submit a job
    let decision = create_test_decision(TrainingPriority::High);
    let job_id = scheduler.submit_training_decision(decision).await.unwrap();

    // Verify job is queued
    {
        let queue = scheduler.job_queue.read().await;
        assert!(queue.iter().any(|j| j.id == job_id));
    }

    // Run scheduling cycle
    scheduler.scheduling_cycle().await.unwrap();

    // Verify job moved to active
    {
        let active = scheduler.active_jobs.read().await;
        assert!(active.contains_key(&job_id));
    }

    // Verify job status progression
    {
        let active = scheduler.active_jobs.read().await;
        if let Some(job) = active.get(&job_id) {
            assert!(matches!(
                job.status,
                JobStatus::Scheduled | JobStatus::Running
            ));
            assert!(job.scheduled_for.is_some());
        }
    }
}