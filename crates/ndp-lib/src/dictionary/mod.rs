//! Dictionary sync operations.
//!
//! Implements `ndp dictionary sync` -- syncing stream configs to the
//! `data_dictionary` schema in TimescaleDB.
//!
//! This module replaces the ~460-line `sync_to_data_dictionary()` Bash function
//! in `deploy/pi/deploy.sh` with tested, parameterized Rust.
//!
//! # Design
//!
//! - Takes `&[StreamDictionaryEntry]` (parsed structs, not file paths).
//! - Uses `&(impl DbClient)` for testability (London TDD with mock).
//! - Bronze tables use DELETE+INSERT (full refresh).
//! - Silver tables use UPSERT (ON CONFLICT DO UPDATE).
//! - All SQL uses parameterized queries ($1, $2, ...) -- never string concat.

pub mod sql;
pub mod types;

use std::collections::BTreeMap;
use std::time::Instant;

use tracing::{debug, error, info};

use crate::db::DbClient;
use crate::error::Result;
use crate::types::{SyncError, SyncOptions, SyncReport};
use sql::map_field_type_to_pg;
use types::*;

/// Collected counts during sync for the final report and sync_status UPDATE.
#[derive(Debug, Default)]
struct SyncCounts {
    streams: i32,
    fields: i32,
    sources: i32,
    schemas: i32,
    attributes: i32,
    silver_tables: i32,
    silver_columns: i32,
    silver_lineage: i32,
    silver_dq_rules: i32,
}

/// Metadata collected per Silver target table across multiple streams.
#[derive(Debug)]
struct SilverTableInfo {
    description: Option<String>,
    grain: Option<String>,
    timestamp_column: String,
    source_streams: Vec<String>,
}

/// Sync stream configurations to the `data_dictionary` tables.
///
/// Caller decides where configs come from (files, etcd, test fixtures).
/// This function takes parsed structs, not file paths.
///
/// # Arguments
/// * `streams` - parsed stream configurations
/// * `db` - database client (real or mock)
/// * `options` - sync options (dry_run, etc.)
///
/// # Returns
/// A `SyncReport` summarizing what was created, updated, or deleted.
pub async fn sync_dictionary(
    streams: &[StreamDictionaryEntry],
    db: &impl DbClient,
    options: &SyncOptions,
) -> Result<SyncReport> {
    let start = Instant::now();
    let mut counts = SyncCounts::default();
    let mut errors: Vec<SyncError> = Vec::new();

    if options.dry_run {
        return Ok(build_dry_run_report(streams));
    }

    // Step 1: BEGIN transaction
    db.batch_execute("BEGIN").await?;

    // Step 2: INSERT sync_status (running)
    if let Err(e) = db
        .execute(sql::INSERT_SYNC_STATUS, &[&"full", &"running"])
        .await
    {
        error!(error = %e, "Failed to insert sync_status");
        // Non-fatal: continue without sync_status tracking
        errors.push(SyncError {
            item: "sync_status".to_string(),
            message: format!("Failed to insert sync_status: {}", e),
        });
    }

    // Step 3-7: DELETE Bronze tables in FK order
    // Order matters: attributes -> schemas -> sources -> fields -> streams
    delete_bronze_tables(db).await?;

    // Step 8: INSERT Bronze data for each stream
    for entry in streams {
        match insert_bronze_stream(db, entry, &mut counts).await {
            Ok(()) => {}
            Err(e) => {
                error!(stream_id = %entry.stream_id, error = %e, "Failed to sync stream");
                errors.push(SyncError {
                    item: entry.stream_id.clone(),
                    message: format!("Bronze insert failed: {}", e),
                });
            }
        }
    }

    // Step 9: Collect Silver-enabled streams, group by target_table
    let silver_tables = collect_silver_tables(streams);

    // Step 10: UPSERT silver_tables
    for (target_table, info) in &silver_tables {
        match upsert_silver_table(db, target_table, info).await {
            Ok(()) => counts.silver_tables += 1,
            Err(e) => {
                error!(table = %target_table, error = %e, "Failed to upsert silver_table");
                errors.push(SyncError {
                    item: target_table.clone(),
                    message: format!("Silver table upsert failed: {}", e),
                });
            }
        }
    }

    // Step 11: UPSERT silver_columns, silver_lineage, silver_dq_rules per stream
    for entry in streams {
        let etl = match &entry.silver_etl {
            Some(etl) if etl.enabled => etl,
            _ => continue,
        };

        if let Err(e) = upsert_silver_stream_details(db, &entry.stream_id, etl, &mut counts).await {
            error!(stream_id = %entry.stream_id, error = %e, "Failed to sync silver details");
            errors.push(SyncError {
                item: entry.stream_id.clone(),
                message: format!("Silver details failed: {}", e),
            });
        }
    }

    // Step 12: UPDATE sync_status (success, counts)
    if let Err(e) = db
        .execute(
            sql::UPDATE_SYNC_STATUS_SUCCESS,
            &[
                &counts.streams,
                &counts.schemas,
                &counts.attributes,
                &counts.silver_tables,
                &counts.silver_columns,
            ],
        )
        .await
    {
        debug!(error = %e, "Failed to update sync_status (non-fatal)");
    }

    // Step 13: COMMIT
    db.batch_execute("COMMIT").await?;

    let duration = start.elapsed();

    info!(
        streams = counts.streams,
        fields = counts.fields,
        sources = counts.sources,
        schemas = counts.schemas,
        attributes = counts.attributes,
        silver_tables = counts.silver_tables,
        silver_columns = counts.silver_columns,
        silver_lineage = counts.silver_lineage,
        silver_dq_rules = counts.silver_dq_rules,
        duration_ms = duration.as_millis() as u64,
        "Dictionary sync complete"
    );

    Ok(SyncReport {
        entity: "dictionary".to_string(),
        items_processed: streams.len(),
        items_created: (counts.streams
            + counts.fields
            + counts.sources
            + counts.schemas
            + counts.attributes) as usize,
        items_updated: (counts.silver_tables
            + counts.silver_columns
            + counts.silver_lineage
            + counts.silver_dq_rules) as usize,
        items_deleted: 0, // We always delete all Bronze then re-insert
        errors,
        duration,
    })
}

