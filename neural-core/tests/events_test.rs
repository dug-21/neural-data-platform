//! Integration tests for event system
//! Test actual event flow and processing

use neural_core::events::{
    InMemoryEventBus, EventBus, Event, PriceUpdateEvent, VolumeEvent, 
    TrendChangeEvent, ModelUpdateEvent
};
use neural_core::types::{Prediction, market::MarketTrend};
use neural_core::events::prediction_events::{ModelUpdateType, ModelPredictionEvent};
use std::sync::Arc;
use futures::StreamExt;

#[tokio::test]
async fn test_event_bus_integration() {
    let bus = InMemoryEventBus::new();
    
    // Subscribe to price updates to create the channel
    let _handle = bus.subscribe("price_update").await.unwrap();
    
    // Get stream first (this will create receiver)
    let mut stream = bus.get_stream("price_update").await.unwrap();
    
    // Test price update event
    let price_event = Arc::new(PriceUpdateEvent::new(
        "AAPL".to_string(),
        155.0,
        150.0,
    ).with_volume(1000000));
    
    // Publish event
    bus.publish(price_event.clone()).await.unwrap();
    
    // Publish another event to test stream
    let price_event2 = Arc::new(PriceUpdateEvent::new(
        "GOOGL".to_string(),
        2500.0,
        2480.0,
    ));
    
    bus.publish(price_event2).await.unwrap();
    
    // Should receive the event from stream
    if let Some(received_event) = stream.next().await {
        assert_eq!(received_event.event_type(), "price_update");
    }
}

#[tokio::test]
async fn test_multiple_event_types() {
    let bus = InMemoryEventBus::new();
    
    // Create streams (which create receivers) for each event type
    let _price_stream = bus.get_stream("price_update").await.unwrap();
    let _volume_stream = bus.get_stream("volume_event").await.unwrap();
    let _trend_stream = bus.get_stream("trend_change").await.unwrap();
    
    // Create different event types
    let price_event = Arc::new(PriceUpdateEvent::new("AAPL".to_string(), 155.0, 150.0));
    let volume_event = Arc::new(VolumeEvent::new("AAPL".to_string(), 2000000, 1000000));
    let trend_event = Arc::new(TrendChangeEvent::new(
        "AAPL".to_string(),
        MarketTrend::Neutral,
        MarketTrend::Bullish,
        0.85
    ));
    
    // Publish all events
    bus.publish(price_event).await.unwrap();
    bus.publish(volume_event).await.unwrap();
    bus.publish(trend_event).await.unwrap();
    
    // Verify we can subscribe to each type
    let _price_handle = bus.subscribe("price_update").await.unwrap();
    let _volume_handle = bus.subscribe("volume_event").await.unwrap();
    let _trend_handle = bus.subscribe("trend_change").await.unwrap();
}

#[tokio::test]
async fn test_prediction_events() {
    let bus = InMemoryEventBus::new();
    
    // Create streams for prediction events
    let _pred_stream = bus.get_stream("model_prediction").await.unwrap();
    let _update_stream = bus.get_stream("model_update").await.unwrap();
    
    let prediction = Prediction::new(155.0, 0.85);
    let prediction_event = Arc::new(ModelPredictionEvent::new(
        "lstm_v1".to_string(),
        "AAPL".to_string(),
        prediction
    ));
    
    let update_event = Arc::new(ModelUpdateEvent::new(
        "lstm_v1".to_string(),
        ModelUpdateType::Deploy,
        "2.0.0".to_string()
    ));
    
    // Test publishing prediction events
    bus.publish(prediction_event).await.unwrap();
    bus.publish(update_event).await.unwrap();
    
    let _pred_handle = bus.subscribe("model_prediction").await.unwrap();
    let _update_handle = bus.subscribe("model_update").await.unwrap();
}

#[tokio::test]
async fn test_event_priorities() {
    let price_event = PriceUpdateEvent::new("AAPL".to_string(), 155.0, 150.0);
    let volume_spike = VolumeEvent::new("AAPL".to_string(), 5000000, 1000000); // 5x volume spike
    let normal_volume = VolumeEvent::new("AAPL".to_string(), 1000000, 1000000);
    
    // Check priorities
    assert_eq!(price_event.priority(), 8);
    assert_eq!(volume_spike.priority(), 9); // Should be higher due to spike
    assert_eq!(normal_volume.priority(), 6);
}

#[tokio::test]
async fn test_event_serialization() {
    let prediction = Prediction::new(155.0, 0.85);
    let event = ModelPredictionEvent::new(
        "lstm_v1".to_string(),
        "AAPL".to_string(),
        prediction
    );
    
    // Test JSON serialization
    let json_value = event.to_json();
    assert!(json_value.is_object());
    assert!(json_value.get("symbol").is_some());
    assert!(json_value.get("base").is_some());
}

#[tokio::test]
async fn test_concurrent_event_publishing() {
    let bus = Arc::new(InMemoryEventBus::new());
    
    // Create stream to keep channel open
    let _stream = bus.get_stream("price_update").await.unwrap();
    
    // Spawn multiple tasks publishing events concurrently
    let mut handles = Vec::new();
    
    for i in 0..10 {
        let bus_clone = bus.clone();
        let handle = tokio::spawn(async move {
            let event = Arc::new(PriceUpdateEvent::new(
                format!("STOCK{}", i),
                100.0 + i as f64,
                99.0 + i as f64,
            ));
            
            bus_clone.publish(event).await.unwrap();
        });
        handles.push(handle);
    }
    
    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }
    
    // All events should have been published successfully
    // (no panics or errors in the tasks above)
}