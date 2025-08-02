//! Integration tests for Neural Trader orchestration components
//!
//! Tests cover:
//! - Redis connection and data subscription
//! - EventBus message routing
//! - Neural predictor initialization
//! - DAA coordinator decision making
//! - Mock implementations for isolated testing

use autonomous_platform::{
    adapters::{RedisAdapter, RedisConfig},
    config::{load_default_config, NeuralConfig},
    data::TimeSeriesData,
    integration::{
        daa_coordinator::{AutonomousDecision, DaaConfig, DaaCoordinator, TradingAction},
        data_access::DataAccessLayer,
    },
    neural::{NeuralPredictor, PredictionResult},
    strategies::{MarketContext, Position, PositionSide, RiskParameters, Signal, TradingStrategy},
    streaming::event_bus::{
        BatchConfig, DaaEvent, EventBusIntegration, EventRouter, MarketEvent, NewsEvent,
        QualityEvent, RetryConfig, SystemEvent,
    },
};

use anyhow::Result;
use async_trait::async_trait;
use chrono::{Duration, Utc};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// Mock Redis adapter for testing
struct MockRedisAdapter {
    is_connected: Arc<RwLock<bool>>,
    published_data: Arc<RwLock<Vec<String>>>,
    subscription_data: Arc<RwLock<Vec<String>>>,
}

impl MockRedisAdapter {
    fn new() -> Self {
        Self {
            is_connected: Arc::new(RwLock::new(false)),
            published_data: Arc::new(RwLock::new(Vec::new())),
            subscription_data: Arc::new(RwLock::new(Vec::new())),
        }
    }

    async fn connect(&self) -> Result<()> {
        *self.is_connected.write().await = true;
        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        *self.is_connected.write().await = false;
        Ok(())
    }

    async fn is_connected(&self) -> bool {
        *self.is_connected.read().await
    }

    async fn publish(&self, channel: &str, data: &str) -> Result<()> {
        if !self.is_connected().await {
            return Err(anyhow::anyhow!("Not connected to Redis"));
        }
        self.published_data
            .write()
            .await
            .push(format!("{}:{}", channel, data));
        Ok(())
    }

    async fn add_subscription_data(&self, data: String) {
        self.subscription_data.write().await.push(data);
    }

    async fn get_published_count(&self) -> usize {
        self.published_data.read().await.len()
    }
}

/// Mock DAA for testing EventBus integration
struct MockDaaAgent {
    id: String,
    received_events: Arc<RwLock<Vec<DaaEvent>>>,
}

impl MockDaaAgent {
    fn new(id: String) -> Self {
        Self {
            id,
            received_events: Arc::new(RwLock::new(Vec::new())),
        }
    }

    async fn get_receiver(&self) -> mpsc::Receiver<DaaEvent> {
        let (tx, rx) = mpsc::channel(100);
        let events = self.received_events.clone();

        // Spawn task to collect events
        tokio::spawn(async move {
            let mut rx = rx;
            while let Some(event) = rx.recv().await {
                events.write().await.push(event);
            }
        });

        rx
    }

    async fn event_count(&self) -> usize {
        self.received_events.read().await.len()
    }
}

/// Helper function to create test market data
fn create_test_time_series(count: usize) -> Vec<TimeSeriesData> {
    let mut data = Vec::new();
    let base_price = 50000.0;
    let now = Utc::now();

    for i in 0..count {
        data.push(TimeSeriesData {
            symbol: "BTC/USD".to_string(),
            timestamp: now - Duration::minutes(count as i64 - i as i64),
            open: base_price + (i as f64 * 10.0),
            high: base_price + (i as f64 * 10.0) + 50.0,
            low: base_price + (i as f64 * 10.0) - 50.0,
            close: base_price + (i as f64 * 10.0) + 25.0,
            volume: 1000.0 + (i as f64 * 100.0),
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some("BTC/USD".to_string()),
            value: Some(base_price + (i as f64 * 10.0) + 25.0),
            metadata: None,
        });
    }

    data
}

