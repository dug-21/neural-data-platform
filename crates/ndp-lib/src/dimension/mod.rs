//! Dimension sync operations.
//!
//! Implements `ndp dimension sync` -- importing dimension data (CSV)
//! into Silver layer tables.
//!
//! # Flow
//!
//! 1. Parse CSV content against the schema definition
//! 2. If strategy is `truncate_and_load`, TRUNCATE the target table
//! 3. Build parameterized INSERT SQL with `$1`, `$2`, etc.
//! 4. Execute in batches against the database
//! 5. Return a `SyncReport` with counts

pub mod csv_import;
pub mod types;

use std::time::Instant;

use crate::config::DimensionConfig;
use crate::db::DbClient;
use crate::error::{NdpLibError, Result};
use crate::types::{SyncOptions, SyncReport};

use csv_import::{build_insert_sql, build_truncate_sql, parse_csv};

/// Sync a dimension table from its source data.
///
/// Caller provides the parsed dimension config and the raw CSV bytes.
/// This function takes parsed structs, not file paths (Pattern 2).
///
/// # Arguments
/// * `config` - parsed dimension configuration
/// * `csv_content` - raw CSV content (bytes) to import
/// * `db` - database client (real or mock)
/// * `options` - sync options (dry_run, etc.)
///
/// # Returns
/// A `SyncReport` summarizing what was created, updated, or deleted.
pub async fn sync_dimension(
    config: &DimensionConfig,
    csv_content: &[u8],
    db: &impl DbClient,
    options: &SyncOptions,
) -> Result<SyncReport> {
    let start = Instant::now();

    // 1. Parse CSV
    let rows = parse_csv(csv_content, config)?;
    let total_rows = rows.len();

    if total_rows == 0 {
        return Ok(SyncReport {
            entity: format!("dimension:{}", config.dimension_id),
            items_processed: 0,
            items_created: 0,
            items_updated: 0,
            items_deleted: 0,
            errors: Vec::new(),
            duration: start.elapsed(),
        });
    }

    // Resolve load strategy
    let load = config.load.as_ref();
    let strategy = load
        .map(|l| l.strategy.as_str())
        .unwrap_or("truncate_and_load");
    let batch_size = load.map(|l| l.batch_size).unwrap_or(1000);

    // 2. Truncate if strategy requires it
    if strategy == "truncate_and_load" {
        let truncate_sql = build_truncate_sql(config);
        if options.dry_run {
            tracing::info!(sql = %truncate_sql, "DRY RUN: would execute TRUNCATE");
        } else {
            tracing::info!(
                table = %format!("{}.{}", config.target.schema, config.target.table),
                "Truncating dimension table"
            );
            db.batch_execute(&truncate_sql).await?;
        }
    }

    // 3. Insert in batches
    let mut items_created: usize = 0;

    // Identify which fields are array types (need Vec<String> params)
    let field_types: Vec<&str> = config
        .schema
        .fields
        .iter()
        .map(|f| f.field_type.as_str())
        .collect();

    for chunk in rows.chunks(batch_size) {
        let (insert_sql, params_per_row) = build_insert_sql(config, chunk.len());

        if options.dry_run {
            tracing::info!(
                sql = %insert_sql,
                rows = chunk.len(),
                "DRY RUN: would execute INSERT"
            );
            items_created += chunk.len();
        } else {
            // Build typed params: Box<dyn ToSql> to handle mixed types
            // TEXT columns -> Option<String>, TEXT[] columns -> Option<Vec<String>>
            let mut boxed_params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync>> =
                Vec::with_capacity(params_per_row * chunk.len());

            for row in chunk.iter() {
                for (i, cell) in row.iter().enumerate() {
                    let ft = field_types.get(i).copied().unwrap_or("text");
                    if ft.ends_with("[]") {
                        // Array column: parse "{a,b}" into Vec<String>
                        let arr: Option<Vec<String>> =
                            cell.as_ref().map(|s| csv_import::parse_pg_array(s));
                        boxed_params.push(Box::new(arr));
                    } else {
                        boxed_params.push(Box::new(cell.clone()));
                    }
                }
            }

            let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = boxed_params
                .iter()
                .map(|b| b.as_ref())
                .collect();

            let affected = db.execute(&insert_sql, &param_refs).await.map_err(|e| {
                NdpLibError::SyncFailed {
                    entity: config.dimension_id.clone(),
                    reason: format!(
                        "INSERT failed for batch of {} rows ({} params): {}",
                        chunk.len(),
                        params_per_row * chunk.len(),
                        e
                    ),
                }
            })?;

            items_created += affected as usize;
        }
    }

    Ok(SyncReport {
        entity: format!("dimension:{}", config.dimension_id),
        items_processed: total_rows,
        items_created,
        items_updated: 0,
        items_deleted: 0,
        errors: Vec::new(),
        duration: start.elapsed(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        DimensionConfig, DimensionField, DimensionLoad, DimensionSchema, DimensionSource,
        DimensionTarget,
    };
    use crate::db::DbClient;
    use crate::error::Result as NdpResult;
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};
    use tokio_postgres::types::ToSql;
    use tokio_postgres::Row;

    // -----------------------------------------------------------------------
    // MockDbClient for London TDD
    // -----------------------------------------------------------------------

    /// Records every SQL call for assertion.
    #[derive(Debug, Clone)]
    struct MockDbClient {
        /// Queries executed via `execute()` -- (sql, param_count).
        execute_calls: Arc<Mutex<Vec<(String, usize)>>>,
        /// Queries executed via `batch_execute()`.
        batch_calls: Arc<Mutex<Vec<String>>>,
        /// What `execute()` should return (rows affected).
        execute_return: u64,
    }

    impl MockDbClient {
        fn new(execute_return: u64) -> Self {
            Self {
                execute_calls: Arc::new(Mutex::new(Vec::new())),
                batch_calls: Arc::new(Mutex::new(Vec::new())),
                execute_return,
            }
        }

        fn execute_calls(&self) -> Vec<(String, usize)> {
            self.execute_calls.lock().unwrap().clone()
        }

        fn batch_calls(&self) -> Vec<String> {
            self.batch_calls.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl DbClient for MockDbClient {
        async fn query(
            &self,
            _query: &str,
            _params: &[&(dyn ToSql + Sync)],
        ) -> NdpResult<Vec<Row>> {
            Ok(Vec::new())
        }

        async fn execute(&self, query: &str, params: &[&(dyn ToSql + Sync)]) -> NdpResult<u64> {
            self.execute_calls
                .lock()
                .unwrap()
                .push((query.to_string(), params.len()));
            Ok(self.execute_return)
        }

        async fn batch_execute(&self, sql: &str) -> NdpResult<()> {
            self.batch_calls.lock().unwrap().push(sql.to_string());
            Ok(())
        }
    }

    /// Helper to build a test config matching the entity_context shape.
    fn entity_context_config() -> DimensionConfig {
        DimensionConfig {
            dimension_id: "entity_context".to_string(),
            description: "Test entity context".to_string(),
            version: "1.0.0".to_string(),
            target: DimensionTarget {
                table: "entity_context".to_string(),
                schema: "silver".to_string(),
            },
            source: DimensionSource {
                source_type: "csv".to_string(),
                path: Some("data/dimensions/entity_context.csv".to_string()),
                delimiter: ",".to_string(),
                has_header: true,
            },
            schema: DimensionSchema {
                primary_key: vec!["ndp_id".to_string()],
                fields: vec![
                    dim_field("ndp_id"),
                    dim_field("category"),
                    dim_field("friendly_name"),
                    dim_field("location_path"),
                    dim_field("correlates_with"),
                    dim_field("orientation"),
                ],
            },
            load: Some(DimensionLoad {
                strategy: "truncate_and_load".to_string(),
                batch_size: 1000,
            }),
        }
    }

    fn dim_field(name: &str) -> DimensionField {
        DimensionField {
            name: name.to_string(),
            field_type: "text".to_string(),
            nullable: name != "ndp_id",
            description: None,
        }
    }

    // -----------------------------------------------------------------------
    // Tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_sync_dimension_truncate_and_load() {
        let config = entity_context_config();
        let csv = b"ndp_id,category,friendly_name,location_path,correlates_with,orientation\n\
                     temp_living,temperature,Living Room Temp,home/living_room,{humidity_living},\n";

        let db = MockDbClient::new(1);
        let options = SyncOptions { dry_run: false };

        let report = sync_dimension(&config, csv, &db, &options).await.unwrap();

        // Should have called TRUNCATE first
        let batch = db.batch_calls();
        assert_eq!(batch.len(), 1);
        assert!(
            batch[0].contains("TRUNCATE TABLE silver.entity_context"),
            "Expected TRUNCATE, got: {}",
            batch[0]
        );

        // Should have called INSERT
        let exec = db.execute_calls();
        assert_eq!(exec.len(), 1);
        assert!(
            exec[0].0.contains("INSERT INTO silver.entity_context"),
            "Expected INSERT, got: {}",
            exec[0].0
        );

        assert_eq!(report.items_processed, 1);
        assert_eq!(report.items_created, 1);
    }

    #[tokio::test]
    async fn test_sync_dimension_inserts_all_rows() {
        let config = entity_context_config();
        let csv = b"ndp_id,category,friendly_name,location_path,correlates_with,orientation\n\
                     temp_living,temperature,Living Room Temp,home/living_room,{humidity_living},\n\
                     humidity_living,humidity,Living Room Humidity,home/living_room,{temp_living},\n\
                     temp_outdoor,temperature,Outdoor Temperature,outdoor,\"{humidity_outdoor,aqi_outdoor}\",north\n";

        let db = MockDbClient::new(3);
        let options = SyncOptions { dry_run: false };

        let report = sync_dimension(&config, csv, &db, &options).await.unwrap();

        assert_eq!(report.items_processed, 3);
        assert_eq!(report.items_created, 3);
        assert!(report.errors.is_empty());
    }

    #[tokio::test]
    async fn test_sync_dimension_parameterized_sql() {
        let config = entity_context_config();
        let csv = b"ndp_id,category,friendly_name,location_path,correlates_with,orientation\n\
                     a,b,c,d,e,f\n";

        let db = MockDbClient::new(1);
        let options = SyncOptions { dry_run: false };

        sync_dimension(&config, csv, &db, &options).await.unwrap();

        let exec = db.execute_calls();
        assert_eq!(exec.len(), 1);
        let (sql, param_count) = &exec[0];

        // 6 fields, 1 row => $1..$6
        assert!(
            sql.contains("$1"),
            "SQL should use parameterized placeholders"
        );
        assert!(sql.contains("$6"), "SQL should have $6 for 6 columns");
        assert!(!sql.contains("$7"), "SQL should not have $7 for 1 row");
        assert_eq!(*param_count, 6, "Should pass 6 parameters for 6 columns");
    }

    #[tokio::test]
    async fn test_sync_dimension_dry_run() {
        let config = entity_context_config();
        let csv = b"ndp_id,category,friendly_name,location_path,correlates_with,orientation\n\
                     a,b,c,d,e,f\n";

        let db = MockDbClient::new(0);
        let options = SyncOptions { dry_run: true };

        let report = sync_dimension(&config, csv, &db, &options).await.unwrap();

        // Dry run: no SQL should be executed against the database
        assert!(
            db.batch_calls().is_empty(),
            "TRUNCATE should not execute in dry_run"
        );
        assert!(
            db.execute_calls().is_empty(),
            "INSERT should not execute in dry_run"
        );

        // But report should still reflect what would happen
        assert_eq!(report.items_processed, 1);
        assert_eq!(report.items_created, 1);
    }

    #[tokio::test]
    async fn test_sync_dimension_report_counts() {
        let config = entity_context_config();
        let csv = b"ndp_id,category,friendly_name,location_path,correlates_with,orientation\n\
                     a,b,c,d,e,f\n\
                     g,h,i,j,k,l\n";

        let db = MockDbClient::new(2);
        let options = SyncOptions { dry_run: false };

        let report = sync_dimension(&config, csv, &db, &options).await.unwrap();

        assert_eq!(report.entity, "dimension:entity_context");
        assert_eq!(report.items_processed, 2);
        assert_eq!(report.items_created, 2);
        assert_eq!(report.items_updated, 0);
        assert!(report.errors.is_empty());
        assert!(report.duration.as_nanos() > 0, "Duration should be nonzero");
    }

    #[tokio::test]
    async fn test_sync_dimension_empty_csv() {
        let config = entity_context_config();
        let csv = b"ndp_id,category,friendly_name,location_path,correlates_with,orientation\n";

        let db = MockDbClient::new(0);
        let options = SyncOptions { dry_run: false };

        let report = sync_dimension(&config, csv, &db, &options).await.unwrap();

        assert_eq!(report.items_processed, 0);
        assert_eq!(report.items_created, 0);
        assert!(db.batch_calls().is_empty(), "No TRUNCATE for empty CSV");
        assert!(db.execute_calls().is_empty(), "No INSERT for empty CSV");
    }

    #[test]
    fn test_dimension_config_deserialize() {
        let json = r#"{
            "dimension_id": "entity_context",
            "description": "Test",
            "version": "1.0.0",
            "target": { "table": "entity_context", "schema": "silver" },
            "source": { "type": "csv", "path": "data/dimensions/entity_context.csv" },
            "schema": {
                "primary_key": ["ndp_id"],
                "fields": [
                    { "name": "ndp_id", "type": "text", "nullable": false, "description": "ID" }
                ]
            },
            "load": { "strategy": "truncate_and_load", "batch_size": 1000 }
        }"#;

        let config: DimensionConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.dimension_id, "entity_context");
        assert_eq!(config.target.table, "entity_context");
        assert_eq!(config.target.schema, "silver");
        assert_eq!(config.source.source_type, "csv");
        assert_eq!(config.schema.fields.len(), 1);
        assert_eq!(config.schema.fields[0].name, "ndp_id");
        assert_eq!(config.schema.primary_key, vec!["ndp_id"]);
        let load = config.load.unwrap();
        assert_eq!(load.strategy, "truncate_and_load");
        assert_eq!(load.batch_size, 1000);
    }

    #[test]
    fn test_dimension_config_from_json_file() {
        // Load the real entity_context.json that was migrated from YAML
        let content = include_str!("../../../../config/base/dimensions/entity_context.json");
        let config: DimensionConfig = serde_json::from_str(content).unwrap();

        assert_eq!(config.dimension_id, "entity_context");
        assert_eq!(config.target.table, "entity_context");
        assert_eq!(config.target.schema, "silver");
        assert_eq!(config.source.source_type, "csv");
        assert!(config.source.has_header);
        assert_eq!(config.schema.primary_key, vec!["ndp_id"]);
        assert_eq!(config.schema.fields.len(), 6);

        // Verify all 6 field names
        let names: Vec<&str> = config
            .schema
            .fields
            .iter()
            .map(|f| f.name.as_str())
            .collect();
        assert_eq!(
            names,
            vec![
                "ndp_id",
                "category",
                "friendly_name",
                "location_path",
                "correlates_with",
                "orientation"
            ]
        );

        let load = config.load.unwrap();
        assert_eq!(load.strategy, "truncate_and_load");
        assert_eq!(load.batch_size, 1000);
    }
}
