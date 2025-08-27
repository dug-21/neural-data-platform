//! Proto transformation module - JSON to Protocol Buffer conversion

use crate::{DataStagingError, DataStagingResult, generated::*, redis_consumer::RawMessage};
use chrono::{DateTime, Utc};
use prost::Message;
use prost_types::{Any, Timestamp};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, error, warn};
use uuid::Uuid;

/// Proto transformation engine
pub struct ProtoTransformer {
    /// Source identifier for messages
    source_id: String,
    
    /// Domain for events
    domain: String,
    
    /// Enable detailed logging
    verbose: bool,
}

impl ProtoTransformer {
    /// Create new proto transformer
    pub fn new(source_id: String, domain: String, verbose: bool) -> Self {
        Self {
            source_id,
            domain,
            verbose,
        }
    }
    
    /// Transform raw message to EventEnvelope proto
    pub fn transform_to_proto(
        &self,
        raw_message: &RawMessage,
        quality_score: f32,
    ) -> DataStagingResult<EventEnvelope> {
        debug!("Transforming message {} to proto", raw_message.id);
        
        // Determine event type from data structure
        let event_type = self.infer_event_type(&raw_message.data)?;
        
        // Transform payload to appropriate proto message
        let payload_any = self.transform_payload(&raw_message.data, &event_type)?;
        
        // Create EventEnvelope
        let mut envelope = EventEnvelope {
            message_id: Uuid::new_v4().to_string(),
            correlation_id: raw_message.metadata.get("correlation_id")
                .cloned()
                .unwrap_or_else(|| raw_message.id.clone()),
            source: self.source_id.clone(),
            domain: self.domain.clone(),
            event_type: event_type.clone(),
            schema_version: "1.0".to_string(),
            created_at: Some(self.json_timestamp_to_proto(&raw_message.data)?),
            ingested_at: Some(Timestamp {
                seconds: Utc::now().timestamp(),
                nanos: 0,
            }),
            routing: Some(self.create_routing_metadata(&event_type)),
            quality: Some(self.create_quality_metadata(quality_score)),
            payload: Some(payload_any),
            headers: self.create_headers(&raw_message.metadata),
            tracing: Some(self.create_tracing_context(&raw_message.id)),
        };
        
        if self.verbose {
            debug!("Created EventEnvelope: message_id={}, event_type={}, quality_score={}", 
                   envelope.message_id, envelope.event_type, quality_score);
        }
        
        Ok(envelope)
    }
    
    /// Infer event type from JSON structure
    fn infer_event_type(&self, data: &Value) -> DataStagingResult<String> {
        // Check if it has typical market data fields
        if let Some(obj) = data.as_object() {
            if obj.contains_key("symbol") && obj.contains_key("price") {
                // Check for trade-specific fields
                if obj.contains_key("size") || obj.contains_key("volume") {
                    return Ok("market_data.trade".to_string());
                }
                
                // Check for quote-specific fields
                if obj.contains_key("bid") && obj.contains_key("ask") {
                    return Ok("market_data.quote".to_string());
                }
                
                // Check for bar data
                if obj.contains_key("open") && obj.contains_key("high") && 
                   obj.contains_key("low") && obj.contains_key("close") {
                    return Ok("market_data.bar".to_string());
                }
                
                // Generic market data
                return Ok("market_data.general".to_string());
            }
            
            // Check for news data
            if obj.contains_key("headline") || obj.contains_key("body") {
                return Ok("market_data.news".to_string());
            }
        }
        
        // Default to generic event
        Ok("generic".to_string())
    }
    
    /// Transform JSON payload to protobuf Any
    fn transform_payload(&self, data: &Value, event_type: &str) -> DataStagingResult<Any> {
        match event_type {
            "market_data.trade" => self.transform_trade_data(data),
            "market_data.quote" => self.transform_quote_data(data),
            "market_data.bar" => self.transform_bar_data(data),
            "market_data.news" => self.transform_news_data(data),
            "market_data.general" | _ => self.transform_generic_market_data(data),
        }
    }
    
