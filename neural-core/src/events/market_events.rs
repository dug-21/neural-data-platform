//! Market-related events
//! Module size: <200 lines as per requirements

use crate::events::traits::Event;
use crate::types::market::{MarketTrend};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Base market event structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketEvent {
    pub id: Uuid,
    pub symbol: String,
    pub timestamp: DateTime<Utc>,
    pub source: String,
    pub correlation_id: Option<Uuid>,
}

impl MarketEvent {
    pub fn new(symbol: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            symbol,
            timestamp: Utc::now(),
            source: "market_data".to_string(),
            correlation_id: None,
        }
    }
}

/// Price update event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceUpdateEvent {
    pub base: MarketEvent,
    pub new_price: f64,
    pub previous_price: f64,
    pub volume: Option<u64>,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
}

impl PriceUpdateEvent {
    pub fn new(symbol: String, new_price: f64, previous_price: f64) -> Self {
        Self {
            base: MarketEvent::new(symbol),
            new_price,
            previous_price,
            volume: None,
            bid: None,
            ask: None,
        }
    }
    
    pub fn with_volume(mut self, volume: u64) -> Self {
        self.volume = Some(volume);
        self
    }
    
    pub fn with_bid_ask(mut self, bid: f64, ask: f64) -> Self {
        self.bid = Some(bid);
        self.ask = Some(ask);
        self
    }
    
    /// Calculate price change percentage
    pub fn price_change_percent(&self) -> f64 {
        if self.previous_price == 0.0 {
            return 0.0;
        }
        ((self.new_price - self.previous_price) / self.previous_price) * 100.0
    }
}

impl Event for PriceUpdateEvent {
    fn event_type(&self) -> String {
        "price_update".to_string()
    }
    
    fn timestamp(&self) -> DateTime<Utc> {
        self.base.timestamp
    }
    
    fn event_id(&self) -> Uuid {
        self.base.id
    }
    
    fn source(&self) -> String {
        self.base.source.clone()
    }
    
    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
    
    fn priority(&self) -> u8 {
        8 // High priority for price updates
    }
    
    fn correlation_id(&self) -> Option<Uuid> {
        self.base.correlation_id
    }
}

/// Volume spike event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeEvent {
    pub base: MarketEvent,
    pub current_volume: u64,
    pub average_volume: u64,
    pub volume_ratio: f64,
    pub is_spike: bool,
}

impl VolumeEvent {
    pub fn new(symbol: String, current_volume: u64, average_volume: u64) -> Self {
        let volume_ratio = if average_volume > 0 {
            current_volume as f64 / average_volume as f64
        } else {
            1.0
        };
        
        let is_spike = volume_ratio > 2.0; // 2x average is considered a spike
        
        Self {
            base: MarketEvent::new(symbol),
            current_volume,
            average_volume,
            volume_ratio,
            is_spike,
        }
    }
}

impl Event for VolumeEvent {
    fn event_type(&self) -> String {
        "volume_event".to_string()
    }
    
    fn timestamp(&self) -> DateTime<Utc> {
        self.base.timestamp
    }
    
    fn event_id(&self) -> Uuid {
        self.base.id
    }
    
    fn source(&self) -> String {
        self.base.source.clone()
    }
    
    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
    
    fn priority(&self) -> u8 {
        if self.is_spike { 9 } else { 6 }
    }
}

/// Market trend change event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendChangeEvent {
    pub base: MarketEvent,
    pub previous_trend: MarketTrend,
    pub new_trend: MarketTrend,
    pub confidence: f64,
}

impl TrendChangeEvent {
    pub fn new(symbol: String, previous_trend: MarketTrend, new_trend: MarketTrend, confidence: f64) -> Self {
        Self {
            base: MarketEvent::new(symbol),
            previous_trend,
            new_trend,
            confidence,
        }
    }
}

impl Event for TrendChangeEvent {
    fn event_type(&self) -> String {
        "trend_change".to_string()
    }
    
    fn timestamp(&self) -> DateTime<Utc> {
        self.base.timestamp
    }
    
    fn event_id(&self) -> Uuid {
        self.base.id
    }
    
    fn source(&self) -> String {
        self.base.source.clone()
    }
    
    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
    
    fn priority(&self) -> u8 {
        7 // Medium-high priority for trend changes
    }
    
    fn is_persistent(&self) -> bool {
        true // Trend changes should be persisted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_price_update_event() {
        let event = PriceUpdateEvent::new("AAPL".to_string(), 155.0, 150.0)
            .with_volume(1000000);
        
        assert_eq!(event.base.symbol, "AAPL");
        assert_eq!(event.new_price, 155.0);
        assert_eq!(event.previous_price, 150.0);
        assert_eq!(event.volume, Some(1000000));
        
        // Test price change calculation
        let change_percent = event.price_change_percent();
        assert!((change_percent - 3.333333333333334).abs() < 0.0001);
        
        // Test event trait implementation
        assert_eq!(event.event_type(), "price_update");
        assert_eq!(event.priority(), 8);
    }
    
    #[test]
    fn test_volume_event_spike_detection() {
        let normal_volume = VolumeEvent::new("AAPL".to_string(), 1000000, 1000000);
        assert!(!normal_volume.is_spike);
        assert_eq!(normal_volume.volume_ratio, 1.0);
        
        let spike_volume = VolumeEvent::new("AAPL".to_string(), 3000000, 1000000);
        assert!(spike_volume.is_spike);
        assert_eq!(spike_volume.volume_ratio, 3.0);
        assert_eq!(spike_volume.priority(), 9);
    }
    
    #[test]
    fn test_trend_change_event() {
        let event = TrendChangeEvent::new(
            "AAPL".to_string(),
            MarketTrend::Neutral,
            MarketTrend::Bullish,
            0.85
        );
        
        assert_eq!(event.previous_trend, MarketTrend::Neutral);
        assert_eq!(event.new_trend, MarketTrend::Bullish);
        assert_eq!(event.confidence, 0.85);
        assert!(event.is_persistent());
    }
}