//! Proto message implementations for EventBus including EventEnvelope
//!
//! This module provides the EventEnvelope proto definitions and other
//! proto message implementations for the neural-trader EventBus system.

use prost::Message;
use std::collections::HashMap;
use crate::eventbus::{
    error::EventBusError,
    types::ProtoMessage,
};

// EventEnvelope and related proto messages from ingestion-eventbus.proto

/// Standard envelope for all messages entering the event bus
#[derive(Clone, PartialEq, prost::Message)]
pub struct EventEnvelope {
    /// Unique message identifier
    #[prost(string, tag = "1")]
    pub message_id: String,
    
    /// Correlation ID for tracking related messages
    #[prost(string, tag = "2")]
    pub correlation_id: String,
    
    /// Source system identifier
    #[prost(string, tag = "3")]
    pub source: String,
    
    /// Domain this message belongs to
    #[prost(string, tag = "4")]
    pub domain: String,
    
    /// Event type for routing
    #[prost(string, tag = "5")]
    pub event_type: String,
    
    /// Schema version of the payload
    #[prost(string, tag = "6")]
    pub schema_version: String,
    
    /// When the event was created
    #[prost(message, optional, tag = "7")]
    pub created_at: Option<prost_types::Timestamp>,
    
    /// When the event was ingested
    #[prost(message, optional, tag = "8")]
    pub ingested_at: Option<prost_types::Timestamp>,
    
    /// Routing metadata
    #[prost(message, optional, tag = "9")]
    pub routing: Option<RoutingMetadata>,
    
    /// Quality metadata
    #[prost(message, optional, tag = "10")]
    pub quality: Option<QualityMetadata>,
    
    /// The actual event payload (domain-specific)
    #[prost(message, optional, tag = "11")]
    pub payload: Option<prost_types::Any>,
    
    /// Headers for additional metadata
    #[prost(map = "string, string", tag = "12")]
    pub headers: HashMap<String, String>,
    
    /// Tracing context for distributed tracing
    #[prost(message, optional, tag = "13")]
    pub tracing: Option<TracingContext>,
}

/// Routing information for the message
#[derive(Clone, PartialEq, prost::Message)]
pub struct RoutingMetadata {
    /// Target topic/stream
    #[prost(string, tag = "1")]
    pub topic: String,
    
    /// Partition key for ordering
    #[prost(string, tag = "2")]
    pub partition_key: String,
    
    /// Priority level (0-9, 0 highest)
    #[prost(int32, tag = "3")]
    pub priority: i32,
    
    /// TTL in seconds (0 = no expiry)
    #[prost(int64, tag = "4")]
    pub ttl_seconds: i64,
    
    /// Routing tags for filtering
    #[prost(string, repeated, tag = "5")]
    pub tags: Vec<String>,
    
    /// Retry policy
    #[prost(message, optional, tag = "6")]
    pub retry_policy: Option<RetryPolicy>,
}

/// Data quality indicators
#[derive(Clone, PartialEq, prost::Message)]
pub struct QualityMetadata {
    /// Completeness score (0-100)
    #[prost(float, tag = "1")]
    pub completeness: f32,
    
    /// Timeliness in milliseconds
    #[prost(int64, tag = "2")]
    pub latency_ms: i64,
    
    /// Data validation status
    #[prost(enumeration = "ValidationStatus", tag = "3")]
    pub validation_status: i32,
    
    /// Quality score (0-100)
    #[prost(float, tag = "4")]
    pub quality_score: f32,
    
