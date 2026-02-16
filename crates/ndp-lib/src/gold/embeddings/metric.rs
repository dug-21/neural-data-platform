//! MetricEmbedder — converts Gold rows to z-score normalized vector embeddings
//!
//! The MetricEmbedder transforms time-bucketed Gold layer records into
//! fixed-dimension vectors suitable for similarity search. Each dimension
//! represents either a temporal encoding, a direct metric (z-score normalized),
//! or a derived feature.

use std::collections::HashMap;

use chrono::{Datelike, Timelike};

use super::config::{EmbeddingConfig, EmbeddingFieldsConfig, NullStrategyConfig};
use super::stats::RunningStats;
use super::{Embedder, Embedding, EmbeddingError, EmbeddingResult, GoldRow};

/// Metric-based embedder that converts Gold rows to z-score normalized vectors.
///
/// # Thread Safety Lifecycle
///
/// `MetricEmbedder` has a two-phase lifecycle:
///
/// 1. **Warmup phase** (single-threaded): Call [`observe()`](Self::observe) repeatedly
///    with historical data to build running statistics. This requires `&mut self`.
///
/// 2. **Embedding phase** (shareable): Once [`is_ready()`](Self::is_ready) returns `true`,
///    the embedder can be wrapped in `Arc` and shared across threads. Only
///    [`embed()`](Self::embed) (via the `Embedder` trait, `&self`) is called.
///
/// **Do not call `observe()` concurrently.** The warmup phase must complete
/// on a single thread before the embedder is shared. In Phase 2's daemon,
/// the expected pattern is:
///
/// ```ignore
/// let mut embedder = MetricEmbedder::from_config(&config);
/// for row in historical_rows { embedder.observe(&row); }
/// let embedder = Arc::new(embedder); // now shareable
/// ```
pub struct MetricEmbedder {
    fields: Vec<EmbeddingField>,
    stats: HashMap<String, RunningStats>,
    dimensions: usize,
    warmup_threshold: usize,
    observations: usize,
    last_known: HashMap<String, f64>,
}

/// A single field in the embedding vector.
#[derive(Debug, Clone)]
pub struct EmbeddingField {
    /// Human-readable field name
    pub name: String,
    /// How to extract the value
    pub source: FieldSource,
    /// How to handle null values
    pub null_strategy: NullStrategy,
}

/// Source of a field's value.
#[derive(Debug, Clone)]
pub enum FieldSource {
    /// Direct lookup from GoldRow fields
    Direct(String),
    /// Temporal encoding computed from the timestamp
    Temporal(TemporalEncoding),
}

/// Temporal encoding types.
#[derive(Debug, Clone)]
pub enum TemporalEncoding {
    /// sin(2*pi*hour/24)
    HourSin,
    /// cos(2*pi*hour/24)
    HourCos,
    /// 1.0 if Saturday or Sunday, 0.0 otherwise
    IsWeekend,
}

/// Strategy for handling null/missing values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NullStrategy {
    /// Replace null with 0.0
    Zero,
    /// Replace null with last known value
    LastKnown,
    /// Replace null with running mean (maps to 0.0 in z-score space)
    Mean,
}

impl From<NullStrategyConfig> for NullStrategy {
    fn from(config: NullStrategyConfig) -> Self {
        match config {
            NullStrategyConfig::Zero => NullStrategy::Zero,
            NullStrategyConfig::LastKnown => NullStrategy::LastKnown,
            NullStrategyConfig::Mean => NullStrategy::Mean,
        }
    }
}

impl MetricEmbedder {
    /// Create a MetricEmbedder from an EmbeddingConfig.
    pub fn from_config(config: &EmbeddingConfig) -> Self {
        Self::from_fields_config(&config.fields)
    }

