use autonomous_platform::integration::streaming::{
    StreamingPipeline, StreamEvent, MarketData, NewsData,
};
use autonomous_platform::data::DataPipeline;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;
use chrono::Utc;

#[tokio::test]
async fn test_streaming_pipeline_creation() {
    let data_pipeline = DataPipeline::new()
        .await
        .expect("Failed to create data pipeline");
    
    let streaming_pipeline = StreamingPipeline::new(data_pipeline)
        .await
        .expect("Failed to create streaming pipeline");
    
    assert!(streaming_pipeline.is_running() == false);
}

#[tokio::test]
async fn test_market_feed_ingestion() {
    let data_pipeline = DataPipeline::new()
        .await
        .expect("Failed to create data pipeline");
    
    let mut streaming_pipeline = StreamingPipeline::new(data_pipeline)
        .await
        .expect("Failed to create streaming pipeline");
    
    // Start market stream for specific symbols
    let symbols = vec!["AAPL".to_string(), "GOOGL".to_string()];
    streaming_pipeline.start_market_stream(symbols.clone())
        .await
        .expect("Failed to start market stream");
    
    // Simulate market data
    let market_data = MarketData {
        symbol: "AAPL".to_string(),
        timestamp: Utc::now(),
        open: 180.0,
        high: 182.5,
        low: 179.8,
        close: 181.2,
        volume: 1_500_000,
        bid: 181.1,
        ask: 181.3,
        bid_size: 100,
        ask_size: 150,
    };
    
    // Process the data
    let event = streaming_pipeline.process_market_data(market_data.clone())
        .await
        .expect("Failed to process market data");
    
    assert_eq!(event.symbol(), "AAPL");
    assert!(event.quality_score() > 0.0);
}

#[tokio::test]
async fn test_news_feed_ingestion() {
    let data_pipeline = DataPipeline::new()
        .await
        .expect("Failed to create data pipeline");
    
    let mut streaming_pipeline = StreamingPipeline::new(data_pipeline)
        .await
        .expect("Failed to create streaming pipeline");
    
    // Start news stream for specific topics
    let topics = vec!["earnings".to_string(), "economic-indicators".to_string()];
    streaming_pipeline.start_news_stream(topics.clone())
        .await
        .expect("Failed to start news stream");
    
    // Simulate news data
    let news_data = NewsData {
        id: "news-001".to_string(),
        timestamp: Utc::now(),
        title: "Apple Reports Strong Q4 Earnings".to_string(),
        content: "Apple Inc. reported better-than-expected earnings...".to_string(),
        source: "Reuters".to_string(),
        symbols: vec!["AAPL".to_string()],
        sentiment_score: 0.85,
        relevance_score: 0.95,
        categories: vec!["earnings".to_string(), "technology".to_string()],
    };
    
    // Process the data
    let event = streaming_pipeline.process_news_data(news_data.clone())
        .await
        .expect("Failed to process news data");
    
    assert_eq!(event.news_id(), "news-001");
    assert!(event.sentiment_score() > 0.0);
}

#[tokio::test]
async fn test_data_quality_processor() {
    let processor = DataQualityProcessor::new();
    
    // Test valid market data
    let valid_data = MarketData {
        symbol: "MSFT".to_string(),
        timestamp: Utc::now(),
        open: 350.0,
        high: 355.0,
        low: 349.0,
        close: 353.0,
        volume: 2_000_000,
        bid: 352.9,
        ask: 353.1,
        bid_size: 200,
        ask_size: 250,
    };
    
    let quality_report = processor.assess_market_data(&valid_data);
    assert!(quality_report.is_valid);
    assert_eq!(quality_report.quality_score, 1.0);
    assert!(quality_report.issues.is_empty());
    
    // Test invalid data (negative prices)
    let invalid_data = MarketData {
        symbol: "INVALID".to_string(),
        timestamp: Utc::now(),
        open: -10.0,
        high: 355.0,
        low: 349.0,
        close: 353.0,
        volume: 2_000_000,
        bid: 352.9,
        ask: 353.1,
        bid_size: 200,
        ask_size: 250,
    };
    
    let quality_report = processor.assess_market_data(&invalid_data);
    assert!(!quality_report.is_valid);
    assert!(quality_report.quality_score < 1.0);
    assert!(!quality_report.issues.is_empty());
}

