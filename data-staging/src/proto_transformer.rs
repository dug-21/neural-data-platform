//! Proto Transformation - Converts validated JSON to EventEnvelope protobuf
//!
//! This is the core transformation logic that creates properly structured
//! EventEnvelope protos from validated raw market data.

use anyhow::{Result, Context};
use chrono::Timelike;
use prost::Message;
use prost_types::{Timestamp, Any};
use uuid::Uuid;

use crate::{
    RawMarketData, DataQualityMetrics, DataStagingError,
    generated::{EventEnvelope, RoutingMetadata, QualityMetadata, TracingContext}
};

/// Transforms validated JSON to protobuf EventEnvelope
pub struct ProtoTransformer;

impl ProtoTransformer {
    pub fn new() -> Self {
        Self
    }
    
    /// Transform raw market data to EventEnvelope proto
    pub fn transform_to_event_envelope(
        &self,
        raw_data: &RawMarketData,
        quality_metrics: &DataQualityMetrics,
    ) -> Result<EventEnvelope> {
        // Create market data payload
        let payload_bytes = self.create_market_data_proto(raw_data)?;
        
        let payload_any = Any {
            type_url: "type.googleapis.com/neural_trader.market_data.PriceUpdate".to_string(),
            value: payload_bytes,
        };
        
        // Create timestamps
        let now = chrono::Utc::now();
        let created_at = Some(Timestamp {
            seconds: raw_data.timestamp.unwrap_or(now.timestamp()),
            nanos: 0,
        });
        let ingested_at = Some(Timestamp {
            seconds: now.timestamp(),
            nanos: (now.nanosecond() % 1_000_000_000) as i32,
        });
        
        // Create routing metadata
        let routing = Some(RoutingMetadata {
            topic: "market-data-stream".to_string(),
            partition_key: raw_data.symbol.clone().unwrap_or_default(),
            priority: self.calculate_priority(raw_data),
            ttl_seconds: 300, // 5 minutes TTL
            tags: self.generate_routing_tags(raw_data),
            retry_policy: None, // Use default retry policy
        });
        
        // Create quality metadata
        let quality = Some(QualityMetadata {
            completeness: quality_metrics.completeness_score,
            latency_ms: quality_metrics.data_age_seconds * 1000,
            validation_status: crate::generated::ValidationStatus::Passed as i32,
            quality_score: quality_metrics.overall_score,
            anomalies: vec![], // Anomaly detection can be added later
        });
        
        // Create tracing context
        let tracing = Some(TracingContext {
            trace_id: Uuid::new_v4().to_string(),
            span_id: Uuid::new_v4().simple().to_string(),
            parent_span_id: String::new(),
            baggage: std::collections::HashMap::new(),
        });
        
        // Create headers
        let mut headers = std::collections::HashMap::new();
        headers.insert("source".to_string(), "data-staging".to_string());
        headers.insert("version".to_string(), "2.0".to_string());
        if let Some(ref exchange) = raw_data.exchange {
            headers.insert("exchange".to_string(), exchange.clone());
        }
        
        // Create EventEnvelope
        let event_envelope = EventEnvelope {
            message_id: Uuid::new_v4().to_string(),
            correlation_id: self.generate_correlation_id(raw_data),
            source: "data-staging".to_string(),
            domain: "market-data".to_string(),
            event_type: "market_data.price_update".to_string(),
            schema_version: "1.0".to_string(),
            created_at,
            ingested_at,
            routing,
            quality,
            payload: Some(payload_any),
            headers,
            tracing,
        };
        
        // Final validation
        self.validate_event_envelope(&event_envelope)?;
        
        Ok(event_envelope)
    }
    
    /// Create market data proto payload as serialized bytes
    fn create_market_data_proto(&self, raw_data: &RawMarketData) -> Result<Vec<u8>> {
        // Create a simple JSON representation as payload for now
        // This could be replaced with actual proto types from market_data.proto
        let payload = serde_json::json!({
            "symbol": raw_data.symbol,
            "price": raw_data.price,
            "volume": raw_data.volume,
            "timestamp": raw_data.timestamp,
            "bid": raw_data.bid,
            "ask": raw_data.ask,
            "exchange": raw_data.exchange,
            "sequence": raw_data.sequence,
            "high": raw_data.high,
            "low": raw_data.low,
            "open": raw_data.open,
            "close": raw_data.close,
            "vwap": raw_data.vwap
        });
        
        Ok(serde_json::to_vec(&payload)?)
    }
    
    /// Calculate message priority based on data characteristics
    fn calculate_priority(&self, raw_data: &RawMarketData) -> i32 {
        let mut priority = 5; // Default priority
        
        // High-value stocks get higher priority
        if let Some(price) = raw_data.price {
            if price > 1000.0 {
                priority -= 1; // Higher priority (lower number)
            }
        }
        
        // Large volumes get higher priority
        if let Some(volume) = raw_data.volume {
            if volume > 100_000.0 {
                priority -= 1;
            }
        }
        
        // Recent data gets higher priority
        if let Some(timestamp) = raw_data.timestamp {
            let age_seconds = chrono::Utc::now().timestamp() - timestamp;
            if age_seconds < 30 {
                priority -= 1; // Very fresh data
            }
        }
        
        priority.max(0).min(9) // Clamp between 0 (highest) and 9 (lowest)
    }
    