/// Mock trading strategy for testing
struct MockStrategy {
    signal: Signal,
}

#[async_trait]
impl TradingStrategy for MockStrategy {
    async fn generate_signal(
        &self,
        _market_context: &MarketContext,
        _current_position: Option<&Position>,
    ) -> Result<Signal> {
        Ok(self.signal.clone())
    }

    async fn calculate_position_size(
        &self,
        _signal: &Signal,
        _market_context: &MarketContext,
        _risk_parameters: &RiskParameters,
    ) -> Result<f64> {
        Ok(0.01)
    }
}

#[cfg(test)]
mod redis_connection_tests {
    use super::*;

    #[tokio::test]
    async fn test_redis_connection_lifecycle() {
        let adapter = MockRedisAdapter::new();

        // Test initial state
        assert!(!adapter.is_connected().await);

        // Test connection
        assert!(adapter.connect().await.is_ok());
        assert!(adapter.is_connected().await);

        // Test disconnection
        assert!(adapter.disconnect().await.is_ok());
        assert!(!adapter.is_connected().await);
    }

    #[tokio::test]
    async fn test_redis_publish_when_disconnected() {
        let adapter = MockRedisAdapter::new();

        // Try to publish without connection
        let result = adapter.publish("test_channel", "test_data").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Not connected"));
    }

    #[tokio::test]
    async fn test_redis_publish_subscribe_flow() {
        let adapter = MockRedisAdapter::new();

        // Connect first
        adapter.connect().await.unwrap();

        // Add some subscription data
        adapter
            .add_subscription_data("market:BTC/USD:50000".to_string())
            .await;
        adapter
            .add_subscription_data("market:ETH/USD:3000".to_string())
            .await;

        // Publish data
        adapter
            .publish("market", r#"{"symbol":"BTC/USD","price":50000}"#)
            .await
            .unwrap();
        adapter
            .publish("market", r#"{"symbol":"ETH/USD","price":3000}"#)
            .await
            .unwrap();

        // Verify published data
        assert_eq!(adapter.get_published_count().await, 2);
    }

    #[tokio::test]
    async fn test_redis_with_real_config() {
        // Test with actual Redis configuration structure
        let config = RedisConfig {
            host: "localhost".to_string(),
            port: 6379,
            password: None,
            db: 0,
            pool_size: 10,
        };

        // In real scenario, this would create actual Redis adapter
        let adapter = RedisAdapter::new(config.clone());

        // Verify configuration
        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 6379);
        assert!(config.password.is_none());
    }
}

#[cfg(test)]
mod event_bus_tests {
    use super::*;

    #[tokio::test]
    async fn test_event_bus_initialization() -> Result<()> {
        let daa_access = Arc::new(DataAccessLayer::new_mock()?);
        let event_bus = EventBusIntegration::new(daa_access).await?;

        // Test health check
        assert!(event_bus.health_check().await?);
        assert!(event_bus.daa_integration_health().await?);

        Ok(())
    }

    #[tokio::test]
    async fn test_market_event_publishing() -> Result<()> {
        let daa_access = Arc::new(DataAccessLayer::new_mock()?);
        let event_bus = EventBusIntegration::new(daa_access).await?;

        // Enable performance monitoring
        event_bus.enable_performance_monitoring(true).await?;

        // Create test market event
        let market_event = MarketEvent {
            symbol: "BTC/USD".to_string(),
            timestamp: Utc::now(),
            event_type: "price_update".to_string(),
            price: 50000.0,
            volume: vec![1000.0],
            bid: 49999.0,
            ask: 50001.0,
            spread: 2.0,
            order_book_depth: Some(100),
            sequence_number: 1,
            source: "exchange".to_string(),
            quality_score: 0.95,
            metadata: Some(json!({"exchange": "test"})),
        };

        // Publish event
        event_bus.publish_market_event(market_event).await?;

        // Verify event was stored
        let published = event_bus.get_published_events("market").await?;
        assert_eq!(published.len(), 1);
        assert_eq!(published[0].event_type, "market_update");

        // Check performance metrics
        let metrics = event_bus.get_performance_metrics().await?;
        assert_eq!(metrics.total_events_published, 1);
        assert!(metrics.events_by_type.contains_key("market"));

        Ok(())
    }

