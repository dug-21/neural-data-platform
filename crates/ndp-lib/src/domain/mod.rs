//! Domain sync operations.
//!
//! Implements `ndp domain sync` -- syncing domain configs to the
//! `data_dictionary` schema in TimescaleDB.
//!
//! # Design
//!
//! - Takes `&[DomainSyncEntry]` (parsed structs, not file paths).
//! - Uses `&impl DbClient` for testability (London TDD with mock).
//! - Domains use UPSERT (ON CONFLICT DO UPDATE).
//! - Children (streams, objectives, constraints) use DELETE+INSERT per domain.
//! - All SQL uses parameterized queries ($1, $2, ...) -- never string concat.

pub mod sql;
pub mod types;

use std::time::Instant;

use tracing::{debug, error, info};

use crate::db::DbClient;
use crate::error::Result;
use crate::types::{SyncError, SyncOptions, SyncReport};
use types::*;

/// Sync domain configurations to the `data_dictionary` tables.
///
/// Caller decides where configs come from (files, etcd, test fixtures).
/// This function takes parsed structs, not file paths.
///
/// Per-domain errors are non-fatal: collected in `SyncReport.errors`,
/// sync continues to the next domain.
///
/// # Arguments
/// * `domains` - parsed domain configurations
/// * `db` - database client (real or mock)
/// * `options` - sync options (dry_run, etc.)
///
/// # Returns
/// A `SyncReport` summarizing what was created, updated, or deleted.
pub async fn sync_domains(
    domains: &[DomainSyncEntry],
    db: &impl DbClient,
    options: &SyncOptions,
) -> Result<SyncReport> {
    let start = Instant::now();
    let mut errors: Vec<SyncError> = Vec::new();
    let mut domains_synced: usize = 0;
    let mut streams_synced: usize = 0;
    let mut objectives_synced: usize = 0;
    let mut constraints_synced: usize = 0;

    if options.dry_run {
        return Ok(build_dry_run_report(domains));
    }

    // Step 1: BEGIN transaction
    db.batch_execute("BEGIN").await?;

    // Step 2: Sync each domain
    for domain in domains {
        match sync_single_domain(db, domain).await {
            Ok((s, o, c)) => {
                domains_synced += 1;
                streams_synced += s;
                objectives_synced += o;
                constraints_synced += c;
            }
            Err(e) => {
                error!(domain_id = %domain.domain_id, error = %e, "Failed to sync domain");
                errors.push(SyncError {
                    item: domain.domain_id.clone(),
                    message: format!("Domain sync failed: {}", e),
                });
            }
        }
    }

    // Step 3: COMMIT transaction
    db.batch_execute("COMMIT").await?;

    let duration = start.elapsed();

    info!(
        domains = domains_synced,
        streams = streams_synced,
        objectives = objectives_synced,
        constraints = constraints_synced,
        duration_ms = duration.as_millis() as u64,
        "Domain sync complete"
    );

    Ok(SyncReport {
        entity: "domain".to_string(),
        items_processed: domains.len(),
        items_created: domains_synced + streams_synced + objectives_synced + constraints_synced,
        items_updated: 0,
        items_deleted: 0,
        errors,
        duration,
    })
}

