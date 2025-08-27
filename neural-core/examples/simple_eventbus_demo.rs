//! Simple Proto-Only EventBus Demo
//!
//! This example demonstrates the new proto-only EventBus implementation
//! using strongly-typed protobuf messages instead of JSON or raw bytes.

use neural_core::eventbus::{
    implementations::inmemory::ProtoInMemoryEventBus,
    traits::proto_event_bus::{ProtoEventBus, ProtoEventBusConfig},
    types::{ProtoEvent, ProtoMessage},
    proto_messages::MarketDataEvent,
    error::EventBusError,
};
use prost::Message;
use tokio;

// Define a simple demo proto message
#[derive(Clone, PartialEq, Message)]
pub struct DemoMessage {
    #[prost(string, tag = "1")]
    pub message: String,
    #[prost(string, tag = "2")]
    pub symbol: String,
    #[prost(double, tag = "3")]
    pub price: f64,
    #[prost(int64, tag = "4")]
    pub timestamp: i64,
}

impl ProtoMessage for DemoMessage {
    fn proto_type_name() -> &'static str {
        "demo.DemoMessage"
    }

    fn validate(&self) -> Result<(), EventBusError> {
        if self.message.is_empty() {
            return Err(EventBusError::ValidationError(
                "Message cannot be empty".to_string()
            ));
        }
        if self.price <= 0.0 {
            return Err(EventBusError::ValidationError(
                "Price must be positive".to_string()
            ));
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Proto-Only EventBus Simple Demonstration");
    println!("============================================");

    // Create proto-only EventBus with configuration
    let config = ProtoEventBusConfig::default()
        .register_proto_type::<DemoMessage>()
        .register_proto_type::<MarketDataEvent>();
        
    let event_bus = ProtoInMemoryEventBus::with_config(config);
    println!("✅ Created ProtoInMemoryEventBus with proto-only enforcement");

    // Create a proto message - NO JSON or Vec<u8> allowed!
    let demo_message = DemoMessage {
        message: "Hello Proto EventBus!".to_string(),
        symbol: "AAPL".to_string(),
        price: 150.00,
        timestamp: chrono::Utc::now().timestamp(),
    };

    // Wrap in ProtoEvent
    let proto_event = ProtoEvent::new(demo_message.clone())
        .with_metadata("source".to_string(), "demo".to_string())
        .with_metadata("priority".to_string(), "high".to_string())
        .with_quality_score(0.95);

    println!("📦 Created proto event: {}", proto_event.event_type);
    println!("   Message: {}", demo_message.message);
    println!("   Symbol: {}", demo_message.symbol);
    println!("   Price: ${:.2}", demo_message.price);
    println!("   Quality Score: {:.1}%", proto_event.quality_score * 100.0);
    println!("   Metadata: {:?}", proto_event.metadata);

    // Publish proto event to a channel
    println!("\n📡 Publishing proto event to channel...");
    let event_id = event_bus.publish_proto("market.demo", proto_event.clone()).await?;
    println!("✅ Published with Event ID: {}", event_id);

    // Create and publish market data event
    let market_data = MarketDataEvent::new_trade("AAPL", 150.25, 1000.0, "NASDAQ");
    let market_event = ProtoEvent::new(market_data)
        .with_metadata("exchange".to_string(), "NASDAQ".to_string())
        .with_quality_score(0.98);

    println!("\n📊 Publishing market data event...");
    let market_event_id = event_bus.publish_proto("market.data", market_event).await?;
    println!("✅ Published market data with ID: {}", market_event_id);

    // Get channel information
    let demo_channel_info = event_bus.get_channel_info("market.demo").await?;
    println!("\n🔍 Channel info for 'market.demo':");
    println!("   - Name: {}", demo_channel_info.name);
    println!("   - Event count: {}", demo_channel_info.event_count);
    println!("   - Subscriber count: {}", demo_channel_info.subscriber_count);
    println!("   - Last event ID: {:?}", demo_channel_info.last_event_id);

    let market_channel_info = event_bus.get_channel_info("market.data").await?;
    println!("\n🔍 Channel info for 'market.data':");
    println!("   - Name: {}", market_channel_info.name);
    println!("   - Event count: {}", market_channel_info.event_count);
    println!("   - Subscriber count: {}", market_channel_info.subscriber_count);
    println!("   - Last event ID: {:?}", market_channel_info.last_event_id);

    println!("\n✅ Proto-only EventBus demonstration completed!");
    println!("📋 Key Features Demonstrated:");
    println!("   ✓ Proto-only event creation (NO Vec<u8> or JSON)");
    println!("   ✓ Type-safe ProtoEvent<T> publishing");
    println!("   ✓ Channel management with proto events");
    println!("   ✓ Event metadata and quality scoring");
    println!("   ✓ Multiple proto message types");
    println!("   ✓ Validation enforcement");
    
    println!("\n⚠️  IMPORTANT: This EventBus ONLY accepts protobuf messages!");
    println!("   - Vec<u8> payloads: BLOCKED ❌");
    println!("   - JSON payloads: BLOCKED ❌");
    println!("   - Only ProtoEvent<T>: ACCEPTED ✅");

    Ok(())
}