#[tokio::test]
async fn test_data_normalizer() {
    let normalizer = DataNormalizer::new();
    
    // Test market data normalization
    let market_data = MarketData {
        symbol: "AAPL".to_string(),
        timestamp: Utc::now(),
        open: 180.0,
        high: 182.5,
        low: 179.8,
        close: 181.2,
        volume: 1_500_000,
        bid: 181.1,
        ask: 181.3,
        bid_size: 100,
        ask_size: 150,
    };
    
    let normalized = normalizer.normalize_market_data(&market_data)
        .expect("Failed to normalize market data");
    
    assert_eq!(normalized.symbol, "AAPL");
    assert!(normalized.normalized_volume > 0.0);
    assert!(normalized.spread_percentage >= 0.0);
}

#[tokio::test]
async fn test_event_bus_integration() {
    let data_pipeline = DataPipeline::new()
        .await
        .expect("Failed to create data pipeline");
    
    let mut streaming_pipeline = StreamingPipeline::new(data_pipeline)
        .await
        .expect("Failed to create streaming pipeline");
    
    // Subscribe to events
    let (tx, mut rx) = mpsc::channel(100);
    streaming_pipeline.subscribe_to_events(tx).await;
    
    // Process market data
    let market_data = MarketData {
        symbol: "TSLA".to_string(),
        timestamp: Utc::now(),
        open: 800.0,
        high: 820.0,
        low: 795.0,
        close: 815.0,
        volume: 3_000_000,
        bid: 814.9,
        ask: 815.1,
        bid_size: 50,
        ask_size: 75,
    };
    
    streaming_pipeline.process_market_data(market_data)
        .await
        .expect("Failed to process market data");
    
    // Verify event is published
    let event = timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("Timeout waiting for event")
        .expect("No event received");
    
    match event {
        StreamEvent::MarketDataProcessed { symbol, .. } => {
            assert_eq!(symbol, "TSLA");
        }
        _ => panic!("Unexpected event type"),
    }
}

#[tokio::test]
async fn test_anomaly_detection() {
    let processor = DataQualityProcessor::new();
    
    // Normal price movement
    let normal_data = MarketData {
        symbol: "AAPL".to_string(),
        timestamp: Utc::now(),
        open: 180.0,
        high: 182.0,
        low: 179.0,
        close: 181.0,
        volume: 1_500_000,
        bid: 180.9,
        ask: 181.1,
        bid_size: 100,
        ask_size: 150,
    };
    
    let report = processor.detect_anomalies(&normal_data, 180.0); // previous close
    assert!(report.anomalies.is_empty());
    
    // Anomalous price spike (>20% change)
    let anomalous_data = MarketData {
        symbol: "AAPL".to_string(),
        timestamp: Utc::now(),
        open: 180.0,
        high: 250.0, // Huge spike
        low: 179.0,
        close: 181.0,
        volume: 1_500_000,
        bid: 180.9,
        ask: 181.1,
        bid_size: 100,
        ask_size: 150,
    };
    
    let report = processor.detect_anomalies(&anomalous_data, 180.0);
    assert!(!report.anomalies.is_empty());
    assert!(report.anomalies.iter().any(|a| a.contains("price spike")));
}