    /// Transform to TradeData proto
    fn transform_trade_data(&self, data: &Value) -> DataStagingResult<Any> {
        let obj = data.as_object().ok_or_else(|| DataStagingError::InvalidFormat {
            message: "Expected JSON object for trade data".to_string(),
        })?;
        
        let trade_data = TradeData {
            price: obj.get("price")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| DataStagingError::MissingRequiredField {
                    field: "price".to_string(),
                })?,
            size: obj.get("size")
                .or_else(|| obj.get("volume"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            timestamp: Some(self.json_timestamp_to_proto(data)?),
            exchange: obj.get("exchange")
                .and_then(|v| v.as_str())
                .unwrap_or("UNKNOWN")
                .to_string(),
            sequence: obj.get("sequence")
                .or_else(|| obj.get("trade_id"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
        };
        
        let mut buf = Vec::new();
        trade_data.encode(&mut buf)?;
        
        Ok(Any {
            type_url: "type.googleapis.com/neural_trader.market_data.v1.TradeData".to_string(),
            value: buf,
        })
    }
    
    /// Transform to QuoteData proto
    fn transform_quote_data(&self, data: &Value) -> DataStagingResult<Any> {
        let obj = data.as_object().ok_or_else(|| DataStagingError::InvalidFormat {
            message: "Expected JSON object for quote data".to_string(),
        })?;
        
        let quote_data = QuoteData {
            bid_price: obj.get("bid")
                .or_else(|| obj.get("bid_price"))
                .and_then(|v| v.as_f64())
                .ok_or_else(|| DataStagingError::MissingRequiredField {
                    field: "bid".to_string(),
                })?,
            bid_size: obj.get("bid_size")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            ask_price: obj.get("ask")
                .or_else(|| obj.get("ask_price"))
                .and_then(|v| v.as_f64())
                .ok_or_else(|| DataStagingError::MissingRequiredField {
                    field: "ask".to_string(),
                })?,
            ask_size: obj.get("ask_size")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            timestamp: Some(self.json_timestamp_to_proto(data)?),
            exchange: obj.get("exchange")
                .and_then(|v| v.as_str())
                .unwrap_or("UNKNOWN")
                .to_string(),
        };
        
        let mut buf = Vec::new();
        quote_data.encode(&mut buf)?;
        
        Ok(Any {
            type_url: "type.googleapis.com/neural_trader.market_data.v1.QuoteData".to_string(),
            value: buf,
        })
    }
    
    /// Transform to BarData proto
    fn transform_bar_data(&self, data: &Value) -> DataStagingResult<Any> {
        let obj = data.as_object().ok_or_else(|| DataStagingError::InvalidFormat {
            message: "Expected JSON object for bar data".to_string(),
        })?;
        
        let bar_data = BarData {
            open: obj.get("open")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| DataStagingError::MissingRequiredField {
                    field: "open".to_string(),
                })?,
            high: obj.get("high")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| DataStagingError::MissingRequiredField {
                    field: "high".to_string(),
                })?,
            low: obj.get("low")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| DataStagingError::MissingRequiredField {
                    field: "low".to_string(),
                })?,
            close: obj.get("close")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| DataStagingError::MissingRequiredField {
                    field: "close".to_string(),
                })?,
            volume: obj.get("volume")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            vwap: obj.get("vwap")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            start_time: Some(self.json_timestamp_to_proto(data)?),
            end_time: obj.get("end_time")
                .and_then(|v| v.as_i64())
                .map(|ts| Timestamp { seconds: ts, nanos: 0 }),
            trade_count: obj.get("trade_count")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32,
        };
        
        let mut buf = Vec::new();
        bar_data.encode(&mut buf)?;
        
