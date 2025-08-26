use neural_core::eventbus::{InMemoryEventBus, EventBus, Event};
use std::sync::Arc;
use std::collections::HashMap;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 EventBus Simple Demonstration");
    println!("=================================");

    // Create EventBus
    let event_bus = Arc::new(InMemoryEventBus::new());
    println!("✅ Created InMemoryEventBus");

    // Create a test event
    let event = Event::new(
        "test_event".to_string(),
        serde_json::json!({
            "message": "Hello EventBus!",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "data": {
                "symbol": "AAPL",
                "price": 150.00
            }
        })
    );

    println!("📦 Created test event: {}", event.event_type());
    println!("   Event ID: {}", event.event_id());
    println!("   Event data: {:?}", event.data());

    // Test basic event bus operations
    let channels = event_bus.list_channels().await?;
    println!("📋 Available channels before publishing: {}", channels.len());

    // Publish event to a channel
    println!("📡 Publishing event to 'test_channel'...");
    event_bus.publish("test_channel", event.clone()).await?;
    
    // Check channels after publishing
    let channels_after = event_bus.list_channels().await?;
    println!("📋 Available channels after publishing: {}", channels_after.len());
    
    for channel in &channels_after {
        println!("   - Channel: {} (subscribers: {})", 
                channel.name, channel.subscriber_count);
    }

    // Get channel info
    if let Some(info) = event_bus.channel_info("test_channel").await? {
        println!("🔍 Channel info for 'test_channel':");
        println!("   - Name: {}", info.name);
        println!("   - Subscribers: {}", info.subscriber_count);
        println!("   - Last message ID: {:?}", info.last_message_id);
        println!("   - Created: {:?}", info.created_at);
    }

    println!("\n✅ EventBus simple demonstration completed!");
    println!("   - Event creation: ✓");
    println!("   - Event publishing: ✓");
    println!("   - Channel management: ✓");
    println!("   - Channel information: ✓");

    // Test event properties
    println!("\n📊 Event Properties Test:");
    println!("   - Event type: {}", event.event_type());
    println!("   - Event ID: {}", event.event_id());
    println!("   - Event timestamp: {}", event.timestamp());
    println!("   - Event data: {}", event.data());

    // Test event cloning
    let cloned_event = event.clone();
    println!("   - Cloning works: {}", event.event_id() == cloned_event.event_id());

    Ok(())
}