#[tokio::test]
async fn test_missing_data_handling() {
    let processor = DataQualityProcessor::new();
    
    // Data with missing volume
    let incomplete_data = MarketData {
        symbol: "MSFT".to_string(),
        timestamp: Utc::now(),
        open: 350.0,
        high: 355.0,
        low: 349.0,
        close: 353.0,
        volume: 0, // Missing volume
        bid: 352.9,
        ask: 353.1,
        bid_size: 0, // Missing bid size
        ask_size: 250,
    };
    
    let report = processor.assess_market_data(&incomplete_data);
    assert!(report.issues.iter().any(|i| i.contains("volume")));
    assert!(report.issues.iter().any(|i| i.contains("bid_size")));
    assert!(report.quality_score < 1.0);
}

#[tokio::test]
async fn test_latency_monitoring() {
    let data_pipeline = DataPipeline::new()
        .await
        .expect("Failed to create data pipeline");
    
    let streaming_pipeline = StreamingPipeline::new(data_pipeline)
        .await
        .expect("Failed to create streaming pipeline");
    
    // Get latency metrics
    let metrics = streaming_pipeline.get_latency_metrics().await;
    
    assert!(metrics.avg_processing_time_ms >= 0.0);
    assert!(metrics.max_processing_time_ms >= 0.0);
    assert_eq!(metrics.processed_count, 0); // No data processed yet
}

#[tokio::test]
async fn test_batch_processing() {
    let data_pipeline = DataPipeline::new()
        .await
        .expect("Failed to create data pipeline");
    
    let mut streaming_pipeline = StreamingPipeline::new(data_pipeline)
        .await
        .expect("Failed to create streaming pipeline");
    
    // Create batch of market data
    let batch_data: Vec<MarketData> = vec![
        MarketData {
            symbol: "AAPL".to_string(),
            timestamp: Utc::now(),
            open: 180.0,
            high: 182.0,
            low: 179.0,
            close: 181.0,
            volume: 1_500_000,
            bid: 180.9,
            ask: 181.1,
            bid_size: 100,
            ask_size: 150,
        },
        MarketData {
            symbol: "GOOGL".to_string(),
            timestamp: Utc::now(),
            open: 2800.0,
            high: 2850.0,
            low: 2790.0,
            close: 2830.0,
            volume: 1_200_000,
            bid: 2829.9,
            ask: 2830.1,
            bid_size: 25,
            ask_size: 30,
        },
    ];
    
    // Process batch
    let results = streaming_pipeline.process_market_batch(batch_data)
        .await
        .expect("Failed to process batch");
    
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|r| r.quality_score > 0.0));
}

#[tokio::test]
async fn test_stream_lifecycle() {
    let data_pipeline = DataPipeline::new()
        .await
        .expect("Failed to create data pipeline");
    
    let mut streaming_pipeline = StreamingPipeline::new(data_pipeline)
        .await
        .expect("Failed to create streaming pipeline");
    
    // Start stream
    streaming_pipeline.start_market_stream(vec!["AAPL".to_string()])
        .await
        .expect("Failed to start stream");
    
    assert!(streaming_pipeline.is_running());
    
    // Pause stream
    streaming_pipeline.pause_stream()
        .await
        .expect("Failed to pause stream");
    
    assert!(!streaming_pipeline.is_running());
    assert!(streaming_pipeline.is_paused());
    
    // Resume stream
    streaming_pipeline.resume_stream()
        .await
        .expect("Failed to resume stream");
    
    assert!(streaming_pipeline.is_running());
    assert!(!streaming_pipeline.is_paused());
    
    // Stop stream
    streaming_pipeline.stop_stream()
        .await
        .expect("Failed to stop stream");
    
    assert!(!streaming_pipeline.is_running());
    assert!(!streaming_pipeline.is_paused());
}

// =================== ENHANCED INTEGRATION TESTS ===================

mod common;
use common::{
    create_realistic_market_data, create_high_volatility_market_data,
    MarketScenario, generate_time_series, assertions
};