/// Delete all Bronze dictionary tables in FK-safe order.
async fn delete_bronze_tables(db: &impl DbClient) -> Result<()> {
    db.execute(sql::DELETE_ENTITY_SCHEMA_ATTRIBUTES, &[])
        .await?;
    db.execute(sql::DELETE_ENTITY_SCHEMAS, &[]).await?;
    db.execute(sql::DELETE_SOURCES, &[]).await?;
    db.execute(sql::DELETE_FIELDS, &[]).await?;
    db.execute(sql::DELETE_STREAMS, &[]).await?;
    Ok(())
}

/// INSERT a single stream and its children (fields, sources, entity_schemas).
async fn insert_bronze_stream(
    db: &impl DbClient,
    entry: &StreamDictionaryEntry,
    counts: &mut SyncCounts,
) -> Result<()> {
    // INSERT stream
    db.execute(
        sql::INSERT_STREAM,
        &[
            &entry.stream_id,
            &entry.description,
            &entry.version,
            &entry.enabled,
            &entry.retention_days,
        ],
    )
    .await?;
    counts.streams += 1;

    // INSERT fields
    for (idx, field) in entry.fields.iter().enumerate() {
        let sort_order = idx as i32;
        db.execute(
            sql::INSERT_FIELD,
            &[
                &entry.stream_id,
                &field.name,
                &field.field_type,
                &field.nullable,
                &field.unit,
                &field.description,
                &field.validation_min,
                &field.validation_max,
                &sort_order,
            ],
        )
        .await?;
        counts.fields += 1;
    }

    // INSERT sources
    for source in &entry.sources {
        db.execute(
            sql::INSERT_SOURCE,
            &[
                &entry.stream_id,
                &source.source_id,
                &source.source_type,
                &source.enabled,
                &source.config,
                &source.parser_type,
            ],
        )
        .await?;
        counts.sources += 1;
    }

    // INSERT entity_schemas + attributes
    for schema in &entry.entity_schemas {
        db.execute(
            sql::INSERT_ENTITY_SCHEMA,
            &[
                &entry.stream_id,
                &schema.schema_name,
                &schema.description,
                &schema.device_class,
            ],
        )
        .await?;
        counts.schemas += 1;

        for (idx, attr) in schema.attributes.iter().enumerate() {
            let sort_order = idx as i32;
            db.execute(
                sql::INSERT_ENTITY_SCHEMA_ATTRIBUTE,
                &[
                    &attr.name,
                    &attr.attribute_type,
                    &attr.unit,
                    &attr.description,
                    &attr.nullable,
                    &sort_order,
                    &entry.stream_id,
                    &schema.schema_name,
                ],
            )
            .await?;
            counts.attributes += 1;
        }
    }

    Ok(())
}

/// Collect Silver-enabled streams grouped by target_table.
///
/// This is the "two-pass Silver collection" from the Bash implementation.
/// We use a BTreeMap for deterministic ordering in tests.
fn collect_silver_tables(streams: &[StreamDictionaryEntry]) -> BTreeMap<String, SilverTableInfo> {
    let mut tables: BTreeMap<String, SilverTableInfo> = BTreeMap::new();

    for entry in streams {
        let etl = match &entry.silver_etl {
            Some(etl) if etl.enabled => etl,
            _ => continue,
        };

        tables
            .entry(etl.target_table.clone())
            .and_modify(|info| {
                info.source_streams.push(entry.stream_id.clone());
            })
            .or_insert_with(|| SilverTableInfo {
                description: etl.description.clone(),
                grain: etl.grain.clone(),
                timestamp_column: etl.timestamp_column.clone(),
                source_streams: vec![entry.stream_id.clone()],
            });
    }

    tables
}