    /// Anomaly indicators
    #[prost(message, repeated, tag = "5")]
    pub anomalies: Vec<AnomalyIndicator>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
pub enum ValidationStatus {
    ValidationStatusUnspecified = 0,
    ValidationStatusPassed = 1,
    ValidationStatusFailed = 2,
    ValidationStatusPartial = 3,
    ValidationStatusSkipped = 4,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct AnomalyIndicator {
    #[prost(string, tag = "1")]
    pub r#type: String,
    #[prost(float, tag = "2")]
    pub severity: f32,
    #[prost(string, tag = "3")]
    pub description: String,
}

/// Retry configuration
#[derive(Clone, PartialEq, prost::Message)]
pub struct RetryPolicy {
    #[prost(int32, tag = "1")]
    pub max_attempts: i32,
    #[prost(int64, tag = "2")]
    pub initial_delay_ms: i64,
    #[prost(float, tag = "3")]
    pub backoff_multiplier: f32,
    #[prost(int64, tag = "4")]
    pub max_delay_ms: i64,
    #[prost(string, repeated, tag = "5")]
    pub retryable_errors: Vec<String>,
}

/// Distributed tracing context
#[derive(Clone, PartialEq, prost::Message)]
pub struct TracingContext {
    #[prost(string, tag = "1")]
    pub trace_id: String,
    #[prost(string, tag = "2")]
    pub span_id: String,
    #[prost(string, tag = "3")]
    pub parent_span_id: String,
    #[prost(map = "string, string", tag = "4")]
    pub baggage: HashMap<String, String>,
}

// Market Data Proto Messages

/// Market data event message for real-time trading data
#[derive(Clone, PartialEq, prost::Message)]
pub struct MarketDataEvent {
    #[prost(string, tag = "1")]
    pub event_id: String,
    #[prost(message, optional, tag = "2")]
    pub timestamp: Option<prost_types::Timestamp>,
    #[prost(string, tag = "3")]
    pub symbol: String,
    #[prost(enumeration = "DataType", tag = "4")]
    pub data_type: i32,
    #[prost(message, optional, tag = "5")]
    pub payload: Option<MarketDataPayload>,
    #[prost(message, optional, tag = "6")]
    pub quality: Option<DataQuality>,
    #[prost(string, tag = "7")]
    pub provider: String,
    #[prost(map = "string, string", tag = "8")]
    pub metadata: std::collections::HashMap<String, String>,
}

impl ProtoMessage for MarketDataEvent {
    fn proto_type_name() -> &'static str {
        "neural_trader.market_data.v1.MarketDataEvent"
    }
    
