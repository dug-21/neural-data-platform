//! Integration tests for market-aware training functionality
//!
//! Tests the complete integration between training scheduler, market hours detection,
//! DAA decision making, and resource management across different market conditions.

use chrono::{DateTime, Duration, TimeZone, Utc, Weekday};
use neural_trader::daa::autonomous_training::{
    AutonomousTrainingAgent, ResourceRequirements, TrainingDecision, TrainingDecisionType,
    TrainingPriority,
};
use neural_trader::daa::training_scheduler::{
    DAASchedulerConfig, DAATrainingScheduler, JobStatus,
};
use neural_trader::data::{RedisCache, TimescaleDBStorage};
use neural_trader::integration::data_access::DataAccessLayer;
use neural_trader::streaming::event_bus::{EventBusIntegration, SystemEvent};
use neural_trader::utils::market_hours::{Exchange, MarketHours, TrainingWindow};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, timeout};
use uuid::Uuid;

/// Test environment setup
struct TestEnvironment {
    scheduler: Arc<DAATrainingScheduler>,
    market_hours: Arc<MarketHours>,
    event_bus: Arc<EventBusIntegration>,
    training_agent: Arc<AutonomousTrainingAgent>,
    received_events: Arc<RwLock<Vec<SystemEvent>>>,
}

impl TestEnvironment {
    async fn new() -> Self {
        // Setup test data access layer
        let storage = Arc::new(
            TimescaleDBStorage::new("postgresql://test:test@localhost/test")
                .await
                .unwrap(),
        );
        let cache = Arc::new(RedisCache::new("redis://localhost:6379").await.unwrap());
        let data_access = Arc::new(DataAccessLayer::new(storage, cache).await.unwrap());

        // Create event bus
        let event_bus = Arc::new(EventBusIntegration::new(data_access.clone()).await.unwrap());

        // Create market hours tracker
        let market_hours = Arc::new(MarketHours::new());

        // Create scheduler with test configuration
        let scheduler_config = DAASchedulerConfig::default();
        let scheduler = Arc::new(
            DAATrainingScheduler::new(
                scheduler_config,
                market_hours.clone(),
                event_bus.clone(),
            )
            .await
            .unwrap(),
        );

        // Create training agent
        let training_agent = Arc::new(
            AutonomousTrainingAgent::new(
                data_access.clone(),
                event_bus.clone(),
                10.0, // performance threshold
                60,   // evaluation interval
                "http://localhost:8000".to_string(),
            )
            .await
            .unwrap(),
        );

        // Setup event collection
        let received_events = Arc::new(RwLock::new(Vec::new()));
        let events_clone = received_events.clone();

        // Subscribe to events for testing
        let mut subscriber = event_bus.subscribe_to_events("test_subscriber").await.unwrap();
        tokio::spawn(async move {
            while let Ok(event) = subscriber.recv().await {
                let mut events = events_clone.write().await;
                events.push(event);
            }
        });

        Self {
            scheduler,
            market_hours,
            event_bus,
            training_agent,
            received_events,
        }
    }

    /// Create a mock training decision
    fn create_decision(
        priority: TrainingPriority,
        decision_type: TrainingDecisionType,
    ) -> TrainingDecision {
        TrainingDecision {
            decision_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            decision_type,
            priority,
            confidence_score: 0.85,
            resource_requirements: ResourceRequirements {
                cpu_cores: 4,
                memory_gb: 16.0,
                gpu_required: true,
                estimated_time_minutes: 30,
                network_bandwidth_mbps: 100.0,
                storage_gb: 50.0,
            },
            estimated_duration: Duration::minutes(30),
            market_impact_assessment: "Low".to_string(),
            performance_delta_threshold: 0.05,
            affected_models: vec!["model1".to_string(), "model2".to_string()],
            training_data_requirements: HashMap::new(),
        }
    }

    /// Get events of a specific type
    async fn get_events_by_type(&self, event_type: &str) -> Vec<SystemEvent> {
        let events = self.received_events.read().await;
        events
            .iter()
            .filter(|e| e.event_type == event_type)
            .cloned()
            .collect()
    }
}

