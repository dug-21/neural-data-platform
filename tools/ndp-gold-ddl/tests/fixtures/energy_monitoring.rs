//! Energy Monitoring Fictional Domain Fixtures (OPS-002)
//!
//! A completely fictional domain with ZERO overlap with air-quality.
//! Used to prove the EventsGenerator is truly config-driven:
//! if any air-quality literal leaks into SQL generated for this domain,
//! the test fails.

use ndp_gold_ddl::config::types::{
    AggregatesConfig, FeaturesConfig, FieldConfig, FieldMetricsConfig, GoldEtlConfig,
    SilverEtlConfig, TransitionsConfig,
};
use ndp_gold_ddl::{
    AlignmentConfig, DomainConfig, JoinStrategy, NullHandling,
    ObjectiveConfig, Priority, StreamConfig, StreamRef, StreamRole, TargetConfig,
};
use std::collections::HashMap;

use super::phase_c::MockConfigLoader;

// ============================================================================
// Forbidden Literals
// ============================================================================

/// Air-quality-specific strings that MUST NOT appear in generated SQL
/// when using a non-air-quality domain config.
pub const FORBIDDEN_AIR_QUALITY_LITERALS: &[&str] = &[
    // Stream identifiers
    "home-assistant-state",
    "home_assistant_state",
    "air-quality",
    "air_quality",
    // Table references
    "silver.state_events",
    "silver.home_assistant_state",
    "gold.air_quality_hourly",
    // Column names from air-quality domain
    "co2_mean",
    "pm25_mean",
    "co2_value",
    "pm25_value",
    "co2_prev",
    "pm25_prev",
    "indoor_co2",
    "indoor_pm25",
    "indoor_temperature_c",
    "outdoor_temperature_c",
    "outdoor_aqi_pm25",
    "state_state_last",
    "indoor_co2_mean",
    "indoor_pm25_mean",
    "indoor_temperature_c_mean",
    "outdoor_temperature_c_mean",
    "outdoor_aqi_pm25_mean",
    // Threshold values
    "800.0",
    "800",
    "12.0",
    // Objective identifiers
    "healthy_co2",
    "healthy_pm25",
    // Metric names (quoted as SQL literals)
    "'co2'",
    "'pm25'",
    // Unit literals (quoted as SQL literals)
    "'ppm'",
    "'ug/m3'",
    // CTE names
    "co2_crossings",
    "pm25_crossings",
];

// ============================================================================
// Energy Monitoring Domain Config
// ============================================================================

/// Create a fictional "energy-monitoring" domain config.
/// This domain has ZERO overlap with air-quality.
pub fn create_energy_monitoring_domain() -> DomainConfig {
    DomainConfig {
        id: "energy-monitoring".to_string(),
        description: "Fictional energy monitoring domain for testing".to_string(),
        streams: vec![
            StreamRef {
                stream_id: "smart-meter".to_string(),
                alias: "meter".to_string(),
                role: StreamRole::Primary,
                null_handling: None,
            },
            StreamRef {
                stream_id: "grid-relay-state".to_string(),
                alias: "relay".to_string(),
                role: StreamRole::Actuator,
                null_handling: Some(NullHandling::CarryForward),
            },
        ],
        alignment: AlignmentConfig {
            view_name: "energy_monitoring_aligned".to_string(),
            granularity: "1 hour".to_string(),
            join_strategy: JoinStrategy::FullOuter,
            null_handling: NullHandling::Preserve,
        },
        objectives: vec![
            ObjectiveConfig {
                id: "safe_voltage".to_string(),
                description: "Keep voltage below safe threshold".to_string(),
                target: TargetConfig {
                    stream: "smart-meter".to_string(),
                    metric: "voltage".to_string(),
                    condition: "<".to_string(),
                    threshold: 240.0,
                    unit: Some("volts".to_string()),
                },
                priority: Priority::High,
            },
            ObjectiveConfig {
                id: "efficient_current".to_string(),
                description: "Keep current draw below efficiency threshold".to_string(),
                target: TargetConfig {
                    stream: "smart-meter".to_string(),
                    metric: "current".to_string(),
                    condition: "<".to_string(),
                    threshold: 30.0,
                    unit: Some("amps".to_string()),
                },
                priority: Priority::Medium,
            },
        ],
        events: Some(ndp_gold_ddl::EventsConfig {
            enabled: true,
            chunk_interval: "7 days".to_string(),
            retention: Some("1 year".to_string()),
            detection_schedule: "15 minutes".to_string(),
        }),
    }
}

/// Create a domain with a single objective (safe_voltage only).
pub fn create_single_objective_domain() -> DomainConfig {
    let mut domain = create_energy_monitoring_domain();
    domain.objectives = vec![ObjectiveConfig {
        id: "safe_voltage".to_string(),
        description: "Keep voltage below safe threshold".to_string(),
        target: TargetConfig {
            stream: "smart-meter".to_string(),
            metric: "voltage".to_string(),
            condition: "<".to_string(),
            threshold: 240.0,
            unit: Some("volts".to_string()),
        },
        priority: Priority::High,
    }];
    domain
}