    /// Generate routing tags for filtering
    fn generate_routing_tags(&self, raw_data: &RawMarketData) -> Vec<String> {
        let mut tags = Vec::new();
        
        if let Some(ref symbol) = raw_data.symbol {
            tags.push(format!("symbol:{}", symbol));
            
            // Add sector tags for common symbols
            if ["AAPL", "MSFT", "GOOGL", "AMZN", "TSLA"].contains(&symbol.as_str()) {
                tags.push("sector:tech".to_string());
            }
        }
        
        if let Some(ref exchange) = raw_data.exchange {
            tags.push(format!("exchange:{}", exchange));
        }
        
        if let Some(price) = raw_data.price {
            if price > 1000.0 {
                tags.push("price:high".to_string());
            } else if price < 10.0 {
                tags.push("price:low".to_string());
            } else {
                tags.push("price:medium".to_string());
            }
        }
        
        tags.push("data_type:real_time".to_string());
        
        tags
    }
    
    /// Generate correlation ID for tracking related messages
    fn generate_correlation_id(&self, raw_data: &RawMarketData) -> String {
        let symbol = raw_data.symbol.as_deref().unwrap_or("UNKNOWN");
        let timestamp = raw_data.timestamp.unwrap_or(chrono::Utc::now().timestamp());
        format!("{}_{}", symbol, timestamp)
    }
    
    /// Validate the created EventEnvelope
    fn validate_event_envelope(&self, envelope: &EventEnvelope) -> Result<()> {
        if envelope.message_id.is_empty() {
            return Err(DataStagingError::Validation("EventEnvelope missing message_id".to_string()).into());
        }
        
        if envelope.source.is_empty() {
            return Err(DataStagingError::Validation("EventEnvelope missing source".to_string()).into());
        }
        
        if envelope.event_type.is_empty() {
            return Err(DataStagingError::Validation("EventEnvelope missing event_type".to_string()).into());
        }
        
        if envelope.payload.is_none() {
            return Err(DataStagingError::Validation("EventEnvelope missing payload".to_string()).into());
        }
        
        if envelope.created_at.is_none() {
            return Err(DataStagingError::Validation("EventEnvelope missing created_at timestamp".to_string()).into());
        }
        
        if envelope.ingested_at.is_none() {
            return Err(DataStagingError::Validation("EventEnvelope missing ingested_at timestamp".to_string()).into());
        }
        
        Ok(())
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    
    fn create_test_raw_data() -> RawMarketData {
        RawMarketData {
            symbol: Some("AAPL".to_string()),
            price: Some(150.25),
            volume: Some(1000.0),
            timestamp: Some(chrono::Utc::now().timestamp()),
            bid: Some(150.20),
            ask: Some(150.30),
            exchange: Some("NASDAQ".to_string()),
            sequence: Some(12345),
            high: Some(151.0),
            low: Some(149.5),
            open: Some(150.0),
            close: Some(150.25),
            vwap: Some(150.1),
            metadata: HashMap::new(),
        }
    }
    
    fn create_test_quality_metrics() -> DataQualityMetrics {
        DataQualityMetrics {
            overall_score: 0.95,
            freshness_score: 1.0,
            completeness_score: 0.9,
            validity_score: 1.0,
            missing_required_fields: 0,
            present_optional_fields: 8,
            data_age_seconds: 5,
            validation_errors: vec![],
        }
    }
    
    #[test]
    fn test_transform_to_event_envelope() {
        let transformer = ProtoTransformer::new();
        let raw_data = create_test_raw_data();
        let quality_metrics = create_test_quality_metrics();
        
        let result = transformer.transform_to_event_envelope(&raw_data, &quality_metrics);
        assert!(result.is_ok());
        
        let envelope = result.unwrap();
        assert!(!envelope.message_id.is_empty());
        assert_eq!(envelope.source, "data-staging");
        assert_eq!(envelope.event_type, "market_data.price_update");
        assert!(envelope.payload.is_some());
        assert!(envelope.created_at.is_some());
        assert!(envelope.ingested_at.is_some());
    }
    
    #[test]
    fn test_priority_calculation() {
        let transformer = ProtoTransformer::new();
        
        // High-price stock should get higher priority
        let mut raw_data = create_test_raw_data();
        raw_data.price = Some(2000.0); // Expensive stock
        raw_data.volume = Some(200_000.0); // High volume
        
        let priority = transformer.calculate_priority(&raw_data);
        assert!(priority < 5); // Should be higher than default
    }
    
    #[test]
    fn test_routing_tags_generation() {
        let transformer = ProtoTransformer::new();
        let raw_data = create_test_raw_data();
        
        let tags = transformer.generate_routing_tags(&raw_data);
        
        assert!(tags.contains(&"symbol:AAPL".to_string()));
        assert!(tags.contains(&"exchange:NASDAQ".to_string()));
        assert!(tags.contains(&"sector:tech".to_string()));
        assert!(tags.contains(&"price:medium".to_string()));
        assert!(tags.contains(&"data_type:real_time".to_string()));
    }
    
    #[test]
    fn test_correlation_id_generation() {
        let transformer = ProtoTransformer::new();
        let raw_data = create_test_raw_data();
        
        let correlation_id = transformer.generate_correlation_id(&raw_data);
        assert!(correlation_id.starts_with("AAPL_"));
        assert!(correlation_id.contains(&raw_data.timestamp.unwrap().to_string()));
    }
    
    #[test] 
    fn test_validation_fails_for_invalid_envelope() {
        let transformer = ProtoTransformer::new();
        
        // Create invalid envelope (missing required fields)
        let invalid_envelope = EventEnvelope {
            message_id: String::new(), // Empty message_id should fail
            correlation_id: String::new(),
            source: String::new(),
            domain: String::new(),
            event_type: String::new(),
            schema_version: String::new(),
            created_at: None,
            ingested_at: None,
            routing: None,
            quality: None,
            payload: None,
            headers: HashMap::new(),
            tracing: None,
        };
        
        let result = transformer.validate_event_envelope(&invalid_envelope);
        assert!(result.is_err());
    }
}