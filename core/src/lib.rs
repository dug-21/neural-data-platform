pub mod config;
pub mod coordinator;
pub mod dimensions;
pub mod error;
pub mod event_bus;
// pub mod forecast;
pub mod mcp;
pub mod outputs;
pub mod parsers;
pub mod processors;
pub mod silver;
pub mod sources;
pub mod storage;
pub mod subscribers;
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
    EntitySchema, EntitySchemaAttribute, FieldType, RecordMetadata, SchemaField, SourceConfig,
    SourceType, StorageConfig, StreamConfig, StreamConfigError, StreamRecord,
};

// Silver ETL config types (DP-006)
pub use config::{
    ConversionFormula, DeduplicationConfig, DeduplicationStrategy, DqAction, DqOutputConfig,
    DqRule, IdentityField, IncrementalConfig, SilverConfigError, SilverEtlConfig,
    SilverFieldMapping, TimestampMapping, TimestampTransform, TransformConfig,
};

// Pre-transform config types (DP-007)
pub use config::{
    ArrayExplosionConfig, FieldSource, MetricExplosionMapping, PreTransformConfig,
    PreTransformType, ValidTimestampMapping, ValidTimestampSource,
};

// Event Bus (DP-012)
pub use event_bus::{EventBus, EventBusConfig, EventBusError, EventBusMetrics, OverflowStrategy};

// Subscribers (DP-012)
pub use subscribers::{
    BronzeReader, BronzeSubscriber, BronzeSubscriberConfig, CatchUpConfig, CoordinatorHealth,
    CoordinatorState, EventNotification, EventNotifier, EventNotifierConfig, EventNotifierState,
    NoBronzeReader, ProcessorSubscriber, ProcessorSubscriberConfig, ProcessorSubscriberState,
    SilverSubscriber, SilverSubscriberConfig, Subscriber, SubscriberCoordinator, SubscriberError,
    SubscriberState,
};

// Processors (DP-012 Phase 3)
pub use processors::{
    Alert, AlertSeverity, Metric, Processor, ProcessorConfig, ProcessorEvent, ProcessorOutput,
    Severity, ThresholdAlert, ThresholdConfig, ThresholdProcessor, ThresholdRule,
};

// Outputs (DP-012 Phase 3)
pub use outputs::{MqttOutput, MqttOutputConfig, OutputError, OutputSink};

// Silver layer (DP-012 Phase 2)
pub use silver::{
    evaluate_and_apply_dq_rules, evaluate_dq_rules, transform_to_silver, DqResult, DqViolation,
    InMemorySilverOutput, SilverOutput, SilverOutputError, SilverRecord, TimescaleOutput,
    TransformError,
};

// Dimension tables (DP-013)
pub use dimensions::{
    CsvDimensionLoader, DdlGenerator, DimensionError, DimensionLoadStats, DimensionLoader,
};
pub use types::{
    DimensionConfig, DimensionField, DimensionFieldType, DimensionSchema, DimensionSource,
    DimensionSourceType, DimensionTarget, IndexConfig, LoadConfig, LoadStrategy,
};

// TimescaleDB dimension loading (DP-013, requires 'timescale' feature)
#[cfg(feature = "timescale")]
pub use dimensions::TimescaleDimensionLoader;
