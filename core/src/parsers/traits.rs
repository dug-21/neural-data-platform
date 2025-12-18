//! Parser trait definition
//!
//! This trait defines the interface for all parsers in the NDP system.
//! Parsers convert raw JSON payloads into TimeSeriesPoint vectors.

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::error::CoreResult;
use crate::traits::TimeSeriesPoint;

use super::ParserConfig;

/// Main parser trait - all parsers must implement this
pub trait Parser: Send + Sync {
    /// Parse raw JSON payload into time series points
    fn parse(&self, payload: &Value, timestamp: DateTime<Utc>) -> CoreResult<Vec<TimeSeriesPoint>>;

    /// Return parser name for logging/debugging
    fn name(&self) -> &str;

    /// Return parser configuration for introspection
    fn config(&self) -> &ParserConfig;
}