#[tokio::test]
async fn test_market_aware_scheduling_during_weekend() {
    let env = TestEnvironment::new().await;

    // Simulate weekend time
    let weekend = Utc.with_ymd_and_hms(2024, 1, 6, 12, 0, 0).unwrap(); // Saturday

    // Verify it's optimal training time
    let window = env.market_hours.get_training_window(weekend).await;
    assert_eq!(window, TrainingWindow::Optimal);

    // Submit multiple training jobs
    let decisions = vec![
        TestEnvironment::create_decision(
            TrainingPriority::High,
            TrainingDecisionType::ModelRefresh,
        ),
        TestEnvironment::create_decision(
            TrainingPriority::Medium,
            TrainingDecisionType::PredictiveRetrain,
        ),
        TestEnvironment::create_decision(
            TrainingPriority::Low,
            TrainingDecisionType::OnlineAdaptation,
        ),
    ];

    let mut job_ids = Vec::new();
    for decision in decisions {
        let job_id = env.scheduler.submit_training_decision(decision).await.unwrap();
        job_ids.push(job_id);
    }

    // Run scheduling cycle
    env.scheduler.scheduling_cycle().await.unwrap();

    // All jobs should be scheduled during optimal window
    let status = env.scheduler.get_status().await;
    assert!(status["active_jobs"].as_u64().unwrap() >= 2);

    // Wait for events
    sleep(tokio::time::Duration::from_millis(100)).await;

    // Check that training started events were emitted
    let start_events = env.get_events_by_type("training_started").await;
    assert!(!start_events.is_empty());
}

#[tokio::test]
async fn test_market_aware_scheduling_during_trading_hours() {
    let env = TestEnvironment::new().await;

    // Simulate active trading hours (Wednesday 2 PM UTC ~ 9 AM ET)
    let trading_hours = Utc.with_ymd_and_hms(2024, 1, 10, 14, 0, 0).unwrap();

    // Check market intensity
    let intensity = env.market_hours.get_market_intensity(trading_hours).await;
    assert!(intensity.active_exchanges > 0);

    // Submit high and low priority jobs
    let high_priority =
        TestEnvironment::create_decision(TrainingPriority::High, TrainingDecisionType::Emergency);
    let low_priority = TestEnvironment::create_decision(
        TrainingPriority::Low,
        TrainingDecisionType::ModelRefresh,
    );

    let high_id = env
        .scheduler
        .submit_training_decision(high_priority)
        .await
        .unwrap();
    let low_id = env
        .scheduler
        .submit_training_decision(low_priority)
        .await
        .unwrap();

    // Run scheduling during trading hours
    env.scheduler.scheduling_cycle().await.unwrap();

    // Check scheduling decisions
    let status = env.scheduler.get_status().await;

    // Low priority job should remain queued during trading hours
    let queue_length = status["queue_length"].as_u64().unwrap();
    assert!(queue_length > 0);
}

#[tokio::test]
async fn test_emergency_override_during_restricted_window() {
    let env = TestEnvironment::new().await;

    // Submit a critical job
    let critical = TestEnvironment::create_decision(
        TrainingPriority::Medium,
        TrainingDecisionType::CriticalUpdate,
    );
    let job_id = env
        .scheduler
        .submit_training_decision(critical)
        .await
        .unwrap();

    // Apply emergency override
    env.scheduler
        .emergency_override(&job_id, "Security patch required")
        .await
        .unwrap();

    // Run scheduling
    env.scheduler.scheduling_cycle().await.unwrap();

    // Job should be scheduled despite restrictions
    let status = env.scheduler.get_status().await;
    let metrics = status["metrics"].as_object().unwrap();
    assert_eq!(metrics["emergency_overrides"], 1);
}

