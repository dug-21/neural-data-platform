//! Parser trait definition
//!
//! This trait defines the interface for all parsers in the NDP system.
//! Parsers convert raw JSON payloads into TimeSeriesPoint vectors.

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::error::CoreResult;
use crate::traits::TimeSeriesPoint;

use super::ParserConfig;

/// Context passed to parsers for ndp_id and context injection
///
/// This struct carries stable identifiers and mutable context through
/// the parsing pipeline, enabling parsers to inject this metadata into
/// the resulting TimeSeriesPoints.
#[derive(Debug, Clone, Default)]
pub struct ParseContext {
    /// Stable source identifier from config (e.g., "sensor-001")
    pub ndp_id: Option<String>,
    /// Mutable attributes as JSON blob (e.g., {"room": "office"})
    pub context: Option<Value>,
}

impl ParseContext {
    /// Create a new ParseContext with optional ndp_id and context
    pub fn new(ndp_id: Option<String>, context: Option<Value>) -> Self {
        Self { ndp_id, context }
    }
}

/// Main parser trait - all parsers must implement this
pub trait Parser: Send + Sync {
    /// Parse raw JSON payload into time series points
    fn parse(&self, payload: &Value, timestamp: DateTime<Utc>) -> CoreResult<Vec<TimeSeriesPoint>>;

    /// Return parser name for logging/debugging
    fn name(&self) -> &str;

    /// Return parser configuration for introspection
    fn config(&self) -> &ParserConfig;

    /// Parse with context for ndp_id and context injection
    ///
    /// This method allows parsers to receive stable identifiers and mutable
    /// context that can be injected into the resulting TimeSeriesPoints.
    /// Default implementation ignores context for backward compatibility.
    fn parse_with_context(
        &self,
        payload: &Value,
        timestamp: DateTime<Utc>,
        _context: &ParseContext,
    ) -> CoreResult<Vec<TimeSeriesPoint>> {
        // Default implementation ignores context for backward compatibility
        self.parse(payload, timestamp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_context_creation() {
        let ctx = ParseContext::new(
            Some("sensor-001".to_string()),
            Some(serde_json::json!({"room": "office"})),
        );

        assert_eq!(ctx.ndp_id, Some("sensor-001".to_string()));
        assert!(ctx.context.is_some());

        // Verify the context value
        let context_value = ctx.context.unwrap();
        assert_eq!(context_value["room"], "office");
    }

    #[test]
    fn test_parse_context_empty() {
        let ctx = ParseContext::default();
        assert!(ctx.ndp_id.is_none());
        assert!(ctx.context.is_none());
    }

    #[test]
    fn test_parse_context_with_ndp_id_only() {
        let ctx = ParseContext::new(Some("my-sensor".to_string()), None);

        assert_eq!(ctx.ndp_id, Some("my-sensor".to_string()));
        assert!(ctx.context.is_none());
    }

    #[test]
    fn test_parse_context_with_context_only() {
        let ctx = ParseContext::new(
            None,
            Some(serde_json::json!({"location": "basement", "floor": 1})),
        );

        assert!(ctx.ndp_id.is_none());
        assert!(ctx.context.is_some());

        let context_value = ctx.context.unwrap();
        assert_eq!(context_value["location"], "basement");
        assert_eq!(context_value["floor"], 1);
    }

    #[test]
    fn test_parse_context_clone() {
        let ctx = ParseContext::new(
            Some("sensor-001".to_string()),
            Some(serde_json::json!({"room": "office"})),
        );

        let cloned = ctx.clone();

        assert_eq!(cloned.ndp_id, ctx.ndp_id);
        assert_eq!(cloned.context, ctx.context);
    }

    #[test]
    fn test_parse_context_debug() {
        let ctx = ParseContext::new(
            Some("sensor-001".to_string()),
            Some(serde_json::json!({"room": "office"})),
        );

        // Verify Debug trait is implemented
        let debug_str = format!("{:?}", ctx);
        assert!(debug_str.contains("sensor-001"));
        assert!(debug_str.contains("office"));
    }
}