/// UPSERT a single silver_tables row.
async fn upsert_silver_table(
    db: &impl DbClient,
    target_table: &str,
    info: &SilverTableInfo,
) -> Result<()> {
    let schema_name = sql::extract_schema_name(target_table);

    db.execute(
        sql::UPSERT_SILVER_TABLE,
        &[
            &target_table,
            &schema_name,
            &info.description,
            &info.grain,
            &info.source_streams,
            &info.timestamp_column,
        ],
    )
    .await?;

    Ok(())
}

/// UPSERT silver_columns, silver_lineage, and silver_dq_rules for one stream.
async fn upsert_silver_stream_details(
    db: &impl DbClient,
    stream_id: &str,
    etl: &SilverEtlEntry,
    counts: &mut SyncCounts,
) -> Result<()> {
    let target_table = &etl.target_table;

    // Process field_mappings
    for (idx, mapping) in etl.field_mappings.iter().enumerate() {
        let pg_type = map_field_type_to_pg(&mapping.data_type);
        let sort_order = idx as i32;

        // UPSERT silver_columns
        db.execute(
            sql::UPSERT_SILVER_COLUMN,
            &[
                &target_table.as_str(),
                &mapping.target_column,
                &pg_type,
                &mapping.unit,
                &mapping.description,
                &mapping.nullable,
                &sort_order,
            ],
        )
        .await?;
        counts.silver_columns += 1;

        // Determine transformation type
        let transform = mapping.transform_type.as_deref().unwrap_or("direct");

        // UPSERT silver_lineage
        db.execute(
            sql::UPSERT_SILVER_LINEAGE,
            &[
                &target_table.as_str(),
                &mapping.target_column,
                &stream_id,
                &mapping.source_path,
                &transform,
            ],
        )
        .await?;
        counts.silver_lineage += 1;

        // UPSERT column-level DQ rules
        for dq in &mapping.dq_rules {
            db.execute(
                sql::UPSERT_SILVER_DQ_RULE_COLUMN,
                &[
                    &target_table.as_str(),
                    &mapping.target_column,
                    &dq.rule_name,
                    &dq.params,
                    &dq.action,
                ],
            )
            .await?;
            counts.silver_dq_rules += 1;
        }
    }

    // Process table-level DQ rules
    for dq in &etl.dq_rules {
        db.execute(
            sql::UPSERT_SILVER_DQ_RULE_TABLE,
            &[
                &target_table.as_str(),
                &dq.rule_name,
                &dq.params,
                &dq.action,
            ],
        )
        .await?;
        counts.silver_dq_rules += 1;
    }

    Ok(())
}

/// Build a SyncReport for dry_run mode without executing SQL.
fn build_dry_run_report(streams: &[StreamDictionaryEntry]) -> SyncReport {
    let mut fields = 0usize;
    let mut sources = 0usize;
    let mut schemas = 0usize;
    let mut attributes = 0usize;
    let mut silver_columns = 0usize;

    let silver_table_set = collect_silver_tables(streams);
    let silver_tables = silver_table_set.len();

    for entry in streams {
        fields += entry.fields.len();
        sources += entry.sources.len();
        for es in &entry.entity_schemas {
            schemas += 1;
            attributes += es.attributes.len();
        }
        if let Some(etl) = &entry.silver_etl {
            if etl.enabled {
                silver_columns += etl.field_mappings.len();
            }
        }
    }

    SyncReport {
        entity: "dictionary".to_string(),
        items_processed: streams.len(),
        items_created: streams.len() + fields + sources + schemas + attributes,
        items_updated: silver_tables + silver_columns,
        items_deleted: 0,
        errors: Vec::new(),
        duration: std::time::Duration::ZERO,
    }
}