/// Sync a single domain and its children (streams, objectives, constraints).
///
/// Returns (streams_count, objectives_count, constraints_count) on success.
async fn sync_single_domain(
    db: &impl DbClient,
    domain: &DomainSyncEntry,
) -> Result<(usize, usize, usize)> {
    // a. UPSERT domain
    db.execute(
        sql::UPSERT_DOMAIN,
        &[
            &domain.domain_id,
            &domain.description,
            &domain.stream_count,
            &domain.config_path,
        ],
    )
    .await?;

    // b. DELETE + INSERT domain_streams
    db.execute(sql::DELETE_DOMAIN_STREAMS, &[&domain.domain_id])
        .await?;

    for stream in &domain.streams {
        db.execute(
            sql::INSERT_DOMAIN_STREAM,
            &[
                &domain.domain_id,
                &stream.stream_id,
                &stream.alias,
                &stream.role,
            ],
        )
        .await?;
    }

    // d. DELETE + INSERT objectives
    db.execute(sql::DELETE_OBJECTIVES, &[&domain.domain_id])
        .await?;

    for obj in &domain.objectives {
        db.execute(
            sql::INSERT_OBJECTIVE,
            &[
                &obj.objective_id,
                &domain.domain_id,
                &obj.description,
                &obj.target_stream,
                &obj.target_metric,
                &obj.condition,
                &obj.threshold,
                &obj.threshold_upper,
                &obj.unit,
                &obj.priority,
            ],
        )
        .await?;
    }

    // f. DELETE + INSERT constraints
    db.execute(sql::DELETE_CONSTRAINTS, &[&domain.domain_id])
        .await?;

    for con in &domain.constraints {
        db.execute(
            sql::INSERT_CONSTRAINT,
            &[
                &con.constraint_id,
                &domain.domain_id,
                &con.description,
                &con.constraint_stream,
                &con.constraint_metric,
                &con.condition,
                &con.threshold,
                &con.unit,
            ],
        )
        .await?;
    }

    debug!(
        domain_id = %domain.domain_id,
        streams = domain.streams.len(),
        objectives = domain.objectives.len(),
        constraints = domain.constraints.len(),
        "Synced domain"
    );

    Ok((
        domain.streams.len(),
        domain.objectives.len(),
        domain.constraints.len(),
    ))
}

