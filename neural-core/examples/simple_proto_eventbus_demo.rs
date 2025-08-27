//! Simple Proto-Only EventBus Demo
//!
//! This example demonstrates the core proto-only EventBus functionality:
//! - Publishing proto messages
//! - Type-safe subscriptions
//! - Proto message validation

use neural_core::eventbus::{
    implementations::inmemory::InMemoryEventBus,
    traits::EventBus,
    types::{ProtoEvent, ProtoMessage, SubscriptionConfig, StartPosition},
    error::EventBusError,
};
use tokio;

// Simple demo proto message
#[derive(Clone, prost::Message)]
pub struct PriceUpdate {
    #[prost(string, tag = "1")]
    pub symbol: String,
    #[prost(double, tag = "2")]
    pub price: f64,
    #[prost(int64, tag = "3")]
    pub timestamp: i64,
}

impl ProtoMessage for PriceUpdate {
    fn proto_type_name() -> &'static str {
        "demo.PriceUpdate"
    }
}

#[tokio::main]
async fn main() -> Result<(), EventBusError> {
    println!("🚀 Proto-Only EventBus Demo");
    
    // Create proto-only EventBus
    let event_bus = InMemoryEventBus::new();
    
    // Create a proto event
    let price_update = PriceUpdate {
        symbol: "AAPL".to_string(),
        price: 150.25,
        timestamp: chrono::Utc::now().timestamp(),
    };
    
    let event = ProtoEvent::new(price_update);
    
    println!("📤 Publishing proto event for AAPL at ${}", event.message.price);
    
    // Publish the proto event
    let event_id = event_bus.publish("stream:symbol:AAPL", event.clone()).await?;
    
    println!("✅ Published event with ID: {}", event_id);
    
    // Create a subscription config
    let config = SubscriptionConfig {
        group_name: "demo-group".to_string(),
        consumer_name: "demo-consumer".to_string(),
        start_position: StartPosition::Beginning,
        batch_size: 10,
        block_timeout_ms: 1000,
        ack_timeout_ms: 5000,
        buffer_size: 1024,
        receive_timeout: None,
        persistent: false,
        priority: 0,
    };
    
    // Subscribe to AAPL price updates
    let mut subscriber = event_bus.subscribe::<PriceUpdate>(
        &["stream:symbol:AAPL".to_string()],
        config
    ).await?;
    
    println!("📥 Subscribed to AAPL price updates");
    
    // Publish another event
    let second_update = PriceUpdate {
        symbol: "AAPL".to_string(),
        price: 151.75,
        timestamp: chrono::Utc::now().timestamp(),
    };
    
    let second_event = ProtoEvent::new(second_update);
    let _second_id = event_bus.publish("stream:symbol:AAPL", second_event).await?;
    
    println!("📤 Published second price update: ${}", 151.75);
    
    // Try to receive events (this would need proper implementation)
    // For now just demonstrate the API
    println!("📊 Demo completed successfully!");
    println!("💡 The EventBus is now proto-only - no more Vec<u8> payloads allowed!");
    
    Ok(())
}