/// Create a domain with three objectives (voltage, current, frequency).
pub fn create_three_objective_domain() -> DomainConfig {
    let mut domain = create_energy_monitoring_domain();
    domain.objectives.push(ObjectiveConfig {
        id: "stable_frequency".to_string(),
        description: "Keep frequency within tolerance".to_string(),
        target: TargetConfig {
            stream: "smart-meter".to_string(),
            metric: "frequency".to_string(),
            condition: "<".to_string(),
            threshold: 51.0,
            unit: Some("Hz".to_string()),
        },
        priority: Priority::High,
    });
    domain
}

/// Create a domain with zero objectives (only state transitions).
pub fn create_zero_objective_domain() -> DomainConfig {
    let mut domain = create_energy_monitoring_domain();
    domain.objectives = vec![];
    domain
}

/// Create a domain with an objective that has no unit.
pub fn create_no_unit_objective_domain() -> DomainConfig {
    let mut domain = create_energy_monitoring_domain();
    domain.objectives = vec![ObjectiveConfig {
        id: "safe_voltage".to_string(),
        description: "Keep voltage below safe threshold".to_string(),
        target: TargetConfig {
            stream: "smart-meter".to_string(),
            metric: "voltage".to_string(),
            condition: "<".to_string(),
            threshold: 240.0,
            unit: None,
        },
        priority: Priority::High,
    }];
    domain
}

// ============================================================================
// Stream Configuration Fixtures
// ============================================================================

/// Create a mock StreamConfig for "smart-meter" stream.
pub fn create_smart_meter_stream_config() -> StreamConfig {
    let mut fields_map = HashMap::new();
    fields_map.insert(
        "voltage".to_string(),
        FieldMetricsConfig {
            metrics: vec!["mean".to_string(), "max".to_string()],
        },
    );
    fields_map.insert(
        "current".to_string(),
        FieldMetricsConfig {
            metrics: vec!["mean".to_string()],
        },
    );
    fields_map.insert(
        "power_w".to_string(),
        FieldMetricsConfig {
            metrics: vec!["mean".to_string(), "sum".to_string()],
        },
    );

    StreamConfig {
        stream_id: "smart-meter".to_string(),
        stream_type: None,
        fields: vec![
            FieldConfig {
                name: "voltage".to_string(),
                field_type: "float".to_string(),
            },
            FieldConfig {
                name: "current".to_string(),
                field_type: "float".to_string(),
            },
            FieldConfig {
                name: "power_w".to_string(),
                field_type: "float".to_string(),
            },
        ],
        silver_etl: Some(SilverEtlConfig {
            target_table: "silver.smart_meter_observations".to_string(),
            timestamp: None,
        }),
        gold_etl: Some(GoldEtlConfig {
            enabled: true,
            aggregates: Some(AggregatesConfig {
                granularities: vec!["1 hour".to_string()],
                fields: fields_map,
            }),
            features: None,
            refresh_policy: None,
        }),
    }
}

/// Create a mock StreamConfig for "grid-relay-state" stream.
/// Uses "relay_state" as the state field and "device_id" as entity field
/// via the transitions config.
pub fn create_relay_state_stream_config() -> StreamConfig {
    StreamConfig {
        stream_id: "grid-relay-state".to_string(),
        stream_type: None,
        fields: vec![
            FieldConfig {
                name: "relay_state".to_string(),
                field_type: "string".to_string(),
            },
            FieldConfig {
                name: "device_id".to_string(),
                field_type: "string".to_string(),
            },
        ],
        silver_etl: Some(SilverEtlConfig {
            target_table: "silver.grid_relay_state".to_string(),
            timestamp: None,
        }),
        gold_etl: Some(GoldEtlConfig {
            enabled: true,
            aggregates: None,
            features: Some(FeaturesConfig {
                lag: None,
                rolling: None,
                trend: None,
                transitions: Some(TransitionsConfig {
                    enabled: true,
                    field: "relay_state".to_string(),
                    states: vec!["open".to_string(), "closed".to_string()],
                }),
            }),
            refresh_policy: None,
        }),
    }
}

/// Create a smart-meter stream config that also has a "frequency" field
/// (for the 3-objective test).
pub fn create_smart_meter_with_frequency_config() -> StreamConfig {
    let mut config = create_smart_meter_stream_config();
    if let Some(ref mut gold_etl) = config.gold_etl {
        if let Some(ref mut aggregates) = gold_etl.aggregates {
            aggregates.fields.insert(
                "frequency".to_string(),
                FieldMetricsConfig {
                    metrics: vec!["mean".to_string()],
                },
            );
        }
    }
    config.fields.push(FieldConfig {
        name: "frequency".to_string(),
        field_type: "float".to_string(),
    });
    config
}

// ============================================================================
// Mock ConfigLoader Builder for Energy Monitoring
// ============================================================================

/// Build a MockConfigLoader for the standard energy-monitoring domain.
pub fn energy_monitoring_loader() -> MockConfigLoader {
    MockConfigLoader::new()
        .with_stream("smart-meter", create_smart_meter_stream_config())
        .with_stream("grid-relay-state", create_relay_state_stream_config())
}

/// Build a MockConfigLoader that includes the frequency field for 3-objective tests.
pub fn energy_monitoring_loader_with_frequency() -> MockConfigLoader {
    MockConfigLoader::new()
        .with_stream("smart-meter", create_smart_meter_with_frequency_config())
        .with_stream("grid-relay-state", create_relay_state_stream_config())
}