#[tokio::test]
async fn test_market_volatility_response() {
    let data_pipeline = DataPipeline::new()
        .await
        .expect("Failed to create data pipeline");
    
    let mut streaming_pipeline = StreamingPipeline::new(data_pipeline)
        .await
        .expect("Failed to create streaming pipeline");
    
    // Test normal volatility
    let normal_data = create_realistic_market_data("BTC/USD", 45000.0, 0.02);
    let normal_event = streaming_pipeline.process_market_data(normal_data)
        .await
        .expect("Failed to process normal volatility data");
    
    assert!(normal_event.quality_score() > 0.9);
    assert_eq!(normal_event.volatility_level(), "NORMAL");
    
    // Test high volatility
    let high_vol_data = create_high_volatility_market_data("BTC/USD", 45000.0);
    let high_vol_event = streaming_pipeline.process_market_data(high_vol_data)
        .await
        .expect("Failed to process high volatility data");
    
    assert!(high_vol_event.quality_score() > 0.7); // Lower threshold for high volatility
    assert_eq!(high_vol_event.volatility_level(), "HIGH");
    assert!(high_vol_event.requires_immediate_attention());
}

#[tokio::test]
async fn test_flash_crash_scenario() {
    let data_pipeline = DataPipeline::new()
        .await
        .expect("Failed to create data pipeline");
    
    let mut streaming_pipeline = StreamingPipeline::new(data_pipeline)
        .await
        .expect("Failed to create streaming pipeline");
    
    // Simulate flash crash scenario
    let flash_crash_data = MarketScenario::FlashCrashRecovery.generate_data("ETH/USD", 3000.0);
    let crash_event = streaming_pipeline.process_market_data(flash_crash_data)
        .await
        .expect("Failed to process flash crash data");
    
    // Flash crash should trigger alerts
    assert!(crash_event.is_anomaly());
    assert!(crash_event.alert_level() >= AlertLevel::HIGH);
    assert!(crash_event.price_drop_percentage() > 10.0);
    
    // System should flag for immediate review
    assert!(crash_event.requires_human_review());
    assert!(crash_event.trading_halt_recommended());
}

#[tokio::test]
async fn test_multi_symbol_correlation_analysis() {
    let data_pipeline = DataPipeline::new()
        .await
        .expect("Failed to create data pipeline");
    
    let mut streaming_pipeline = StreamingPipeline::new(data_pipeline)
        .await
        .expect("Failed to create streaming pipeline");
    
    // Process correlated market data (crypto markets tend to move together)
    let symbols = vec!["BTC/USD", "ETH/USD", "ADA/USD"];
    let base_prices = vec![45000.0, 3000.0, 1.2];
    let correlation_factor = 0.8; // Strong positive correlation
    
    let mut events = Vec::new();
    
    for (symbol, base_price) in symbols.iter().zip(base_prices.iter()) {
        // All moving in same direction due to correlation
        let correlated_data = MarketScenario::TrendingUp.generate_data(symbol, *base_price);
        let event = streaming_pipeline.process_market_data(correlated_data)
            .await
            .expect("Failed to process correlated data");
        events.push(event);
    }
    
    // Analyze correlation
    let correlation_analysis = streaming_pipeline.analyze_market_correlation(&events).await;
    assert!(correlation_analysis.correlation_strength > 0.7);
    assert_eq!(correlation_analysis.trend_direction, "UPWARD");
    assert!(correlation_analysis.market_regime == "RISK_ON");
}

