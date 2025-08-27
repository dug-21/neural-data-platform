/// Example: How to integrate EventBus into an existing Neural-Trader service
/// This shows the exact changes needed in neural-trading or neural-ml-ops

use neural_core::eventbus::{EventBus, RedisEventBus, InMemoryEventBus, Event, SubscriptionConfig};
use std::sync::Arc;
use tokio::task::JoinHandle;
use serde::{Serialize, Deserialize};

// Step 1: Define your event types
#[derive(Serialize, Deserialize, Debug)]
struct MarketDataEvent {
    symbol: String,
    price: f64,
    volume: u64,
    timestamp: i64,
}

#[derive(Serialize, Deserialize, Debug)]
struct TradeSignalEvent {
    symbol: String,
    action: String, // "BUY" or "SELL"
    confidence: f64,
    quantity: f64,
}

// Step 2: Create EventBus wrapper for your service
pub struct TradingServiceEventBus {
    event_bus: Arc<dyn EventBus>,
    consumer_handle: Option<JoinHandle<()>>,
}

impl TradingServiceEventBus {
    /// Initialize with Redis for production
    pub async fn new_production(redis_url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let event_bus = Arc::new(RedisEventBus::new(redis_url).await?);
        Ok(Self {
            event_bus,
            consumer_handle: None,
        })
    }

    /// Initialize with InMemory for testing
    pub async fn new_testing() -> Self {
        Self {
            event_bus: Arc::new(InMemoryEventBus::new()),
            consumer_handle: None,
        }
    }

    /// Publish market data
    pub async fn publish_market_data(
        &self,
        symbol: &str,
        price: f64,
        volume: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let event_data = MarketDataEvent {
            symbol: symbol.to_string(),
            price,
            volume,
            timestamp: chrono::Utc::now().timestamp(),
        };

        let payload = serde_json::to_vec(&event_data)?;
        let event = Event::new("MarketData".to_string(), payload)
            .with_metadata("symbol".to_string(), symbol.to_string())
            .with_metadata("exchange".to_string(), "NASDAQ".to_string());

        let channel = format!("stream:symbol:{}", symbol);
        self.event_bus.publish(&channel, event).await?;
        
        println!("✅ Published market data for {}: ${}", symbol, price);
        Ok(())
    }

    /// Publish trade signal
    pub async fn publish_trade_signal(
        &self,
        signal: TradeSignalEvent,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let payload = serde_json::to_vec(&signal)?;
        let event = Event::new("TradeSignal".to_string(), payload)
            .with_metadata("symbol".to_string(), signal.symbol.clone())
            .with_metadata("action".to_string(), signal.action.clone());

        self.event_bus.publish("stream:action:trades", event).await?;
        
        println!("✅ Published {} signal for {}", signal.action, signal.symbol);
        Ok(())
    }

    /// Start consuming events
    pub async fn start_consumer(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let event_bus = self.event_bus.clone();
        
        // Subscribe to relevant channels
        let config = SubscriptionConfig {
            group_name: "trading-service".to_string(),
            consumer_name: "main-consumer".to_string(),
            ..Default::default()
        };

        let mut subscriber = event_bus.subscribe(
            &[
                "stream:symbol:AAPL".to_string(),
                "stream:symbol:GOOGL".to_string(),
                "stream:ml:predictions".to_string(),
            ],
            config,
        ).await?;

        // Spawn consumer task
        let consumer_handle = tokio::spawn(async move {
            println!("🔄 Consumer started, waiting for events...");
            
            loop {
                match subscriber.next().await {
                    Ok(Some(envelope)) => {
                        println!("📨 Received event: {}", envelope.event.event_type);
                        
                        // Process based on event type
                        match envelope.event.event_type.as_str() {
                            "MarketData" => {
                                if let Ok(data) = serde_json::from_slice::<MarketDataEvent>(&envelope.event.payload) {
                                    println!("   Market: {} @ ${}", data.symbol, data.price);
                                }
                            }
                            "MLPrediction" => {
                                println!("   ML Prediction received");
                            }
                            _ => {}
                        }

                        // ACK the message
                        let _ = event_bus.ack(
                            &envelope.channel,
                            "trading-service",
                            &envelope.event_id,
                        ).await;
                    }
                    Ok(None) => {
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                    Err(e) => {
                        eprintln!("❌ Consumer error: {}", e);
                        break;
                    }
                }
            }
        });

        self.consumer_handle = Some(consumer_handle);
        Ok(())
    }
}

// Step 3: Example integration in main service
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Neural-Trader Service with EventBus Integration");
    println!("==================================================\n");

    // Initialize EventBus (use Redis in production)
    let mut service = TradingServiceEventBus::new_testing().await;
    
    // Start consuming events
    service.start_consumer().await?;
    
    // Simulate market data publishing
    println!("📡 Publishing market data...");
    service.publish_market_data("AAPL", 150.25, 1000000).await?;
    service.publish_market_data("GOOGL", 2800.50, 500000).await?;
    
    // Simulate trade signal
    println!("\n📈 Publishing trade signals...");
    let signal = TradeSignalEvent {
        symbol: "AAPL".to_string(),
        action: "BUY".to_string(),
        confidence: 0.85,
        quantity: 100.0,
    };
    service.publish_trade_signal(signal).await?;
    
    // Let consumer process events
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    println!("\n✅ EventBus integration complete!");
    println!("\n📋 Integration Checklist:");
    println!("   ✓ EventBus initialized");
    println!("   ✓ Event schemas defined");
    println!("   ✓ Publisher configured");
    println!("   ✓ Consumer subscribed");
    println!("   ✓ Events flowing");
    
    Ok(())
}

// Step 4: Migration helper for existing Redis pub/sub code
pub mod migration {
    use super::*;
    
    /// Drop-in replacement for existing Redis publish
    pub async fn publish_redis_compatible(
        event_bus: &Arc<dyn EventBus>,
        old_channel: &str,
        data: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Map old Redis channel to new EventBus channel
        let new_channel = map_redis_channel(old_channel);
        
        let event = Event::new(
            "LegacyRedisEvent".to_string(),
            data.as_bytes().to_vec()
        );
        
        event_bus.publish(&new_channel, event).await?;
        Ok(())
    }
    
    fn map_redis_channel(old: &str) -> String {
        if old.starts_with("market:") {
            let symbol = old.strip_prefix("market:").unwrap_or("unknown");
            format!("stream:symbol:{}", symbol)
        } else if old.starts_with("trades:") {
            "stream:action:trades".to_string()
        } else if old.starts_with("ml:") {
            let suffix = old.strip_prefix("ml:").unwrap_or("unknown");
            format!("stream:ml:{}", suffix)
        } else {
            format!("stream:legacy:{}", old)
        }
    }
}