/// Build a SyncReport for dry_run mode without executing SQL.
fn build_dry_run_report(domains: &[DomainSyncEntry]) -> SyncReport {
    let mut streams = 0usize;
    let mut objectives = 0usize;
    let mut constraints = 0usize;

    for domain in domains {
        streams += domain.streams.len();
        objectives += domain.objectives.len();
        constraints += domain.constraints.len();
    }

    SyncReport {
        entity: "domain".to_string(),
        items_processed: domains.len(),
        items_created: domains.len() + streams + objectives + constraints,
        items_updated: 0,
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
    use crate::error::{NdpLibError, Result};
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

    /// A mock that fails on execute() calls to test error collection.
    struct FailingMockDbClient {
        calls: Mutex<Vec<SqlCall>>,
    }

    impl FailingMockDbClient {
        fn new() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl DbClient for FailingMockDbClient {
        async fn query(&self, _query: &str, _params: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>> {
            Ok(vec![])
        }

        async fn execute(&self, _query: &str, _params: &[&(dyn ToSql + Sync)]) -> Result<u64> {
            Err(NdpLibError::Database("simulated DB failure".to_string()))
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

    fn make_minimal_domain(id: &str) -> DomainSyncEntry {
        DomainSyncEntry {
            domain_id: id.to_string(),
            description: Some(format!("Test domain {}", id)),
            stream_count: 0,
            config_path: format!("config/domains/{}/domain.json", id),
            streams: vec![],
            objectives: vec![],
            constraints: vec![],
        }
    }

    fn make_stream_mapping(stream_id: &str, alias: &str, role: &str) -> StreamMappingEntry {
        StreamMappingEntry {
            stream_id: stream_id.to_string(),
            alias: alias.to_string(),
            role: role.to_string(),
        }
    }

    fn make_objective(id: &str, stream: &str, metric: &str) -> ObjectiveSyncEntry {
        ObjectiveSyncEntry {
            objective_id: id.to_string(),
            description: Some(format!("Objective {}", id)),
            target_stream: stream.to_string(),
            target_metric: metric.to_string(),
            condition: "below".to_string(),
            threshold: 50.0,
            threshold_upper: None,
            unit: Some("ug/m3".to_string()),
            priority: "primary".to_string(),
        }
    }

    fn make_constraint(id: &str, stream: &str, metric: &str) -> ConstraintSyncEntry {
        ConstraintSyncEntry {
            constraint_id: id.to_string(),
            description: Some(format!("Constraint {}", id)),
            constraint_stream: stream.to_string(),
            constraint_metric: metric.to_string(),
            condition: "below".to_string(),
            threshold: 100.0,
            unit: Some("dB".to_string()),
        }
    }

    fn opts() -> SyncOptions {
        SyncOptions { dry_run: false }
    }

    // -----------------------------------------------------------------------
    // 1. test_sync_empty_domains
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_empty_domains() {
        let db = MockDbClient::new();
        let report = sync_domains(&[], &db, &opts()).await.unwrap();

        assert_eq!(report.items_processed, 0);
        assert_eq!(report.items_created, 0);

        let queries = db.query_strings();
        assert_eq!(queries.len(), 2, "Should only have BEGIN+COMMIT");
        assert_eq!(queries[0], "BEGIN");
        assert_eq!(queries[1], "COMMIT");
    }

    // -----------------------------------------------------------------------
    // 2. test_sync_single_domain_upsert
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_single_domain_upsert() {
        let db = MockDbClient::new();
        let domain = make_minimal_domain("indoor-air-quality");
        sync_domains(&[domain], &db, &opts()).await.unwrap();

        let upserts = db.calls_starting_with("INSERT INTO data_dictionary.domains");
        assert_eq!(upserts.len(), 1);
        assert!(
            upserts[0].query.contains("ON CONFLICT"),
            "Domain INSERT must use ON CONFLICT (UPSERT)"
        );
        assert_eq!(
            upserts[0].params.len(),
            4,
            "UPSERT_DOMAIN needs 4 params: domain_id, description, stream_count, config_path"
        );
    }

    // -----------------------------------------------------------------------
    // 3. test_sync_domain_streams_delete_insert
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_domain_streams_delete_insert() {
        let db = MockDbClient::new();
        let mut domain = make_minimal_domain("test");
        domain.streams = vec![
            make_stream_mapping("air-quality", "aq", "primary"),
            make_stream_mapping("outdoor-weather", "weather", "reference"),
        ];

        sync_domains(&[domain], &db, &opts()).await.unwrap();

        let deletes = db.calls_starting_with("DELETE FROM data_dictionary.domain_streams");
        assert_eq!(
            deletes.len(),
            1,
            "Should DELETE domain_streams once per domain"
        );

        let inserts = db.calls_starting_with("INSERT INTO data_dictionary.domain_streams");
        assert_eq!(inserts.len(), 2, "Should INSERT 2 stream mappings");

        // DELETE must come before INSERTs
        let queries = db.query_strings();
        let delete_pos = queries
            .iter()
            .position(|q| q.starts_with("DELETE FROM data_dictionary.domain_streams"))
            .unwrap();
        let first_insert_pos = queries
            .iter()
            .position(|q| q.starts_with("INSERT INTO data_dictionary.domain_streams"))
            .unwrap();
        assert!(
            delete_pos < first_insert_pos,
            "DELETE domain_streams must precede INSERT"
        );
    }

    // -----------------------------------------------------------------------
    // 4. test_sync_domain_streams_values
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_domain_streams_values() {
        let db = MockDbClient::new();
        let mut domain = make_minimal_domain("test");
        domain.streams = vec![make_stream_mapping("air-quality", "aq", "primary")];

        sync_domains(&[domain], &db, &opts()).await.unwrap();

        let inserts = db.calls_starting_with("INSERT INTO data_dictionary.domain_streams");
        assert_eq!(inserts.len(), 1);
        assert!(
            inserts[0].query.contains("$1, $2, $3, $4"),
            "INSERT_DOMAIN_STREAM must use $1-$4 placeholders"
        );
        assert_eq!(
            inserts[0].params.len(),
            4,
            "INSERT_DOMAIN_STREAM needs 4 params: domain_id, stream_id, alias, role"
        );
    }

    // -----------------------------------------------------------------------
    // 5. test_sync_objectives_delete_insert
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_objectives_delete_insert() {
        let db = MockDbClient::new();
        let mut domain = make_minimal_domain("test");
        domain.objectives = vec![
            make_objective("obj-pm25", "air-quality", "pm25"),
            make_objective("obj-co2", "air-quality", "co2"),
        ];

        sync_domains(&[domain], &db, &opts()).await.unwrap();

        let deletes = db.calls_starting_with("DELETE FROM data_dictionary.objectives");
        assert_eq!(deletes.len(), 1, "Should DELETE objectives once per domain");

        let inserts = db.calls_starting_with("INSERT INTO data_dictionary.objectives");
        assert_eq!(inserts.len(), 2, "Should INSERT 2 objectives");

        // DELETE must come before INSERTs
        let queries = db.query_strings();
        let delete_pos = queries
            .iter()
            .position(|q| q.starts_with("DELETE FROM data_dictionary.objectives"))
            .unwrap();
        let first_insert_pos = queries
            .iter()
            .position(|q| q.starts_with("INSERT INTO data_dictionary.objectives"))
            .unwrap();
        assert!(
            delete_pos < first_insert_pos,
            "DELETE objectives must precede INSERT"
        );
    }

    // -----------------------------------------------------------------------
    // 6. test_sync_objective_values
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_objective_values() {
        let db = MockDbClient::new();
        let mut domain = make_minimal_domain("test");
        domain.objectives = vec![make_objective("obj-pm25", "air-quality", "pm25")];

        sync_domains(&[domain], &db, &opts()).await.unwrap();

        let inserts = db.calls_starting_with("INSERT INTO data_dictionary.objectives");
        assert_eq!(inserts.len(), 1);
        assert_eq!(
            inserts[0].params.len(),
            10,
            "INSERT_OBJECTIVE needs 10 params ($1-$10)"
        );
    }

    // -----------------------------------------------------------------------
    // 7. test_sync_objective_between_condition
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_objective_between_condition() {
        let db = MockDbClient::new();
        let mut domain = make_minimal_domain("test");
        let mut obj = make_objective("obj-temp", "air-quality", "temperature");
        obj.condition = "between".to_string();
        obj.threshold = 18.0;
        obj.threshold_upper = Some(60.0);
        domain.objectives = vec![obj];

        sync_domains(&[domain], &db, &opts()).await.unwrap();

        let inserts = db.calls_starting_with("INSERT INTO data_dictionary.objectives");
        assert_eq!(inserts.len(), 1);
        // Param index 7 (0-based) is threshold_upper which should be Some(60.0)
        assert!(
            !inserts[0].params[7].contains("None"),
            "threshold_upper should be Some(60.0), not None: {}",
            inserts[0].params[7]
        );
    }

    // -----------------------------------------------------------------------
    // 8. test_sync_objective_single_condition
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_objective_single_condition() {
        let db = MockDbClient::new();
        let mut domain = make_minimal_domain("test");
        let obj = make_objective("obj-pm25", "air-quality", "pm25");
        // make_objective sets threshold_upper = None by default
        domain.objectives = vec![obj];

        sync_domains(&[domain], &db, &opts()).await.unwrap();

        let inserts = db.calls_starting_with("INSERT INTO data_dictionary.objectives");
        assert_eq!(inserts.len(), 1);
        // Param index 7 (0-based) is threshold_upper which should be None
        assert!(
            inserts[0].params[7].contains("None"),
            "threshold_upper should be None for single condition: {}",
            inserts[0].params[7]
        );
    }

    // -----------------------------------------------------------------------
    // 9. test_sync_constraints_delete_insert
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_constraints_delete_insert() {
        let db = MockDbClient::new();
        let mut domain = make_minimal_domain("test");
        domain.constraints = vec![
            make_constraint("con-noise", "outdoor-weather", "noise_level"),
            make_constraint("con-wind", "outdoor-weather", "wind_speed"),
        ];

        sync_domains(&[domain], &db, &opts()).await.unwrap();

        let deletes = db.calls_starting_with("DELETE FROM data_dictionary.constraints");
        assert_eq!(
            deletes.len(),
            1,
            "Should DELETE constraints once per domain"
        );

        let inserts = db.calls_starting_with("INSERT INTO data_dictionary.constraints");
        assert_eq!(inserts.len(), 2, "Should INSERT 2 constraints");

        // DELETE must come before INSERTs
        let queries = db.query_strings();
        let delete_pos = queries
            .iter()
            .position(|q| q.starts_with("DELETE FROM data_dictionary.constraints"))
            .unwrap();
        let first_insert_pos = queries
            .iter()
            .position(|q| q.starts_with("INSERT INTO data_dictionary.constraints"))
            .unwrap();
        assert!(
            delete_pos < first_insert_pos,
            "DELETE constraints must precede INSERT"
        );
    }

    // -----------------------------------------------------------------------
    // 10. test_sync_constraint_values
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_constraint_values() {
        let db = MockDbClient::new();
        let mut domain = make_minimal_domain("test");
        domain.constraints = vec![make_constraint(
            "con-noise",
            "outdoor-weather",
            "noise_level",
        )];

        sync_domains(&[domain], &db, &opts()).await.unwrap();

        let inserts = db.calls_starting_with("INSERT INTO data_dictionary.constraints");
        assert_eq!(inserts.len(), 1);
        assert_eq!(
            inserts[0].params.len(),
            8,
            "INSERT_CONSTRAINT needs 8 params ($1-$8)"
        );
    }

    // -----------------------------------------------------------------------
    // 11. test_sync_no_constraints
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_no_constraints() {
        let db = MockDbClient::new();
        let domain = make_minimal_domain("test");
        // domain has empty constraints vec by default

        sync_domains(&[domain], &db, &opts()).await.unwrap();

        // DELETE should still be issued for the domain
        let deletes = db.calls_starting_with("DELETE FROM data_dictionary.constraints");
        assert_eq!(
            deletes.len(),
            1,
            "DELETE constraints should still be issued even with no constraints"
        );

        // No INSERT should be issued
        let inserts = db.calls_starting_with("INSERT INTO data_dictionary.constraints");
        assert_eq!(inserts.len(), 0, "No INSERT constraints when vec is empty");
    }

    // -----------------------------------------------------------------------
    // 12. test_sync_transaction_wrapping
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_transaction_wrapping() {
        let db = MockDbClient::new();
        let domain = make_minimal_domain("test");
        sync_domains(&[domain], &db, &opts()).await.unwrap();

        let queries = db.query_strings();
        assert_eq!(
            queries.first().unwrap(),
            "BEGIN",
            "First query must be BEGIN"
        );
        assert_eq!(
            queries.last().unwrap(),
            "COMMIT",
            "Last query must be COMMIT"
        );
    }

    // -----------------------------------------------------------------------
    // 13. test_sync_fk_ordering
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_fk_ordering() {
        let db = MockDbClient::new();
        let mut domain = make_minimal_domain("test");
        domain.streams = vec![make_stream_mapping("air-quality", "aq", "primary")];
        domain.objectives = vec![make_objective("obj-pm25", "air-quality", "pm25")];
        domain.constraints = vec![make_constraint("con-noise", "outdoor-weather", "noise")];

        sync_domains(&[domain], &db, &opts()).await.unwrap();

        let queries = db.query_strings();

        // UPSERT domain must come before any DELETE of children
        let upsert_pos = queries
            .iter()
            .position(|q| q.starts_with("INSERT INTO data_dictionary.domains"))
            .expect("Should have UPSERT domain");
        let delete_streams_pos = queries
            .iter()
            .position(|q| q.starts_with("DELETE FROM data_dictionary.domain_streams"))
            .expect("Should have DELETE domain_streams");
        let delete_objectives_pos = queries
            .iter()
            .position(|q| q.starts_with("DELETE FROM data_dictionary.objectives"))
            .expect("Should have DELETE objectives");
        let delete_constraints_pos = queries
            .iter()
            .position(|q| q.starts_with("DELETE FROM data_dictionary.constraints"))
            .expect("Should have DELETE constraints");

        assert!(
            upsert_pos < delete_streams_pos,
            "UPSERT domain must precede DELETE domain_streams"
        );
        assert!(
            upsert_pos < delete_objectives_pos,
            "UPSERT domain must precede DELETE objectives"
        );
        assert!(
            upsert_pos < delete_constraints_pos,
            "UPSERT domain must precede DELETE constraints"
        );
    }

    // -----------------------------------------------------------------------
    // 14. test_sync_multi_domain
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_multi_domain() {
        let db = MockDbClient::new();
        let mut domain1 = make_minimal_domain("indoor-air");
        domain1.streams = vec![make_stream_mapping("air-quality", "aq", "primary")];

        let mut domain2 = make_minimal_domain("outdoor-env");
        domain2.streams = vec![make_stream_mapping("outdoor-weather", "weather", "primary")];

        sync_domains(&[domain1, domain2], &db, &opts())
            .await
            .unwrap();

        // Should have 2 domain UPSERTs
        let upserts = db.calls_starting_with("INSERT INTO data_dictionary.domains");
        assert_eq!(upserts.len(), 2, "Should UPSERT 2 domains");

        // Should have 2 DELETE domain_streams (one per domain)
        let deletes = db.calls_starting_with("DELETE FROM data_dictionary.domain_streams");
        assert_eq!(
            deletes.len(),
            2,
            "Should DELETE domain_streams once per domain"
        );

        // Should have 2 INSERT domain_streams (one per domain)
        let inserts = db.calls_starting_with("INSERT INTO data_dictionary.domain_streams");
        assert_eq!(inserts.len(), 2, "Should INSERT 2 stream mappings total");
    }

    // -----------------------------------------------------------------------
    // 15. test_sync_report_counts
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_report_counts() {
        let db = MockDbClient::new();
        let mut domain = make_minimal_domain("test");
        domain.streams = vec![
            make_stream_mapping("aq", "aq", "primary"),
            make_stream_mapping("weather", "weather", "reference"),
        ];
        domain.objectives = vec![make_objective("obj-pm25", "aq", "pm25")];
        domain.constraints = vec![make_constraint("con-noise", "weather", "noise")];

        let report = sync_domains(&[domain], &db, &opts()).await.unwrap();

        assert_eq!(report.entity, "domain");
        assert_eq!(report.items_processed, 1);
        // items_created = 1 domain + 2 streams + 1 objective + 1 constraint = 5
        assert_eq!(report.items_created, 5);
        assert!(report.errors.is_empty());
    }

    // -----------------------------------------------------------------------
    // 16. test_dry_run_no_sql
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_dry_run_no_sql() {
        let db = MockDbClient::new();
        let mut domain = make_minimal_domain("test");
        domain.streams = vec![make_stream_mapping("aq", "aq", "primary")];
        domain.objectives = vec![make_objective("obj-pm25", "aq", "pm25")];

        let dry_opts = SyncOptions { dry_run: true };
        let report = sync_domains(&[domain], &db, &dry_opts).await.unwrap();

        assert!(db.calls().is_empty(), "Dry run must not execute any SQL");

        // Report should still have counts
        assert_eq!(report.items_processed, 1);
        // items_created = 1 domain + 1 stream + 1 objective + 0 constraints = 3
        assert_eq!(report.items_created, 3);
        assert_eq!(report.duration, std::time::Duration::ZERO);
    }

    // -----------------------------------------------------------------------
    // 17. test_sync_domain_error_collected
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_domain_error_collected() {
        let db = FailingMockDbClient::new();
        let domain = make_minimal_domain("failing-domain");

        let report = sync_domains(&[domain], &db, &opts()).await.unwrap();

        assert_eq!(report.items_processed, 1);
        assert_eq!(
            report.errors.len(),
            1,
            "Should collect 1 error for the failing domain"
        );
        assert_eq!(report.errors[0].item, "failing-domain");
        assert!(report.errors[0].message.contains("simulated DB failure"));
    }

    // -----------------------------------------------------------------------
    // 18. test_sync_all_sql_parameterized
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_sync_all_sql_parameterized() {
        let db = MockDbClient::new();
        let mut domain = make_minimal_domain("test");
        domain.streams = vec![make_stream_mapping("aq", "aq", "primary")];
        domain.objectives = vec![make_objective("obj-pm25", "aq", "pm25")];
        domain.constraints = vec![make_constraint("con-noise", "weather", "noise")];

        sync_domains(&[domain], &db, &opts()).await.unwrap();

        let calls = db.calls();
        for call in &calls {
            // Skip BEGIN/COMMIT
            if call.query == "BEGIN" || call.query == "COMMIT" {
                continue;
            }
            // DELETE statements have $1 for the WHERE clause
            if call.query.starts_with("DELETE") {
                assert!(
                    call.query.contains("$1"),
                    "DELETE must use parameterized WHERE: {}",
                    call.query
                );
                continue;
            }
            // INSERT/UPDATE must use $1 and contain no literal values in SQL
            assert!(
                call.query.contains("$1"),
                "All INSERT/UPDATE must use $1 placeholder: {}",
                call.query
            );
            // Verify no single-quoted literal values in VALUES clause
            // (parameterized queries use $N, not 'literal')
            if let Some(values_part) = call.query.split("VALUES").nth(1) {
                assert!(
                    !values_part.contains("'"),
                    "VALUES clause must not contain literal strings (use $N): {}",
                    call.query
                );
            }
        }
    }
}