#[tokio::test]
async fn test_real_world_trading_session_simulation() {
    let data_pipeline = DataPipeline::new()
        .await
        .expect("Failed to create data pipeline");
    
    let mut streaming_pipeline = StreamingPipeline::new(data_pipeline)
        .await
        .expect("Failed to create streaming pipeline");
    
    // Simulate 24-hour trading session with varying conditions
    let trading_scenarios = vec![
        (MarketScenario::Sideways, "Asian Session - Low Volume"),
        (MarketScenario::TrendingUp, "European Session - Moderate Volume"),
        (MarketScenario::HighVolatility, "US Session - High Volume"),
        (MarketScenario::TrendingDown, "After Hours - Lower Volume"),
    ];
    
    let mut session_events = Vec::new();
    
    for (scenario, session_name) in trading_scenarios {
        let session_data = scenario.generate_data("BTC/USD", 45000.0);
        let mut event = streaming_pipeline.process_market_data(session_data)
            .await
            .expect("Failed to process session data");
        
        event.set_session_context(session_name);
        session_events.push(event);
    }
    
    // Analyze session performance
    let session_analysis = streaming_pipeline.analyze_trading_session(&session_events).await;
    assert!(session_analysis.total_volume > 0.0);
    assert!(session_analysis.price_range_percentage > 0.0);
    assert!(!session_analysis.session_breakdown.is_empty());
    
    // Verify session-specific behavior
    assert!(session_analysis.peak_volume_session == "US Session - High Volume");
    assert!(session_analysis.most_volatile_session == "US Session - High Volume");
}

#[tokio::test]
async fn test_high_frequency_data_processing() {
    let data_pipeline = DataPipeline::new()
        .await
        .expect("Failed to create data pipeline");
    
    let mut streaming_pipeline = StreamingPipeline::new(data_pipeline)
        .await
        .expect("Failed to create streaming pipeline");
    
    let start_time = std::time::Instant::now();
    let num_ticks = 1000;
    let mut processing_times = Vec::new();
    
    // Simulate high-frequency tick data
    for i in 0..num_ticks {
        let tick_start = std::time::Instant::now();
        
        // Create realistic tick data with microsecond precision
        let tick_data = create_streaming_tick_data("BTC/USD", 45000.0, i);
        let _event = streaming_pipeline.process_market_data(tick_data)
            .await
            .expect("Failed to process tick data");
        
        let tick_duration = tick_start.elapsed();
        processing_times.push(tick_duration.as_micros());
    }
    
    let total_duration = start_time.elapsed();
    let throughput = num_ticks as f64 / total_duration.as_secs_f64();
    
    // Performance assertions for high-frequency processing
    assert!(throughput > 100.0); // Should handle at least 100 ticks/second
    
    let avg_processing_time = processing_times.iter().sum::<u128>() / processing_times.len() as u128;
    assert!(avg_processing_time < 10000); // Under 10ms average per tick
    
    // P95 latency should be reasonable
    processing_times.sort();
    let p95_index = (processing_times.len() as f64 * 0.95) as usize;
    let p95_latency = processing_times[p95_index];
    assert!(p95_latency < 50000); // P95 under 50ms
}

#[tokio::test]
async fn test_market_microstructure_analysis() {
    let data_pipeline = DataPipeline::new()
        .await
        .expect("Failed to create data pipeline");
    
    let mut streaming_pipeline = StreamingPipeline::new(data_pipeline)
        .await
        .expect("Failed to create streaming pipeline");
    
    // Simulate order book depth and spread analysis
    let symbols = vec!["BTC/USD", "ETH/USD"];
    
    for symbol in &symbols {
        // Create market data with realistic order book
        let mut market_data = create_realistic_market_data(symbol, 45000.0, 0.02);
        market_data.bid_size = 150;
        market_data.ask_size = 200;
        market_data.bid = market_data.close - 5.0;
        market_data.ask = market_data.close + 5.0;
        
        let event = streaming_pipeline.process_market_data(market_data)
            .await
            .expect("Failed to process market microstructure data");
        
        // Analyze market microstructure
        let microstructure = event.get_microstructure_analysis();
        assert!(microstructure.spread_bps > 0.0);
        assert!(microstructure.order_book_depth > 0);
        assert!(microstructure.market_impact_estimate.is_some());
        
        // Liquidity analysis
        assert!(microstructure.liquidity_score > 0.0);
        assert!(microstructure.liquidity_score <= 1.0);
        
        if microstructure.liquidity_score < 0.5 {
            assert!(event.liquidity_warning_triggered());
        }
    }
}

