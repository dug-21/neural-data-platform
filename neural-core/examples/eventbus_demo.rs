//! Comprehensive Proto-Only EventBus Demo
//!
//! This example demonstrates advanced proto-only EventBus features including:
//! - Multiple proto message types
//! - Type-safe subscriptions
//! - Batch publishing
//! - Quality scoring and validation
//! - Channel management

use neural_core::eventbus::{
    implementations::inmemory::ProtoInMemoryEventBus,
    traits::proto_event_bus::{ProtoEventBus, ProtoEventBusConfig, ProtoEventSubscriber},
    types::{ProtoEvent, ProtoMessage, SubscriptionConfig, StartPosition},
    proto_messages::{MarketDataEvent, OrderRequest, TradingSignal},
    error::EventBusError,
};
use prost::Message;
use tokio;

// Demo trading proto message
#[derive(Clone, PartialEq, Message)]
pub struct TradingUpdate {
    #[prost(string, tag = "1")]
    pub symbol: String,
    #[prost(double, tag = "2")]
    pub price: f64,
    #[prost(double, tag = "3")]
    pub volume: f64,
    #[prost(string, tag = "4")]
    pub action: String, // "BUY" or "SELL"
    #[prost(int64, tag = "5")]
    pub timestamp: i64,
}

impl ProtoMessage for TradingUpdate {
    fn proto_type_name() -> &'static str {
        "demo.TradingUpdate"
    }

    fn validate(&self) -> Result<(), EventBusError> {
        if self.symbol.is_empty() {
            return Err(EventBusError::ValidationError("Symbol required".to_string()));
        }
        if self.price <= 0.0 {
            return Err(EventBusError::ValidationError("Price must be positive".to_string()));
        }
        if self.volume <= 0.0 {
            return Err(EventBusError::ValidationError("Volume must be positive".to_string()));
        }
        if !matches!(self.action.as_str(), "BUY" | "SELL") {
            return Err(EventBusError::ValidationError("Action must be BUY or SELL".to_string()));
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Comprehensive Proto-Only EventBus Demo");
    println!("==========================================");

    // Step 1: Create proto-only EventBus with all required types
    let config = ProtoEventBusConfig::default()
        .register_proto_type::<TradingUpdate>()
        .register_proto_type::<MarketDataEvent>()
        .register_proto_type::<OrderRequest>()
        .register_proto_type::<TradingSignal>()
        .min_quality_score(0.7)  // Set minimum quality threshold
        .enable_validation(true);
        
    let event_bus = ProtoInMemoryEventBus::with_config(config);
    println!("✅ Created ProtoInMemoryEventBus with 4 proto types registered");

    // Step 2: Create and publish market data events
    println!("\n📊 Step 2: Publishing Market Data Events");
    let market_data = MarketDataEvent::new_trade("AAPL", 150.25, 1000.0, "NASDAQ");
    let market_event = ProtoEvent::new(market_data)
        .with_metadata("source".to_string(), "market-feed".to_string())
        .with_metadata("priority".to_string(), "high".to_string())
        .with_quality_score(0.95);

    let market_event_id = event_bus.publish_proto("market.data.aapl", market_event).await?;
    println!("   ✅ Published MarketDataEvent with ID: {}", market_event_id);

    // Step 3: Create and publish trading updates
    println!("\n📈 Step 3: Publishing Trading Updates");
    let trading_updates = vec![
        TradingUpdate {
            symbol: "AAPL".to_string(),
            price: 151.00,
            volume: 500.0,
            action: "BUY".to_string(),
            timestamp: chrono::Utc::now().timestamp(),
        },
        TradingUpdate {
            symbol: "AAPL".to_string(), 
            price: 150.75,
            volume: 300.0,
            action: "SELL".to_string(),
            timestamp: chrono::Utc::now().timestamp() + 1,
        },
        TradingUpdate {
            symbol: "TSLA".to_string(),
            price: 800.50,
            volume: 200.0,
            action: "BUY".to_string(),
            timestamp: chrono::Utc::now().timestamp() + 2,
        },
    ];

    // Batch publish trading updates
    let mut trading_events = Vec::new();
    for update in trading_updates {
        let event = ProtoEvent::new(update.clone())
            .with_metadata("trader_id".to_string(), "demo_trader".to_string())
            .with_quality_score(0.88);
        trading_events.push(event);
    }

    let batch_ids = event_bus.publish_batch_proto("trading.updates", trading_events).await?;
    println!("   ✅ Published {} trading updates in batch", batch_ids.len());

    // Step 4: Create and publish orders
    println!("\n🎯 Step 4: Publishing Order Requests");
    let buy_order = OrderRequest::new_market_buy("AAPL", 100.0);
    let buy_event = ProtoEvent::new(buy_order)
        .with_metadata("order_type".to_string(), "market".to_string())
        .with_quality_score(0.92);

    let sell_order = OrderRequest::new_limit_sell("TSLA", 50.0, 805.00);
    let sell_event = ProtoEvent::new(sell_order)
        .with_metadata("order_type".to_string(), "limit".to_string())
        .with_quality_score(0.94);

    let buy_order_id = event_bus.publish_proto("orders.buy", buy_event).await?;
    let sell_order_id = event_bus.publish_proto("orders.sell", sell_event).await?;
    
    println!("   ✅ Published buy order with ID: {}", buy_order_id);
    println!("   ✅ Published sell order with ID: {}", sell_order_id);

    // Step 5: Create subscriptions to different channels
    println!("\n🔗 Step 5: Creating Subscriptions");
    let subscription_config = SubscriptionConfig {
        group_name: "demo-group".to_string(),
        consumer_name: "demo-consumer".to_string(),
        start_position: StartPosition::Beginning,
        batch_size: 10,
        block_timeout_ms: 1000,
        ack_timeout_ms: 5000,
        buffer_size: 100,
        receive_timeout: None,
        persistent: false,
        priority: 0,
    };

    // Subscribe to trading updates
    let mut trading_subscriber = event_bus.subscribe_proto::<TradingUpdate>(
        &["trading.updates".to_string()],
        subscription_config.clone()
    ).await?;
    println!("   ✅ Subscribed to trading.updates channel");

    // Subscribe to market data
    let mut market_subscriber = event_bus.subscribe_proto::<MarketDataEvent>(
        &["market.data.aapl".to_string()],
        subscription_config.clone()
    ).await?;
    println!("   ✅ Subscribed to market.data.aapl channel");

    // Step 6: Read events from subscriptions
    println!("\n📥 Step 6: Reading Events from Subscriptions");
    
    // Read trading updates
    println!("   📊 Reading trading updates...");
    for i in 0..3 {
        if let Ok(Some(trading_event)) = trading_subscriber.next_proto().await {
            println!("      Event {}: {} {} @ ${:.2} (vol: {:.0})", 
                i + 1,
                trading_event.message.action,
                trading_event.message.symbol,
                trading_event.message.price,
                trading_event.message.volume
            );
        }
    }

    // Read market data
    println!("   📈 Reading market data...");
    if let Ok(Some(market_event)) = market_subscriber.next_proto().await {
        println!("      MarketData: {} @ ${:.2} (vol: {:.0})", 
            market_event.message.symbol,
            market_event.message.price,
            market_event.message.volume
        );
    }

    // Step 7: Channel information and statistics
    println!("\n📊 Step 7: Channel Information");
    
    let channels = vec![
        "market.data.aapl",
        "trading.updates", 
        "orders.buy",
        "orders.sell"
    ];

    for channel in channels {
        match event_bus.get_channel_info(channel).await {
            Ok(info) => {
                println!("   📋 Channel '{}': {} events, {} subscribers",
                    info.name, info.event_count, info.subscriber_count);
            },
            Err(e) => {
                println!("   ❌ Failed to get info for '{}': {}", channel, e);
            }
        }
    }

    // Step 8: Demonstrate validation enforcement
    println!("\n🔍 Step 8: Validation Enforcement Test");
    
    // Try to publish invalid trading update
    let invalid_update = TradingUpdate {
        symbol: "".to_string(), // Empty symbol - should fail validation
        price: -10.0,           // Negative price - should fail validation
        volume: 0.0,           // Zero volume - should fail validation
        action: "INVALID".to_string(), // Invalid action - should fail validation
        timestamp: chrono::Utc::now().timestamp(),
    };

    let invalid_event = ProtoEvent::new(invalid_update)
        .with_quality_score(0.1); // Low quality score

    match event_bus.publish_proto("trading.updates", invalid_event).await {
        Ok(_) => println!("   ❌ Invalid event was published (this shouldn't happen!)"),
        Err(e) => println!("   ✅ Invalid event correctly rejected: {}", e),
    }

    println!("\n🎉 Proto-Only EventBus Demo Completed!");
    println!("=" * 50);
    println!("\n📋 Features Demonstrated:");
    println!("   ✅ Proto-only event creation and publishing");
    println!("   ✅ Batch publishing of multiple events");
    println!("   ✅ Type-safe subscriptions with ProtoEventSubscriber<T>");
    println!("   ✅ Quality scoring and validation enforcement");
    println!("   ✅ Multiple proto message types in single EventBus");
    println!("   ✅ Channel management and statistics");
    println!("   ✅ Subscription configuration and message reading");
    println!("   ✅ Business rule validation");
    
    println!("\n🔒 Proto-Only Enforcement:");
    println!("   • ALL events must be protobuf messages");
    println!("   • Vec<u8> payloads are REJECTED");
    println!("   • JSON payloads are REJECTED");
    println!("   • Only strongly-typed ProtoEvent<T> accepted");
    println!("   • Compile-time type safety guaranteed");

    Ok(())
}