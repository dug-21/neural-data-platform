//! Proto-only Event System Demo
//! 
//! This example demonstrates the new proto-only Event system implementation
//! that replaces ALL legacy Event structs with Vec<u8> payloads.

use neural_core::events::{Event, EventBus, InMemoryEventBus};
use prost::Message;

// Sample proto message
#[derive(Clone, prost::Message)]
struct MarketDataMessage {
    #[prost(string, tag = "1")]
    symbol: String,
    #[prost(double, tag = "2")]
    price: f64,
    #[prost(int64, tag = "3")]
    volume: i64,
    #[prost(string, tag = "4")]
    exchange: String,
}

#[derive(Clone, prost::Message)]
struct TradingSignalMessage {
    #[prost(string, tag = "1")]
    signal_id: String,
    #[prost(string, tag = "2")]
    symbol: String,
    #[prost(enumeration = "SignalType", tag = "3")]
    signal_type: i32,
    #[prost(double, tag = "4")]
    confidence: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
enum SignalType {
    Unknown = 0,
    Buy = 1,
    Sell = 2,
    Hold = 3,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Proto-only Event System Demo");
    println!("=================================");
    
    // Create the proto-only event bus
    let event_bus = InMemoryEventBus::new();
    
    println!("\n✅ Created InMemoryEventBus with proto-only support");
    
    // 1. Create and publish market data events
    let market_data = MarketDataMessage {
        symbol: "AAPL".to_string(),
        price: 175.50,
        volume: 1000000,
        exchange: "NASDAQ".to_string(),
    };
    
    let market_event = Event::new(
        "neural_trader.market_data.v1.MarketData",
        market_data.clone(),
        "market-data-service",
        "trading"
    )?
    .with_correlation_id("trade-session-123")
    .with_header("priority", "high")
    .with_header("region", "us-east")
    .with_routing("market.data.realtime", "AAPL", 9)
    .with_quality(100.0, 98.5);
    
    println!("\n📊 Created Market Data Event:");
    println!("   Event Type: {}", market_event.event_type());
    println!("   Message ID: {}", market_event.message_id());
    println!("   Symbol: {}", market_data.symbol);
    println!("   Price: ${:.2}", market_data.price);
    println!("   Volume: {}", market_data.volume);
    println!("   Quality Score: {:.1}%", market_event.quality_score());
    
    // Publish the market data event
    event_bus.publish(market_event.clone()).await?;
    println!("   ✅ Published to EventBus");
    
    // 2. Create and publish trading signal events
    let trading_signal = TradingSignalMessage {
        signal_id: "signal-456".to_string(),
        symbol: "AAPL".to_string(),
        signal_type: SignalType::Buy as i32,
        confidence: 0.87,
    };
    
    let signal_event = Event::new(
        "neural_trader.signals.v1.TradingSignal",
        trading_signal.clone(),
        "ml-prediction-service",
        "signals"
    )?
    .with_correlation_id("trade-session-123") // Same correlation ID
    .with_header("model_version", "v2.1.3")
    .with_header("confidence_level", "high")
    .with_routing("signals.ml.predictions", "AAPL", 8)
    .with_quality(95.0, 87.0);
    
    println!("\n🤖 Created Trading Signal Event:");
    println!("   Event Type: {}", signal_event.event_type());
    println!("   Signal ID: {}", trading_signal.signal_id);
    println!("   Symbol: {}", trading_signal.symbol);
    println!("   Signal: {:?}", SignalType::try_from(trading_signal.signal_type).unwrap_or(SignalType::Unknown));
    println!("   Confidence: {:.1}%", trading_signal.confidence * 100.0);
    println!("   Correlation ID: {}", signal_event.correlation_id());
    
    // Publish the trading signal event
    event_bus.publish(signal_event.clone()).await?;
    println!("   ✅ Published to EventBus");
    
    // 3. Demonstrate event serialization/deserialization
    println!("\n🔄 Testing Event Serialization:");
    let event_bytes = market_event.to_bytes();
    println!("   Serialized size: {} bytes", event_bytes.len());
    
    let recovered_event = Event::from_bytes(&event_bytes)?;
    println!("   ✅ Successfully deserialized event");
    println!("   Recovered Event Type: {}", recovered_event.event_type());
    println!("   Recovered Message ID: {}", recovered_event.message_id());
    
    // 4. Demonstrate payload extraction
    println!("\n📦 Testing Payload Extraction:");
    let extracted_market_data: MarketDataMessage = market_event.payload()?;
    println!("   Extracted Symbol: {}", extracted_market_data.symbol);
    println!("   Extracted Price: ${:.2}", extracted_market_data.price);
    println!("   ✅ Proto payload extraction successful");
    
    let extracted_signal: TradingSignalMessage = signal_event.payload()?;
    println!("   Extracted Signal ID: {}", extracted_signal.signal_id);
    println!("   Extracted Confidence: {:.1}%", extracted_signal.confidence * 100.0);
    println!("   ✅ Proto payload extraction successful");
    
    // 5. Demonstrate event validation
    println!("\n🔍 Testing Event Validation:");
    match market_event.validate() {
        Ok(_) => println!("   ✅ Market event validation passed"),
        Err(e) => println!("   ❌ Market event validation failed: {}", e),
    }
    
    match signal_event.validate() {
        Ok(_) => println!("   ✅ Signal event validation passed"),
        Err(e) => println!("   ❌ Signal event validation failed: {}", e),
    }
    
    // 6. Demonstrate EventBus subscription (simplified)
    println!("\n📡 Testing EventBus Subscription:");
    let subscription = event_bus.subscribe("neural_trader.market_data.v1.MarketData").await?;
    println!("   ✅ Subscribed to market data events");
    println!("   Subscription ID: {}", subscription.id);
    println!("   Event Type: {}", subscription.event_type);
    
    // Clean up subscription
    event_bus.unsubscribe(subscription).await?;
    println!("   ✅ Unsubscribed from market data events");
    
    println!("\n🎉 Proto-only Event System Demo completed successfully!");
    println!("📋 Key Features Demonstrated:");
    println!("   • Proto-only Event creation with EventEnvelope");
    println!("   • Event publishing through EventBus");
    println!("   • Event serialization/deserialization");
    println!("   • Typed payload extraction");
    println!("   • Event validation");
    println!("   • EventBus subscription/unsubscription");
    println!("\n⚠️  NO Vec<u8> payloads - Only protobuf messages allowed!");
    
    Ok(())
}