#[tokio::test]
async fn test_cross_venue_arbitrage_detection() {
    let data_pipeline = DataPipeline::new()
        .await
        .expect("Failed to create data pipeline");
    
    let mut streaming_pipeline = StreamingPipeline::new(data_pipeline)
        .await
        .expect("Failed to create streaming pipeline");
    
    // Simulate price feeds from different exchanges
    let exchanges = vec!["Binance", "Coinbase", "Kraken"];
    let base_price = 45000.0;
    let mut exchange_events = Vec::new();
    
    for (i, exchange) in exchanges.iter().enumerate() {
        // Create price discrepancy between exchanges
        let price_offset = (i as f64 - 1.0) * 50.0; // -50, 0, +50 price difference
        let mut exchange_data = create_realistic_market_data("BTC/USD", base_price + price_offset, 0.01);
        exchange_data.source = exchange.to_string();
        
        let event = streaming_pipeline.process_market_data(exchange_data)
            .await
            .expect("Failed to process exchange data");
        
        exchange_events.push(event);
    }
    
    // Analyze arbitrage opportunities
    let arbitrage_analysis = streaming_pipeline.detect_arbitrage_opportunities(&exchange_events).await;
    
    assert!(arbitrage_analysis.opportunities_detected > 0);
    assert!(arbitrage_analysis.max_spread_bps > 10.0); // Should detect the 100 USD spread
    assert!(!arbitrage_analysis.profitable_pairs.is_empty());
    
    // Verify specific arbitrage opportunity
    let best_opportunity = &arbitrage_analysis.profitable_pairs[0];
    assert_eq!(best_opportunity.buy_exchange, "Binance"); // Lowest price
    assert_eq!(best_opportunity.sell_exchange, "Kraken"); // Highest price
    assert!(best_opportunity.profit_bps > 20.0);
}

#[tokio::test]
async fn test_news_sentiment_market_impact() {
    let data_pipeline = DataPipeline::new()
        .await
        .expect("Failed to create data pipeline");
    
    let mut streaming_pipeline = StreamingPipeline::new(data_pipeline)
        .await
        .expect("Failed to create streaming pipeline");
    
    // Process negative news
    let negative_news = NewsData {
        id: "negative_news_001".to_string(),
        timestamp: Utc::now(),
        title: "Major Exchange Hack Reported".to_string(),
        content: "Security breach affects millions of users and trading halted".to_string(),
        source: "CryptoNews".to_string(),
        symbols: vec!["BTC/USD".to_string(), "ETH/USD".to_string()],
        sentiment_score: -0.8, // Very negative
        relevance_score: 0.95,
        categories: vec!["security".to_string(), "exchange".to_string()],
    };
    
    let news_event = streaming_pipeline.process_news_data(negative_news)
        .await
        .expect("Failed to process negative news");
    
    // Simulate market reaction to negative news
    let post_news_market_data = MarketScenario::TrendingDown.generate_data("BTC/USD", 45000.0);
    let market_event = streaming_pipeline.process_market_data(post_news_market_data)
        .await
        .expect("Failed to process post-news market data");
    
    // Analyze news-market correlation
    let impact_analysis = streaming_pipeline.analyze_news_market_impact(&news_event, &market_event).await;
    
    assert!(impact_analysis.correlation_strength > 0.7);
    assert_eq!(impact_analysis.market_reaction, "NEGATIVE");
    assert!(impact_analysis.reaction_time_minutes < 30); // Quick market reaction
    assert!(impact_analysis.price_impact_percentage > 2.0);
}