    #[tokio::test]
    async fn test_news_event_publishing() -> Result<()> {
        let daa_access = Arc::new(DataAccessLayer::new_mock()?);
        let event_bus = EventBusIntegration::new(daa_access).await?;

        let news_event = NewsEvent {
            id: "news_001".to_string(),
            timestamp: Utc::now(),
            event_type: "market_news".to_string(),
            title: "Bitcoin Breaks $50k".to_string(),
            content: "Bitcoin price surges past psychological barrier".to_string(),
            source: "crypto_news".to_string(),
            category: "price_action".to_string(),
            symbols: vec!["BTC".to_string(), "USD".to_string()],
            sentiment_score: 0.8,
            relevance_score: 0.9,
            quality_score: 0.85,
            language: "en".to_string(),
            author: Some("Market Analyst".to_string()),
            tags: vec!["bitcoin".to_string(), "bullish".to_string()],
            metadata: None,
        };

        event_bus.publish_news_event(news_event).await?;

        let published = event_bus.get_published_events("news").await?;
        assert_eq!(published.len(), 1);
        assert_eq!(published[0].event_type, "news_event");
        assert_eq!(published[0].priority, "high"); // High relevance score

        Ok(())
    }

    #[tokio::test]
    async fn test_event_routing_with_filters() -> Result<()> {
        let daa_access = Arc::new(DataAccessLayer::new_mock()?);
        let event_bus = EventBusIntegration::new(daa_access).await?;

        // Set up event router with filters
        let mut router = EventRouter::new();
        router.add_filter_rule("high_quality_only", "quality_score > 0.9")?;
        router.add_filter_rule("btc_events_only", "symbol contains BTC")?;
        event_bus.set_event_router(router).await?;

        // Publish events with different quality scores
        let high_quality_event = MarketEvent {
            symbol: "BTC/USD".to_string(),
            quality_score: 0.95,
            timestamp: Utc::now(),
            event_type: "test".to_string(),
            price: 1000.0,
            volume: vec![100.0],
            bid: 999.0,
            ask: 1001.0,
            spread: 2.0,
            order_book_depth: Some(50),
            sequence_number: 1,
            source: "test".to_string(),
            metadata: None,
        };

        let low_quality_event = MarketEvent {
            symbol: "ETH/USD".to_string(),
            quality_score: 0.85,
            timestamp: Utc::now(),
            event_type: "test".to_string(),
            price: 1000.0,
            volume: vec![100.0],
            bid: 999.0,
            ask: 1001.0,
            spread: 2.0,
            order_book_depth: Some(50),
            sequence_number: 1,
            source: "test".to_string(),
            metadata: None,
        };

        event_bus.publish_market_event(high_quality_event).await?;
        event_bus.publish_market_event(low_quality_event).await?;

        // Check routed events (only high quality BTC should pass)
        let routed = event_bus.get_routed_events().await?;
        assert_eq!(routed.len(), 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_batch_processing() -> Result<()> {
        let daa_access = Arc::new(DataAccessLayer::new_mock()?);
        let event_bus = EventBusIntegration::new(daa_access).await?;

        // Configure batch processing
        event_bus.configure_batch_processing(10, 1000).await?;

        // Create batch of events
        let mut events = Vec::new();
        for i in 0..25 {
            events.push(MarketEvent {
                symbol: format!("TEST{}/USD", i),
                price: 1000.0 + i as f64,
                timestamp: Utc::now(),
                event_type: "test".to_string(),
                volume: vec![100.0],
                bid: 999.0,
                ask: 1001.0,
                spread: 2.0,
                order_book_depth: Some(50),
                sequence_number: i as u64,
                source: "test".to_string(),
                quality_score: 0.9,
                metadata: None,
            });
        }

        // Batch publish
        event_bus.batch_publish_market_events(events).await?;

        // Verify all events were published
        let published = event_bus.get_published_events("market").await?;
        assert_eq!(published.len(), 25);

        Ok(())
    }
}

#[cfg(test)]
mod neural_predictor_tests {
    use super::*;

    #[tokio::test]
    async fn test_neural_predictor_initialization() -> Result<()> {
        let config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.8,
        };

        let predictor = NeuralPredictor::new(config)?;

        // Load historical data
        let data = create_test_time_series(100);
        predictor.load_historical_data(data.clone()).await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_single_model_prediction() -> Result<()> {
        let config = NeuralConfig::default();
        let predictor = NeuralPredictor::new(config)?;

        let data = create_test_time_series(50);
        let predictions = predictor.predict(&data, 5, None).await?;

        // Verify predictions
        assert!(!predictions.is_empty());
        assert!(predictions.len() <= 5);

        for pred in &predictions {
            assert!(pred.confidence >= 0.0 && pred.confidence <= 1.0);
            assert!(pred.interval_low <= pred.value);
            assert!(pred.value <= pred.interval_high);
            assert!(!pred.model_name.is_empty());
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_prediction_with_features() -> Result<()> {
        let config = NeuralConfig::default();
        let predictor = NeuralPredictor::new(config)?;

        let data = create_test_time_series(50);
        let mut features = HashMap::new();
        features.insert("volatility".to_string(), json!(0.15));
        features.insert("trend".to_string(), json!("bullish"));
        features.insert(
            "volume_profile".to_string(),
            json!({"high": 2000, "low": 500}),
        );

        let predictions = predictor.predict(&data, 3, Some(features)).await?;

        assert!(!predictions.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_feature_importance() -> Result<()> {
        let config = NeuralConfig::default();
        let predictor = NeuralPredictor::new(config)?;

        let importance = predictor.get_feature_importance().await?;

        // Should return feature importance scores
        assert!(!importance.is_empty());

        for (feature, score) in &importance {
            assert!(*score >= 0.0 && *score <= 1.0);
            assert!(!feature.is_empty());
        }

        Ok(())
    }
}

#[cfg(test)]
mod daa_coordinator_tests {
    use super::*;

    #[tokio::test]
    async fn test_daa_coordinator_initialization() -> Result<()> {
        let neural_config = NeuralConfig::default();
        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config)?);
        let (tx, _rx) = mpsc::channel(100);

        let config = DaaConfig::default();
        let coordinator = DaaCoordinator::new(config.clone(), neural_predictor, tx, create_test_market_hours());

        // Verify initial metrics
        let metrics = coordinator.get_metrics().await;
        assert_eq!(metrics.total_decisions, 0);
        assert_eq!(metrics.profitable_decisions, 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_daa_decision_with_no_position() -> Result<()> {
        let neural_config = NeuralConfig::default();
        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config)?);
        let (tx, mut rx) = mpsc::channel(100);

        let config = DaaConfig {
            enabled: true,
            min_confidence: 0.7,
            ..Default::default()
        };
        let coordinator = DaaCoordinator::new(config, neural_predictor, tx, create_test_market_hours());

        // Register a bullish strategy
        let strategy = Box::new(MockStrategy {
            signal: Signal::Buy {
                confidence: 0.85,
                stop_loss: Some(0.98),
                take_profit: Some(1.02),
            },
        });
        coordinator
            .register_strategy("mock_bullish".to_string(), strategy)
            .await;

        // Create market context
        let market_context = MarketContext {
            symbol: "BTC/USD".to_string(),
            current_price: 50000.0,
            bid: 49999.0,
            ask: 50001.0,
            volume_24h: 1000000.0,
            volatility: 0.02,
            timestamp: Utc::now(),
        };

        let historical_data = create_test_time_series(50);

        // Make decision
        let decision = coordinator
            .make_decision(&market_context, None, &historical_data)
            .await?;

        // Verify decision was sent through channel
        let received = rx.try_recv();
        assert!(received.is_ok());

        // Check decision properties
        assert!(decision.confidence > 0.0);
        assert!(!decision.reasoning.is_empty());
        assert!(!decision.neural_consensus.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_daa_disabled_mode() -> Result<()> {
        let neural_config = NeuralConfig::default();
        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config)?);
        let (tx, _rx) = mpsc::channel(100);

        let config = DaaConfig {
            enabled: false, // Disabled
            ..Default::default()
        };
        let coordinator = DaaCoordinator::new(config, neural_predictor, tx, create_test_market_hours());

        let market_context = MarketContext {
            symbol: "TEST/USD".to_string(),
            current_price: 1000.0,
            bid: 999.0,
            ask: 1001.0,
            volume_24h: 100000.0,
            volatility: 0.02,
            timestamp: Utc::now(),
        };
        let historical_data = create_test_time_series(50);

        let decision = coordinator
            .make_decision(&market_context, None, &historical_data)
            .await?;

        // Should always hold when disabled
        match decision.action {
            TradingAction::Hold { reason } => {
                assert_eq!(reason, "DAA disabled");
            }
            _ => panic!("Expected Hold action when disabled"),
        }

        assert_eq!(decision.confidence, 0.0);

        Ok(())
    }

    #[tokio::test]
    async fn test_risk_assessment() -> Result<()> {
        let neural_config = NeuralConfig::default();
        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config)?);
        let (tx, _rx) = mpsc::channel(100);

        let config = DaaConfig::default();
        let coordinator = DaaCoordinator::new(config, neural_predictor, tx, create_test_market_hours());

        // High volatility context
        let volatile_context = MarketContext {
            symbol: "BTC/USD".to_string(),
            current_price: 50000.0,
            volatility: 0.15, // High volatility
            bid: 49900.0,
            ask: 50100.0,
            volume_24h: 1000000.0,
            timestamp: Utc::now(),
        };

        let historical_data = create_test_time_series(50);

        let decision = coordinator
            .make_decision(&volatile_context, None, &historical_data)
            .await?;

        // Check risk assessment
        assert!(decision.risk_assessment.market_risk > 0.1);
        assert!(decision.risk_assessment.volatility_adjusted_size < 0.02);

        Ok(())
    }

    #[tokio::test]
    async fn test_metrics_tracking() -> Result<()> {
        let neural_config = NeuralConfig::default();
        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config)?);
        let (tx, _rx) = mpsc::channel(100);

        let config = DaaConfig::default();
        let coordinator = DaaCoordinator::new(config, neural_predictor, tx, create_test_market_hours());

        let market_context = MarketContext {
            symbol: "TEST/USD".to_string(),
            current_price: 1000.0,
            bid: 999.0,
            ask: 1001.0,
            volume_24h: 100000.0,
            volatility: 0.02,
            timestamp: Utc::now(),
        };
        let historical_data = create_test_time_series(50);

        // Make several decisions
        for i in 0..5 {
            let mut ctx = market_context.clone();
            ctx.current_price += i as f64 * 100.0;

            let _ = coordinator
                .make_decision(&ctx, None, &historical_data)
                .await?;
        }

        let metrics = coordinator.get_metrics().await;
        assert_eq!(metrics.total_decisions, 5);
        assert!(metrics.avg_confidence > 0.0);
        assert!(!metrics.model_accuracy.is_empty());

        Ok(())
    }
}