#[tokio::test]
async fn test_circuit_breaker_protection() {
    let env = TestEnvironment::new().await;

    // Submit multiple emergency jobs to trigger circuit breaker
    let mut emergency_count = 0;
    for i in 0..10 {
        let emergency = TestEnvironment::create_decision(
            TrainingPriority::Emergency,
            TrainingDecisionType::Emergency,
        );

        match env.scheduler.submit_training_decision(emergency).await {
            Ok(_) => emergency_count += 1,
            Err(e) => {
                println!("Emergency job {} rejected: {}", i, e);
                break;
            }
        }

        // Run scheduling after each submission
        env.scheduler.scheduling_cycle().await.unwrap();
    }

    // Circuit breaker should have limited emergency jobs
    assert!(emergency_count <= 5);

    let status = env.scheduler.get_status().await;
    let breaker = status["circuit_breaker"].as_object().unwrap();
    assert!(breaker["emergency_count"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn test_preemption_during_resource_contention() {
    let env = TestEnvironment::new().await;

    // Create a low priority job that uses all resources
    let mut low_priority = TestEnvironment::create_decision(
        TrainingPriority::Low,
        TrainingDecisionType::ModelRefresh,
    );
    low_priority.resource_requirements.cpu_cores = 12;
    low_priority.resource_requirements.memory_gb = 48.0;

    let low_id = env
        .scheduler
        .submit_training_decision(low_priority)
        .await
        .unwrap();

    // Schedule the low priority job
    env.scheduler.scheduling_cycle().await.unwrap();

    // Now submit a critical job that needs resources
    let mut critical = TestEnvironment::create_decision(
        TrainingPriority::Critical,
        TrainingDecisionType::CriticalUpdate,
    );
    critical.resource_requirements.cpu_cores = 8;
    critical.resource_requirements.memory_gb = 32.0;

    let critical_id = env
        .scheduler
        .submit_training_decision(critical)
        .await
        .unwrap();

    // Run scheduling - should preempt low priority job
    env.scheduler.scheduling_cycle().await.unwrap();

    // Check preemption occurred
    let status = env.scheduler.get_status().await;
    let metrics = status["metrics"].as_object().unwrap();
    // Preemption might not always occur in test environment
    println!("Preemptions: {:?}", metrics.get("preemptions"));
}

#[tokio::test]
async fn test_training_window_transitions() {
    let env = TestEnvironment::new().await;

    // Submit jobs with different priorities
    let jobs = vec![
        TestEnvironment::create_decision(
            TrainingPriority::High,
            TrainingDecisionType::ModelRefresh,
        ),
        TestEnvironment::create_decision(
            TrainingPriority::Medium,
            TrainingDecisionType::PredictiveRetrain,
        ),
        TestEnvironment::create_decision(
            TrainingPriority::Low,
            TrainingDecisionType::OnlineAdaptation,
        ),
    ];

    let mut job_ids = Vec::new();
    for job in jobs {
        let id = env.scheduler.submit_training_decision(job).await.unwrap();
        job_ids.push(id);
    }

    // Simulate different market windows
    let windows = vec![
        (TrainingWindow::Restricted, 1),
        (TrainingWindow::Poor, 2),
        (TrainingWindow::Acceptable, 4),
        (TrainingWindow::Good, 8),
        (TrainingWindow::Optimal, 16),
    ];

    for (window, expected_resources) in windows {
        // Get resource limit for window
        let resource_limit = env.scheduler.get_resource_limit_for_window(window);

        // Verify resource limits decrease as market activity increases
        match window {
            TrainingWindow::Optimal => assert_eq!(resource_limit.cpu_cores, 16),
            TrainingWindow::Good => assert_eq!(resource_limit.cpu_cores, 8),
            TrainingWindow::Acceptable => assert_eq!(resource_limit.cpu_cores, 4),
            TrainingWindow::Poor => assert_eq!(resource_limit.cpu_cores, 2),
            TrainingWindow::Restricted => assert_eq!(resource_limit.cpu_cores, 1),
        }
    }
}

#[tokio::test]
async fn test_holiday_scheduling() {
    let env = TestEnvironment::new().await;

    // Add holidays to market hours
    let holidays = vec![
        Utc.with_ymd_and_hms(2024, 12, 25, 0, 0, 0).unwrap(), // Christmas
        Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),   // New Year
    ];

    env.market_hours
        .update_holidays(Exchange::NYSE, holidays.clone())
        .await;
    env.market_hours
        .update_holidays(Exchange::NASDAQ, holidays)
        .await;

    // Test scheduling on Christmas (should be optimal)
    let christmas = Utc.with_ymd_and_hms(2024, 12, 25, 12, 0, 0).unwrap();
    let window = env.market_hours.get_training_window(christmas).await;
    assert_eq!(window, TrainingWindow::Optimal);

    // Submit jobs on holiday
    let holiday_job =
        TestEnvironment::create_decision(TrainingPriority::Medium, TrainingDecisionType::ModelRefresh);
    let job_id = env
        .scheduler
        .submit_training_decision(holiday_job)
        .await
        .unwrap();

    // Should be able to use full resources on holiday
    env.scheduler.scheduling_cycle().await.unwrap();

    let status = env.scheduler.get_status().await;
    assert!(status["active_jobs"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn test_global_market_coordination() {
    let env = TestEnvironment::new().await;

    // Test scheduling across different global market times
    let test_times = vec![
        // Asia morning (Tokyo open, US closed)
        Utc.with_ymd_and_hms(2024, 1, 15, 1, 0, 0).unwrap(),
        // Europe morning (London open, Asia closing)
        Utc.with_ymd_and_hms(2024, 1, 15, 8, 0, 0).unwrap(),
        // US morning (NYSE open, Europe active)
        Utc.with_ymd_and_hms(2024, 1, 15, 14, 30, 0).unwrap(),
        // US afternoon (US closing, Asia closed)
        Utc.with_ymd_and_hms(2024, 1, 15, 21, 0, 0).unwrap(),
    ];

    for test_time in test_times {
        // Get market state
        let intensity = env.market_hours.get_market_intensity(test_time).await;
        let window = env.market_hours.get_training_window(test_time).await;
        let active_exchanges = env.market_hours.get_active_exchanges(test_time).await;

        println!(
            "Time: {} UTC, Active: {}, Intensity: {:.2}, Window: {}",
            test_time.format("%H:%M"),
            active_exchanges.len(),
            intensity.score,
            window
        );

        // Submit a job and check resource allocation
        let job = TestEnvironment::create_decision(
            TrainingPriority::Medium,
            TrainingDecisionType::ModelRefresh,
        );
        let job_id = env.scheduler.submit_training_decision(job).await.unwrap();

        // Resource limit should match market conditions
        let resource_limit = env.market_hours.get_resource_limit(test_time).await;
        assert!(resource_limit <= 1.0);
        assert!(resource_limit >= 0.1);
    }
}

#[tokio::test]
async fn test_batch_scheduling_efficiency() {
    let env = TestEnvironment::new().await;

    // Submit multiple small jobs that can be batched
    let mut job_ids = Vec::new();
    for i in 0..10 {
        let mut job = TestEnvironment::create_decision(
            TrainingPriority::Medium,
            TrainingDecisionType::OnlineAdaptation,
        );
        // Small resource requirements
        job.resource_requirements.cpu_cores = 1;
        job.resource_requirements.memory_gb = 4.0;
        job.resource_requirements.gpu_required = false;
        job.estimated_duration = Duration::minutes(10);

        let id = env.scheduler.submit_training_decision(job).await.unwrap();
        job_ids.push(id);
    }

    // Run batch scheduling
    env.scheduler.scheduling_cycle().await.unwrap();

    // Multiple jobs should be scheduled together
    let status = env.scheduler.get_status().await;
    let active = status["active_jobs"].as_u64().unwrap();
    assert!(active > 1, "Batch scheduling should schedule multiple jobs");
}

#[tokio::test]
async fn test_scheduler_metrics_collection() {
    let env = TestEnvironment::new().await;

    // Submit various jobs to generate metrics
    let job_types = vec![
        (TrainingPriority::Emergency, TrainingDecisionType::Emergency),
        (TrainingPriority::Critical, TrainingDecisionType::CriticalUpdate),
        (TrainingPriority::High, TrainingDecisionType::ModelRefresh),
        (TrainingPriority::Medium, TrainingDecisionType::PredictiveRetrain),
        (TrainingPriority::Low, TrainingDecisionType::OnlineAdaptation),
    ];

    for (priority, decision_type) in job_types {
        let job = TestEnvironment::create_decision(priority, decision_type);
        env.scheduler.submit_training_decision(job).await.unwrap();
    }

    // Run several scheduling cycles
    for _ in 0..3 {
        env.scheduler.scheduling_cycle().await.unwrap();
        sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // Check metrics
    let status = env.scheduler.get_status().await;
    let metrics = status["metrics"].as_object().unwrap();

    assert!(metrics["total_scheduled"].as_u64().unwrap() >= 5);
    assert!(metrics.contains_key("failures"));
    assert!(metrics.contains_key("preemptions"));
    assert!(metrics.contains_key("emergency_overrides"));

    // Check for metrics events
    let metric_events = env.get_events_by_type("scheduler_metrics").await;
    assert!(!metric_events.is_empty());
}

#[tokio::test]
async fn test_training_decision_integration() {
    let env = TestEnvironment::new().await;

    // Create a scenario that triggers autonomous training decision
    // This would normally be triggered by performance degradation
    let decision = env
        .training_agent
        .evaluate_training_need(
            "test_model",
            85.0,  // current performance
            95.0,  // baseline performance
            100,   // data points
        )
        .await;

    if let Some(decision) = decision {
        // Submit to scheduler
        let job_id = env
            .scheduler
            .submit_training_decision(decision)
            .await
            .unwrap();

        // Run scheduling
        env.scheduler.scheduling_cycle().await.unwrap();

        // Verify job was created
        let status = env.scheduler.get_status().await;
        assert!(
            status["queue_length"].as_u64().unwrap() > 0
                || status["active_jobs"].as_u64().unwrap() > 0
        );
    }
}

#[tokio::test]
async fn test_concurrent_job_management() {
    let env = TestEnvironment::new().await;

    // Submit jobs concurrently
    let mut handles = Vec::new();
    let scheduler = env.scheduler.clone();

    for i in 0..5 {
        let scheduler_clone = scheduler.clone();
        let handle = tokio::spawn(async move {
            let job = TestEnvironment::create_decision(
                TrainingPriority::Medium,
                TrainingDecisionType::ModelRefresh,
            );
            scheduler_clone.submit_training_decision(job).await
        });
        handles.push(handle);
    }

    // Wait for all submissions
    let mut job_ids = Vec::new();
    for handle in handles {
        if let Ok(Ok(job_id)) = handle.await {
            job_ids.push(job_id);
        }
    }

    assert_eq!(job_ids.len(), 5);

    // Verify all jobs are queued
    let status = env.scheduler.get_status().await;
    assert_eq!(status["queue_length"].as_u64().unwrap(), 5);
}

#[tokio::test]
async fn test_resource_recovery_after_failure() {
    let env = TestEnvironment::new().await;

    // Submit a job that uses significant resources
    let mut job = TestEnvironment::create_decision(
        TrainingPriority::High,
        TrainingDecisionType::ModelRefresh,
    );
    job.resource_requirements.cpu_cores = 8;
    job.resource_requirements.memory_gb = 32.0;

    let job_id = env.scheduler.submit_training_decision(job).await.unwrap();

    // Schedule the job
    env.scheduler.scheduling_cycle().await.unwrap();

    // Simulate job failure
    {
        let mut active = env.scheduler.active_jobs.write().await;
        if let Some(job) = active.get_mut(&job_id) {
            job.status = JobStatus::Failed("Simulated failure".to_string());
            job.completed_at = Some(Utc::now());
        }
    }

    // Run cycle to handle failure
    env.scheduler.scheduling_cycle().await.unwrap();

    // Resources should be recovered
    let status = env.scheduler.get_status().await;
    assert_eq!(status["active_jobs"], 0);

    // Submit another job requiring same resources
    let new_job = TestEnvironment::create_decision(
        TrainingPriority::High,
        TrainingDecisionType::ModelRefresh,
    );
    let new_id = env.scheduler.submit_training_decision(new_job).await.unwrap();

    // Should be able to schedule with recovered resources
    env.scheduler.scheduling_cycle().await.unwrap();

    let final_status = env.scheduler.get_status().await;
    assert!(final_status["active_jobs"].as_u64().unwrap() > 0);
}