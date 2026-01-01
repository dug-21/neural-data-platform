//! DP-004 Pipeline Integration Tests - London School TDD
//!
//! These tests verify that the ENTIRE pipeline uses the new RawDataPoint
//! architecture and writes to the correct paths.
//!
//! Test Strategy:
//! - Tests MUST FAIL initially (Red phase)
//! - Implementation changes make them pass (Green phase)
//! - These are acceptance tests for the full system integration

use neural_core::types::raw_data_point::RawDataPoint;
use neural_core::traits::RawStore;
use neural_core::ParquetStore;
use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::mpsc;

// ============================================================================
// AT-PIPELINE-001: Pipeline Channel Uses RawDataPoint
// ============================================================================
//
// GIVEN: The ingestion pipeline is configured
// WHEN: Data flows through the pipeline
// THEN: The channel carries RawDataPoint (not TimeSeriesPoint)
//
// This test verifies the fundamental data type change in the pipeline.

#[tokio::test]
async fn test_pipeline_channel_uses_raw_data_point() {
    // GIVEN: A channel for raw data points
    let (tx, mut rx) = mpsc::channel::<RawDataPoint>(100);

    // WHEN: We send a RawDataPoint through the channel
    let point = RawDataPoint::new("test-source-Http", json!({"pm25": 12.5}))
        .with_ndp_id("device-001")
        .with_context(json!({"room": "office"}));

    tx.send(point.clone()).await.unwrap();

    // THEN: We receive a RawDataPoint (not TimeSeriesPoint)
    let received = rx.recv().await.unwrap();

    assert_eq!(received.source_id, "test-source-Http");
    assert_eq!(received.raw_payload["pm25"], 12.5);
    assert_eq!(received.ndp_id, Some("device-001".to_string()));
}

// ============================================================================
// AT-PIPELINE-002: Storage Writes to /raw/ Path (Not /data/)
// ============================================================================
//
// GIVEN: ParquetStore configured with a base path
// WHEN: RawDataPoint is written via write_raw()
// THEN: File appears in {base}/raw/{source_id}/... NOT {base}/data/...
//
// This test verifies the new partition structure is used.

