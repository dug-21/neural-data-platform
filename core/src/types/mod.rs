// AIR-004: Multi-Stream Data Platform Types
//
// This module provides type definitions for the multi-stream platform,
// extending the existing single-stream architecture with backward compatibility.

pub mod air_quality;
pub mod raw_data_point;
pub mod stream_config;
pub mod stream_record;

// Re-export existing types for backward compatibility
pub use air_quality::{AirQualityReading, GenericTimeSeriesPoint};

// DP-004: Bronze layer raw data types
pub use raw_data_point::RawDataPoint;

// Re-export new multi-stream types
pub use stream_config::{
    FieldType, SchemaField, SourceConfig, SourceType, StorageConfig, StreamConfig,
    StreamConfigError,
};
pub use stream_record::{RecordMetadata, StreamRecord};
