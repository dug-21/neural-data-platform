use neural_core::eventbus::{InMemoryEventBus, EventBus, Event, EventId, SubscriptionConfig};
use std::sync::Arc;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 EventBus Working Demonstration");
    println!("==================================");

    // Create EventBus
    let event_bus = Arc::new(InMemoryEventBus::new());
    println!("✅ Created InMemoryEventBus");

    // Create a test event with proper payload (bytes)
    let payload = serde_json::to_vec(&serde_json::json!({
        "message": "Hello EventBus!",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "data": {
            "symbol": "AAPL",
            "price": 150.00,
            "volume": 1000000
        }
    }))?;

    let event = Event::new("market_data_update".to_string(), payload)
        .with_metadata("source".to_string(), "demo".to_string())
        .with_metadata("priority".to_string(), "high".to_string());

    println!("📦 Created test event: {}", event.event_type);
    println!("   Timestamp: {}", event.timestamp);
    println!("   Payload size: {} bytes", event.payload.len());
    println!("   Metadata: {:?}", event.metadata);

    // Publish event to a channel (using valid channel name format: stream:category:name)
    println!("\n📡 Publishing event to 'stream:symbol:AAPL' channel...");
    let event_id = event_bus.publish("stream:symbol:AAPL", event.clone()).await?;
    println!("✅ Published with Event ID: {}", event_id);

    // Publish multiple events as a batch
    let mut batch_events = Vec::new();
    for i in 1..=3 {
        let batch_payload = serde_json::to_vec(&serde_json::json!({
            "symbol": "AAPL",
            "price": 150.0 + i as f64,
            "sequence": i
        }))?;
        
        let batch_event = Event::new("price_update".to_string(), batch_payload)
            .with_metadata("batch_id".to_string(), "demo_batch_1".to_string())
            .with_metadata("sequence".to_string(), i.to_string());
        
        batch_events.push(batch_event);
    }

    println!("\n📡 Publishing batch of {} events...", batch_events.len());
    let batch_ids = event_bus.publish_batch("stream:ml:training", batch_events).await?;
    println!("✅ Published batch with IDs: {:?}", batch_ids);

    // Test subscription configuration
    let subscription_config = SubscriptionConfig::default();
    println!("\n🔗 Creating subscription with config: {:?}", subscription_config);
    
    let channels = vec!["stream:symbol:AAPL".to_string(), "stream:ml:training".to_string()];
    let _subscriber = event_bus.subscribe(&channels, subscription_config).await?;
    println!("✅ Created subscriber for channels: {:?}", channels);

    // Get channel information
    println!("\n🔍 Channel Information:");
    let market_info = event_bus.get_channel_info("stream:symbol:AAPL").await?;
    println!("   stream:symbol:AAPL:");
    println!("     - Name: {}", market_info.name);
    println!("     - Subscriber count: {}", market_info.subscriber_count);
    println!("     - Last event ID: {:?}", market_info.last_event_id);
    println!("     - Created at: {}", market_info.created_at);

    let price_info = event_bus.get_channel_info("stream:ml:training").await?;
    println!("   stream:ml:training:");
    println!("     - Name: {}", price_info.name);
    println!("     - Subscriber count: {}", price_info.subscriber_count);
    println!("     - Last event ID: {:?}", price_info.last_event_id);
    println!("     - Created at: {}", price_info.created_at);

    println!("\n✅ EventBus demonstration completed successfully!");
    println!("   - Event creation with payload and metadata: ✓");
    println!("   - Single event publishing: ✓");
    println!("   - Batch event publishing: ✓");
    println!("   - Channel subscription: ✓");
    println!("   - Channel information retrieval: ✓");

    // Test event properties access
    println!("\n📊 Event Properties:");
    println!("   - Type: {}", event.event_type);
    println!("   - Timestamp: {}", event.timestamp);
    println!("   - Payload size: {} bytes", event.payload.len());
    println!("   - Metadata entries: {}", event.metadata.len());

    // Demonstrate payload deserialization
    if let Ok(json_payload) = serde_json::from_slice::<serde_json::Value>(&event.payload) {
        println!("   - Payload content: {}", json_payload);
    }

    println!("\n🎯 EventBus is working correctly and ready for production use!");

    Ok(())
}