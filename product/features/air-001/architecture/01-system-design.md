# SPARC Architecture: Neural Data Platform - Air Quality (air-001)

**Feature ID**: air-001
**Document Version**: 1.1
**Date**: 2025-12-13
**Architecture Phase**: SPARC - Architecture Design
**Revision**: Docker Deployment + Complete AirGradient Fields

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Workspace Structure](#2-workspace-structure)
3. [Component Design](#3-component-design)
4. [Data Flow Diagrams](#4-data-flow-diagrams)
5. [Integration with Existing Codebase](#5-integration-with-existing-codebase)
6. [Deployment Architecture](#6-deployment-architecture)
7. [Key Architectural Decisions (ADRs)](#7-key-architectural-decisions-adrs)
8. [Extension Points](#8-extension-points)
9. [C4 Architecture Diagrams](#9-c4-architecture-diagrams)

---

## 1. Architecture Overview

### 1.1 Hexagonal Architecture (Ports and Adapters)

The Neural Data Platform follows a **Hexagonal Architecture** pattern to achieve domain agnosticism and enable the air quality use case as the first domain implementation.

```
┌─────────────────────────────────────────────────────────────────┐
│                     PRIMARY ADAPTERS                            │
│         (Driving - Initiate interactions with core)             │
│                                                                 │
│   ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐     │
│   │   REST   │  │   CLI    │  │  gRPC    │  │ WebSocket│     │
│   │   API    │  │          │  │          │  │          │     │
│   └─────┬────┘  └─────┬────┘  └─────┬────┘  └─────┬────┘     │
│         └─────────────┴─────────────┴─────────────┘           │
└─────────────────────────┬───────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│                     DRIVING PORTS                               │
│                   (Use Case Interfaces)                         │
│                                                                 │
│   TimeSeriesQueryPort   │   DataIngestionPort                  │
│   ForecastingPort       │   ConfigurationPort                  │
└─────────────────────────┬───────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│                      DOMAIN CORE                                │
│              (Business Logic - Domain Agnostic)                 │
│                                                                 │
│   ┌──────────────────────────────────────────────────────┐     │
│   │  Generic Time-Series Platform                        │     │
│   │                                                       │     │
│   │  • TimeSeriesPoint trait                             │     │
│   │  • Store trait (storage abstraction)                 │     │
│   │  • Source trait (data source abstraction)            │     │
│   │  • Forecast trait (prediction abstraction)           │     │
│   │  • Domain-agnostic processing pipelines              │     │
│   └──────────────────────────────────────────────────────┘     │
└─────────────────────────┬───────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│                     DRIVEN PORTS                                │
│                (Infrastructure Interfaces)                      │
│                                                                 │
│   StoragePort    │   SourcePort    │   ForecastPort            │
│   (Repository)   │   (Ingestion)   │   (ML Models)             │
└─────────────────────────┬───────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│                    SECONDARY ADAPTERS                           │
│          (Driven - Called by core for infrastructure)           │
│                                                                 │
│   ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐     │
│   │ Parquet  │  │   MQTT   │  │ ruv-FANN │  │ Postgres │     │
│   │ Storage  │  │  Source  │  │ Forecast │  │  Config  │     │
│   └──────────┘  └──────────┘  └──────────┘  └──────────┘     │
│                                                                 │
│   ┌──────────────────────────────────────────────────────┐     │
│   │         Air Quality Domain Adapter                   │     │
│   │  (Parses PM2.5, CO2, etc. → Generic TimeSeriesPoint)│     │
│   └──────────────────────────────────────────────────────┘     │
└─────────────────────────────────────────────────────────────────┘
```

### 1.2 Layer Responsibilities

| Layer | Responsibility | Technology |
|-------|---------------|------------|
| **Primary Adapters** | API interfaces, CLI, user-facing protocols | Axum (REST), Clap (CLI), Tonic (gRPC) |
| **Driving Ports** | Use case interfaces defined by domain | Rust traits |
| **Domain Core** | Generic time-series platform logic | Pure Rust (minimal dependencies) |
| **Driven Ports** | Infrastructure abstractions | Rust traits |
| **Secondary Adapters** | Infrastructure implementations | Parquet, MQTT, ruv-FANN, Postgres |
| **Domain Adapters** | Domain-specific parsing/translation | Air quality types → Generic traits |

### 1.3 Key Architectural Principles

1. **Generic Core**: The core is completely domain-agnostic, dealing only with generic time-series concepts
2. **Domain Adapters**: Domain-specific knowledge (air quality, energy, etc.) lives in adapters
3. **Dependency Direction**: Dependencies point inward (adapters → core), never outward (core → adapters)
4. **Interface Segregation**: Small, focused traits rather than monolithic interfaces
5. **Testability**: Core business logic can be tested without infrastructure dependencies

---

## 2. Workspace Structure

```
neural-data-platform/
├── Cargo.toml                          # Workspace root
│
├── core/                               # Generic time-series platform (NEW)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── traits.rs                   # Core trait definitions
│       │   ├── TimeSeriesPoint         # Generic data point
│       │   ├── Store                   # Storage abstraction
│       │   ├── Source                  # Data source abstraction
│       │   └── Forecast                # Prediction abstraction
│       ├── storage/
│       │   ├── mod.rs
│       │   └── parquet.rs              # Parquet store implementation
│       ├── sources/
│       │   ├── mod.rs
│       │   └── mqtt.rs                 # MQTT source implementation
│       ├── forecast/
│       │   ├── mod.rs
│       │   └── fann_adapter.rs         # ruv-FANN integration
│       ├── pipeline/
│       │   ├── mod.rs
│       │   ├── ingestion.rs            # Generic ingestion pipeline
│       │   └── query.rs                # Generic query engine
│       └── error.rs                    # Core error types
│
├── domains/                            # Domain-specific implementations (NEW)
│   └── air-quality/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── types.rs                # AirQualityReading, Sensor, etc.
│           ├── parser.rs               # Parse raw data → AirQualityReading
│           ├── adapter.rs              # AirQualityReading → TimeSeriesPoint
│           ├── validation.rs           # Domain-specific validation
│           └── metrics.rs              # AQI calculations, thresholds
│
├── apps/                               # Runnable applications (NEW)
│   └── air-quality-app/
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs                 # Application entry point
│           ├── config.rs               # Configuration loading
│           ├── api/
│           │   ├── mod.rs
│           │   ├── rest.rs             # REST API server (Axum)
│           │   └── handlers/           # HTTP handlers
│           └── cli/
│               ├── mod.rs
│               └── commands.rs         # CLI commands
│
├── vendor/                             # Existing vendored dependencies
│   └── ruv-fann/                       # Existing forecasting models (REUSE)
│       ├── Cargo.toml
│       └── src/
│           └── lib.rs                  # FANN neural network implementation
│
├── neural-core/                        # Existing shared foundation (REUSE)
│   └── src/
│       ├── types/
│       │   └── prediction.rs           # PredictionResult (REUSE)
│       └── traits/
│           └── predictor.rs            # Predictor trait (REUSE)
│
├── neural-ml-ops/                      # Existing ML operations (REUSE)
│   └── src/
│       ├── training/                   # Training coordinator
│       └── models/                     # Model management
│
├── config-store/                       # Existing configuration (REUSE)
│   └── src/
│       └── lib.rs                      # Configuration management
│
└── product/                            # Documentation and planning
    └── features/
        └── air-001/
            ├── architecture/           # This document
            ├── specs/                  # Specifications
            └── implementation/         # Implementation tracking
```

### 2.1 Crate Dependency Graph

```
apps/air-quality-app
    ├─→ domains/air-quality
    │       └─→ core
    ├─→ core
    │   ├─→ vendor/ruv-fann
    │   └─→ neural-core (Prediction types)
    ├─→ neural-ml-ops (training)
    └─→ config-store

domains/air-quality
    └─→ core

core
    ├─→ vendor/ruv-fann
    └─→ neural-core
```

**Key Design Decisions:**
- `core/` has NO dependencies on `domains/` (unidirectional dependency)
- `apps/` orchestrates both core and domain-specific logic
- Existing crates (`neural-core`, `ruv-fann`, `neural-ml-ops`, `config-store`) are REUSED as libraries

---

## 3. Component Design

### 3.1 Core Traits (Generic Platform)

#### TimeSeriesPoint Trait

```rust
// core/src/traits.rs

use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Generic time-series data point
/// Domain-agnostic abstraction for any time-series data
pub trait TimeSeriesPoint: Send + Sync {
    /// Unique identifier for this data point
    fn id(&self) -> String;

    /// Timestamp when this data was recorded
    fn timestamp(&self) -> DateTime<Utc>;

    /// Metric name (e.g., "pm25", "temperature", "stock_price")
    fn metric(&self) -> &str;

    /// Numeric value (all metrics must be representable as f64)
    fn value(&self) -> f64;

    /// Tags/labels for multi-dimensional queries
    /// Examples:
    /// - Air quality: {"location": "sensor-001", "room": "bedroom"}
    /// - Finance: {"symbol": "AAPL", "exchange": "NASDAQ"}
    fn tags(&self) -> &HashMap<String, String>;

    /// Optional metadata (non-queryable, informational)
    fn metadata(&self) -> Option<&HashMap<String, String>> {
        None
    }
}
```

#### Store Trait

```rust
// core/src/traits.rs

use async_trait::async_trait;
use crate::error::Result;

/// Generic storage abstraction for time-series data
#[async_trait]
pub trait Store: Send + Sync {
    /// Write a batch of time-series points
    async fn write(&self, points: Vec<Box<dyn TimeSeriesPoint>>) -> Result<()>;

    /// Query time-series data by time range and filters
    async fn query(
        &self,
        metric: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        filters: HashMap<String, String>,
    ) -> Result<Vec<Box<dyn TimeSeriesPoint>>>;

    /// Aggregate data over a time window
    async fn aggregate(
        &self,
        metric: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        window: Duration,
        aggregation: AggregationType,
    ) -> Result<Vec<AggregatedPoint>>;

    /// Health check for storage backend
    async fn health(&self) -> Result<HealthStatus>;
}

#[derive(Debug, Clone)]
pub enum AggregationType {
    Mean,
    Sum,
    Min,
    Max,
    Count,
    Percentile(f64),
}

#[derive(Debug)]
pub struct AggregatedPoint {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
}

#[derive(Debug)]
pub struct HealthStatus {
    pub healthy: bool,
    pub message: String,
}
```

#### Source Trait

```rust
// core/src/traits.rs

use futures::Stream;

/// Generic data source abstraction
#[async_trait]
pub trait Source: Send + Sync {
    /// Unique identifier for this source
    fn id(&self) -> &str;

    /// Start receiving data from this source
    /// Returns a stream of time-series points
    async fn start(&mut self) -> Result<()>;

    /// Stream of incoming data points
    fn stream(&self) -> Box<dyn Stream<Item = Result<Box<dyn TimeSeriesPoint>>> + Send + Unpin>;

    /// Stop receiving data
    async fn stop(&mut self) -> Result<()>;

    /// Health check for data source
    async fn health(&self) -> Result<HealthStatus>;
}
```

#### Forecast Trait

```rust
// core/src/traits.rs

/// Generic forecasting abstraction
#[async_trait]
pub trait Forecast: Send + Sync {
    /// Predict future values based on historical data
    async fn predict(
        &self,
        metric: &str,
        historical_data: Vec<Box<dyn TimeSeriesPoint>>,
        forecast_horizon: usize,
    ) -> Result<Vec<ForecastedPoint>>;

    /// Train the forecasting model with new data
    async fn train(
        &self,
        metric: &str,
        training_data: Vec<Box<dyn TimeSeriesPoint>>,
    ) -> Result<()>;

    /// Get model accuracy metrics
    async fn evaluate(
        &self,
        metric: &str,
        test_data: Vec<Box<dyn TimeSeriesPoint>>,
    ) -> Result<ModelMetrics>;
}

#[derive(Debug)]
pub struct ForecastedPoint {
    pub timestamp: DateTime<Utc>,
    pub predicted_value: f64,
    pub confidence_interval: (f64, f64),
}

#[derive(Debug)]
pub struct ModelMetrics {
    pub mse: f64,
    pub mae: f64,
    pub r_squared: f64,
}
```

### 3.2 ParquetStore Implementation

```rust
// core/src/storage/parquet.rs

use crate::traits::{Store, TimeSeriesPoint, AggregatedPoint, AggregationType, HealthStatus};
use crate::error::{Result, CoreError};
use async_trait::async_trait;
use polars::prelude::*;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct ParquetStore {
    base_path: PathBuf,
    schema: Arc<Schema>,
    writer_pool: Arc<RwLock<WriterPool>>,
}

impl ParquetStore {
    pub fn new(base_path: PathBuf) -> Result<Self> {
        // Create schema for time-series data
        let schema = Schema::from_iter(vec![
            Field::new("timestamp", DataType::Datetime(TimeUnit::Milliseconds, None)),
            Field::new("metric", DataType::Utf8),
            Field::new("value", DataType::Float64),
            Field::new("tags", DataType::Utf8), // JSON-serialized tags
            Field::new("id", DataType::Utf8),
        ]);

        Ok(Self {
            base_path,
            schema: Arc::new(schema),
            writer_pool: Arc::new(RwLock::new(WriterPool::new())),
        })
    }

    /// Get partition path for a metric (e.g., pm25/2025-12-13.parquet)
    fn partition_path(&self, metric: &str, timestamp: DateTime<Utc>) -> PathBuf {
        let date = timestamp.format("%Y-%m-%d").to_string();
        self.base_path
            .join(metric)
            .join(format!("{}.parquet", date))
    }
}

#[async_trait]
impl Store for ParquetStore {
    async fn write(&self, points: Vec<Box<dyn TimeSeriesPoint>>) -> Result<()> {
        if points.is_empty() {
            return Ok(());
        }

        // Group points by metric and partition (date)
        let mut partitions: HashMap<String, Vec<Box<dyn TimeSeriesPoint>>> = HashMap::new();

        for point in points {
            let key = format!("{}|{}",
                point.metric(),
                point.timestamp().format("%Y-%m-%d")
            );
            partitions.entry(key).or_insert_with(Vec::new).push(point);
        }

        // Write each partition
        for (partition_key, partition_points) in partitions {
            self.write_partition(partition_points).await?;
        }

        Ok(())
    }

    async fn query(
        &self,
        metric: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        filters: HashMap<String, String>,
    ) -> Result<Vec<Box<dyn TimeSeriesPoint>>> {
        // Scan all partitions in date range
        let mut all_data = Vec::new();

        let mut current_date = start.date_naive();
        let end_date = end.date_naive();

        while current_date <= end_date {
            let partition_path = self.base_path
                .join(metric)
                .join(format!("{}.parquet", current_date.format("%Y-%m-%d")));

            if partition_path.exists() {
                // Use Polars to scan parquet file
                let df = LazyFrame::scan_parquet(&partition_path, Default::default())?
                    .filter(
                        col("timestamp").gt_eq(lit(start.timestamp_millis()))
                            .and(col("timestamp").lt_eq(lit(end.timestamp_millis())))
                    )
                    .collect()?;

                // Convert DataFrame rows to TimeSeriesPoint
                // Apply tag filters
                all_data.extend(self.df_to_points(df, &filters)?);
            }

            current_date = current_date.succ_opt().unwrap();
        }

        Ok(all_data)
    }

    async fn aggregate(
        &self,
        metric: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        window: Duration,
        aggregation: AggregationType,
    ) -> Result<Vec<AggregatedPoint>> {
        // Use Polars for efficient aggregation
        let agg_expr = match aggregation {
            AggregationType::Mean => col("value").mean(),
            AggregationType::Sum => col("value").sum(),
            AggregationType::Min => col("value").min(),
            AggregationType::Max => col("value").max(),
            AggregationType::Count => col("value").count(),
            AggregationType::Percentile(p) => col("value").quantile(lit(p), QuantileInterpolOptions::default()),
        };

        // Scan and aggregate
        // Implementation details...

        Ok(Vec::new())
    }

    async fn health(&self) -> Result<HealthStatus> {
        Ok(HealthStatus {
            healthy: self.base_path.exists(),
            message: format!("Parquet store at {:?}", self.base_path),
        })
    }
}
```

### 3.3 MqttSource Implementation

```rust
// core/src/sources/mqtt.rs

use crate::traits::{Source, TimeSeriesPoint, HealthStatus};
use crate::error::Result;
use async_trait::async_trait;
use rumqttc::{AsyncClient, MqttOptions, Event, Packet, QoS};
use futures::Stream;
use tokio::sync::mpsc;

pub struct MqttSource {
    id: String,
    broker_url: String,
    topic: String,
    client: Option<AsyncClient>,
    data_tx: mpsc::Sender<Result<Box<dyn TimeSeriesPoint>>>,
    data_rx: Option<mpsc::Receiver<Result<Box<dyn TimeSeriesPoint>>>>,
}

impl MqttSource {
    pub fn new(id: String, broker_url: String, topic: String) -> Self {
        let (tx, rx) = mpsc::channel(1000);
        Self {
            id,
            broker_url,
            topic,
            client: None,
            data_tx: tx,
            data_rx: Some(rx),
        }
    }

    async fn process_message(&self, payload: &[u8]) -> Result<Box<dyn TimeSeriesPoint>> {
        // Parse MQTT message payload (implementation varies by domain adapter)
        // This is where domain-specific parsing happens
        // Return a generic TimeSeriesPoint
        todo!("Domain adapter will handle parsing")
    }
}

#[async_trait]
impl Source for MqttSource {
    fn id(&self) -> &str {
        &self.id
    }

    async fn start(&mut self) -> Result<()> {
        let mut mqtt_options = MqttOptions::new(&self.id, &self.broker_url, 1883);
        mqtt_options.set_keep_alive(Duration::from_secs(30));

        let (client, mut eventloop) = AsyncClient::new(mqtt_options, 10);
        client.subscribe(&self.topic, QoS::AtLeastOnce).await?;

        self.client = Some(client);

        // Spawn background task to process MQTT events
        let data_tx = self.data_tx.clone();
        tokio::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(Event::Incoming(Packet::Publish(publish))) => {
                        // Process message and send to channel
                        // Implementation...
                    }
                    Err(e) => {
                        // Handle error
                        break;
                    }
                    _ => {}
                }
            }
        });

        Ok(())
    }

    fn stream(&self) -> Box<dyn Stream<Item = Result<Box<dyn TimeSeriesPoint>>> + Send + Unpin> {
        // Return receiver as stream
        todo!("Convert mpsc::Receiver to Stream")
    }

    async fn stop(&mut self) -> Result<()> {
        if let Some(client) = &self.client {
            client.disconnect().await?;
        }
        Ok(())
    }

    async fn health(&self) -> Result<HealthStatus> {
        Ok(HealthStatus {
            healthy: self.client.is_some(),
            message: format!("MQTT source connected to {}", self.broker_url),
        })
    }
}
```

### 3.4 Air Quality Domain Types

```rust
// domains/air-quality/src/types.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Air quality sensor reading
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirQualityReading {
    pub sensor_id: String,
    pub location: String,
    pub timestamp: DateTime<Utc>,
    pub pm25: Option<f64>,      // μg/m³
    pub pm10: Option<f64>,      // μg/m³
    pub co2: Option<f64>,       // ppm
    pub temperature: Option<f64>, // °C
    pub humidity: Option<f64>,  // %
    pub pressure: Option<f64>,  // hPa
}

impl AirQualityReading {
    /// Calculate Air Quality Index (AQI) from PM2.5
    pub fn aqi_from_pm25(&self) -> Option<u16> {
        self.pm25.map(|pm25| {
            // EPA AQI calculation
            if pm25 <= 12.0 {
                linear_scale(pm25, 0.0, 12.0, 0, 50)
            } else if pm25 <= 35.4 {
                linear_scale(pm25, 12.1, 35.4, 51, 100)
            } else if pm25 <= 55.4 {
                linear_scale(pm25, 35.5, 55.4, 101, 150)
            } else if pm25 <= 150.4 {
                linear_scale(pm25, 55.5, 150.4, 151, 200)
            } else if pm25 <= 250.4 {
                linear_scale(pm25, 150.5, 250.4, 201, 300)
            } else {
                linear_scale(pm25, 250.5, 500.4, 301, 500)
            }
        })
    }

    /// Check if any pollutant exceeds safe thresholds
    pub fn is_unhealthy(&self) -> bool {
        self.aqi_from_pm25().map(|aqi| aqi > 100).unwrap_or(false)
            || self.co2.map(|co2| co2 > 1000.0).unwrap_or(false)
    }
}

fn linear_scale(value: f64, in_min: f64, in_max: f64, out_min: u16, out_max: u16) -> u16 {
    let slope = (out_max - out_min) as f64 / (in_max - in_min);
    (slope * (value - in_min) + out_min as f64).round() as u16
}
```

### 3.5 Air Quality Adapter (Domain → Core)

```rust
// domains/air-quality/src/adapter.rs

use crate::types::AirQualityReading;
use core::traits::TimeSeriesPoint;
use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// Wrapper to convert AirQualityReading to TimeSeriesPoint
/// This enables the generic core to process air quality data
pub struct AirQualityPoint {
    reading: AirQualityReading,
    metric_name: String,
    value: f64,
}

impl AirQualityPoint {
    /// Convert an AirQualityReading into multiple TimeSeriesPoints
    /// (one for each metric: pm25, pm10, co2, temperature, etc.)
    pub fn from_reading(reading: AirQualityReading) -> Vec<Box<dyn TimeSeriesPoint>> {
        let mut points: Vec<Box<dyn TimeSeriesPoint>> = Vec::new();

        if let Some(pm25) = reading.pm25 {
            points.push(Box::new(AirQualityPoint {
                reading: reading.clone(),
                metric_name: "pm25".to_string(),
                value: pm25,
            }));
        }

        if let Some(pm10) = reading.pm10 {
            points.push(Box::new(AirQualityPoint {
                reading: reading.clone(),
                metric_name: "pm10".to_string(),
                value: pm10,
            }));
        }

        if let Some(co2) = reading.co2 {
            points.push(Box::new(AirQualityPoint {
                reading: reading.clone(),
                metric_name: "co2".to_string(),
                value: co2,
            }));
        }

        // Add temperature, humidity, pressure...

        points
    }
}

impl TimeSeriesPoint for AirQualityPoint {
    fn id(&self) -> String {
        format!("{}|{}|{}",
            self.reading.sensor_id,
            self.metric_name,
            self.reading.timestamp.timestamp()
        )
    }

    fn timestamp(&self) -> DateTime<Utc> {
        self.reading.timestamp
    }

    fn metric(&self) -> &str {
        &self.metric_name
    }

    fn value(&self) -> f64 {
        self.value
    }

    fn tags(&self) -> &HashMap<String, String> {
        // Cache tags in the struct (omitted for brevity)
        // Return: {"sensor_id": "...", "location": "..."}
        todo!("Return cached tags")
    }
}
```

### 3.6 Configuration Management (config-store Integration)

The platform uses the existing `config-store` crate for all configuration management, providing centralized, versioned, and secure configuration with GitOps support.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    CONFIGURATION ARCHITECTURE                                │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                     Configuration Sources                            │   │
│  │                                                                       │   │
│  │   ┌────────────┐    ┌────────────┐    ┌────────────┐                │   │
│  │   │  GitHub    │    │   Local    │    │Environment │                │   │
│  │   │ Repository │    │   Files    │    │ Variables  │                │   │
│  │   │ (GitOps)   │    │  (YAML)    │    │  (12-factor)│                │   │
│  │   └──────┬─────┘    └──────┬─────┘    └──────┬─────┘                │   │
│  │          │                 │                 │                       │   │
│  │          └────────────┬────┴────────────────┘                       │   │
│  │                       │                                              │   │
│  │                       ▼                                              │   │
│  │           ┌───────────────────────┐                                  │   │
│  │           │     GitOpsLoader      │  (config-store/loaders/gitops)  │   │
│  │           │   Base + Overlays     │                                  │   │
│  │           └───────────┬───────────┘                                  │   │
│  │                       │                                              │   │
│  │                       ▼                                              │   │
│  │           ┌───────────────────────┐                                  │   │
│  │           │   ConfigStore Trait   │  (config-store/traits.rs)       │   │
│  │           │  get/set/watch/list   │                                  │   │
│  │           └───────────┬───────────┘                                  │   │
│  │                       │                                              │   │
│  │                       ▼                                              │   │
│  │   ┌──────────────────────────────────────────────────────────┐      │   │
│  │   │               Security Layer                              │      │   │
│  │   │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐      │      │   │
│  │   │  │SecretBlocker│  │InputValidator│  │SchemaValidator│     │      │   │
│  │   │  │(block keys) │  │(injection)  │  │(JSON Schema) │      │      │   │
│  │   │  └─────────────┘  └─────────────┘  └─────────────┘      │      │   │
│  │   └──────────────────────────────────────────────────────────┘      │   │
│  │                       │                                              │   │
│  │                       ▼                                              │   │
│  │   ┌──────────────────────────────────────────────────────────┐      │   │
│  │   │               Storage Backends                            │      │   │
│  │   │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐      │      │   │
│  │   │  │InMemoryStore│  │ RedisStore  │  │SecureInMemory│      │      │   │
│  │   │  │(dev/test)   │  │(distributed)│  │(production)  │      │      │   │
│  │   │  └─────────────┘  └─────────────┘  └─────────────┘      │      │   │
│  │   └──────────────────────────────────────────────────────────┘      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### 3.6.1 Configuration Directory Structure

```
config/
├── base/                           # Base configuration (all environments)
│   ├── air-quality.yaml           # Air quality domain config
│   ├── storage.yaml               # Parquet storage settings
│   ├── alerting.yaml              # Alert thresholds and channels
│   ├── forecasting.yaml           # ruv-FANN model configuration
│   └── observability.yaml         # Logging, metrics, tracing
│
└── overlays/                       # Environment-specific overrides
    ├── development/
    │   └── overrides.yaml         # Dev settings (verbose logging, mock data)
    ├── staging/
    │   └── overrides.yaml         # Staging settings
    └── production/
        └── overrides.yaml         # Pi5/cloud production settings
```

#### 3.6.2 Air Quality Configuration (YAML)

```yaml
# config/base/air-quality.yaml
# Air Quality Domain Configuration - Base Template

# Hierarchical path: /air-quality
air_quality:
  # Sensor configuration
  sensors:
    - serial: "ecda3b1eaaaf"
      name: "Living Room AirGradient ONE"
      location_id: "living-room"
      model: "I-9PSL"
      data_source: "both"              # mqtt | local_api | both
      enabled: true

  # Data ingestion settings
  ingestion:
    mqtt:
      broker_url: "${MQTT_BROKER_URL:mqtt://mosquitto:1883}"
      client_id: "neural-air-quality-${HOSTNAME}"
      topic_pattern: "airgradient/readings/{serial}"
      qos: 1                           # AtLeastOnce
      reconnect:
        initial_delay_ms: 1000
        max_delay_ms: 30000
        backoff_multiplier: 2.0

    local_api:
      enabled: true
      poll_interval_seconds: 60
      timeout_seconds: 10
      url_pattern: "http://airgradient_{serial}.local/measures/current"

  # Health thresholds (EPA/WHO guidelines)
  thresholds:
    co2:
      excellent: 400
      good: 800
      moderate: 1000
      poor: 1500
      unhealthy: 2000
      dangerous: 5000
    pm25:
      good: 12.0
      moderate: 35.4
      unhealthy_sensitive: 55.4
      unhealthy: 150.4
      very_unhealthy: 250.4
      hazardous: 500.4
    tvoc_index:
      excellent: 100
      good: 150
      moderate: 200
      poor: 300
      bad: 400

  # Alerting configuration
  alerting:
    enabled: true
    rate_limit:
      cooldown_seconds: 300
      max_per_hour: 10
    escalation:
      - level: info
        delay_minutes: 0
      - level: warning
        delay_minutes: 15
      - level: critical
        delay_minutes: 30
    channels:
      - type: webhook
        url: "${ALERT_WEBHOOK_URL}"
        enabled: "${ALERT_WEBHOOK_ENABLED:false}"
      - type: log
        level: warn
        enabled: true

  # Forecasting settings
  forecasting:
    enabled: true
    model: "nhits"                     # nhits | nbeats | ensemble
    horizon_hours: 24
    confidence_intervals: [50, 80, 90]
    retrain:
      interval_days: 7
      min_data_points: 1000
```

#### 3.6.3 ConfigManager Implementation

```rust
// core/src/config/manager.rs
use config_store::{
    ConfigStore, ConfigValue, ConfigError, ConfigNode,
    InMemoryConfigStore, SecureInMemoryConfigStore,
    loaders::GitOpsLoader,
    validation::SchemaValidator,
};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Central configuration manager for the air quality platform
pub struct ConfigManager {
    store: Arc<dyn ConfigStore>,
    gitops_loader: GitOpsLoader,
    schema_validator: SchemaValidator,
    watch_handle: Option<tokio::task::JoinHandle<()>>,
}

impl ConfigManager {
    /// Initialize configuration from GitOps sources
    pub async fn new(config_path: &str, environment: &str) -> Result<Self, ConfigError> {
        // Initialize store (SecureInMemoryConfigStore for production)
        let store: Arc<dyn ConfigStore> = Arc::new(SecureInMemoryConfigStore::new());

        // Initialize GitOps loader for base + overlay pattern
        let gitops_loader = GitOpsLoader::new(config_path, environment);

        // Load JSON Schema for validation
        let schema_validator = SchemaValidator::from_file(
            "schemas/air-quality-config.json"
        ).await?;

        let manager = Self {
            store,
            gitops_loader,
            schema_validator,
            watch_handle: None,
        };

        // Load initial configuration
        manager.load_configuration().await?;

        Ok(manager)
    }

    /// Load configuration from base + overlay files
    pub async fn load_configuration(&self) -> Result<(), ConfigError> {
        // Load base configurations
        let base_configs = self.gitops_loader.load_base_configs().await?;

        // Load environment-specific overlays
        let overlay_configs = self.gitops_loader.load_overlay_configs().await?;

        // Merge: base <- overlay (overlay overrides base)
        let merged = merge_configs(base_configs, overlay_configs);

        // Validate against schema
        self.schema_validator.validate(&merged)?;

        // Store in config-store with hierarchical paths
        self.store_hierarchical("/air-quality", &merged).await?;

        log::info!("Configuration loaded successfully from {}",
            self.gitops_loader.environment());

        Ok(())
    }

    /// Get typed configuration value
    pub async fn get<T: serde::de::DeserializeOwned>(
        &self,
        path: &str
    ) -> Result<T, ConfigError> {
        let value = self.store.get(path).await?;
        value.try_into()
    }

    /// Get air quality threshold configuration
    pub async fn get_thresholds(&self) -> Result<ThresholdConfig, ConfigError> {
        self.get("/air-quality/thresholds").await
    }

    /// Get sensor configuration
    pub async fn get_sensors(&self) -> Result<Vec<SensorConfig>, ConfigError> {
        self.get("/air-quality/sensors").await
    }

    /// Watch for configuration changes (hot-reload)
    pub async fn watch<F>(&mut self, callback: F) -> Result<(), ConfigError>
    where
        F: Fn(&str, &ConfigValue) + Send + Sync + 'static,
    {
        // Implementation using config-store WatchConfig gRPC stream
        // or file system watcher for local files
        todo!("Implement configuration watching")
    }

    /// Validate configuration without applying
    pub async fn validate(&self, config: &ConfigValue) -> Result<(), ConfigError> {
        self.schema_validator.validate(config)
    }
}

// Configuration structs matching YAML schema
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SensorConfig {
    pub serial: String,
    pub name: String,
    pub location_id: String,
    pub model: String,
    pub data_source: DataSourceMode,
    pub enabled: bool,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub enum DataSourceMode {
    #[serde(rename = "mqtt")]
    Mqtt,
    #[serde(rename = "local_api")]
    LocalApi,
    #[serde(rename = "both")]
    Both,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ThresholdConfig {
    pub co2: Co2Thresholds,
    pub pm25: Pm25Thresholds,
    pub tvoc_index: TvocThresholds,
}
```

#### 3.6.4 GitHub Configuration Sourcing

For GitOps workflows, configuration can be sourced directly from GitHub:

```yaml
# bootstrap.yaml - Initial bootstrap configuration
config_sources:
  - type: github
    repo: "organization/air-quality-config"
    branch: "main"
    path: "config/"
    refresh_interval_seconds: 300    # 5-minute refresh
    auth:
      type: token
      token_env: "GITHUB_CONFIG_TOKEN"

  - type: local
    path: "/config/"                  # Docker volume fallback
    priority: 1                       # Higher priority = used first

  - type: environment
    prefix: "AIR_QUALITY_"           # AIR_QUALITY_MQTT_BROKER_URL
    priority: 2                       # Environment vars override files
```

---

## 4. Data Flow Diagrams

### 4.1 Ingestion Pipeline

```
┌────────────────────────────────────────────────────────────────┐
│                        INGESTION FLOW                          │
└────────────────────────────────────────────────────────────────┘

MQTT Broker                              Neural Data Platform
     │
     │ 1. Publish message
     │    {pm25: 35.2, co2: 650, ...}
     ▼
┌──────────────┐
│ MqttSource   │ (core/sources/mqtt.rs)
│ (Generic)    │
└──────┬───────┘
       │ 2. Receive raw payload
       │
       ▼
┌──────────────────┐
│ Domain Parser    │ (domains/air-quality/parser.rs)
│ (Air Quality)    │
└──────┬───────────┘
       │ 3. Parse → AirQualityReading
       │
       ▼
┌──────────────────┐
│ Validator        │ (domains/air-quality/validation.rs)
│ (Domain-specific)│
└──────┬───────────┘
       │ 4. Validate ranges, required fields
       │
       ▼
┌──────────────────┐
│ Domain Adapter   │ (domains/air-quality/adapter.rs)
│                  │
└──────┬───────────┘
       │ 5. Convert → Vec<TimeSeriesPoint>
       │    (pm25=35.2, co2=650, temp=22.5)
       ▼
┌──────────────────┐
│ Ingestion        │ (core/pipeline/ingestion.rs)
│ Pipeline         │
│ (Generic)        │
└──────┬───────────┘
       │ 6. Batch, buffer, backpressure
       │
       ▼
┌──────────────────┐
│ ParquetStore     │ (core/storage/parquet.rs)
│ (Generic)        │
└──────┬───────────┘
       │ 7. Write to partitioned Parquet files
       │    data/pm25/2025-12-13.parquet
       │    data/co2/2025-12-13.parquet
       ▼
   [Storage]
```

**Key Points:**
- Generic components (`MqttSource`, `ParquetStore`) know nothing about air quality
- Domain-specific logic isolated to `domains/air-quality/`
- Validation happens early (fail fast)
- Adapter converts domain types → generic traits

### 4.2 Query Pipeline

```
┌────────────────────────────────────────────────────────────────┐
│                          QUERY FLOW                            │
└────────────────────────────────────────────────────────────────┘

REST API Request                         Neural Data Platform
     │
     │ 1. GET /api/v1/query
     │    ?metric=pm25
     │    &start=2025-12-10T00:00:00Z
     │    &end=2025-12-13T23:59:59Z
     │    &location=bedroom
     ▼
┌──────────────┐
│ HTTP Handler │ (apps/air-quality-app/api/handlers/query.rs)
│              │
└──────┬───────┘
       │ 2. Parse query parameters
       │
       ▼
┌──────────────────┐
│ Query Engine     │ (core/pipeline/query.rs)
│ (Generic)        │
└──────┬───────────┘
       │ 3. Build query plan
       │    Scan partitions: pm25/2025-12-{10,11,12,13}.parquet
       │
       ▼
┌──────────────────┐
│ ParquetStore     │ (core/storage/parquet.rs)
│ query()          │
└──────┬───────────┘
       │ 4. Polars scan + filter
       │    - timestamp filter (start/end)
       │    - tag filter (location=bedroom)
       │
       ▼
┌──────────────────┐
│ Polars DataFrame │
│ (In-memory)      │
└──────┬───────────┘
       │ 5. Return Vec<TimeSeriesPoint>
       │
       ▼
┌──────────────────┐
│ Response Builder │ (Generic → JSON)
│                  │
└──────┬───────────┘
       │ 6. Serialize to JSON
       │    [{timestamp, value}, ...]
       ▼
   REST API Response
```

**Optimization Strategies:**
- **Partition Pruning**: Only scan relevant date partitions
- **Predicate Pushdown**: Apply filters at Parquet level (Polars handles this)
- **Columnar Reads**: Only read columns needed (timestamp, value, tags)
- **Parallel Scans**: Polars scans multiple partitions concurrently

### 4.3 Forecasting Pipeline

```
┌────────────────────────────────────────────────────────────────┐
│                       FORECASTING FLOW                         │
└────────────────────────────────────────────────────────────────┘

REST API Request                         Neural Data Platform
     │
     │ 1. POST /api/v1/forecast
     │    {metric: "pm25", horizon: 24}
     ▼
┌──────────────┐
│ Forecast API │ (apps/air-quality-app/api/handlers/forecast.rs)
│ Handler      │
└──────┬───────┘
       │ 2. Extract parameters
       │
       ▼
┌──────────────────┐
│ Query Historical │ (core/pipeline/query.rs)
│ Data (7 days)    │
└──────┬───────────┘
       │ 3. Load training data from ParquetStore
       │    Last 7 days of pm25 readings
       │
       ▼
┌──────────────────┐
│ Data Preparation │ (core/forecast/prepare.rs)
│ (Generic)        │
└──────┬───────────┘
       │ 4. Normalize, create windows
       │    [[t-6, t-5, t-4, t-3, t-2, t-1] → t]
       │
       ▼
┌──────────────────┐
│ FANN Forecaster  │ (core/forecast/fann_adapter.rs)
│ (ruv-FANN)       │
└──────┬───────────┘
       │ 5. Neural network prediction
       │    Input: Last 168 hours (7 days)
       │    Output: Next 24 hours
       │
       ▼
┌──────────────────┐
│ Forecast Results │
│                  │
└──────┬───────────┘
       │ 6. Return predictions with confidence intervals
       │    [{timestamp: t+1, value: 28.3, ci: [25.1, 31.5]}, ...]
       ▼
   REST API Response
```

**Integration with ruv-FANN:**
- ruv-FANN is an existing crate in `vendor/ruv-fann/`
- Core creates a thin adapter implementing `Forecast` trait
- Adapter handles data formatting and model invocation
- Neural network models trained periodically via `neural-ml-ops`

---

## 5. Integration with Existing Codebase

### 5.1 What to REUSE

| Component | Location | Usage |
|-----------|----------|-------|
| **PredictionResult** | `neural-core/src/types/prediction.rs` | Return type for forecasts |
| **Predictor trait** | `neural-core/src/traits/predictor.rs` | Base trait for ML models |
| **ruv-FANN** | `vendor/ruv-fann/` | Neural network forecasting |
| **Training Coordinator** | `neural-ml-ops/src/training/` | Model training workflows |
| **Config Store** | `config-store/` | Application configuration |
| **Event Bus** | `neural-core/src/eventbus/` | Pub/sub for events (optional) |

### 5.2 What is NEW

| Component | Location | Purpose |
|-----------|----------|---------|
| **Core traits** | `core/src/traits.rs` | Generic time-series abstractions |
| **Parquet storage** | `core/src/storage/parquet.rs` | Time-series data persistence |
| **MQTT source** | `core/src/sources/mqtt.rs` | Data ingestion from MQTT |
| **Air quality domain** | `domains/air-quality/` | Domain-specific types and logic |
| **Air quality app** | `apps/air-quality-app/` | Runnable application |

### 5.3 Mapping Existing Types to New Traits

```rust
// Example: Using neural-core's PredictionResult in core/forecast

use neural_core::types::prediction::PredictionResult;
use crate::traits::{ForecastedPoint, ModelMetrics};

impl From<PredictionResult> for ForecastedPoint {
    fn from(pred: PredictionResult) -> Self {
        ForecastedPoint {
            timestamp: pred.timestamp,
            predicted_value: pred.value,
            confidence_interval: (
                pred.value - pred.confidence_interval,
                pred.value + pred.confidence_interval,
            ),
        }
    }
}
```

### 5.4 Dependencies in Cargo.toml

```toml
# core/Cargo.toml
[package]
name = "core"
version = "0.1.0"
edition = "2021"

[dependencies]
# Existing crates (REUSE)
neural-core = { path = "../neural-core" }
ruv-fann = { path = "../vendor/ruv-fann" }

# New dependencies for Parquet and MQTT
polars = { version = "0.35", features = ["lazy", "parquet"] }
rumqttc = "0.23"
async-trait = "0.1"
tokio = { workspace = true }
chrono = { workspace = true }
serde = { workspace = true }
anyhow = { workspace = true }
```

```toml
# domains/air-quality/Cargo.toml
[package]
name = "air-quality"
version = "0.1.0"
edition = "2021"

[dependencies]
core = { path = "../../core" }
serde = { workspace = true }
chrono = { workspace = true }
```

```toml
# apps/air-quality-app/Cargo.toml
[package]
name = "air-quality-app"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "air-quality-app"
path = "src/main.rs"

[dependencies]
core = { path = "../../core" }
air-quality = { path = "../../domains/air-quality" }
neural-ml-ops = { path = "../../neural-ml-ops" }
config-store = { path = "../../config-store" }

# Web framework
axum = "0.7"
tower = "0.4"
tokio = { workspace = true }

# CLI
clap = { version = "4.4", features = ["derive"] }
```

---

## 6. Deployment Architecture

### 6.1 Docker-First Deployment Strategy

All deployments use Docker containers for consistent, portable operation across development machines, Raspberry Pi 5, and cloud environments.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    DOCKER CONTAINER ARCHITECTURE                         │
│                                                                         │
│  ┌───────────────────────────────────────────────────────────────────┐  │
│  │  neural-air-quality:latest (Multi-arch: amd64 + arm64)            │  │
│  │                                                                    │  │
│  │  ┌────────────────────────────────────────────────────────────┐   │  │
│  │  │  Application Layer                                          │   │  │
│  │  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐   │   │  │
│  │  │  │  REST    │  │   CLI    │  │   MCP    │  │  Health  │   │   │  │
│  │  │  │  API     │  │ Commands │  │  Server  │  │  Check   │   │   │  │
│  │  │  │  :8080   │  │          │  │  :9090   │  │  /health │   │   │  │
│  │  │  └──────────┘  └──────────┘  └──────────┘  └──────────┘   │   │  │
│  │  └────────────────────────────────────────────────────────────┘   │  │
│  │                                                                    │  │
│  │  ┌────────────────────────────────────────────────────────────┐   │  │
│  │  │  Core Platform (Generic Time-Series)                        │   │  │
│  │  │  • MQTT/HTTP Ingestion   • Parquet Storage                  │   │  │
│  │  │  • ruv-FANN Forecasting  • Alert Engine                     │   │  │
│  │  └────────────────────────────────────────────────────────────┘   │  │
│  │                                                                    │  │
│  │  VOLUMES:                                                         │  │
│  │  ├─ /data    → air-quality-data (Parquet files)                  │  │
│  │  ├─ /models  → air-quality-models (ruv-FANN weights)             │  │
│  │  └─ /config  → config.toml (read-only bind mount)                │  │
│  │                                                                    │  │
│  │  ENVIRONMENT:                                                     │  │
│  │  ├─ MQTT_BROKER_URL      (e.g., mqtt://broker:1883)              │  │
│  │  ├─ AIRGRADIENT_SERIAL   (e.g., ecda3b1eaaaf)                    │  │
│  │  ├─ DATA_SOURCE          (mqtt | local_api | both)               │  │
│  │  └─ LOG_LEVEL            (debug | info | warn | error)           │  │
│  └───────────────────────────────────────────────────────────────────┘  │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 6.2 Docker Compose Configuration

**docker-compose.yml:**
```yaml
version: '3.8'

services:
  # MQTT Broker (optional - can use external broker)
  mosquitto:
    image: eclipse-mosquitto:2
    container_name: air-quality-mqtt
    restart: unless-stopped
    ports:
      - "1883:1883"
      - "9001:9001"  # WebSocket (optional)
    volumes:
      - ./mosquitto/config:/mosquitto/config:ro
      - mosquitto-data:/mosquitto/data
      - mosquitto-log:/mosquitto/log
    networks:
      - air-quality-net

  # Main Air Quality Application
  neural-air-quality:
    image: neural-data-platform/air-quality:latest
    build:
      context: .
      dockerfile: Dockerfile
      args:
        - RUST_VERSION=1.75
    container_name: air-quality-app
    restart: unless-stopped
    depends_on:
      - mosquitto
    ports:
      - "8080:8080"   # REST API / Health / Metrics
      - "9090:9090"   # MCP Server (optional)
    volumes:
      - air-quality-data:/data
      - air-quality-models:/models
      - ./config.toml:/config/config.toml:ro
    environment:
      - MQTT_BROKER_URL=mqtt://mosquitto:1883
      - AIRGRADIENT_SERIAL=${AIRGRADIENT_SERIAL:-ecda3b1eaaaf}
      - DATA_SOURCE=${DATA_SOURCE:-mqtt}
      - LOG_LEVEL=${LOG_LEVEL:-info}
      - RUST_BACKTRACE=1
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 60s
    deploy:
      resources:
        limits:
          memory: 2G
          cpus: '2.0'
        reservations:
          memory: 512M
          cpus: '0.5'
    networks:
      - air-quality-net

volumes:
  air-quality-data:
    driver: local
  air-quality-models:
    driver: local
  mosquitto-data:
    driver: local
  mosquitto-log:
    driver: local

networks:
  air-quality-net:
    driver: bridge
```

### 6.3 Multi-Stage Dockerfile

**Dockerfile:**
```dockerfile
# =============================================================================
# Neural Data Platform - Air Quality Application
# Multi-arch build: amd64 (Mac/Cloud) + arm64 (Raspberry Pi 5)
# =============================================================================

ARG RUST_VERSION=1.75

# -----------------------------------------------------------------------------
# Stage 1: Build dependencies (cached layer)
# -----------------------------------------------------------------------------
FROM rust:${RUST_VERSION}-slim-bookworm AS deps

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Create dummy project structure for dependency caching
RUN cargo new --bin air-quality-app
WORKDIR /app/air-quality-app

# Copy only Cargo files first (better caching)
COPY Cargo.toml Cargo.lock ./
COPY core/Cargo.toml ./core/
COPY domains/air-quality/Cargo.toml ./domains/air-quality/
COPY apps/air-quality-app/Cargo.toml ./apps/air-quality-app/
COPY vendor/ruv-fann/Cargo.toml ./vendor/ruv-fann/

# Build dependencies only (this layer is cached)
RUN mkdir -p core/src domains/air-quality/src apps/air-quality-app/src vendor/ruv-fann/src && \
    echo "fn main() {}" > core/src/lib.rs && \
    echo "fn main() {}" > domains/air-quality/src/lib.rs && \
    echo "fn main() {}" > apps/air-quality-app/src/main.rs && \
    echo "fn main() {}" > vendor/ruv-fann/src/lib.rs && \
    cargo build --release && \
    rm -rf src core domains apps vendor

# -----------------------------------------------------------------------------
# Stage 2: Build application
# -----------------------------------------------------------------------------
FROM deps AS builder

# Copy actual source code
COPY . .

# Build the application
RUN cargo build --release --bin air-quality-app && \
    strip target/release/air-quality-app

# -----------------------------------------------------------------------------
# Stage 3: Runtime image (minimal)
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -s /bin/false airquality

# Create data directories
RUN mkdir -p /data /models /config && \
    chown -R airquality:airquality /data /models /config

# Copy binary from builder
COPY --from=builder /app/target/release/air-quality-app /usr/local/bin/

# Switch to non-root user
USER airquality

# Expose ports
EXPOSE 8080 9090

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Default command
CMD ["air-quality-app", "serve", "--config", "/config/config.toml"]

# Metadata
LABEL org.opencontainers.image.title="Neural Air Quality Platform" \
      org.opencontainers.image.description="Domain-agnostic time-series platform for air quality monitoring" \
      org.opencontainers.image.version="1.1.0" \
      org.opencontainers.image.source="https://github.com/neural-data-platform/air-quality"
```

### 6.4 Deployment Environments

#### Development (Mac M-series / Linux)

```bash
# Clone and start development stack
git clone https://github.com/neural-data-platform/air-quality.git
cd air-quality

# Create .env file
cat > .env << EOF
AIRGRADIENT_SERIAL=ecda3b1eaaaf
DATA_SOURCE=both
LOG_LEVEL=debug
EOF

# Start services
docker compose up -d

# View logs
docker compose logs -f neural-air-quality

# Access API
curl http://localhost:8080/health
curl http://localhost:8080/api/v1/readings/latest
```

#### Raspberry Pi 5 Production

```
┌─────────────────────────────────────────────────────────────┐
│                    Raspberry Pi 5 (8GB)                     │
│                    ARM64 / Debian Bookworm                  │
│                                                             │
│  ┌────────────────────────────────────────────────────────┐│
│  │  Docker Engine                                          ││
│  │  ┌──────────────────┐  ┌──────────────────┐           ││
│  │  │  mosquitto:2     │  │ neural-air-quality │           ││
│  │  │  (MQTT Broker)   │  │   :8080 :9090     │           ││
│  │  └────────┬─────────┘  └─────────┬────────┘           ││
│  │           │ mqtt://             │ REST/MCP            ││
│  │           │ mosquitto:1883      │                      ││
│  │           └──────────┬──────────┘                      ││
│  │                      │                                  ││
│  │  VOLUMES (SSD recommended):                            ││
│  │  /var/lib/docker/volumes/                              ││
│  │  ├─ air-quality-data/   (Parquet files)               ││
│  │  └─ air-quality-models/ (ruv-FANN weights)            ││
│  └────────────────────────────────────────────────────────┘│
│                                                             │
│  RESOURCES:                                                 │
│  • CPU: 2 cores (reserved: 0.5, limit: 2.0)               │
│  • RAM: 2GB limit, 512MB reserved                          │
│  • Storage: 32GB+ SD/SSD (16GB data retention)            │
│                                                             │
└─────────────────────────────────────────────────────────────┘
         ▲
         │ LAN (WiFi/Ethernet)
         │
┌────────┴────────┐
│  AirGradient    │
│  ONE Sensor     │
│  (MQTT publish) │
└─────────────────┘
```

**Pi 5 Setup Script:**
```bash
#!/bin/bash
# setup-pi5.sh - Automated Raspberry Pi 5 deployment

set -e

echo "=== Installing Docker ==="
curl -fsSL https://get.docker.com | sh
sudo usermod -aG docker $USER

echo "=== Creating directories ==="
sudo mkdir -p /opt/air-quality/{config,mosquitto/config}
cd /opt/air-quality

echo "=== Downloading configuration ==="
# Download docker-compose.yml and config files
curl -fsSL https://raw.githubusercontent.com/neural-data-platform/air-quality/main/docker-compose.yml > docker-compose.yml
curl -fsSL https://raw.githubusercontent.com/neural-data-platform/air-quality/main/config.example.toml > config.toml
curl -fsSL https://raw.githubusercontent.com/neural-data-platform/air-quality/main/mosquitto/mosquitto.conf > mosquitto/config/mosquitto.conf

echo "=== Configure sensor serial ==="
read -p "Enter AirGradient serial number: " SERIAL
sed -i "s/AIRGRADIENT_SERIAL=.*/AIRGRADIENT_SERIAL=$SERIAL/" .env

echo "=== Starting services ==="
docker compose pull
docker compose up -d

echo "=== Setup complete! ==="
echo "Access API at: http://$(hostname -I | awk '{print $1}'):8080"
echo "View logs: docker compose logs -f"
```

**systemd Integration (auto-start on boot):**
```ini
# /etc/systemd/system/air-quality.service
[Unit]
Description=Air Quality Docker Compose Stack
Requires=docker.service
After=docker.service

[Service]
Type=oneshot
RemainAfterExit=yes
WorkingDirectory=/opt/air-quality
ExecStart=/usr/bin/docker compose up -d
ExecStop=/usr/bin/docker compose down
TimeoutStartSec=0

[Install]
WantedBy=multi-user.target
```

### 6.5 Cloud Migration Path (Future)

```
┌─────────────────────────────────────────────────────────────┐
│                        Cloud Deployment                     │
│                     (AWS / GCP / Azure)                     │
│                                                             │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  Kubernetes Cluster                                   │  │
│  │                                                       │  │
│  │  ┌────────────────┐  ┌────────────────┐             │  │
│  │  │ Ingestion Pods │  │   Query Pods   │             │  │
│  │  │  (Replicas: 3) │  │  (Replicas: 5) │             │  │
│  │  └───────┬────────┘  └───────┬────────┘             │  │
│  │          │                    │                       │  │
│  │          ▼                    ▼                       │  │
│  │  ┌────────────────────────────────────┐              │  │
│  │  │      S3-Compatible Storage         │              │  │
│  │  │  (Parquet files with Hive          │              │  │
│  │  │   partitioning)                    │              │  │
│  │  │  s3://air-quality/pm25/year=2025/  │              │  │
│  │  │                     month=12/      │              │  │
│  │  │                     day=13/        │              │  │
│  │  └────────────────────────────────────┘              │  │
│  │                                                       │  │
│  │  ┌────────────────────────────────────┐              │  │
│  │  │   Optional: Analytics Layer        │              │  │
│  │  │   (Athena, Presto, Trino)          │              │  │
│  │  └────────────────────────────────────┘              │  │
│  └──────────────────────────────────────────────────────┘  │
│                                                             │
└─────────────────────────────────────────────────────────────┘
        ▲
        │ MQTT over Internet (TLS)
        │
 ┌──────┴───────┐
 │ Edge Device  │
 │ (Pi5 or Mac) │
 └──────────────┘
```

**Migration Steps:**
1. **Phase 1 (Current)**: Local Parquet files on Mac/Pi5
2. **Phase 2**: Add S3 storage adapter to `core/src/storage/s3.rs`
3. **Phase 3**: Deploy to Kubernetes with S3 backend
4. **Phase 4**: Add analytics layer (Athena/Presto) for SQL queries

---

## 7. Key Architectural Decisions (ADRs)

### ADR-001: Parquet over Time-Series Databases

**Status**: Accepted
**Date**: 2025-12-13

**Context**:
We need to choose a storage backend for time-series data. Options considered:
- QuestDB (schema-agnostic, high-speed writes)
- TimescaleDB (PostgreSQL extension, SQL compatibility)
- InfluxDB (established time-series DB)
- **Parquet files** (columnar format, cloud-native)

**Decision**:
Use **Parquet files** as the primary storage format, with Polars for querying.

**Rationale**:
1. **Cloud-Native**: Parquet files on S3 enable serverless analytics (Athena, Presto)
2. **Zero Operations**: No database server to manage on Mac/Pi5
3. **Portability**: Files can be moved, backed up, analyzed anywhere
4. **Efficient**: Columnar format optimized for analytics
5. **Ecosystem**: Polars provides fast DataFrame operations in Rust
6. **Cost**: No licensing or hosting fees (S3 is cheap for cold storage)

**Consequences**:
- **Positive**: Simple deployment, no DB dependencies, cloud-ready
- **Negative**: No built-in indexing (must rely on partitioning and Polars)
- **Mitigation**: Use date-based partitioning for time-range queries

**Alternatives Considered**:
- **QuestDB**: Best for high-speed writes, but requires server management
- **TimescaleDB**: Excellent for complex queries, but PostgreSQL dependency
- **InfluxDB**: Mature ecosystem, but high-cardinality performance issues

---

### ADR-002: Polars over DataFusion

**Status**: Accepted
**Date**: 2025-12-13

**Context**:
For querying Parquet files, we can use:
- **Polars**: DataFrame library with lazy evaluation
- **DataFusion**: Query engine with SQL support
- **Arrow**: Lower-level columnar processing

**Decision**:
Use **Polars** for Parquet querying and data manipulation.

**Rationale**:
1. **Rust-Native**: Polars is written in Rust, excellent type safety
2. **Ergonomic API**: DataFrame operations are intuitive
3. **Lazy Evaluation**: Query optimization out of the box
4. **Performance**: Multi-threaded, SIMD-accelerated
5. **No SQL Parsing**: Direct API avoids SQL injection risks

**Consequences**:
- **Positive**: Fast development, great Rust integration
- **Negative**: Users who prefer SQL must use our API
- **Future**: Can add DataFusion SQL layer on top later

---

### ADR-003: Generic Traits over Domain-Specific Core

**Status**: Accepted
**Date**: 2025-12-13

**Context**:
The existing neural-data-platform has trading domain types mixed into `neural-core`. We need to decide: should the new `core/` crate be:
- **Domain-agnostic** with generic traits
- **Multi-domain** with hardcoded support for multiple domains

**Decision**:
Build a **completely domain-agnostic core** using generic traits (`TimeSeriesPoint`, `Store`, `Source`, `Forecast`).

**Rationale**:
1. **Reusability**: Same core can support air quality, energy, finance, IoT
2. **Testability**: Core logic can be tested with mock implementations
3. **Maintainability**: Domain changes don't require core changes
4. **Clarity**: Clear separation between generic and domain-specific code

**Consequences**:
- **Positive**: Enables air quality as first domain, energy/IoT as future domains
- **Negative**: Requires adapter layer for each domain (slight overhead)
- **Trade-off**: Small runtime cost (trait dispatch) for massive flexibility

---

### ADR-004: MQTT over HTTP Polling

**Status**: Accepted
**Date**: 2025-12-13

**Context**:
AirGradient sensors support multiple protocols:
- **MQTT**: Publish/subscribe, real-time push
- **HTTP API**: REST endpoints, polling required
- **Local API**: Direct device access over WiFi

**Decision**:
Use **MQTT** as the primary ingestion protocol.

**Rationale**:
1. **Real-Time**: Push-based, no polling delay
2. **Efficient**: Lower network overhead than HTTP polling
3. **Scalable**: One broker can handle many sensors
4. **Standard**: MQTT is IoT industry standard
5. **ruv-mqttc**: Excellent Rust MQTT client library

**Consequences**:
- **Positive**: Low-latency data ingestion, efficient bandwidth use
- **Negative**: Requires MQTT broker (Mosquitto)
- **Mitigation**: Mosquitto is lightweight, easy to run on Pi5/Mac

**Future Extensions**:
- Add HTTP polling source adapter for devices without MQTT
- Support WebSocket for browser-based sensors

---

## 8. Extension Points

### 8.1 Adding a New Domain (Energy Example)

**Goal**: Support energy monitoring (solar panels, battery, grid usage)

**Steps**:

1. **Create Domain Crate**:
```bash
mkdir -p domains/energy/src
```

2. **Define Domain Types**:
```rust
// domains/energy/src/types.rs

pub struct EnergyReading {
    pub device_id: String,
    pub timestamp: DateTime<Utc>,
    pub solar_production: Option<f64>,  // kW
    pub battery_charge: Option<f64>,    // %
    pub grid_consumption: Option<f64>,  // kW
}
```

3. **Create Adapter**:
```rust
// domains/energy/src/adapter.rs

impl TimeSeriesPoint for EnergyPoint {
    fn metric(&self) -> &str {
        &self.metric_name  // "solar_production", "battery_charge", etc.
    }

    fn tags(&self) -> &HashMap<String, String> {
        // {"device_id": "...", "type": "solar"}
    }
}
```

4. **Create Application**:
```bash
mkdir -p apps/energy-app/src
```

5. **Reuse Core**:
```toml
# apps/energy-app/Cargo.toml
[dependencies]
core = { path = "../../core" }
energy = { path = "../../domains/energy" }
```

**Key Point**: No changes to `core/` required! The generic traits support energy out of the box.

### 8.2 Adding a New Storage Backend (InfluxDB Example)

**Goal**: Support InfluxDB for users who prefer a traditional time-series database

**Steps**:

1. **Create Storage Adapter**:
```rust
// core/src/storage/influxdb.rs

use influxdb::Client;
use crate::traits::Store;

pub struct InfluxDBStore {
    client: Client,
    database: String,
}

#[async_trait]
impl Store for InfluxDBStore {
    async fn write(&self, points: Vec<Box<dyn TimeSeriesPoint>>) -> Result<()> {
        // Convert TimeSeriesPoint to InfluxDB line protocol
        // Write to InfluxDB
    }

    async fn query(...) -> Result<Vec<Box<dyn TimeSeriesPoint>>> {
        // Query InfluxDB
        // Convert results to TimeSeriesPoint
    }
}
```

2. **Add Configuration**:
```toml
# config.toml
[storage]
type = "influxdb"  # or "parquet"
url = "http://localhost:8086"
database = "air_quality"
```

3. **Factory Pattern**:
```rust
// core/src/storage/mod.rs

pub fn create_store(config: &StorageConfig) -> Result<Box<dyn Store>> {
    match config.storage_type {
        StorageType::Parquet => Ok(Box::new(ParquetStore::new(config.base_path)?)),
        StorageType::InfluxDB => Ok(Box::new(InfluxDBStore::new(config.url, config.database)?)),
        StorageType::TimescaleDB => Ok(Box::new(TimescaleDBStore::new(config.connection_string)?)),
    }
}
```

**Key Point**: Hexagonal architecture makes this trivial. Just implement the `Store` trait.

### 8.3 Adding a New Data Source (HTTP Polling Example)

**Goal**: Support devices that only offer HTTP APIs (no MQTT)

**Steps**:

1. **Create Source Adapter**:
```rust
// core/src/sources/http_poll.rs

use crate::traits::Source;
use reqwest::Client;

pub struct HttpPollingSource {
    id: String,
    endpoint: String,
    poll_interval: Duration,
    client: Client,
}

#[async_trait]
impl Source for HttpPollingSource {
    async fn start(&mut self) -> Result<()> {
        // Spawn background task that polls HTTP endpoint
        tokio::spawn(async move {
            loop {
                match self.client.get(&self.endpoint).send().await {
                    Ok(response) => {
                        // Parse response, send to stream
                    }
                    Err(e) => {
                        // Handle error
                    }
                }
                tokio::time::sleep(self.poll_interval).await;
            }
        });
        Ok(())
    }
}
```

2. **Configuration**:
```toml
[sources]
[[sources.http]]
id = "sensor-002"
endpoint = "http://192.168.1.100/api/readings"
poll_interval = "60s"
```

**Key Point**: The `Source` trait abstracts away protocol differences. Core pipeline doesn't care.

---

## 9. C4 Architecture Diagrams

### 9.1 C4 Context Diagram

```xml
<?xml version="1.0" encoding="UTF-8"?>
<mxfile host="app.diagrams.net">
  <diagram name="C4-Context" id="context">
    <mxGraphModel dx="1422" dy="794" grid="1" gridSize="10" guides="1" tooltips="1" connect="1" arrows="1" fold="1" page="1" pageScale="1" pageWidth="1169" pageHeight="827">
      <root>
        <mxCell id="0"/>
        <mxCell id="1" parent="0"/>

        <mxCell id="system" value="Neural Data Platform&#xa;[Software System]&#xa;&#xa;Generic time-series intelligence platform&#xa;for ingesting, storing, querying, and&#xa;forecasting multi-domain data" style="rounded=1;whiteSpace=wrap;html=1;fillColor=#1168BD;strokeColor=#0D5091;fontColor=#FFFFFF;fontSize=14;align=center;verticalAlign=middle;" vertex="1" parent="1">
          <mxGeometry x="470" y="320" width="240" height="160" as="geometry"/>
        </mxCell>

        <mxCell id="user" value="Data Analyst&#xa;[Person]&#xa;&#xa;Queries air quality data,&#xa;views forecasts, creates alerts" style="shape=umlActor;verticalLabelPosition=bottom;verticalAlign=top;html=1;fillColor=#08427B;strokeColor=#073B6F;fontColor=#FFFFFF;" vertex="1" parent="1">
          <mxGeometry x="100" y="350" width="60" height="100" as="geometry"/>
        </mxCell>

        <mxCell id="admin" value="System Admin&#xa;[Person]&#xa;&#xa;Configures sensors,&#xa;manages storage, monitors health" style="shape=umlActor;verticalLabelPosition=bottom;verticalAlign=top;html=1;fillColor=#08427B;strokeColor=#073B6F;fontColor=#FFFFFF;" vertex="1" parent="1">
          <mxGeometry x="100" y="150" width="60" height="100" as="geometry"/>
        </mxCell>

        <mxCell id="sensor" value="AirGradient Sensor&#xa;[External System]&#xa;&#xa;Measures PM2.5, CO2, temperature,&#xa;humidity. Publishes via MQTT" style="rounded=1;whiteSpace=wrap;html=1;fillColor=#999999;strokeColor=#6D6D6D;fontColor=#FFFFFF;fontSize=12;" vertex="1" parent="1">
          <mxGeometry x="870" y="150" width="200" height="120" as="geometry"/>
        </mxCell>

        <mxCell id="mqtt" value="MQTT Broker&#xa;[External System]&#xa;&#xa;Mosquitto message broker&#xa;for sensor data pub/sub" style="rounded=1;whiteSpace=wrap;html=1;fillColor=#999999;strokeColor=#6D6D6D;fontColor=#FFFFFF;fontSize=12;" vertex="1" parent="1">
          <mxGeometry x="870" y="340" width="200" height="100" as="geometry"/>
        </mxCell>

        <mxCell id="s3" value="Cloud Storage&#xa;[External System]&#xa;&#xa;S3-compatible object storage&#xa;for long-term data retention" style="rounded=1;whiteSpace=wrap;html=1;fillColor=#999999;strokeColor=#6D6D6D;fontColor=#FFFFFF;fontSize=12;" vertex="1" parent="1">
          <mxGeometry x="470" y="580" width="200" height="100" as="geometry"/>
        </mxCell>

        <mxCell id="analytics" value="Analytics Platform&#xa;[External System]&#xa;&#xa;Athena, Presto, or Jupyter&#xa;for advanced analysis" style="rounded=1;whiteSpace=wrap;html=1;fillColor=#999999;strokeColor=#6D6D6D;fontColor=#FFFFFF;fontSize=12;" vertex="1" parent="1">
          <mxGeometry x="100" y="580" width="200" height="100" as="geometry"/>
        </mxCell>

        <mxCell id="edge1" value="Queries data,&#xa;views dashboards" edge="1" parent="1" source="user" target="system">
          <mxGeometry relative="1" as="geometry"/>
        </mxCell>

        <mxCell id="edge2" value="Configures,&#xa;monitors" edge="1" parent="1" source="admin" target="system">
          <mxGeometry relative="1" as="geometry"/>
        </mxCell>

        <mxCell id="edge3" value="Publishes readings" edge="1" parent="1" source="sensor" target="mqtt">
          <mxGeometry relative="1" as="geometry"/>
        </mxCell>

        <mxCell id="edge4" value="Subscribes to&#xa;sensor topics&#xa;[MQTT]" edge="1" parent="1" source="system" target="mqtt">
          <mxGeometry relative="1" as="geometry"/>
        </mxCell>

        <mxCell id="edge5" value="Writes Parquet files&#xa;[S3 API]" edge="1" parent="1" source="system" target="s3">
          <mxGeometry relative="1" as="geometry"/>
        </mxCell>

        <mxCell id="edge6" value="Queries Parquet&#xa;[Athena/SQL]" edge="1" parent="1" source="analytics" target="s3">
          <mxGeometry relative="1" as="geometry"/>
        </mxCell>

        <mxCell id="edge7" value="Exports data for&#xa;analysis" edge="1" parent="1" source="system" target="analytics">
          <mxGeometry relative="1" as="geometry"/>
        </mxCell>

        <mxCell id="title" value="System Context Diagram: Neural Data Platform" style="text;html=1;strokeColor=none;fillColor=none;align=center;verticalAlign=middle;whiteSpace=wrap;fontSize=18;fontStyle=1" vertex="1" parent="1">
          <mxGeometry x="400" y="40" width="400" height="40" as="geometry"/>
        </mxCell>

      </root>
    </mxGraphModel>
  </diagram>
</mxfile>
```

### 9.2 C4 Container Diagram

```xml
<?xml version="1.0" encoding="UTF-8"?>
<mxfile host="app.diagrams.net">
  <diagram name="C4-Container" id="container">
    <mxGraphModel dx="1422" dy="794" grid="1" gridSize="10" guides="1" tooltips="1" connect="1" arrows="1" fold="1" page="1" pageScale="1" pageWidth="1169" pageHeight="827">
      <root>
        <mxCell id="0"/>
        <mxCell id="1" parent="0"/>

        <mxCell id="system-boundary" value="Neural Data Platform" style="swimlane;whiteSpace=wrap;html=1;fillColor=#E6E6E6;strokeColor=#999999;fontSize=16;fontStyle=1;startSize=40;" vertex="1" parent="1">
          <mxGeometry x="200" y="120" width="760" height="560" as="geometry"/>
        </mxCell>

        <mxCell id="rest-api" value="REST API&#xa;[Container: Axum]&#xa;&#xa;HTTP API for querying data,&#xa;creating forecasts, managing config" style="rounded=1;whiteSpace=wrap;html=1;fillColor=#1168BD;strokeColor=#0D5091;fontColor=#FFFFFF;fontSize=12;" vertex="1" parent="system-boundary">
          <mxGeometry x="40" y="80" width="160" height="100" as="geometry"/>
        </mxCell>

        <mxCell id="cli" value="CLI&#xa;[Container: Clap]&#xa;&#xa;Command-line interface&#xa;for admin tasks" style="rounded=1;whiteSpace=wrap;html=1;fillColor=#1168BD;strokeColor=#0D5091;fontColor=#FFFFFF;fontSize=12;" vertex="1" parent="system-boundary">
          <mxGeometry x="40" y="220" width="160" height="100" as="geometry"/>
        </mxCell>

        <mxCell id="core" value="Core Platform&#xa;[Container: Rust Library]&#xa;&#xa;Generic time-series platform&#xa;with traits for storage, sources,&#xa;forecasting" style="rounded=1;whiteSpace=wrap;html=1;fillColor=#438DD5;strokeColor=#2E6CA4;fontColor=#FFFFFF;fontSize=12;" vertex="1" parent="system-boundary">
          <mxGeometry x="280" y="80" width="200" height="120" as="geometry"/>
        </mxCell>

        <mxCell id="domain" value="Air Quality Domain&#xa;[Container: Rust Library]&#xa;&#xa;Domain-specific types, parsers,&#xa;validation, AQI calculations" style="rounded=1;whiteSpace=wrap;html=1;fillColor=#438DD5;strokeColor=#2E6CA4;fontColor=#FFFFFF;fontSize=12;" vertex="1" parent="system-boundary">
          <mxGeometry x="280" y="240" width="200" height="100" as="geometry"/>
        </mxCell>

        <mxCell id="parquet" value="Parquet Storage&#xa;[Container: Polars]&#xa;&#xa;Columnar storage with&#xa;date partitioning" style="rounded=1;whiteSpace=wrap;html=1;fillColor=#438DD5;strokeColor=#2E6CA4;fontColor=#FFFFFF;fontSize=12;" vertex="1" parent="system-boundary">
          <mxGeometry x="560" y="80" width="160" height="100" as="geometry"/>
        </mxCell>

        <mxCell id="mqtt-client" value="MQTT Client&#xa;[Container: rumqttc]&#xa;&#xa;Subscribes to sensor topics,&#xa;receives real-time data" style="rounded=1;whiteSpace=wrap;html=1;fillColor=#438DD5;strokeColor=#2E6CA4;fontColor=#FFFFFF;fontSize=12;" vertex="1" parent="system-boundary">
          <mxGeometry x="560" y="220" width="160" height="100" as="geometry"/>
        </mxCell>

        <mxCell id="forecast" value="FANN Forecaster&#xa;[Container: ruv-FANN]&#xa;&#xa;Neural network models&#xa;for time-series prediction" style="rounded=1;whiteSpace=wrap;html=1;fillColor=#438DD5;strokeColor=#2E6CA4;fontColor=#FFFFFF;fontSize=12;" vertex="1" parent="system-boundary">
          <mxGeometry x="280" y="400" width="200" height="100" as="geometry"/>
        </mxCell>

        <mxCell id="config" value="Config Store&#xa;[Container: PostgreSQL]&#xa;&#xa;Application configuration,&#xa;sensor metadata" style="shape=cylinder3;whiteSpace=wrap;html=1;boundedLbl=1;backgroundOutline=1;fillColor=#5A6C86;strokeColor=#4A5A6F;fontColor=#FFFFFF;fontSize=12;" vertex="1" parent="system-boundary">
          <mxGeometry x="560" y="380" width="160" height="80" as="geometry"/>
        </mxCell>

        <mxCell id="edge-rest-core" value="Uses&#xa;[HTTP/JSON]" edge="1" parent="system-boundary" source="rest-api" target="core">
          <mxGeometry relative="1" as="geometry"/>
        </mxCell>

        <mxCell id="edge-cli-core" value="Invokes" edge="1" parent="system-boundary" source="cli" target="core">
          <mxGeometry relative="1" as="geometry"/>
        </mxCell>

        <mxCell id="edge-core-domain" value="Uses" edge="1" parent="system-boundary" source="core" target="domain">
          <mxGeometry relative="1" as="geometry"/>
        </mxCell>

        <mxCell id="edge-core-parquet" value="Reads/Writes&#xa;[Trait: Store]" edge="1" parent="system-boundary" source="core" target="parquet">
          <mxGeometry relative="1" as="geometry"/>
        </mxCell>

        <mxCell id="edge-core-mqtt" value="Subscribes&#xa;[Trait: Source]" edge="1" parent="system-boundary" source="core" target="mqtt-client">
          <mxGeometry relative="1" as="geometry"/>
        </mxCell>

        <mxCell id="edge-core-forecast" value="Predicts&#xa;[Trait: Forecast]" edge="1" parent="system-boundary" source="core" target="forecast">
          <mxGeometry relative="1" as="geometry"/>
        </mxCell>

        <mxCell id="edge-core-config" value="Loads config" edge="1" parent="system-boundary" source="core" target="config">
          <mxGeometry relative="1" as="geometry"/>
        </mxCell>

        <mxCell id="user-external" value="User" style="shape=umlActor;verticalLabelPosition=bottom;verticalAlign=top;html=1;fillColor=#08427B;strokeColor=#073B6F;fontColor=#FFFFFF;" vertex="1" parent="1">
          <mxGeometry x="80" y="250" width="60" height="80" as="geometry"/>
        </mxCell>

        <mxCell id="mqtt-broker-external" value="MQTT Broker" style="rounded=1;whiteSpace=wrap;html=1;fillColor=#999999;strokeColor=#6D6D6D;fontColor=#FFFFFF;fontSize=12;" vertex="1" parent="1">
          <mxGeometry x="1020" y="350" width="120" height="60" as="geometry"/>
        </mxCell>

        <mxCell id="edge-user-rest" value="Queries" edge="1" parent="1" source="user-external" target="rest-api">
          <mxGeometry relative="1" as="geometry"/>
        </mxCell>

        <mxCell id="edge-mqtt-client-broker" value="Subscribes&#xa;[MQTT]" edge="1" parent="1" source="mqtt-client" target="mqtt-broker-external">
          <mxGeometry relative="1" as="geometry"/>
        </mxCell>

        <mxCell id="title-container" value="Container Diagram: Neural Data Platform" style="text;html=1;strokeColor=none;fillColor=none;align=center;verticalAlign=middle;whiteSpace=wrap;fontSize=18;fontStyle=1" vertex="1" parent="1">
          <mxGeometry x="400" y="40" width="400" height="40" as="geometry"/>
        </mxCell>

      </root>
    </mxGraphModel>
  </diagram>
</mxfile>
```

### 9.3 C4 Component Diagram (Core Platform)

```xml
<?xml version="1.0" encoding="UTF-8"?>
<mxfile host="app.diagrams.net">
  <diagram name="C4-Component" id="component">
    <mxGraphModel dx="1422" dy="794" grid="1" gridSize="10" guides="1" tooltips="1" connect="1" arrows="1" fold="1" page="1" pageScale="1" pageWidth="1169" pageHeight="827">
      <root>
        <mxCell id="0"/>
        <mxCell id="1" parent="0"/>

        <mxCell id="core-boundary" value="Core Platform [Container]" style="swimlane;whiteSpace=wrap;html=1;fillColor=#E1F5FE;strokeColor=#0D47A1;fontSize=16;fontStyle=1;startSize=40;" vertex="1" parent="1">
          <mxGeometry x="120" y="80" width="920" height="640" as="geometry"/>
        </mxCell>

        <mxCell id="traits" value="Core Traits&#xa;[Component]&#xa;&#xa;TimeSeriesPoint, Store,&#xa;Source, Forecast traits" style="rounded=1;whiteSpace=wrap;html=1;fillColor=#4CAF50;strokeColor=#2E7D32;fontColor=#FFFFFF;fontSize=12;" vertex="1" parent="core-boundary">
          <mxGeometry x="380" y="80" width="160" height="100" as="geometry"/>
        </mxCell>

        <mxCell id="ingestion-pipeline" value="Ingestion Pipeline&#xa;[Component]&#xa;&#xa;Batching, validation,&#xa;backpressure, error handling" style="rounded=1;whiteSpace=wrap;html=1;fillColor=#2196F3;strokeColor=#0D47A1;fontColor=#FFFFFF;fontSize=12;" vertex="1" parent="core-boundary">
          <mxGeometry x="40" y="80" width="180" height="100" as="geometry"/>
        </mxCell>

        <mxCell id="query-engine" value="Query Engine&#xa;[Component]&#xa;&#xa;Query planning, partition&#xa;pruning, result aggregation" style="rounded=1;whiteSpace=wrap;html=1;fillColor=#2196F3;strokeColor=#0D47A1;fontColor=#FFFFFF;fontSize=12;" vertex="1" parent="core-boundary">
          <mxGeometry x="40" y="220" width="180" height="100" as="geometry"/>
        </mxCell>

        <mxCell id="parquet-store" value="ParquetStore&#xa;[Component]&#xa;&#xa;Implements Store trait,&#xa;writes/reads Parquet files" style="rounded=1;whiteSpace=wrap;html=1;fillColor=#FF9800;strokeColor=#E65100;fontColor=#FFFFFF;fontSize=12;" vertex="1" parent="core-boundary">
          <mxGeometry x="680" y="80" width="180" height="100" as="geometry"/>
        </mxCell>

        <mxCell id="mqtt-source" value="MqttSource&#xa;[Component]&#xa;&#xa;Implements Source trait,&#xa;subscribes to MQTT topics" style="rounded=1;whiteSpace=wrap;html=1;fillColor=#FF9800;strokeColor=#E65100;fontColor=#FFFFFF;fontSize=12;" vertex="1" parent="core-boundary">
          <mxGeometry x="680" y="220" width="180" height="100" as="geometry"/>
        </mxCell>

        <mxCell id="fann-adapter" value="FannAdapter&#xa;[Component]&#xa;&#xa;Implements Forecast trait,&#xa;wraps ruv-FANN models" style="rounded=1;whiteSpace=wrap;html=1;fillColor=#FF9800;strokeColor=#E65100;fontColor=#FFFFFF;fontSize=12;" vertex="1" parent="core-boundary">
          <mxGeometry x="680" y="360" width="180" height="100" as="geometry"/>
        </mxCell>

        <mxCell id="error-handling" value="Error Handling&#xa;[Component]&#xa;&#xa;CoreError types, Result&#xa;wrappers, retry logic" style="rounded=1;whiteSpace=wrap;html=1;fillColor=#9C27B0;strokeColor=#6A1B9A;fontColor=#FFFFFF;fontSize=12;" vertex="1" parent="core-boundary">
          <mxGeometry x="380" y="360" width="160" height="100" as="geometry"/>
        </mxCell>

        <mxCell id="metrics" value="Metrics & Observability&#xa;[Component]&#xa;&#xa;OpenTelemetry instrumentation,&#xa;health checks" style="rounded=1;whiteSpace=wrap;html=1;fillColor=#9C27B0;strokeColor=#6A1B9A;fontColor=#FFFFFF;fontSize=12;" vertex="1" parent="core-boundary">
          <mxGeometry x="380" y="500" width="180" height="100" as="geometry"/>
        </mxCell>

        <mxCell id="data-model" value="Data Models&#xa;[Component]&#xa;&#xa;AggregatedPoint,&#xa;ForecastedPoint, ModelMetrics" style="rounded=1;whiteSpace=wrap;html=1;fillColor=#607D8B;strokeColor=#37474F;fontColor=#FFFFFF;fontSize=12;" vertex="1" parent="core-boundary">
          <mxGeometry x="40" y="360" width="180" height="100" as="geometry"/>
        </mxCell>

        <mxCell id="edge-ingestion-traits" value="Uses" edge="1" parent="core-boundary" source="ingestion-pipeline" target="traits">
          <mxGeometry relative="1" as="geometry"/>
        </mxCell>

        <mxCell id="edge-query-traits" value="Uses" edge="1" parent="core-boundary" source="query-engine" target="traits">
          <mxGeometry relative="1" as="geometry"/>
        </mxCell>

        <mxCell id="edge-parquet-traits" value="Implements&#xa;Store" edge="1" parent="core-boundary" source="parquet-store" target="traits">
          <mxGeometry relative="1" as="geometry"/>
        </mxCell>

        <mxCell id="edge-mqtt-traits" value="Implements&#xa;Source" edge="1" parent="core-boundary" source="mqtt-source" target="traits">
          <mxGeometry relative="1" as="geometry"/>
        </mxCell>

        <mxCell id="edge-fann-traits" value="Implements&#xa;Forecast" edge="1" parent="core-boundary" source="fann-adapter" target="traits">
          <mxGeometry relative="1" as="geometry"/>
        </mxCell>

        <mxCell id="edge-ingestion-parquet" value="Writes data" edge="1" parent="core-boundary" source="ingestion-pipeline" target="parquet-store">
          <mxGeometry relative="1" as="geometry">
            <Array as="points">
              <mxPoint x="130" y="50"/>
              <mxPoint x="770" y="50"/>
            </Array>
          </mxGeometry>
        </mxCell>

        <mxCell id="edge-query-parquet" value="Reads data" edge="1" parent="core-boundary" source="query-engine" target="parquet-store">
          <mxGeometry relative="1" as="geometry">
            <Array as="points">
              <mxPoint x="130" y="340"/>
              <mxPoint x="770" y="340"/>
            </Array>
          </mxGeometry>
        </mxCell>

        <mxCell id="edge-ingestion-mqtt" value="Receives&#xa;stream" edge="1" parent="core-boundary" source="ingestion-pipeline" target="mqtt-source">
          <mxGeometry relative="1" as="geometry">
            <Array as="points">
              <mxPoint x="220" y="130"/>
              <mxPoint x="680" y="270"/>
            </Array>
          </mxGeometry>
        </mxCell>

        <mxCell id="edge-error-all" value="Used by&#xa;all components" edge="1" parent="core-boundary" source="error-handling" target="traits">
          <mxGeometry relative="1" as="geometry"/>
        </mxCell>

        <mxCell id="external-rest" value="REST API" style="rounded=1;whiteSpace=wrap;html=1;fillColor=#1168BD;strokeColor=#0D5091;fontColor=#FFFFFF;fontSize=12;" vertex="1" parent="1">
          <mxGeometry x="40" y="280" width="120" height="60" as="geometry"/>
        </mxCell>

        <mxCell id="external-domain" value="Air Quality Domain" style="rounded=1;whiteSpace=wrap;html=1;fillColor=#438DD5;strokeColor=#2E6CA4;fontColor=#FFFFFF;fontSize=12;" vertex="1" parent="1">
          <mxGeometry x="40" y="400" width="120" height="60" as="geometry"/>
        </mxCell>

        <mxCell id="edge-rest-query" value="Queries" edge="1" parent="1" source="external-rest" target="query-engine">
          <mxGeometry relative="1" as="geometry"/>
        </mxCell>

        <mxCell id="edge-domain-ingestion" value="Sends&#xa;parsed data" edge="1" parent="1" source="external-domain" target="ingestion-pipeline">
          <mxGeometry relative="1" as="geometry"/>
        </mxCell>

        <mxCell id="title-component" value="Component Diagram: Core Platform Container" style="text;html=1;strokeColor=none;fillColor=none;align=center;verticalAlign=middle;whiteSpace=wrap;fontSize=18;fontStyle=1" vertex="1" parent="1">
          <mxGeometry x="350" y="20" width="480" height="40" as="geometry"/>
        </mxCell>

      </root>
    </mxGraphModel>
  </diagram>
</mxfile>
```

---

## Summary

This SPARC Architecture document provides a comprehensive design for the Neural Data Platform's Air Quality feature (air-001). Key highlights:

1. **Hexagonal Architecture**: Clear separation between generic core and domain-specific adapters
2. **Workspace Structure**: Organized crates for core, domains, and applications
3. **Component Design**: Generic traits with Parquet storage, MQTT ingestion, and FANN forecasting
4. **Data Flows**: Detailed ingestion, query, and forecasting pipelines
5. **Reuse**: Leverages existing `neural-core`, `ruv-fann`, `neural-ml-ops`, and `config-store`
6. **Deployment**: Mac development, Pi5 production, cloud migration path
7. **ADRs**: Documented architectural decisions (Parquet, Polars, MQTT, Generic traits)
8. **Extension Points**: Clear guidance for adding domains, storage backends, and data sources
9. **C4 Diagrams**: Context, Container, and Component views in draw.io format

This architecture enables the air quality use case while maintaining complete domain agnosticism for future expansion to energy, IoT, financial, and other time-series domains.
