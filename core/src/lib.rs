pub mod coordinator;
pub mod error;
// pub mod forecast;
pub mod parsers;
pub mod sources;
pub mod storage;
pub mod traits;
pub mod types;

pub use coordinator::{IngestionCoordinator, SourceManager};
pub use error::CoreError;
// pub use forecast::{FannForecaster, ModelType};
pub use parsers::{FlatJsonParser, Parser, ParserConfig, ParserType};
pub use sources::{
    HttpPollingConfig, HttpPollingSource, MergeConfig, MqttConfig, MqttSource, ReadingMerger,
    SensorConfig,
};
pub use storage::{ParquetStore, WriteAheadLog};
pub use traits::{
    AggregatedPoint, AggregationType, Forecast, ForecastedPoint, HealthStatus, ModelMetrics,
    RawSource, Source, Store, TimeSeriesPoint,
};

// Existing types (backward compatibility)
pub use types::GenericTimeSeriesPoint;

// New multi-stream types (AIR-004)
pub use types::{
    FieldType, RecordMetadata, SchemaField, SourceConfig, SourceType, StorageConfig, StreamConfig,
    StreamConfigError, StreamRecord,
};
