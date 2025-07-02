use neural_trader::streaming::{
    StreamingPipeline, MarketFeedIngester, NewsFeedIngester,
    DataQualityProcessor, DataNormalizer, FeedType, StreamEvent,
    MarketData, NewsData, QualityReport,
};
use neural_trader::data::DataPipeline;
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