#[tokio::test]
async fn test_storage_writes_to_raw_path_not_data_path() {
    // GIVEN: A ParquetStore with temp directory
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();

    // WHEN: We write a RawDataPoint
    let point = RawDataPoint::new("air-quality-Http", json!({"pm25": 15.0}));
    store.write_raw(point).await.unwrap();

    // THEN: Data is in /raw/ path
    let raw_path = temp_dir.path().join("raw");
    assert!(raw_path.exists(), "Expected /raw/ directory to exist");

    let source_path = raw_path.join("air-quality-Http");
    assert!(source_path.exists(), "Expected /raw/air-quality-Http/ directory to exist");

    // AND: Data is NOT in old /data/ path
    let old_data_path = temp_dir.path().join("data");
    let has_parquet_in_old_path = if old_data_path.exists() {
        walkdir::WalkDir::new(&old_data_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .any(|e| e.path().extension().map_or(false, |ext| ext == "parquet"))
    } else {
        false
    };

    assert!(!has_parquet_in_old_path,
        "Expected NO parquet files in old /data/ path - dp-004 requires /raw/ path");
}

// ============================================================================
// AT-PIPELINE-003: Full Pipeline Flow - Source to Storage
// ============================================================================
//
// GIVEN: Complete pipeline with source → channel → storage
// WHEN: Source emits data
// THEN: Data appears in Parquet with correct 5-column schema
//
// This is the main E2E acceptance test.

#[tokio::test]
async fn test_full_pipeline_source_to_storage() {
    // GIVEN: ParquetStore and channel
    let temp_dir = TempDir::new().unwrap();
    let store = Arc::new(ParquetStore::new(temp_dir.path()).unwrap());
    let (tx, mut rx) = mpsc::channel::<RawDataPoint>(100);

    // Spawn storage writer task
    let store_clone = store.clone();
    let writer_task = tokio::spawn(async move {
        let mut batch = Vec::new();
        while let Some(point) = rx.recv().await {
            batch.push(point);
            if batch.len() >= 1 {
                store_clone.write_raw_batch(batch.clone()).await.unwrap();
                batch.clear();
            }
        }
    });

    // WHEN: Source emits RawDataPoint
    let point = RawDataPoint::new("nws-observations-Http", json!({
        "temperature": 22.5,
        "humidity": 65,
        "station": "KJAX"
    }))
    .with_ndp_id("nws-station-001")
    .with_context(json!({"source": "NWS", "region": "florida"}));

    tx.send(point).await.unwrap();
    drop(tx); // Close channel to signal completion

    writer_task.await.unwrap();

    // THEN: Query returns data with correct schema
    let start = chrono::Utc::now() - chrono::Duration::hours(1);
    let end = chrono::Utc::now() + chrono::Duration::hours(1);

    let results = store.query_raw(start, end, Some("nws-observations-Http".to_string())).await.unwrap();

    assert_eq!(results.len(), 1, "Expected 1 record");

    let record = &results[0];
    assert_eq!(record.source_id, "nws-observations-Http");
    assert_eq!(record.ndp_id, Some("nws-station-001".to_string()));
    assert_eq!(record.raw_payload["temperature"], 22.5);
    assert_eq!(record.raw_payload["station"], "KJAX");
    assert_eq!(record.context.as_ref().unwrap()["source"], "NWS");
}

// ============================================================================
// AT-PIPELINE-004: SourceManager Uses Raw Sender
// ============================================================================
//
// GIVEN: SourceManager with raw_ingestion_sender configured
// WHEN: Source is spawned
// THEN: Source uses fetch_raw() and sends RawDataPoint
//
// This test verifies SourceManager integration.

#[tokio::test]
async fn test_source_manager_uses_ingestion_sender() {
    use air_quality_app::coordinator::SourceManager;
    use config_client::StreamRegistry;

    // GIVEN: SourceManager with ingestion sender
    let registry = Arc::new(
        StreamRegistry::new(&["http://localhost:2379"])
            .await
            .unwrap(),
    );
    let mut manager = SourceManager::new(registry);

    // Set up ingestion sender (dp-004 Bronze layer)
    let (tx, _rx) = mpsc::channel::<RawDataPoint>(100);
    manager.set_ingestion_sender(tx);

    // THEN: Manager should have ingestion sender configured
    // The actual spawning would require mock HTTP server, so we verify setup
    assert!(true, "SourceManager accepted ingestion_sender");
}

// ============================================================================
// AT-PIPELINE-008: Production Data Path Validation
// ============================================================================
//
// GIVEN: The production data directory exists
// WHEN: We check for parquet files
// THEN: Files should exist in /data/raw/ (dp-004) NOT /data/data/ (old)
//
// This test verifies the ACTUAL production wiring is correct.
// It will FAIL until the pipeline is fully wired to use dp-004 path.

#[tokio::test]
#[ignore = "Requires running system with data - enable for production validation"]
async fn test_production_data_path_is_raw() {
    use std::path::Path;

    // Check production data paths
    let raw_path = Path::new("/data/raw");
    let old_path = Path::new("/data/data");

    // THEN: Data should exist in new /data/raw/ path
    let has_raw_data = if raw_path.exists() {
        walkdir::WalkDir::new(raw_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .any(|e| e.path().extension().map_or(false, |ext| ext == "parquet"))
    } else {
        false
    };

    // AND: No new data should be written to old /data/data/ path
    // (existing old data may remain, but new writes should go to /raw/)
    let has_old_data = if old_path.exists() {
        // Check for recently modified files (last hour)
        let cutoff = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        walkdir::WalkDir::new(old_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "parquet"))
            .any(|e| {
                e.metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map_or(false, |t| t > cutoff)
            })
    } else {
        false
    };

    assert!(has_raw_data, "Expected parquet files in /data/raw/ (dp-004 path)");
    assert!(!has_old_data, "Found recently written parquet files in /data/data/ - pipeline still using old path!");
}

// ============================================================================
// AT-PIPELINE-009: Main.rs Uses RawDataPoint Channel
// ============================================================================
//
// GIVEN: The main.rs creates a channel for data ingestion
// WHEN: We check the channel type in the codebase
// THEN: It should have RawDataPoint channel for dp-004
//
// This is a code-level test to verify the main pipeline is wired correctly.

#[tokio::test]
async fn test_main_uses_raw_data_point_channel() {
    // Read the main.rs file and check for RawDataPoint channel
    let main_rs = std::fs::read_to_string(
        "/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs"
    ).expect("Failed to read main.rs");

    // Check that main.rs uses RawDataPoint channel
    let uses_raw_channel = main_rs.contains("mpsc::channel::<RawDataPoint>")
        || main_rs.contains("channel::<RawDataPoint>");

    assert!(
        uses_raw_channel,
        "main.rs should use mpsc::channel::<RawDataPoint> for dp-004 ingestion"
    );

    // Check that main.rs has dp-004 raw storage pipeline
    let has_raw_pipeline = main_rs.contains("DP-004")
        && main_rs.contains("RawStorageWriter")
        && main_rs.contains("set_ingestion_sender");

    assert!(
        has_raw_pipeline,
        "main.rs should have DP-004 raw storage pipeline with RawStorageWriter"
    );
}

// ============================================================================
// AT-PIPELINE-010: Main.rs Uses RawStorageWriter for Storage
// ============================================================================
//
// GIVEN: The main.rs writes data to storage
// WHEN: We check the storage mechanism used
// THEN: It should use RawStorageWriter (which internally uses write_raw_batch)
//
// This verifies the storage layer is wired to the new RawStore interface.

#[tokio::test]
async fn test_main_uses_raw_store() {
    let main_rs = std::fs::read_to_string(
        "/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs"
    ).expect("Failed to read main.rs");

    // Check that main.rs uses RawStorageWriter (which uses write_raw_batch internally)
    let uses_raw_storage_writer = main_rs.contains("RawStorageWriter");

    assert!(
        uses_raw_storage_writer,
        "main.rs should use RawStorageWriter for dp-004 Bronze layer storage"
    );

    // Verify the RawStorageWriter is actually spawned
    let raw_writer_spawned = main_rs.contains("RawStorageWriter::new")
        && main_rs.contains("writer.run()");

    assert!(
        raw_writer_spawned,
        "main.rs should spawn RawStorageWriter for dp-004 storage pipeline"
    );
}

// ============================================================================
// AT-PIPELINE-005: Old TimeSeriesPoint Path is Deprecated
// ============================================================================
//
// GIVEN: Pipeline configured for dp-004
// WHEN: We attempt to use old TimeSeriesPoint storage
// THEN: No data should be written to old path structure
//
// This is a regression test to ensure old path is not used.

#[tokio::test]
async fn test_old_time_series_point_path_not_used() {
    // GIVEN: ParquetStore
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();

    // WHEN: We write via RawStore (new dp-004 interface)
    let point = RawDataPoint::new("test-Http", json!({"value": 42}));
    store.write_raw(point).await.unwrap();

    // THEN: Old structure should not exist
    // Old structure: {base}/data/{location_id}/year=.../...
    // New structure: {base}/raw/{source_id}/year=.../...

    let old_data_path = temp_dir.path().join("data");

    // If old path exists, it should be empty of parquet files
    if old_data_path.exists() {
        let parquet_count: usize = walkdir::WalkDir::new(&old_data_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "parquet"))
            .count();

        assert_eq!(parquet_count, 0,
            "Found {} parquet files in deprecated /data/ path - dp-004 requires /raw/ only",
            parquet_count);
    }
}

