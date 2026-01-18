//! Silver output sink traits and implementations
//!
//! This module defines the SilverOutput trait for writing transformed
//! Silver records to storage backends (TimescaleDB, in-memory for testing).

pub mod timescale;

pub use timescale::TimescaleOutput;

use crate::silver::types::SilverRecord;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use thiserror::Error;

/// Errors from Silver output operations
#[derive(Debug, Error)]
pub enum SilverOutputError {
    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Write error: {0}")]
    WriteError(String),

    #[error("Query error: {0}")]
    QueryError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// Trait for Silver layer output sinks
#[async_trait]
pub trait SilverOutput: Send + Sync {
    /// Write a single Silver record
    async fn write(&self, record: &SilverRecord) -> Result<(), SilverOutputError>;

    /// Write a batch of Silver records
    async fn write_batch(&self, records: &[SilverRecord]) -> Result<usize, SilverOutputError> {
        let mut written = 0;
        for record in records {
            if !record.should_drop() {
                self.write(record).await?;
                written += 1;
            }
        }
        Ok(written)
    }

    /// Get the high-water mark (latest timestamp) for a stream
    async fn get_watermark(
        &self,
        stream_id: &str,
    ) -> Result<Option<DateTime<Utc>>, SilverOutputError>;

    /// Health check for the output sink
    async fn health_check(&self) -> Result<bool, SilverOutputError>;

    /// Flush any buffered writes
    async fn flush(&self) -> Result<(), SilverOutputError> {
        Ok(())
    }
}

/// In-memory Silver output for testing
#[derive(Debug, Default)]
pub struct InMemorySilverOutput {
    records: std::sync::RwLock<Vec<SilverRecord>>,
    watermarks: std::sync::RwLock<HashMap<String, DateTime<Utc>>>,
}

impl InMemorySilverOutput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_records(&self) -> Vec<SilverRecord> {
        self.records.read().unwrap().clone()
    }

    pub fn get_records_for_stream(&self, stream_id: &str) -> Vec<SilverRecord> {
        self.records
            .read()
            .unwrap()
            .iter()
            .filter(|r| r.stream_id == stream_id)
            .cloned()
            .collect()
    }

    pub fn clear(&self) {
        self.records.write().unwrap().clear();
    }

    pub fn set_watermark(&self, stream_id: &str, timestamp: DateTime<Utc>) {
        self.watermarks
            .write()
            .unwrap()
            .insert(stream_id.to_string(), timestamp);
    }
}

#[async_trait]
impl SilverOutput for InMemorySilverOutput {
    async fn write(&self, record: &SilverRecord) -> Result<(), SilverOutputError> {
        let mut records = self.records.write().unwrap();

        // UPSERT: find existing record
        let existing_idx = records.iter().position(|r| {
            r.stream_id == record.stream_id
                && r.timestamp == record.timestamp
                && r.device_id == record.device_id
        });

        match existing_idx {
            Some(idx) => {
                records[idx] = record.clone();
            }
            None => {
                records.push(record.clone());
            }
        }

        // Update watermark
        let mut watermarks = self.watermarks.write().unwrap();
        watermarks
            .entry(record.stream_id.clone())
            .and_modify(|ts| {
                if record.timestamp > *ts {
                    *ts = record.timestamp;
                }
            })
            .or_insert(record.timestamp);

        Ok(())
    }

    async fn get_watermark(
        &self,
        stream_id: &str,
    ) -> Result<Option<DateTime<Utc>>, SilverOutputError> {
        Ok(self.watermarks.read().unwrap().get(stream_id).copied())
    }

    async fn health_check(&self) -> Result<bool, SilverOutputError> {
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    #[tokio::test]
    async fn test_in_memory_output_write() {
        let output = InMemorySilverOutput::new();
        let ts = Utc.with_ymd_and_hms(2026, 1, 18, 12, 0, 0).unwrap();

        let record = SilverRecord::new("air-quality", ts)
            .with_device_id("device-001")
            .with_field("pm25", json!(12.5));

        output.write(&record).await.unwrap();

        let records = output.get_records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].stream_id, "air-quality");
    }

    #[tokio::test]
    async fn test_in_memory_output_upsert() {
        let output = InMemorySilverOutput::new();
        let ts = Utc.with_ymd_and_hms(2026, 1, 18, 12, 0, 0).unwrap();

        let record1 = SilverRecord::new("air-quality", ts)
            .with_device_id("device-001")
            .with_field("pm25", json!(12.5));
        output.write(&record1).await.unwrap();

        let record2 = SilverRecord::new("air-quality", ts)
            .with_device_id("device-001")
            .with_field("pm25", json!(15.0));
        output.write(&record2).await.unwrap();

        let records = output.get_records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].get_field_as_f64("pm25"), Some(15.0));
    }

    #[tokio::test]
    async fn test_in_memory_output_watermark() {
        let output = InMemorySilverOutput::new();
        let ts1 = Utc.with_ymd_and_hms(2026, 1, 18, 12, 0, 0).unwrap();
        let ts2 = Utc.with_ymd_and_hms(2026, 1, 18, 13, 0, 0).unwrap();

        let record1 = SilverRecord::new("air-quality", ts1).with_device_id("d1");
        let record2 = SilverRecord::new("air-quality", ts2).with_device_id("d2");

        output.write(&record1).await.unwrap();
        output.write(&record2).await.unwrap();

        let watermark = output.get_watermark("air-quality").await.unwrap();
        assert_eq!(watermark, Some(ts2));
    }

    #[tokio::test]
    async fn test_in_memory_output_health_check() {
        let output = InMemorySilverOutput::new();
        assert!(output.health_check().await.unwrap());
    }
}
