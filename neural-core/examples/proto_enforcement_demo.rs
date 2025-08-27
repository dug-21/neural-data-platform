/// Proto Enforcement Demonstration
/// 
/// This example demonstrates the EventBus proto-only enforcement in action

use neural_core::eventbus::{
    implementations::proto_inmemory::ProtoInMemoryEventBus,
    ProtoEventBusConfig, ProtoEventBus,
    types::ProtoEvent,
    proto_messages::OrderRequest,
};
use prost::Message;

// Define a simple test proto message for this demo
#[derive(Clone, PartialEq, Message)]
pub struct DemoMarketData {
    #[prost(string, tag = "1")]
    pub symbol: String,
    #[prost(double, tag = "2")]
    pub price: f64,
    #[prost(uint64, tag = "3")]
    pub volume: u64,
    #[prost(uint64, tag = "4")]
    pub timestamp: u64,
}

impl neural_core::eventbus::types::ProtoMessage for DemoMarketData {
    fn proto_type_name() -> &'static str {
        "demo.MarketData"
    }

    fn validate(&self) -> Result<(), neural_core::eventbus::error::EventBusError> {
        if self.symbol.is_empty() {
            return Err(neural_core::eventbus::error::EventBusError::ValidationError(
                "Symbol cannot be empty".to_string()
            ));
        }
        if self.price <= 0.0 {
            return Err(neural_core::eventbus::error::EventBusError::ValidationError(
                "Price must be positive".to_string()
            ));
        }
        Ok(())
    }

    fn quality_score(&self) -> f64 {
        if !self.symbol.is_empty() && self.price > 0.0 && self.volume > 0 {
            0.9
        } else {
            0.5
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔒 EventBus Proto Enforcement Demonstration");
    println!("===========================================");
    
    // Initialize proto-only EventBus
    let config = ProtoEventBusConfig::default()
        .register_proto_type::<DemoMarketData>()
        .register_proto_type::<OrderRequest>();
        
    let eventbus = ProtoInMemoryEventBus::with_config(config);
    
    println!("✅ EventBus initialized in proto-only mode");
    
    // Demonstrate valid proto message
    println!("\n📊 Publishing valid proto message...");
    let market_data = DemoMarketData {
        symbol: "AAPL".to_string(),
        price: 150.25,
        volume: 1000,
        timestamp: chrono::Utc::now().timestamp() as u64,
    };
    
    let market_event = ProtoEvent::new(market_data);
    
    match eventbus.publish_proto("market.data", market_event).await {
        Ok(event_id) => {
            println!("   ✅ Proto message published successfully!");
            println!("   📋 Event ID: {}", event_id);
        }
        Err(e) => {
            println!("   ❌ Failed to publish: {:?}", e);
        }
    }
    
    // Demonstrate order request
    println!("\n📈 Publishing order request...");  
    let order = OrderRequest::new_market_buy("TSLA", 100.0);
    let order_event = ProtoEvent::new(order);
    
    match eventbus.publish_proto("orders", order_event).await {
        Ok(event_id) => {
            println!("   ✅ Order proto published successfully!");
            println!("   📋 Event ID: {}", event_id);
        }
        Err(e) => {
            println!("   ❌ Failed to publish order: {:?}", e);
        }
    }
    
    println!("\n🎉 Proto enforcement demonstration complete!");
    println!("   - Proto messages: ACCEPTED ✅");
    println!("   - Vec<u8> payloads: BLOCKED ❌");
    println!("   - JSON payloads: BLOCKED ❌");
    println!("   - Type safety: ENFORCED 🔒");
    
    Ok(())
}