    /// Create a MetricEmbedder from an EmbeddingFieldsConfig.
    pub fn from_fields_config(fields_config: &EmbeddingFieldsConfig) -> Self {
        let mut fields = Vec::new();

        // Temporal fields
        for temporal_name in &fields_config.temporal {
            let encoding = match temporal_name.as_str() {
                "hour_sin" => TemporalEncoding::HourSin,
                "hour_cos" => TemporalEncoding::HourCos,
                "is_weekend" => TemporalEncoding::IsWeekend,
                _ => continue, // Unknown temporal encoding, skip
            };
            fields.push(EmbeddingField {
                name: temporal_name.clone(),
                source: FieldSource::Temporal(encoding),
                null_strategy: NullStrategy::Zero, // Temporal never null
            });
        }

        // Direct fields
        for direct_config in &fields_config.direct {
            fields.push(EmbeddingField {
                name: direct_config.field.clone(),
                source: FieldSource::Direct(direct_config.field.clone()),
                null_strategy: direct_config.null_strategy.into(),
            });
        }

        // Derived fields (treated as direct lookups from Gold row)
        for derived_name in &fields_config.derived {
            fields.push(EmbeddingField {
                name: derived_name.clone(),
                source: FieldSource::Direct(derived_name.clone()),
                null_strategy: NullStrategy::Zero,
            });
        }

        let dimensions = fields.len();

        Self {
            fields,
            stats: HashMap::new(),
            dimensions,
            warmup_threshold: 168,
            observations: 0,
            last_known: HashMap::new(),
        }
    }

    /// Create a MetricEmbedder with a custom warmup threshold.
    pub fn with_warmup(mut self, threshold: usize) -> Self {
        self.warmup_threshold = threshold;
        self
    }

    /// Update running statistics from a Gold row without producing an embedding.
    ///
    /// Call this during warmup to build up stats before generating embeddings.
    pub fn observe(&mut self, row: &GoldRow) {
        self.observations += 1;

        for field in &self.fields {
            if let FieldSource::Direct(ref field_name) = field.source {
                if let Some(Some(value)) = row.fields.get(field_name) {
                    let stats = self
                        .stats
                        .entry(field_name.clone())
                        .or_insert_with(RunningStats::default_params);
                    stats.update(*value);
                    self.last_known.insert(field_name.clone(), *value);
                }
            }
        }
    }

    /// Check if the embedder has sufficient data for reliable embeddings.
    pub fn is_ready(&self) -> bool {
        self.observations >= self.warmup_threshold
    }

    /// Get the number of observations processed.
    pub fn observation_count(&self) -> usize {
        self.observations
    }

    fn compute_value(&self, field: &EmbeddingField, row: &GoldRow) -> EmbeddingResult<f32> {
        match &field.source {
            FieldSource::Temporal(encoding) => Ok(self.compute_temporal(encoding, row)),
            FieldSource::Direct(field_name) => {
                let raw_value = row.fields.get(field_name).cloned();

                match raw_value {
                    Some(Some(value)) => {
                        // Have a value: z-score normalize it
                        let stats = self.stats.get(field_name);
                        match stats {
                            Some(s) => Ok(s.z_score(value) as f32),
                            None => Ok(0.0), // No stats yet, return 0
                        }
                    }
                    Some(None) | None => {
                        // Null or missing: apply null strategy
                        self.apply_null_strategy(field_name, &field.null_strategy)
                    }
                }
            }
        }
    }

