// AIR-004: Multi-Stream Data Platform Types
//
// This module provides type definitions for the multi-stream platform,
// extending the existing single-stream architecture with backward compatibility.

pub mod air_quality;
pub mod dimension_config;
pub mod raw_data_point;
pub mod stream_config;
pub mod stream_record;

// BUG-001-fix: Re-export types from ndp-types for backward compatibility
// Consumers can use either:
//   use neural_core::types::SourceType;  (re-exported)
//   use ndp_types::SourceType;           (preferred)
pub use ndp_types::{DqAction, DqRuleType, ErrorCode, FieldType, MonotonicDirection, SourceType};

// Re-export existing types for backward compatibility
pub use air_quality::{AirQualityReading, GenericTimeSeriesPoint};

// DP-004: Bronze layer raw data types
pub use raw_data_point::RawDataPoint;

// Re-export new multi-stream types
// Note: FieldType and SourceType are re-exported from ndp-types above
pub use stream_config::{
    CsvSourceConfig, EntitySchema, EntitySchemaAttribute, OnError, SchemaField,
    SourceConfig, StorageConfig, StreamConfig, StreamConfigError, TimestampFormat,
};
pub use stream_record::{RecordMetadata, StreamRecord};

// DP-013: Dimension table configuration types
pub use dimension_config::{
    DimensionConfig, DimensionField, DimensionSchema, DimensionSource, DimensionSourceType,
    DimensionTarget, FieldType as DimensionFieldType, IndexConfig, LoadConfig, LoadStrategy,
};