        Ok(Any {
            type_url: "type.googleapis.com/neural_trader.market_data.v1.BarData".to_string(),
            value: buf,
        })
    }
    
    /// Transform to NewsData proto
    fn transform_news_data(&self, data: &Value) -> DataStagingResult<Any> {
        let obj = data.as_object().ok_or_else(|| DataStagingError::InvalidFormat {
            message: "Expected JSON object for news data".to_string(),
        })?;
        
        let news_data = NewsData {
            headline: obj.get("headline")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            body: obj.get("body")
                .or_else(|| obj.get("content"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            source: obj.get("source")
                .and_then(|v| v.as_str())
                .unwrap_or("UNKNOWN")
                .to_string(),
            published_at: Some(self.json_timestamp_to_proto(data)?),
            symbols: obj.get("symbols")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.to_string())
                        .collect()
                })
                .unwrap_or_else(|| {
                    // If no symbols array, try to extract from symbol field
                    obj.get("symbol")
                        .and_then(|v| v.as_str())
                        .map(|s| vec![s.to_string()])
                        .unwrap_or_else(Vec::new)
                }),
            sentiment_score: obj.get("sentiment")
                .or_else(|| obj.get("sentiment_score"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
        };
        
        let mut buf = Vec::new();
        news_data.encode(&mut buf)?;
        
        Ok(Any {
            type_url: "type.googleapis.com/neural_trader.market_data.v1.NewsData".to_string(),
            value: buf,
        })
    }
    
    /// Transform to generic MarketDataEvent
    fn transform_generic_market_data(&self, data: &Value) -> DataStagingResult<Any> {
        // For generic market data, create a TradeData with minimal fields
        let obj = data.as_object().ok_or_else(|| DataStagingError::InvalidFormat {
            message: "Expected JSON object".to_string(),
        })?;
        
        let trade_data = TradeData {
            price: obj.get("price")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| DataStagingError::MissingRequiredField {
                    field: "price".to_string(),
                })?,
            size: obj.get("size")
                .or_else(|| obj.get("volume"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            timestamp: Some(self.json_timestamp_to_proto(data)?),
            exchange: obj.get("exchange")
                .and_then(|v| v.as_str())
                .unwrap_or("GENERIC")
                .to_string(),
            sequence: 0, // No sequence for generic data
        };
        
        let mut buf = Vec::new();
        trade_data.encode(&mut buf)?;
        
        Ok(Any {
            type_url: "type.googleapis.com/neural_trader.market_data.v1.TradeData".to_string(),
            value: buf,
        })
    }
    
    /// Extract timestamp from JSON and convert to proto Timestamp
    fn json_timestamp_to_proto(&self, data: &Value) -> DataStagingResult<Timestamp> {
        let timestamp = data.get("timestamp")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| DataStagingError::MissingRequiredField {
                field: "timestamp".to_string(),
            })?;
        
        Ok(Timestamp {
            seconds: timestamp,
            nanos: 0,
        })
    }
    
    /// Create routing metadata
    fn create_routing_metadata(&self, event_type: &str) -> RoutingMetadata {
        RoutingMetadata {
            topic: format!("neural-trader.{}", event_type),
            partition_key: "".to_string(), // Will be set based on symbol later
            priority: 5, // Medium priority
            ttl_seconds: 86400, // 24 hours
            tags: vec![event_type.to_string(), "data-staging".to_string()],
            retry_policy: Some(RetryPolicy {
                max_attempts: 3,
                initial_delay_ms: 1000,
                backoff_multiplier: 2.0,
                max_delay_ms: 10000,
                retryable_errors: vec![
                    "UNAVAILABLE".to_string(),
                    "DEADLINE_EXCEEDED".to_string(),
                ],
            }),
        }
    }
    
    /// Create quality metadata
    fn create_quality_metadata(&self, overall_score: f32) -> QualityMetadata {
        QualityMetadata {
            completeness: overall_score, // Simplified - use overall score
            latency_ms: 0, // Will be calculated later
            validation_status: if overall_score >= 0.7 {
                ValidationStatus::ValidationStatusPassed as i32
            } else if overall_score >= 0.5 {
                ValidationStatus::ValidationStatusPartial as i32
            } else {
                ValidationStatus::ValidationStatusFailed as i32
            },
            quality_score: overall_score,
            anomalies: vec![], // Anomaly detection handled at transform level
        }
    }
    
    /// Create headers from metadata
    fn create_headers(&self, metadata: &HashMap<String, String>) -> HashMap<String, String> {
        let mut headers = metadata.clone();
        headers.insert("transformer".to_string(), "data-staging".to_string());
        headers.insert("version".to_string(), "1.0".to_string());
        headers
    }
    
    /// Create tracing context
    fn create_tracing_context(&self, message_id: &str) -> TracingContext {
        TracingContext {
            trace_id: Uuid::new_v4().to_string(),
            span_id: Uuid::new_v4().to_string(),
            parent_span_id: "".to_string(),
            baggage: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn create_test_raw_message(data: Value) -> RawMessage {
        RawMessage {
            id: "test-123".to_string(),
            data,
            metadata: HashMap::new(),
            received_at: Utc::now(),
        }
    }

    #[test]
    fn test_transform_trade_data() {
        let transformer = ProtoTransformer::new(
            "test-source".to_string(),
            "trading".to_string(),
            false,
        );
        
        let data = json!({
            "symbol": "AAPL",
            "price": 150.25,
            "size": 100.0,
            "timestamp": 1640000000,
            "exchange": "NASDAQ"
        });
        
        let raw_message = create_test_raw_message(data);
        let result = transformer.transform_to_proto(&raw_message, 0.9);
        
        assert!(result.is_ok());
        let envelope = result.unwrap();
        assert_eq!(envelope.event_type, "market_data.trade");
        assert_eq!(envelope.source, "test-source");
        assert_eq!(envelope.domain, "trading");
        assert!(envelope.payload.is_some());
        assert!(envelope.quality.is_some());
    }
    
    #[test]
    fn test_infer_event_type_trade() {
        let transformer = ProtoTransformer::new(
            "test".to_string(),
            "test".to_string(),
            false,
        );
        
        let data = json!({
            "symbol": "AAPL",
            "price": 150.0,
            "size": 100.0
        });
        
        let event_type = transformer.infer_event_type(&data).unwrap();
        assert_eq!(event_type, "market_data.trade");
    }
    
    #[test]
    fn test_infer_event_type_quote() {
        let transformer = ProtoTransformer::new(
            "test".to_string(),
            "test".to_string(),
            false,
        );
        
        let data = json!({
            "symbol": "AAPL",
            "price": 150.0,
            "bid": 149.95,
            "ask": 150.05
        });
        
        let event_type = transformer.infer_event_type(&data).unwrap();
        assert_eq!(event_type, "market_data.quote");
    }
    
    #[test]
    fn test_missing_required_field() {
        let transformer = ProtoTransformer::new(
            "test".to_string(),
            "test".to_string(),
            false,
        );
        
        let data = json!({
            "symbol": "AAPL",
            "timestamp": 1640000000
            // Missing price
        });
        
        let raw_message = create_test_raw_message(data);
        let result = transformer.transform_to_proto(&raw_message, 0.9);
        
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DataStagingError::MissingRequiredField { .. }));
    }
}