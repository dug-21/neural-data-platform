use crate::traits::TimeSeriesPoint;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Metadata attached to a stream record during ingestion
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecordMetadata {
    /// Source ID that generated this record
    pub source_id: String,
    /// Source type (mqtt, http_poll, webhook, etc.)
    pub source_type: String,
    /// Time when record entered the platform
    pub ingestion_time: DateTime<Utc>,
}

/// StreamRecord wraps TimeSeriesPoint with stream context and metadata
/// This allows multi-stream support while preserving backward compatibility
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamRecord {
    /// Stream identifier this record belongs to
    pub stream_id: String,
    /// The actual time series data point
    pub point: TimeSeriesPoint,
    /// Optional metadata about ingestion
    pub metadata: Option<RecordMetadata>,
}

impl StreamRecord {
    /// Create a new StreamRecord with required fields
    pub fn new(stream_id: String, point: TimeSeriesPoint) -> Self {
        Self {
            stream_id,
            point,
            metadata: None,
        }
    }

    /// Create a StreamRecord with full metadata
    pub fn with_metadata(
        stream_id: String,
        point: TimeSeriesPoint,
        source_id: String,
        source_type: String,
    ) -> Self {
        Self {
            stream_id,
            point,
            metadata: Some(RecordMetadata {
                source_id,
                source_type,
                ingestion_time: Utc::now(),
            }),
        }
    }

    /// Add metadata to an existing record
    pub fn with_metadata_details(mut self, source_id: String, source_type: String) -> Self {
        self.metadata = Some(RecordMetadata {
            source_id,
            source_type,
            ingestion_time: Utc::now(),
        });
        self
    }

    /// Get the timestamp from the underlying point
    pub fn timestamp(&self) -> DateTime<Utc> {
        self.point.timestamp
    }

    /// Get the location_id from the underlying point
    pub fn location_id(&self) -> &str {
        &self.point.location_id
    }

    /// Get the value from the underlying point
    pub fn value(&self) -> f64 {
        self.point.value
    }
}

/// Convert TimeSeriesPoint to StreamRecord with default stream ID
/// This provides backward compatibility for existing code
impl From<TimeSeriesPoint> for StreamRecord {
    fn from(point: TimeSeriesPoint) -> Self {
        Self {
            stream_id: "air-quality".to_string(), // Default for backward compatibility
            point,
            metadata: None,
        }
    }
}

/// Convert tuple of (stream_id, point) to StreamRecord
impl From<(String, TimeSeriesPoint)> for StreamRecord {
    fn from((stream_id, point): (String, TimeSeriesPoint)) -> Self {
        Self::new(stream_id, point)
    }
}