    fn compute_temporal(&self, encoding: &TemporalEncoding, row: &GoldRow) -> f32 {
        let hour = row.bucket.hour() as f64;
        match encoding {
            TemporalEncoding::HourSin => (2.0 * std::f64::consts::PI * hour / 24.0).sin() as f32,
            TemporalEncoding::HourCos => (2.0 * std::f64::consts::PI * hour / 24.0).cos() as f32,
            TemporalEncoding::IsWeekend => {
                let weekday = row.bucket.weekday();
                if weekday == chrono::Weekday::Sat || weekday == chrono::Weekday::Sun {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }

    fn apply_null_strategy(
        &self,
        field_name: &str,
        strategy: &NullStrategy,
    ) -> EmbeddingResult<f32> {
        match strategy {
            NullStrategy::Zero => Ok(0.0),
            NullStrategy::Mean => {
                // Mean in z-score space is 0.0
                Ok(0.0)
            }
            NullStrategy::LastKnown => {
                match self.last_known.get(field_name) {
                    Some(last_value) => {
                        // Z-score the last known value
                        let stats = self.stats.get(field_name);
                        match stats {
                            Some(s) => Ok(s.z_score(*last_value) as f32),
                            None => Ok(0.0),
                        }
                    }
                    None => Ok(0.0), // No last known value, fall back to 0
                }
            }
        }
    }
}

impl Embedder for MetricEmbedder {
    fn embed(&self, row: &GoldRow) -> EmbeddingResult<Embedding> {
        if !self.is_ready() {
            return Err(EmbeddingError::InsufficientData {
                reason: format!(
                    "Only {} observations, need {} for warmup",
                    self.observations, self.warmup_threshold
                ),
            });
        }

        let mut vector = Vec::with_capacity(self.dimensions);
        for field in &self.fields {
            let value = self.compute_value(field, row)?;
            vector.push(value);
        }

        Embedding::with_dimensions(vector, self.dimensions, HashMap::new())
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn name(&self) -> &str {
        "MetricEmbedder"
    }
}

#[cfg(test)]
mod tests {
    use super::super::config::DirectFieldConfig;
    use super::*;
    use chrono::TimeZone;
    use std::collections::BTreeMap;

    fn test_fields_config() -> EmbeddingFieldsConfig {
        EmbeddingFieldsConfig {
            temporal: vec![
                "hour_sin".to_string(),
                "hour_cos".to_string(),
                "is_weekend".to_string(),
            ],
            direct: vec![
                DirectFieldConfig {
                    field: "pm25_mean".to_string(),
                    null_strategy: NullStrategyConfig::Zero,
                },
                DirectFieldConfig {
                    field: "co2_mean".to_string(),
                    null_strategy: NullStrategyConfig::LastKnown,
                },
                DirectFieldConfig {
                    field: "temperature_c_mean".to_string(),
                    null_strategy: NullStrategyConfig::Mean,
                },
            ],
            derived: vec![],
        }
    }

    fn make_row(hour: u32, day: u32, fields: Vec<(&str, Option<f64>)>) -> GoldRow {
        let bucket = chrono::Utc
            .with_ymd_and_hms(2026, 2, day, hour, 0, 0)
            .unwrap();
        let mut map = BTreeMap::new();
        for (k, v) in fields {
            map.insert(k.to_string(), v);
        }
        GoldRow {
            bucket,
            domain_id: "test".to_string(),
            fields: map,
        }
    }

    fn warm_up_embedder(embedder: &mut MetricEmbedder) {
        // Feed 200 observations to pass warmup threshold
        for i in 0..200 {
            let hour = (i % 24) as u32;
            let day = 1 + ((i / 24) % 28) as u32;
            let day = day.min(28);
            let row = make_row(
                hour,
                day,
                vec![
                    ("pm25_mean", Some(25.0 + (i as f64 * 0.1))),
                    ("co2_mean", Some(400.0 + (i as f64 * 0.5))),
                    ("temperature_c_mean", Some(20.0 + (i as f64 * 0.05))),
                ],
            );
            embedder.observe(&row);
        }
    }

    #[test]
    fn test_insufficient_data_before_warmup() {
        let config = test_fields_config();
        let embedder = MetricEmbedder::from_fields_config(&config).with_warmup(168);

        let row = make_row(
            12,
            14,
            vec![
                ("pm25_mean", Some(25.0)),
                ("co2_mean", Some(400.0)),
                ("temperature_c_mean", Some(22.0)),
            ],
        );

        let result = embedder.embed(&row);
        assert!(result.is_err());
        match result.unwrap_err() {
            EmbeddingError::InsufficientData { reason } => {
                assert!(reason.contains("warmup"));
            }
            e => panic!("Expected InsufficientData, got {:?}", e),
        }
    }

    #[test]
    fn test_temporal_hour_zero() {
        // hour=0: sin(0) = 0.0, cos(0) = 1.0
        let config = test_fields_config();
        let mut embedder = MetricEmbedder::from_fields_config(&config).with_warmup(1);

        let row = make_row(
            0,
            10,
            vec![
                // Feb 10, 2026 is a Tuesday
                ("pm25_mean", Some(25.0)),
                ("co2_mean", Some(400.0)),
                ("temperature_c_mean", Some(22.0)),
            ],
        );
        embedder.observe(&row);

        let embedding = embedder.embed(&row).unwrap();
        // First dimension is hour_sin
        assert!(
            embedding.vector[0].abs() < 0.001,
            "hour_sin at hour=0 should be ~0.0, got {}",
            embedding.vector[0]
        );
        // Second dimension is hour_cos
        assert!(
            (embedding.vector[1] - 1.0).abs() < 0.001,
            "hour_cos at hour=0 should be ~1.0, got {}",
            embedding.vector[1]
        );
    }

    #[test]
    fn test_temporal_hour_six() {
        // hour=6: sin(2*pi*6/24) = sin(pi/2) = 1.0, cos(pi/2) = 0.0
        let config = test_fields_config();
        let mut embedder = MetricEmbedder::from_fields_config(&config).with_warmup(1);

        let row = make_row(
            6,
            10,
            vec![
                ("pm25_mean", Some(25.0)),
                ("co2_mean", Some(400.0)),
                ("temperature_c_mean", Some(22.0)),
            ],
        );
        embedder.observe(&row);

        let embedding = embedder.embed(&row).unwrap();
        assert!(
            (embedding.vector[0] - 1.0).abs() < 0.001,
            "hour_sin at hour=6 should be ~1.0, got {}",
            embedding.vector[0]
        );
        assert!(
            embedding.vector[1].abs() < 0.001,
            "hour_cos at hour=6 should be ~0.0, got {}",
            embedding.vector[1]
        );
    }

    #[test]
    fn test_weekend_detection() {
        let config = test_fields_config();
        let mut embedder = MetricEmbedder::from_fields_config(&config).with_warmup(1);

        // Feb 14, 2026 is a Saturday
        let saturday_row = make_row(
            12,
            14,
            vec![
                ("pm25_mean", Some(25.0)),
                ("co2_mean", Some(400.0)),
                ("temperature_c_mean", Some(22.0)),
            ],
        );
        embedder.observe(&saturday_row);
        let emb = embedder.embed(&saturday_row).unwrap();
        assert!(
            (emb.vector[2] - 1.0).abs() < f32::EPSILON,
            "Saturday should be is_weekend=1.0, got {}",
            emb.vector[2]
        );

        // Feb 9, 2026 is a Monday
        let monday_row = make_row(
            12,
            9,
            vec![
                ("pm25_mean", Some(25.0)),
                ("co2_mean", Some(400.0)),
                ("temperature_c_mean", Some(22.0)),
            ],
        );
        let emb = embedder.embed(&monday_row).unwrap();
        assert!(
            emb.vector[2].abs() < f32::EPSILON,
            "Monday should be is_weekend=0.0, got {}",
            emb.vector[2]
        );
    }

    #[test]
    fn test_z_score_known_values() {
        let config = EmbeddingFieldsConfig {
            temporal: vec![],
            direct: vec![DirectFieldConfig {
                field: "value".to_string(),
                null_strategy: NullStrategyConfig::Zero,
            }],
            derived: vec![],
        };
        let mut embedder = MetricEmbedder::from_fields_config(&config).with_warmup(1);

        // Feed constant values to establish stats
        for _ in 0..100 {
            let row = make_row(12, 10, vec![("value", Some(10.0))]);
            embedder.observe(&row);
        }

        // Embed a row with the mean value -- z-score should be ~0.0
        let row = make_row(12, 10, vec![("value", Some(10.0))]);
        let emb = embedder.embed(&row).unwrap();
        assert!(
            emb.vector[0].abs() < 0.1,
            "Z-score of mean value should be ~0.0, got {}",
            emb.vector[0]
        );
    }

    #[test]
    fn test_null_strategy_zero() {
        let config = EmbeddingFieldsConfig {
            temporal: vec![],
            direct: vec![DirectFieldConfig {
                field: "value".to_string(),
                null_strategy: NullStrategyConfig::Zero,
            }],
            derived: vec![],
        };
        let mut embedder = MetricEmbedder::from_fields_config(&config).with_warmup(1);
        embedder.observe(&make_row(12, 10, vec![("value", Some(10.0))]));

        let null_row = make_row(12, 10, vec![("value", None)]);
        let emb = embedder.embed(&null_row).unwrap();
        assert!(
            emb.vector[0].abs() < f32::EPSILON,
            "NullStrategy::Zero should produce 0.0, got {}",
            emb.vector[0]
        );
    }

    #[test]
    fn test_null_strategy_mean() {
        let config = EmbeddingFieldsConfig {
            temporal: vec![],
            direct: vec![DirectFieldConfig {
                field: "value".to_string(),
                null_strategy: NullStrategyConfig::Mean,
            }],
            derived: vec![],
        };
        let mut embedder = MetricEmbedder::from_fields_config(&config).with_warmup(1);
        embedder.observe(&make_row(12, 10, vec![("value", Some(10.0))]));

        let null_row = make_row(12, 10, vec![("value", None)]);
        let emb = embedder.embed(&null_row).unwrap();
        // Mean in z-score space is 0.0
        assert!(
            emb.vector[0].abs() < f32::EPSILON,
            "NullStrategy::Mean should produce 0.0 in z-score space, got {}",
            emb.vector[0]
        );
    }

    #[test]
    fn test_null_strategy_last_known() {
        let config = EmbeddingFieldsConfig {
            temporal: vec![],
            direct: vec![DirectFieldConfig {
                field: "value".to_string(),
                null_strategy: NullStrategyConfig::LastKnown,
            }],
            derived: vec![],
        };
        let mut embedder = MetricEmbedder::from_fields_config(&config).with_warmup(1);

        // Feed a known value
        embedder.observe(&make_row(12, 10, vec![("value", Some(50.0))]));

        // Now a null -- should use z_score(50.0)
        let null_row = make_row(13, 10, vec![("value", None)]);
        let emb = embedder.embed(&null_row).unwrap();

        // After one observation, mean=50.0, std=0.0, so z_score returns 0.0
        // (division-by-zero protection)
        assert!(
            emb.vector[0].abs() < f32::EPSILON,
            "LastKnown with one observation should produce z_score(last)=0.0, got {}",
            emb.vector[0]
        );
    }

    #[test]
    fn test_dimension_count() {
        let config = test_fields_config();
        let embedder = MetricEmbedder::from_fields_config(&config);
        // 3 temporal + 3 direct + 0 derived = 6
        assert_eq!(embedder.dimensions(), 6);
    }

    #[test]
    fn test_from_config() {
        use super::super::config::{EmbeddingConfig, EmbeddingType};

        let config = EmbeddingConfig {
            embedding_type: EmbeddingType::Metric,
            fields: test_fields_config(),
        };
        let embedder = MetricEmbedder::from_config(&config);
        assert_eq!(embedder.dimensions(), 6);
        assert_eq!(embedder.name(), "MetricEmbedder");
    }

    #[test]
    fn test_observation_count() {
        let config = test_fields_config();
        let mut embedder = MetricEmbedder::from_fields_config(&config);
        assert_eq!(embedder.observation_count(), 0);

        embedder.observe(&make_row(
            12,
            10,
            vec![
                ("pm25_mean", Some(25.0)),
                ("co2_mean", Some(400.0)),
                ("temperature_c_mean", Some(22.0)),
            ],
        ));
        assert_eq!(embedder.observation_count(), 1);
    }

    #[test]
    fn test_embedding_vector_length_matches_dimensions() {
        let config = test_fields_config();
        let mut embedder = MetricEmbedder::from_fields_config(&config).with_warmup(1);

        let row = make_row(
            12,
            10,
            vec![
                ("pm25_mean", Some(25.0)),
                ("co2_mean", Some(400.0)),
                ("temperature_c_mean", Some(22.0)),
            ],
        );
        embedder.observe(&row);

        let emb = embedder.embed(&row).unwrap();
        assert_eq!(
            emb.vector.len(),
            embedder.dimensions(),
            "Embedding vector length should match declared dimensions"
        );
    }
}