#[tokio::test]
async fn test_streaming_backpressure_handling() {
    let data_pipeline = DataPipeline::new()
        .await
        .expect("Failed to create data pipeline");
    
    let mut streaming_pipeline = StreamingPipeline::new(data_pipeline)
        .await
        .expect("Failed to create streaming pipeline");
    
    // Configure backpressure monitoring
    streaming_pipeline.set_backpressure_threshold(100).await;
    
    let mut handles = Vec::new();
    let overload_factor = 5; // Send 5x normal capacity
    
    // Simulate overwhelming data flow
    for i in 0..(streaming_pipeline.capacity() * overload_factor) {
        let pipeline_clone = streaming_pipeline.clone();
        let handle = tokio::spawn(async move {
            let overload_data = create_realistic_market_data(
                &format!("OVERLOAD{}/USD", i % 100), 
                1000.0 + (i as f64)
            );
            pipeline_clone.process_market_data(overload_data).await
        });
        handles.push(handle);
    }
    
    // Wait for processing
    let mut successful = 0;
    let mut backpressure_triggered = 0;
    
    for handle in handles {
        match handle.await {
            Ok(Ok(_)) => successful += 1,
            Ok(Err(e)) if e.to_string().contains("backpressure") => backpressure_triggered += 1,
            _ => {}
        }
    }
    
    // Verify backpressure mechanism worked
    assert!(backpressure_triggered > 0, "Backpressure should have been triggered");
    assert!(successful > 0, "Some messages should still be processed");
    
    let backpressure_metrics = streaming_pipeline.get_backpressure_metrics().await;
    assert!(backpressure_metrics.triggered_count > 0);
    assert!(backpressure_metrics.recovery_time_ms > 0.0);
}

#[tokio::test]
async fn test_streaming_circuit_breaker() {
    let data_pipeline = DataPipeline::new()
        .await
        .expect("Failed to create data pipeline");
    
    let mut streaming_pipeline = StreamingPipeline::new(data_pipeline)
        .await
        .expect("Failed to create streaming pipeline");
    
    // Configure circuit breaker
    streaming_pipeline.set_circuit_breaker_threshold(0.5, Duration::from_secs(10)).await; // 50% error rate
    
    // Send data that will cause errors (invalid data)
    let error_count = 20;
    let success_count = 5;
    
    // Send failing requests
    for i in 0..error_count {
        let invalid_data = MarketData {
            symbol: "".to_string(), // Invalid empty symbol
            timestamp: Utc::now(),
            price: -100.0, // Invalid negative price
            volume: -50.0, // Invalid negative volume
            bid: 0.0,
            ask: 0.0,
            source: "error_test".to_string(),
            sequence_number: i,
            order_book_depth: None,
            metadata: None,
        };
        
        let _ = streaming_pipeline.process_market_data(invalid_data).await; // Expect failures
    }
    
    // Send some successful requests
    for _ in 0..success_count {
        let valid_data = create_realistic_market_data("BTC/USD", 45000.0, 0.02);
        let _ = streaming_pipeline.process_market_data(valid_data).await;
    }
    
    // Check if circuit breaker activated
    let circuit_state = streaming_pipeline.get_circuit_breaker_state().await;
    assert_eq!(circuit_state.state, "OPEN"); // Circuit should be open due to high error rate
    assert!(circuit_state.error_rate > 0.5);
    assert!(circuit_state.last_failure_time.is_some());
    
    // Verify circuit breaker prevents further processing
    let test_data = create_realistic_market_data("ETH/USD", 3000.0, 0.02);
    let result = streaming_pipeline.process_market_data(test_data).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("circuit breaker"));
}

// Helper function for creating streaming tick data
fn create_streaming_tick_data(symbol: &str, base_price: f64, sequence: u64) -> MarketData {
    let microsecond_variation = (sequence % 1000) as f64 * 0.01;
    MarketData {
        symbol: symbol.to_string(),
        timestamp: Utc::now(),
        price: base_price + microsecond_variation,
        volume: 100.0 + (sequence % 50) as f64,
        bid: base_price + microsecond_variation - 0.5,
        ask: base_price + microsecond_variation + 0.5,
        source: "high_frequency_feed".to_string(),
        sequence_number: sequence,
        order_book_depth: Some(((sequence % 20) + 5) as i32),
        metadata: Some(json!({
            "tick_type": "trade",
            "latency_us": sequence % 1000,
            "venue": "primary_exchange"
        })),
    }
}