/// Convert StreamRecord back to TimeSeriesPoint
/// This allows seamless integration with existing storage layers
impl From<StreamRecord> for TimeSeriesPoint {
    fn from(record: StreamRecord) -> Self {
        record.point
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // ========== LONDON SCHOOL TDD: UNIT TESTS ==========

    #[test]
    fn test_stream_record_new_creates_record_without_metadata() {
        let point = create_test_point();
        let record = StreamRecord::new("test-stream".to_string(), point.clone());

        assert_eq!(record.stream_id, "test-stream");
        assert_eq!(record.point, point);
        assert!(record.metadata.is_none());
    }

    #[test]
    fn test_stream_record_with_metadata_creates_full_record() {
        let point = create_test_point();
        let record = StreamRecord::with_metadata(
            "test-stream".to_string(),
            point.clone(),
            "source-001".to_string(),
            "mqtt".to_string(),
        );

        assert_eq!(record.stream_id, "test-stream");
        assert_eq!(record.point, point);
        assert!(record.metadata.is_some());

        let metadata = record.metadata.unwrap();
        assert_eq!(metadata.source_id, "source-001");
        assert_eq!(metadata.source_type, "mqtt");
    }

    #[test]
    fn test_stream_record_with_metadata_details_adds_metadata() {
        let point = create_test_point();
        let record = StreamRecord::new("test-stream".to_string(), point.clone())
            .with_metadata_details("source-002".to_string(), "http_poll".to_string());

        assert!(record.metadata.is_some());
        let metadata = record.metadata.unwrap();
        assert_eq!(metadata.source_id, "source-002");
        assert_eq!(metadata.source_type, "http_poll");
    }

    #[test]
    fn test_stream_record_timestamp_accessor() {
        let now = Utc::now();
        let point = TimeSeriesPoint {
            timestamp: now,
            location_id: "loc-001".to_string(),
            value: 42.0,
            tags: HashMap::new(),
            ndp_id: None,
            context: None,
        };
        let record = StreamRecord::new("test-stream".to_string(), point);

        assert_eq!(record.timestamp(), now);
    }

    #[test]
    fn test_stream_record_location_id_accessor() {
        let point = create_test_point();
        let record = StreamRecord::new("test-stream".to_string(), point);

        assert_eq!(record.location_id(), "test-location");
    }

    #[test]
    fn test_stream_record_value_accessor() {
        let point = create_test_point();
        let record = StreamRecord::new("test-stream".to_string(), point);

        assert_eq!(record.value(), 23.5);
    }

    #[test]
    fn test_from_time_series_point_uses_default_stream_id() {
        let point = create_test_point();
        let record = StreamRecord::from(point.clone());

        assert_eq!(record.stream_id, "air-quality");
        assert_eq!(record.point, point);
        assert!(record.metadata.is_none());
    }

    #[test]
    fn test_from_tuple_creates_record() {
        let point = create_test_point();
        let record = StreamRecord::from(("custom-stream".to_string(), point.clone()));

        assert_eq!(record.stream_id, "custom-stream");
        assert_eq!(record.point, point);
    }

    #[test]
    fn test_into_time_series_point_extracts_point() {
        let point = create_test_point();
        let record = StreamRecord::new("test-stream".to_string(), point.clone());

        let extracted: TimeSeriesPoint = record.into();
        assert_eq!(extracted, point);
    }

    #[test]
    fn test_record_metadata_equality() {
        let metadata1 = RecordMetadata {
            source_id: "source-001".to_string(),
            source_type: "mqtt".to_string(),
            ingestion_time: Utc::now(),
        };

        let metadata2 = RecordMetadata {
            source_id: "source-001".to_string(),
            source_type: "mqtt".to_string(),
            ingestion_time: metadata1.ingestion_time,
        };

        assert_eq!(metadata1, metadata2);
    }

    #[test]
    fn test_record_metadata_inequality_different_source() {
        let now = Utc::now();
        let metadata1 = RecordMetadata {
            source_id: "source-001".to_string(),
            source_type: "mqtt".to_string(),
            ingestion_time: now,
        };

        let metadata2 = RecordMetadata {
            source_id: "source-002".to_string(),
            source_type: "mqtt".to_string(),
            ingestion_time: now,
        };

        assert_ne!(metadata1, metadata2);
    }

    #[test]
    fn test_stream_record_serialization() {
        let point = create_test_point();
        let record = StreamRecord::with_metadata(
            "test-stream".to_string(),
            point,
            "source-001".to_string(),
            "mqtt".to_string(),
        );

        let json = serde_json::to_string(&record).expect("Serialization should succeed");
        let deserialized: StreamRecord =
            serde_json::from_str(&json).expect("Deserialization should succeed");

        assert_eq!(deserialized.stream_id, record.stream_id);
        assert_eq!(deserialized.point, record.point);
        assert_eq!(
            deserialized.metadata.as_ref().unwrap().source_id,
            record.metadata.as_ref().unwrap().source_id
        );
    }

    #[test]
    fn test_stream_record_with_tags() {
        let mut tags = HashMap::new();
        tags.insert("sensor_type".to_string(), "PM2.5".to_string());
        tags.insert("location".to_string(), "living-room".to_string());

        let point = TimeSeriesPoint {
            timestamp: Utc::now(),
            location_id: "sensor-001".to_string(),
            value: 15.5,
            tags,
            ndp_id: None,
            context: None,
        };

        let record = StreamRecord::new("air-quality".to_string(), point.clone());

        assert_eq!(
            record.point.tags.get("sensor_type"),
            Some(&"PM2.5".to_string())
        );
        assert_eq!(
            record.point.tags.get("location"),
            Some(&"living-room".to_string())
        );
    }

    #[test]
    fn test_stream_record_clone() {
        let point = create_test_point();
        let record = StreamRecord::with_metadata(
            "test-stream".to_string(),
            point,
            "source-001".to_string(),
            "mqtt".to_string(),
        );

        let cloned = record.clone();
        assert_eq!(cloned, record);
    }

    // ========== HELPER FUNCTIONS ==========

    fn create_test_point() -> TimeSeriesPoint {
        TimeSeriesPoint {
            timestamp: Utc::now(),
            location_id: "test-location".to_string(),
            value: 23.5,
            tags: HashMap::new(),
            ndp_id: None,
            context: None,
        }
    }
}