    fn validate(&self) -> Result<(), EventBusError> {
        if self.event_id.is_empty() {
            return Err(EventBusError::schema_validation("event_id cannot be empty"));
        }
        
        if self.symbol.is_empty() {
            return Err(EventBusError::schema_validation("symbol cannot be empty"));
        }
        
        if self.timestamp.is_none() {
            return Err(EventBusError::schema_validation("timestamp is required"));
        }
        
        if self.payload.is_none() {
            return Err(EventBusError::schema_validation("payload is required"));
        }
        
        // Validate the payload based on data type
        if let Some(payload) = &self.payload {
            let data_type = DataType::try_from(self.data_type).ok();
            payload.validate_for_data_type(data_type)?;
        }
        
        Ok(())
    }
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct MarketDataPayload {
    #[prost(oneof = "market_data_payload::Data", tags = "1, 2, 3, 4")]
    pub data: Option<market_data_payload::Data>,
}

pub mod market_data_payload {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Data {
        #[prost(message, tag = "1")]
        Trade(super::TradeData),
        #[prost(message, tag = "2")]
        Quote(super::QuoteData),
        #[prost(message, tag = "3")]
        Bar(super::BarData),
        #[prost(message, tag = "4")]
        News(super::NewsData),
    }
}

impl MarketDataPayload {
    fn validate_for_data_type(&self, data_type: Option<DataType>) -> Result<(), EventBusError> {
        let expected_type = data_type.unwrap_or(DataType::Unspecified);
        
        match (&self.data, expected_type) {
            (Some(market_data_payload::Data::Trade(_)), DataType::Trade) |
            (Some(market_data_payload::Data::Quote(_)), DataType::Quote) |
            (Some(market_data_payload::Data::Bar(_)), DataType::Bar1m | DataType::Bar5m | DataType::Bar1h | DataType::Bar1d) |
            (Some(market_data_payload::Data::News(_)), DataType::News) => Ok(()),
            (None, _) => Err(EventBusError::schema_validation("payload data is required")),
            (Some(_), _) => Err(EventBusError::schema_validation("payload data type doesn't match expected type")),
        }
    }
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct TradeData {
    #[prost(double, tag = "1")]
    pub price: f64,
    #[prost(double, tag = "2")]
    pub size: f64,
    #[prost(message, optional, tag = "3")]
    pub timestamp: Option<prost_types::Timestamp>,
    #[prost(string, tag = "4")]
    pub exchange: String,
    #[prost(int64, tag = "5")]
    pub sequence: i64,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct QuoteData {
    #[prost(double, tag = "1")]
    pub bid_price: f64,
    #[prost(double, tag = "2")]
    pub bid_size: f64,
    #[prost(double, tag = "3")]
    pub ask_price: f64,
    #[prost(double, tag = "4")]
    pub ask_size: f64,
    #[prost(message, optional, tag = "5")]
    pub timestamp: Option<prost_types::Timestamp>,
    #[prost(string, tag = "6")]
    pub exchange: String,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct BarData {
    #[prost(double, tag = "1")]
    pub open: f64,
    #[prost(double, tag = "2")]
    pub high: f64,
    #[prost(double, tag = "3")]
    pub low: f64,
    #[prost(double, tag = "4")]
    pub close: f64,
    #[prost(double, tag = "5")]
    pub volume: f64,
    #[prost(double, tag = "6")]
    pub vwap: f64,
    #[prost(message, optional, tag = "7")]
    pub start_time: Option<prost_types::Timestamp>,
    #[prost(message, optional, tag = "8")]
    pub end_time: Option<prost_types::Timestamp>,
    #[prost(int32, tag = "9")]
    pub trade_count: i32,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct NewsData {
    #[prost(string, tag = "1")]
    pub headline: String,
    #[prost(string, tag = "2")]
    pub body: String,
    #[prost(string, tag = "3")]
    pub source: String,
    #[prost(message, optional, tag = "4")]
    pub published_at: Option<prost_types::Timestamp>,
    #[prost(string, repeated, tag = "5")]
    pub symbols: Vec<String>,
    #[prost(double, tag = "6")]
    pub sentiment_score: f64,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct DataQuality {
    #[prost(double, tag = "1")]
    pub completeness_score: f64,
    #[prost(double, tag = "2")]
    pub timeliness_score: f64,
    #[prost(double, tag = "3")]
    pub accuracy_score: f64,
    #[prost(double, tag = "4")]
    pub overall_score: f64,
    #[prost(string, repeated, tag = "5")]
    pub issues: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
pub enum DataType {
    Unspecified = 0,
    Trade = 1,
    Quote = 2,
    Bar1m = 3,
    Bar5m = 4,
    Bar1h = 5,
    Bar1d = 6,
    News = 7,
}

impl DataType {
    pub fn as_str_name(&self) -> &'static str {
        match self {
            DataType::Unspecified => "DATA_TYPE_UNSPECIFIED",
            DataType::Trade => "DATA_TYPE_TRADE",
            DataType::Quote => "DATA_TYPE_QUOTE",
            DataType::Bar1m => "DATA_TYPE_BAR_1M",
            DataType::Bar5m => "DATA_TYPE_BAR_5M",
            DataType::Bar1h => "DATA_TYPE_BAR_1H",
            DataType::Bar1d => "DATA_TYPE_BAR_1D",
            DataType::News => "DATA_TYPE_NEWS",
        }
    }
    
    // from_i32 is auto-generated by prost::Enumeration
}

// Trading Proto Messages

/// Trading order request message
#[derive(Clone, PartialEq, prost::Message)]
pub struct OrderRequest {
    #[prost(string, tag = "1")]
    pub request_id: String,
    #[prost(string, tag = "2")]
    pub symbol: String,
    #[prost(enumeration = "OrderSide", tag = "3")]
    pub side: i32,
    #[prost(enumeration = "OrderType", tag = "4")]
    pub order_type: i32,
    #[prost(double, tag = "5")]
    pub quantity: f64,
    #[prost(double, optional, tag = "6")]
    pub price: Option<f64>,
    #[prost(double, optional, tag = "7")]
    pub stop_price: Option<f64>,
    #[prost(message, optional, tag = "8")]
    pub timestamp: Option<prost_types::Timestamp>,
    #[prost(map = "string, string", tag = "9")]
    pub metadata: std::collections::HashMap<String, String>,
}

impl ProtoMessage for OrderRequest {
    fn proto_type_name() -> &'static str {
        "neural_trader.trading.v1.OrderRequest"
    }
    
    fn validate(&self) -> Result<(), EventBusError> {
        if self.request_id.is_empty() {
            return Err(EventBusError::schema_validation("request_id cannot be empty"));
        }
        
        if self.symbol.is_empty() {
            return Err(EventBusError::schema_validation("symbol cannot be empty"));
        }
        
        if self.quantity <= 0.0 {
            return Err(EventBusError::schema_validation("quantity must be positive"));
        }
        
        let order_type = OrderType::try_from(self.order_type).unwrap_or(OrderType::Unspecified);
        match order_type {
            OrderType::Limit => {
                if self.price.is_none() {
                    return Err(EventBusError::schema_validation("price required for limit orders"));
                }
            }
            OrderType::StopLimit => {
                if self.price.is_none() {
                    return Err(EventBusError::schema_validation("price required for limit orders"));
                }
                if self.stop_price.is_none() {
                    return Err(EventBusError::schema_validation("stop_price required for stop orders"));
                }
            }
            OrderType::Stop => {
                if self.stop_price.is_none() {
                    return Err(EventBusError::schema_validation("stop_price required for stop orders"));
                }
            }
            _ => {}
        }
        
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
pub enum OrderSide {
    Unspecified = 0,
    Buy = 1,
    Sell = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
pub enum OrderType {
    Unspecified = 0,
    Market = 1,
    Limit = 2,
    Stop = 3,
    StopLimit = 4,
}

impl OrderType {
    // from_i32 is auto-generated by prost::Enumeration
}

// ML-Ops Proto Messages

/// Feature extraction request from EventBus to ML-Ops
#[derive(Clone, PartialEq, prost::Message)]
pub struct FeatureExtractionRequest {
    #[prost(string, tag = "1")]
    pub request_id: String,
    #[prost(string, tag = "2")]
    pub pipeline_id: String,
    #[prost(message, optional, tag = "3")]
    pub source: Option<DataSource>,
    #[prost(message, optional, tag = "4")]
    pub config: Option<FeatureConfig>,
    #[prost(message, optional, tag = "5")]
    pub window: Option<TimeWindow>,
    #[prost(message, optional, tag = "6")]
    pub quality: Option<QualityRequirements>,
}

impl ProtoMessage for FeatureExtractionRequest {
    fn proto_type_name() -> &'static str {
        "neural_trader.interfaces.mlops.FeatureExtractionRequest"
    }
    
    fn validate(&self) -> Result<(), EventBusError> {
        if self.request_id.is_empty() {
            return Err(EventBusError::schema_validation("request_id cannot be empty"));
        }
        
        if self.pipeline_id.is_empty() {
            return Err(EventBusError::schema_validation("pipeline_id cannot be empty"));
        }
        
        Ok(())
    }
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct DataSource {
    #[prost(enumeration = "SourceType", tag = "1")]
    pub source_type: i32,
    #[prost(string, tag = "2")]
    pub topic: String,
    #[prost(string, tag = "3")]
    pub query: String,
    #[prost(string, repeated, tag = "4")]
    pub partitions: Vec<String>,
    #[prost(map = "string, string", tag = "5")]
    pub filters: std::collections::HashMap<String, String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
pub enum SourceType {
    Unspecified = 0,
    Stream = 1,
    Batch = 2,
    Hybrid = 3,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct FeatureConfig {
    #[prost(string, tag = "1")]
    pub feature_set_id: String,
    #[prost(string, tag = "2")]
    pub version: String,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct TimeWindow {
    #[prost(message, optional, tag = "1")]
    pub start: Option<prost_types::Timestamp>,
    #[prost(message, optional, tag = "2")]
    pub end: Option<prost_types::Timestamp>,
    #[prost(int64, tag = "3")]
    pub duration_seconds: i64,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct QualityRequirements {
    #[prost(float, tag = "1")]
    pub min_completeness: f32,
    #[prost(int64, tag = "2")]
    pub max_latency_ms: i64,
    #[prost(float, tag = "3")]
    pub min_quality_score: f32,
    #[prost(bool, tag = "4")]
    pub allow_missing: bool,
    #[prost(bool, tag = "5")]
    pub allow_outliers: bool,
}

// Configuration Proto Messages

/// Configuration change event
#[derive(Clone, PartialEq, prost::Message)]
pub struct ConfigChangeEvent {
    #[prost(string, tag = "1")]
    pub event_id: String,
    #[prost(string, tag = "2")]
    pub config_key: String,
    #[prost(string, tag = "3")]
    pub old_value: String,
    #[prost(string, tag = "4")]
    pub new_value: String,
    #[prost(string, tag = "5")]
    pub changed_by: String,
    #[prost(message, optional, tag = "6")]
    pub timestamp: Option<prost_types::Timestamp>,
    #[prost(string, tag = "7")]
    pub reason: String,
}

impl ProtoMessage for ConfigChangeEvent {
    fn proto_type_name() -> &'static str {
        "neural_trader.config.v1.ConfigChangeEvent"
    }
    
    fn validate(&self) -> Result<(), EventBusError> {
        if self.event_id.is_empty() {
            return Err(EventBusError::schema_validation("event_id cannot be empty"));
        }
        
        if self.config_key.is_empty() {
            return Err(EventBusError::schema_validation("config_key cannot be empty"));
        }
        
        if self.changed_by.is_empty() {
            return Err(EventBusError::schema_validation("changed_by cannot be empty"));
        }
        
        Ok(())
    }
}

/// Helper functions for creating common proto messages
impl MarketDataEvent {
    /// Create a trade event
    pub fn new_trade(symbol: &str, price: f64, size: f64, exchange: &str) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap();
        
        let timestamp = prost_types::Timestamp {
            seconds: now.as_secs() as i64,
            nanos: now.subsec_nanos() as i32,
        };
        
        let trade_data = TradeData {
            price,
            size,
            timestamp: Some(timestamp.clone()),
            exchange: exchange.to_string(),
            sequence: chrono::Utc::now().timestamp_millis(),
        };
        
        Self {
            event_id: uuid::Uuid::new_v4().to_string(),
            timestamp: Some(timestamp),
            symbol: symbol.to_string(),
            data_type: DataType::Trade as i32,
            payload: Some(MarketDataPayload {
                data: Some(market_data_payload::Data::Trade(trade_data)),
            }),
            quality: Some(DataQuality {
                completeness_score: 1.0,
                timeliness_score: 1.0,
                accuracy_score: 1.0,
                overall_score: 1.0,
                issues: vec![],
            }),
            provider: "test".to_string(),
            metadata: std::collections::HashMap::new(),
        }
    }
}

impl OrderRequest {
    /// Create a market buy order
    pub fn new_market_buy(symbol: &str, quantity: f64) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap();
        
        let timestamp = prost_types::Timestamp {
            seconds: now.as_secs() as i64,
            nanos: now.subsec_nanos() as i32,
        };
        
        Self {
            request_id: uuid::Uuid::new_v4().to_string(),
            symbol: symbol.to_string(),
            side: OrderSide::Buy as i32,
            order_type: OrderType::Market as i32,
            quantity,
            price: None,
            stop_price: None,
            timestamp: Some(timestamp),
            metadata: std::collections::HashMap::new(),
        }
    }
    
    /// Create a limit sell order
    pub fn new_limit_sell(symbol: &str, quantity: f64, price: f64) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap();
        
        let timestamp = prost_types::Timestamp {
            seconds: now.as_secs() as i64,
            nanos: now.subsec_nanos() as i32,
        };
        
        Self {
            request_id: uuid::Uuid::new_v4().to_string(),
            symbol: symbol.to_string(),
            side: OrderSide::Sell as i32,
            order_type: OrderType::Limit as i32,
            quantity,
            price: Some(price),
            stop_price: None,
            timestamp: Some(timestamp),
            metadata: std::collections::HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eventbus::types::ProtoEvent;
    
    #[test]
    fn test_market_data_event_validation() {
        // Valid market data event
        let event = MarketDataEvent::new_trade("AAPL", 150.25, 100.0, "NASDAQ");
        assert!(event.validate().is_ok());
        
        // Invalid: empty event_id
        let mut invalid_event = event.clone();
        invalid_event.event_id.clear();
        assert!(invalid_event.validate().is_err());
        
        // Invalid: empty symbol
        let mut invalid_event = event.clone();
        invalid_event.symbol.clear();
        assert!(invalid_event.validate().is_err());
    }
    
    #[test]
    fn test_order_request_validation() {
        // Valid market order
        let order = OrderRequest::new_market_buy("AAPL", 100.0);
        assert!(order.validate().is_ok());
        
        // Valid limit order
        let order = OrderRequest::new_limit_sell("AAPL", 100.0, 150.0);
        assert!(order.validate().is_ok());
        
        // Invalid: zero quantity
        let invalid_order = OrderRequest::new_market_buy("AAPL", 0.0);
        assert!(invalid_order.validate().is_err());
        
        // Invalid: limit order without price
        let mut invalid_order = OrderRequest::new_limit_sell("AAPL", 100.0, 150.0);
        invalid_order.price = None;
        assert!(invalid_order.validate().is_err());
    }
    
    #[test]
    fn test_proto_event_integration() {
        let trade_event = MarketDataEvent::new_trade("AAPL", 150.25, 100.0, "NASDAQ");
        let proto_event = ProtoEvent::new(trade_event)
            .with_quality_score(0.95);
        
        assert!(proto_event.validate().is_ok());
        assert_eq!(proto_event.proto_type_name(), "neural_trader.market_data.v1.MarketDataEvent");
        assert_eq!(proto_event.quality_score, 0.95);
    }
    
    #[test]
    fn test_feature_extraction_request() {
        let request = FeatureExtractionRequest {
            request_id: "req-123".to_string(),
            pipeline_id: "ml-pipeline-v1".to_string(),
            source: None,
            config: None,
            window: None,
            quality: None,
        };
        
        assert!(request.validate().is_ok());
        assert_eq!(request.proto_type_name(), "neural_trader.interfaces.mlops.FeatureExtractionRequest");
    }
    
    #[test]
    fn test_config_change_event() {
        let config_event = ConfigChangeEvent {
            event_id: "cfg-123".to_string(),
            config_key: "model.threshold".to_string(),
            old_value: "0.5".to_string(),
            new_value: "0.7".to_string(),
            changed_by: "admin".to_string(),
            timestamp: None,
            reason: "Performance optimization".to_string(),
        };
        
        assert!(config_event.validate().is_ok());
        assert_eq!(config_event.proto_type_name(), "neural_trader.config.v1.ConfigChangeEvent");
    }
}

// Test message for migration purposes
#[derive(Clone, PartialEq, prost::Message)]
pub struct TestMessage {
    #[prost(string, tag = "1")]
    pub content: String,
    #[prost(int64, tag = "2")]
    pub timestamp: i64,
}

impl ProtoMessage for TestMessage {
    fn proto_type_name() -> &'static str {
        "neural_trader.test.TestMessage"
    }
    
    fn validate(&self) -> Result<(), EventBusError> {
        if self.content.is_empty() {
            return Err(EventBusError::schema_validation("TestMessage content cannot be empty"));
        }
        Ok(())
    }
}