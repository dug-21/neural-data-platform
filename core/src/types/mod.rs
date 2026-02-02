// AIR-004: Multi-Stream Data Platform Types
//
// This module provides type definitions for the multi-stream platform,
// extending the existing single-stream architecture with backward compatibility.

pub mod air_quality;
pub mod dimension_config;
pub mod raw_data_point;
pub mod stream_config;
pub mod stream_record;

// Re-export existing types for backward compatibility
pub use air_quality::{AirQualityReading, GenericTimeSeriesPoint};

// DP-004: Bronze layer raw data types
pub use raw_data_point::RawDataPoint;

// Re-export new multi-stream types
pub use stream_config::{
    CsvSourceConfig, EntitySchema, EntitySchemaAttribute, FieldType, OnError, SchemaField,
    SourceConfig, SourceType, StorageConfig, StreamConfig, StreamConfigError, TimestampFormat,
};
pub use stream_record::{RecordMetadata, StreamRecord};

// DP-013: Dimension table configuration types
pub use dimension_config::{
    DimensionConfig, DimensionField, DimensionSchema, DimensionSource, DimensionSourceType,
    DimensionTarget, FieldType as DimensionFieldType, IndexConfig, LoadConfig, LoadStrategy,
};