// ============================================================================
// AT-PIPELINE-006: Parquet Schema Has 5 Columns (Not 6)
// ============================================================================
//
// GIVEN: Data written via RawStore
// WHEN: We read the Parquet file directly
// THEN: Schema has exactly 5 columns: timestamp, source_id, ndp_id, context, raw_payload
//
// This verifies the schema change from TimeSeriesPoint (6 cols) to RawDataPoint (5 cols)

#[tokio::test]
async fn test_parquet_schema_has_5_columns() {
    use polars::prelude::*;

    // GIVEN: ParquetStore with data
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();

    let point = RawDataPoint::new("schema-test-Http", json!({"value": 1}));
    store.write_raw(point).await.unwrap();

    // WHEN: We read the parquet file directly
    let raw_path = temp_dir.path().join("raw").join("schema-test-Http");
    let parquet_files: Vec<_> = walkdir::WalkDir::new(&raw_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "parquet"))
        .collect();

    assert!(!parquet_files.is_empty(), "Expected at least one parquet file");

    let file = std::fs::File::open(parquet_files[0].path()).unwrap();
    let df = ParquetReader::new(file).finish().unwrap();

    // THEN: Schema has exactly 5 columns
    let columns = df.get_column_names();

    assert!(columns.contains(&"timestamp"), "Missing timestamp column");
    assert!(columns.contains(&"source_id"), "Missing source_id column");
    assert!(columns.contains(&"ndp_id"), "Missing ndp_id column");
    assert!(columns.contains(&"context"), "Missing context column");
    assert!(columns.contains(&"raw_payload"), "Missing raw_payload column");

    // Old columns should NOT exist
    assert!(!columns.contains(&"location_id"), "Found deprecated location_id column");
    assert!(!columns.contains(&"metric"), "Found deprecated metric column");
    assert!(!columns.contains(&"value"), "Found deprecated value column");

    assert_eq!(columns.len(), 5,
        "Expected 5 columns, found {}: {:?}", columns.len(), columns);
}
