//! Domain sync types for data dictionary population.
//!
//! These structs represent the pre-parsed, database-ready domain data
//! that gets synced to the `data_dictionary` schema. The caller is
//! responsible for loading and converting configs; this module operates
//! on pre-parsed structs.

/// A domain ready for sync. Maps to data_dictionary.domains + children.
#[derive(Debug, Clone)]
pub struct DomainSyncEntry {
    pub domain_id: String,
    pub description: Option<String>,
    pub stream_count: i32,
    pub config_path: String,
    pub streams: Vec<StreamMappingEntry>,
    pub objectives: Vec<ObjectiveSyncEntry>,
    pub constraints: Vec<ConstraintSyncEntry>,
}

/// Stream-to-domain mapping. Maps to data_dictionary.domain_streams.
#[derive(Debug, Clone)]
pub struct StreamMappingEntry {
    pub stream_id: String,
    pub alias: String,
    pub role: String,
}

/// An objective to sync. Maps to data_dictionary.objectives.
#[derive(Debug, Clone)]
pub struct ObjectiveSyncEntry {
    pub objective_id: String,
    pub description: Option<String>,
    pub target_stream: String,
    pub target_metric: String,
    pub condition: String,
    pub threshold: f64,
    pub threshold_upper: Option<f64>,
    pub unit: Option<String>,
    pub priority: String,
}

/// A constraint to sync. Maps to data_dictionary.constraints.
#[derive(Debug, Clone)]
pub struct ConstraintSyncEntry {
    pub constraint_id: String,
    pub description: Option<String>,
    pub constraint_stream: String,
    pub constraint_metric: String,
    pub condition: String,
    pub threshold: f64,
    pub unit: Option<String>,
}