// ==========================================================================
// Tests (London TDD)
// ==========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::DbClient;
    use crate::error::Result;
    use async_trait::async_trait;
    use std::sync::Mutex;
    use tokio_postgres::types::ToSql;
    use tokio_postgres::Row;

    /// A recorded SQL call: (query_string, debug-formatted params).
    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    struct SqlCall {
        query: String,
        params: Vec<String>,
    }

    /// Mock database client that records all execute/query/batch_execute calls.
    struct MockDbClient {
        calls: Mutex<Vec<SqlCall>>,
    }

    impl MockDbClient {
        fn new() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<SqlCall> {
            self.calls.lock().unwrap().clone()
        }

        /// Return all queries that start with the given prefix.
        fn calls_starting_with(&self, prefix: &str) -> Vec<SqlCall> {
            self.calls()
                .into_iter()
                .filter(|c| c.query.starts_with(prefix))
                .collect()
        }

        /// Return queries in the order they were executed.
        fn query_strings(&self) -> Vec<String> {
            self.calls().iter().map(|c| c.query.clone()).collect()
        }
    }

    #[async_trait]
    impl DbClient for MockDbClient {
        async fn query(&self, query: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>> {
            let param_strs: Vec<String> = params.iter().map(|p| format!("{:?}", p)).collect();
            self.calls.lock().unwrap().push(SqlCall {
                query: query.to_string(),
                params: param_strs,
            });
            Ok(vec![])
        }

        async fn execute(&self, query: &str, params: &[&(dyn ToSql + Sync)]) -> Result<u64> {
            let param_strs: Vec<String> = params.iter().map(|p| format!("{:?}", p)).collect();
            self.calls.lock().unwrap().push(SqlCall {
                query: query.to_string(),
                params: param_strs,
            });
            Ok(1)
        }

        async fn batch_execute(&self, sql_text: &str) -> Result<()> {
            self.calls.lock().unwrap().push(SqlCall {
                query: sql_text.to_string(),
                params: vec![],
            });
            Ok(())
        }
    }

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    fn make_minimal_stream(id: &str) -> StreamDictionaryEntry {
        StreamDictionaryEntry {
            stream_id: id.to_string(),
            description: Some("Test stream".to_string()),
            version: "1.0.0".to_string(),
            enabled: true,
            retention_days: 90,
            fields: vec![],
            sources: vec![],
            entity_schemas: vec![],
            silver_etl: None,
        }
    }

    fn make_field(name: &str, field_type: &str) -> FieldEntry {
        FieldEntry {
            name: name.to_string(),
            field_type: field_type.to_string(),
            nullable: true,
            unit: Some("test_unit".to_string()),
            description: Some("test desc".to_string()),
            validation_min: Some(0.0),
            validation_max: Some(100.0),
        }
    }

    fn make_source(source_id: &str, source_type: &str) -> SourceEntry {
        SourceEntry {
            source_id: source_id.to_string(),
            source_type: source_type.to_string(),
            enabled: true,
            config: serde_json::json!({"broker_url": "mosquitto"}),
            parser_type: Some("flat_json".to_string()),
        }
    }

    fn make_entity_schema(name: &str) -> EntitySchemaEntry {
        EntitySchemaEntry {
            schema_name: name.to_string(),
            description: Some("Test schema".to_string()),
            device_class: Some("sensor".to_string()),
            attributes: vec![EntitySchemaAttribute {
                name: "temperature".to_string(),
                attribute_type: "Float".to_string(),
                unit: Some("celsius".to_string()),
                description: Some("Temp reading".to_string()),
                nullable: true,
                range_min: Some(-40.0),
                range_max: Some(85.0),
            }],
        }
    }

    fn make_silver_etl(target_table: &str) -> SilverEtlEntry {
        SilverEtlEntry {
            enabled: true,
            target_table: target_table.to_string(),
            description: Some("Silver ETL".to_string()),
            grain: Some("One row per reading".to_string()),
            timestamp_column: "observation_time".to_string(),
            field_mappings: vec![SilverFieldMapping {
                source_path: "raw_payload.pm02".to_string(),
                target_column: "pm25".to_string(),
                data_type: "double_precision".to_string(),
                unit: Some("ug/m3".to_string()),
                description: Some("PM2.5".to_string()),
                nullable: false,
                transform_type: None,
                dq_rules: vec![SilverColumnDqRule {
                    rule_name: "range_check".to_string(),
                    params: serde_json::json!({"min": 0, "max": 1000}),
                    action: "flag".to_string(),
                }],
            }],
            dq_rules: vec![],
        }
    }

    fn opts() -> SyncOptions {
        SyncOptions {
            dry_run: false,
            ..Default::default()
        }
    }

    // -----------------------------------------------------------------------
    // 1. test_sync_empty_streams
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_empty_streams() {
        let db = MockDbClient::new();
        let report = sync_dictionary(&[], &db, &opts()).await.unwrap();

        assert_eq!(report.items_processed, 0);
        assert_eq!(report.items_created, 0);
        assert_eq!(report.items_updated, 0);

        // Should still have BEGIN, sync_status INSERT, DELETEs, sync_status UPDATE, COMMIT
        let queries = db.query_strings();
        assert!(queries.contains(&"BEGIN".to_string()));
        assert!(queries.contains(&"COMMIT".to_string()));
    }

    // -----------------------------------------------------------------------
    // 2. test_sync_single_stream_bronze
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_single_stream_bronze() {
        let db = MockDbClient::new();
        let stream = make_minimal_stream("air-quality");
        let report = sync_dictionary(&[stream], &db, &opts()).await.unwrap();

        assert_eq!(report.items_processed, 1);

        let inserts = db.calls_starting_with("INSERT INTO data_dictionary.streams");
        assert_eq!(inserts.len(), 1);
        assert!(inserts[0].query.contains("VALUES ($1, $2, $3, $4, $5)"));
    }

    // -----------------------------------------------------------------------
    // 3. test_sync_stream_fields
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_stream_fields() {
        let db = MockDbClient::new();
        let mut stream = make_minimal_stream("test");
        stream.fields = vec![
            make_field("pm02", "float"),
            make_field("temperature", "float"),
        ];

        sync_dictionary(&[stream], &db, &opts()).await.unwrap();

        let field_inserts = db.calls_starting_with("INSERT INTO data_dictionary.fields");
        assert_eq!(field_inserts.len(), 2);
    }

    // -----------------------------------------------------------------------
    // 4. test_sync_stream_sources
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_stream_sources() {
        let db = MockDbClient::new();
        let mut stream = make_minimal_stream("test");
        stream.sources = vec![make_source("aq_sensor_1", "mqtt")];

        sync_dictionary(&[stream], &db, &opts()).await.unwrap();

        let source_inserts = db.calls_starting_with("INSERT INTO data_dictionary.sources");
        assert_eq!(source_inserts.len(), 1);
        // Verify the INSERT_SOURCE query includes JSONB config param
        assert!(source_inserts[0].query.contains("$5"));
    }

    // -----------------------------------------------------------------------
    // 5. test_sync_entity_schemas
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_entity_schemas() {
        let db = MockDbClient::new();
        let mut stream = make_minimal_stream("test");
        stream.entity_schemas = vec![make_entity_schema("indoor_monitor")];

        sync_dictionary(&[stream], &db, &opts()).await.unwrap();

        let schema_inserts = db.calls_starting_with("INSERT INTO data_dictionary.entity_schemas");
        assert_eq!(schema_inserts.len(), 1);
    }

    // -----------------------------------------------------------------------
    // 6. test_sync_entity_schema_attributes_uses_subselect
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_entity_schema_attributes_uses_subselect() {
        let db = MockDbClient::new();
        let mut stream = make_minimal_stream("test");
        stream.entity_schemas = vec![make_entity_schema("indoor_monitor")];

        sync_dictionary(&[stream], &db, &opts()).await.unwrap();

        let attr_inserts =
            db.calls_starting_with("INSERT INTO data_dictionary.entity_schema_attributes");
        assert_eq!(attr_inserts.len(), 1);
        // The query must use SELECT for schema_id (subselect pattern)
        assert!(
            attr_inserts[0].query.contains("SELECT id"),
            "Entity schema attribute INSERT must use subselect for schema_id"
        );
        assert!(
            attr_inserts[0]
                .query
                .contains("FROM data_dictionary.entity_schemas WHERE stream_id"),
            "Must reference entity_schemas table for FK resolution"
        );
    }

    // -----------------------------------------------------------------------
    // 7. test_sync_silver_tables
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_silver_tables() {
        let db = MockDbClient::new();
        let mut stream = make_minimal_stream("air-quality");
        stream.silver_etl = Some(make_silver_etl("silver.air_quality_observations"));

        sync_dictionary(&[stream], &db, &opts()).await.unwrap();

        let upserts = db.calls_starting_with("INSERT INTO data_dictionary.silver_tables");
        assert_eq!(upserts.len(), 1);
        assert!(
            upserts[0].query.contains("ON CONFLICT"),
            "Silver table must use UPSERT"
        );
    }

    // -----------------------------------------------------------------------
    // 8. test_sync_silver_columns_type_mapping
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_silver_columns_type_mapping() {
        let db = MockDbClient::new();
        let mut stream = make_minimal_stream("test");
        let mut etl = make_silver_etl("silver.test");
        etl.field_mappings = vec![
            SilverFieldMapping {
                source_path: "raw_payload.val1".to_string(),
                target_column: "val1".to_string(),
                data_type: "double_precision".to_string(),
                unit: None,
                description: None,
                nullable: true,
                transform_type: None,
                dq_rules: vec![],
            },
            SilverFieldMapping {
                source_path: "raw_payload.val2".to_string(),
                target_column: "val2".to_string(),
                data_type: "smallint".to_string(),
                unit: None,
                description: None,
                nullable: true,
                transform_type: None,
                dq_rules: vec![],
            },
            SilverFieldMapping {
                source_path: "raw_payload.val3".to_string(),
                target_column: "val3".to_string(),
                data_type: "boolean".to_string(),
                unit: None,
                description: None,
                nullable: true,
                transform_type: None,
                dq_rules: vec![],
            },
        ];
        stream.silver_etl = Some(etl);

        sync_dictionary(&[stream], &db, &opts()).await.unwrap();

        let col_upserts = db.calls_starting_with("INSERT INTO data_dictionary.silver_columns");
        assert_eq!(col_upserts.len(), 3);
        // Type mapping is verified by sql::tests. Here we verify all 3 columns
        // are processed and each uses UPSERT.
        for upsert in &col_upserts {
            assert!(upsert.query.contains("ON CONFLICT"));
        }
    }

    // -----------------------------------------------------------------------
    // 9. test_sync_silver_lineage
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_silver_lineage() {
        let db = MockDbClient::new();
        let mut stream = make_minimal_stream("air-quality");
        let mut etl = make_silver_etl("silver.air_quality_observations");
        etl.field_mappings[0].transform_type = Some("unit_conversion".to_string());
        stream.silver_etl = Some(etl);

        sync_dictionary(&[stream], &db, &opts()).await.unwrap();

        let lineage_upserts = db.calls_starting_with("INSERT INTO data_dictionary.silver_lineage");
        assert_eq!(lineage_upserts.len(), 1);
        assert!(
            lineage_upserts[0].query.contains("ON CONFLICT"),
            "Silver lineage must use UPSERT"
        );
    }

    // -----------------------------------------------------------------------
    // 10. test_sync_silver_column_dq_rules
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_silver_column_dq_rules() {
        let db = MockDbClient::new();
        let mut stream = make_minimal_stream("test");
        stream.silver_etl = Some(make_silver_etl("silver.test"));

        sync_dictionary(&[stream], &db, &opts()).await.unwrap();

        let dq_upserts = db.calls_starting_with("INSERT INTO data_dictionary.silver_dq_rules");
        // make_silver_etl has 1 column-level dq_rule
        assert!(dq_upserts.len() >= 1, "Expected at least 1 DQ rule upsert");
        // Column-level rules include a silver_column parameter (not NULL)
        let col_rule = dq_upserts
            .iter()
            .find(|c| c.query.contains("VALUES ($1, $2, $3, $4, $5)"));
        assert!(
            col_rule.is_some(),
            "Should have column-level DQ rule with 5 params"
        );
    }

    // -----------------------------------------------------------------------
    // 11. test_sync_silver_table_dq_rules_cross_field
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_silver_table_dq_rules_cross_field() {
        let db = MockDbClient::new();
        let mut stream = make_minimal_stream("test");
        let mut etl = make_silver_etl("silver.test");
        etl.dq_rules = vec![SilverTableDqRule {
            rule_type: "cross_field_check".to_string(),
            rule_name: "pm10_gte_pm25".to_string(),
            params: serde_json::json!({
                "expression": "pm10 IS NULL OR pm25 IS NULL OR pm10 >= pm25",
                "message": "pm10_less_than_pm25"
            }),
            action: "flag".to_string(),
        }];
        stream.silver_etl = Some(etl);

        sync_dictionary(&[stream], &db, &opts()).await.unwrap();

        let table_rules: Vec<_> = db
            .calls_starting_with("INSERT INTO data_dictionary.silver_dq_rules")
            .into_iter()
            .filter(|c| c.query.contains("NULL"))
            .collect();
        assert!(
            !table_rules.is_empty(),
            "Should have table-level DQ rule with silver_column = NULL"
        );
    }

    // -----------------------------------------------------------------------
    // 12. test_sync_silver_table_dq_rules_freshness
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_silver_table_dq_rules_freshness() {
        let db = MockDbClient::new();
        let mut stream = make_minimal_stream("test");
        let mut etl = make_silver_etl("silver.test");
        etl.dq_rules = vec![SilverTableDqRule {
            rule_type: "freshness_check".to_string(),
            rule_name: "freshness_check_observation_time".to_string(),
            params: serde_json::json!({
                "field": "observation_time",
                "max_age": "2 hours",
                "max_future": "5 minutes",
                "reference": "ingestion_time"
            }),
            action: "flag".to_string(),
        }];
        stream.silver_etl = Some(etl);

        sync_dictionary(&[stream], &db, &opts()).await.unwrap();

        let table_rules: Vec<_> = db
            .calls_starting_with("INSERT INTO data_dictionary.silver_dq_rules")
            .into_iter()
            .filter(|c| c.query.contains("NULL"))
            .collect();
        assert!(
            !table_rules.is_empty(),
            "Should have freshness_check table-level DQ rule"
        );
    }

    // -----------------------------------------------------------------------
    // 13. test_sync_silver_table_dq_rules_rate_of_change
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_silver_table_dq_rules_rate_of_change() {
        let db = MockDbClient::new();
        let mut stream = make_minimal_stream("test");
        let mut etl = make_silver_etl("silver.test");
        etl.dq_rules = vec![SilverTableDqRule {
            rule_type: "rate_of_change".to_string(),
            rule_name: "rate_of_change_pm25".to_string(),
            params: serde_json::json!({
                "field": "pm25",
                "max_change_per_minute": 100
            }),
            action: "flag".to_string(),
        }];
        stream.silver_etl = Some(etl);

        sync_dictionary(&[stream], &db, &opts()).await.unwrap();

        let table_rules: Vec<_> = db
            .calls_starting_with("INSERT INTO data_dictionary.silver_dq_rules")
            .into_iter()
            .filter(|c| c.query.contains("NULL"))
            .collect();
        assert!(
            !table_rules.is_empty(),
            "Should have rate_of_change table-level DQ rule"
        );
    }

    // -----------------------------------------------------------------------
    // 14. test_sync_silver_table_dq_rules_completeness
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_silver_table_dq_rules_completeness() {
        let db = MockDbClient::new();
        let mut stream = make_minimal_stream("test");
        let mut etl = make_silver_etl("silver.test");
        etl.dq_rules = vec![SilverTableDqRule {
            rule_type: "completeness_check".to_string(),
            rule_name: "completeness_check_pm25".to_string(),
            params: serde_json::json!({
                "level": "batch",
                "field": "pm25",
                "min_completeness": 0.95
            }),
            action: "warn".to_string(),
        }];
        stream.silver_etl = Some(etl);

        sync_dictionary(&[stream], &db, &opts()).await.unwrap();

        let table_rules: Vec<_> = db
            .calls_starting_with("INSERT INTO data_dictionary.silver_dq_rules")
            .into_iter()
            .filter(|c| c.query.contains("NULL"))
            .collect();
        assert!(
            !table_rules.is_empty(),
            "Should have completeness_check table-level DQ rule"
        );
    }

    // -----------------------------------------------------------------------
    // 15. test_sync_deletes_bronze_before_insert
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_deletes_bronze_before_insert() {
        let db = MockDbClient::new();
        let stream = make_minimal_stream("test");
        sync_dictionary(&[stream], &db, &opts()).await.unwrap();

        let queries = db.query_strings();

        // Find positions of DELETE and INSERT statements
        let delete_attrs_pos = queries
            .iter()
            .position(|q| q.contains("DELETE FROM data_dictionary.entity_schema_attributes"))
            .expect("Should have DELETE entity_schema_attributes");
        let delete_schemas_pos = queries
            .iter()
            .position(|q| q.contains("DELETE FROM data_dictionary.entity_schemas"))
            .expect("Should have DELETE entity_schemas");
        let delete_sources_pos = queries
            .iter()
            .position(|q| q.contains("DELETE FROM data_dictionary.sources"))
            .expect("Should have DELETE sources");
        let delete_fields_pos = queries
            .iter()
            .position(|q| q.contains("DELETE FROM data_dictionary.fields"))
            .expect("Should have DELETE fields");
        let delete_streams_pos = queries
            .iter()
            .position(|q| q.contains("DELETE FROM data_dictionary.streams"))
            .expect("Should have DELETE streams");
        let insert_stream_pos = queries
            .iter()
            .position(|q| q.contains("INSERT INTO data_dictionary.streams"))
            .expect("Should have INSERT streams");

        // FK-safe ordering: attributes -> schemas -> sources -> fields -> streams
        assert!(
            delete_attrs_pos < delete_schemas_pos,
            "Must delete attributes before schemas"
        );
        assert!(
            delete_schemas_pos < delete_sources_pos,
            "Must delete schemas before sources"
        );
        assert!(
            delete_sources_pos < delete_fields_pos,
            "Must delete sources before fields"
        );
        assert!(
            delete_fields_pos < delete_streams_pos,
            "Must delete fields before streams"
        );
        assert!(
            delete_streams_pos < insert_stream_pos,
            "Must delete all before any insert"
        );
    }

    // -----------------------------------------------------------------------
    // 16. test_sync_silver_uses_upsert
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_silver_uses_upsert() {
        let db = MockDbClient::new();
        let mut stream = make_minimal_stream("test");
        stream.silver_etl = Some(make_silver_etl("silver.test"));

        sync_dictionary(&[stream], &db, &opts()).await.unwrap();

        // All silver operations must use ON CONFLICT
        let silver_calls: Vec<_> = db
            .calls()
            .into_iter()
            .filter(|c| c.query.contains("silver_"))
            .filter(|c| c.query.starts_with("INSERT"))
            .collect();

        for call in &silver_calls {
            assert!(
                call.query.contains("ON CONFLICT"),
                "Silver INSERT must use ON CONFLICT: {}",
                call.query
            );
        }
        assert!(
            !silver_calls.is_empty(),
            "Should have at least one Silver upsert"
        );
    }

    // -----------------------------------------------------------------------
    // 17. test_sync_records_sync_status
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_records_sync_status() {
        let db = MockDbClient::new();
        let stream = make_minimal_stream("test");
        sync_dictionary(&[stream], &db, &opts()).await.unwrap();

        let queries = db.query_strings();

        // Should have sync_status INSERT at start
        let insert_pos = queries
            .iter()
            .position(|q| q.contains("INSERT INTO data_dictionary.sync_status"))
            .expect("Should insert sync_status");

        // Should have sync_status UPDATE at end
        let update_pos = queries
            .iter()
            .position(|q| {
                q.contains("UPDATE data_dictionary.sync_status") && q.contains("status = 'success'")
            })
            .expect("Should update sync_status to success");

        assert!(
            insert_pos < update_pos,
            "sync_status INSERT must come before UPDATE"
        );
    }

    // -----------------------------------------------------------------------
    // 18. test_sync_report_counts
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_report_counts() {
        let db = MockDbClient::new();
        let mut stream = make_minimal_stream("test");
        stream.fields = vec![make_field("f1", "float"), make_field("f2", "float")];
        stream.sources = vec![make_source("s1", "mqtt")];
        stream.entity_schemas = vec![make_entity_schema("schema1")];
        stream.silver_etl = Some(make_silver_etl("silver.test"));

        let report = sync_dictionary(&[stream], &db, &opts()).await.unwrap();

        assert_eq!(report.entity, "dictionary");
        assert_eq!(report.items_processed, 1);

        // items_created = 1 stream + 2 fields + 1 source + 1 schema + 1 attribute = 6
        assert_eq!(report.items_created, 6);

        // items_updated = 1 silver_table + 1 silver_column + 1 lineage + 1 dq_rule = 4
        assert_eq!(report.items_updated, 4);
    }

    // -----------------------------------------------------------------------
    // 19. test_dry_run_returns_sql
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_dry_run_returns_sql() {
        let db = MockDbClient::new();
        let mut stream = make_minimal_stream("test");
        stream.fields = vec![make_field("f1", "float")];
        stream.silver_etl = Some(make_silver_etl("silver.test"));

        let dry_opts = SyncOptions {
            dry_run: true,
            ..Default::default()
        };
        let report = sync_dictionary(&[stream], &db, &dry_opts).await.unwrap();

        // No SQL should have been executed
        assert!(db.calls().is_empty(), "Dry run must not execute any SQL");

        // Report should still have counts
        assert_eq!(report.items_processed, 1);
        assert!(report.items_created > 0);
        assert_eq!(report.duration, std::time::Duration::ZERO);
    }

    // -----------------------------------------------------------------------
    // 20. test_multi_stream_silver_table_aggregates_sources
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_multi_stream_silver_table_aggregates_sources() {
        let db = MockDbClient::new();

        // Two streams feeding the same Silver table
        let mut stream1 = make_minimal_stream("outdoor-weather");
        stream1.silver_etl = Some(SilverEtlEntry {
            enabled: true,
            target_table: "silver.weather_observations".to_string(),
            description: Some("Weather observations".to_string()),
            grain: Some("One row per reading".to_string()),
            timestamp_column: "observation_time".to_string(),
            field_mappings: vec![SilverFieldMapping {
                source_path: "raw_payload.main.temp".to_string(),
                target_column: "temperature_c".to_string(),
                data_type: "double_precision".to_string(),
                unit: Some("Celsius".to_string()),
                description: Some("Temperature".to_string()),
                nullable: false,
                transform_type: None,
                dq_rules: vec![],
            }],
            dq_rules: vec![],
        });

        let mut stream2 = make_minimal_stream("nws-observations");
        stream2.silver_etl = Some(SilverEtlEntry {
            enabled: true,
            target_table: "silver.weather_observations".to_string(),
            description: Some("NWS weather".to_string()),
            grain: Some("One row per NWS obs".to_string()),
            timestamp_column: "observation_time".to_string(),
            field_mappings: vec![SilverFieldMapping {
                source_path: "raw_payload.temperature".to_string(),
                target_column: "nws_temperature_c".to_string(),
                data_type: "double_precision".to_string(),
                unit: Some("Celsius".to_string()),
                description: Some("NWS temperature".to_string()),
                nullable: true,
                transform_type: None,
                dq_rules: vec![],
            }],
            dq_rules: vec![],
        });

        sync_dictionary(&[stream1, stream2], &db, &opts())
            .await
            .unwrap();

        // Should have exactly 1 silver_tables UPSERT (not 2)
        let table_upserts = db.calls_starting_with("INSERT INTO data_dictionary.silver_tables");
        assert_eq!(
            table_upserts.len(),
            1,
            "Two streams feeding same Silver table should produce 1 UPSERT"
        );

        // Should have 2 silver_columns UPSERTs (one per stream)
        let col_upserts = db.calls_starting_with("INSERT INTO data_dictionary.silver_columns");
        assert_eq!(col_upserts.len(), 2);

        // Should have 2 silver_lineage UPSERTs (one per stream)
        let lineage_upserts = db.calls_starting_with("INSERT INTO data_dictionary.silver_lineage");
        assert_eq!(lineage_upserts.len(), 2);
    }

    // -----------------------------------------------------------------------
    // Additional: test_transaction_wrapping
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_transaction_wrapping() {
        let db = MockDbClient::new();
        let stream = make_minimal_stream("test");
        sync_dictionary(&[stream], &db, &opts()).await.unwrap();

        let queries = db.query_strings();
        assert_eq!(queries.first().unwrap(), "BEGIN");
        assert_eq!(queries.last().unwrap(), "COMMIT");
    }
}
