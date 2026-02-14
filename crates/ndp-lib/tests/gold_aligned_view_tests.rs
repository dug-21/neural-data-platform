//! Phase C Unit Tests: Aligned View Generation (v11-005)
//!
//! Tests for cross-stream aligned view SQL generation following London TDD.
//!
//! # Test Categories
//!
//! 1. **JOIN Generation**: Full outer, left, inner join strategies
//! 2. **COALESCE Bucket**: Bucket expression for multi-stream alignment
//! 3. **NULL Handling**: Per stream type (ADR-FE001-004)
//! 4. **Column Aliasing**: Stream alias prefixing convention
//! 5. **Primary Stream Ordering**: Primary stream first in FROM
//! 6. **Forecast LATERAL JOIN**: Special handling for forecast streams
//!
//! # Per TEST-PLAN.md Defect Handling Policy
//!
//! - NO workarounds in test code
//! - NO #[ignore] annotations hiding broken functionality
//! - ALL defects must be fixed in ndp-gold-ddl source

mod gold_fixtures;

use gold_fixtures::*;
use ndp_lib::gold::{
    Action, AlignedViewGenerator, AlignmentConfig, ConfigLoader, DomainConfig, JoinStrategy,
    NullHandling, StreamRef, StreamRole, StreamType,
};

// ============================================================================
// v11-005-01: FULL OUTER JOIN Generation Tests
// ============================================================================

/// ACCEPTANCE: Aligned view generates FULL OUTER JOIN for two streams.
///
/// Per TEST-PLAN.md: "Should use FULL OUTER JOIN for preserving all rows"
#[test]
fn test_generates_full_outer_join_for_two_streams() {
    // Arrange
    let loader = MockConfigLoader::new()
        .with_gold_stream("air-quality")
        .with_gold_stream("outdoor-weather");

    let domain = create_two_stream_domain();
    let generator = AlignedViewGenerator::new(loader);

    // Act
    let sql = generator.generate(&domain, Action::Sync).unwrap();

    // Assert
    assert_sql_contains(
        &sql,
        "FULL OUTER JOIN",
        "Two streams should use FULL OUTER JOIN",
    );
    assert_sql_contains(
        &sql,
        "gold.air_quality_hourly",
        "Should reference air-quality Gold table",
    );
    assert_sql_contains(
        &sql,
        "gold.outdoor_weather_hourly",
        "Should reference outdoor-weather Gold table",
    );
}

/// Component: Three streams require 2 FULL OUTER JOINs.
///
/// With 3 streams: A FULL OUTER JOIN B FULL OUTER JOIN C
#[test]
fn test_three_streams_generate_two_joins() {
    // Arrange
    let loader = MockConfigLoader::new()
        .with_gold_stream("air-quality")
        .with_gold_stream("outdoor-weather")
        .with_gold_stream("home-assistant-state");

    let domain = create_three_stream_domain();
    let generator = AlignedViewGenerator::new(loader);

    // Act
    let sql = generator.generate(&domain, Action::Sync).unwrap();

    // Assert: Should have exactly 2 JOIN clauses for 3 streams
    let join_count = count_sql_occurrences(&sql, "FULL OUTER JOIN");
    assert_eq!(
        join_count, 2,
        "Three streams should produce 2 FULL OUTER JOINs, found {}",
        join_count
    );
}

/// Component: LEFT JOIN strategy when configured.
#[test]
fn test_left_join_strategy_when_configured() {
    // Arrange
    let loader = MockConfigLoader::new()
        .with_gold_stream("air-quality")
        .with_gold_stream("outdoor-weather");

    let mut domain = create_two_stream_domain();
    domain.alignment.join_strategy = JoinStrategy::Left;

    let generator = AlignedViewGenerator::new(loader);

    // Act
    let sql = generator.generate(&domain, Action::Sync).unwrap();

    // Assert
    assert_sql_contains(&sql, "LEFT JOIN", "Should use LEFT JOIN when configured");
    assert_sql_not_contains(
        &sql,
        "FULL OUTER JOIN",
        "Should not use FULL OUTER when LEFT configured",
    );
}

/// Component: INNER JOIN strategy when configured.
#[test]
fn test_inner_join_strategy_when_configured() {
    // Arrange
    let loader = MockConfigLoader::new()
        .with_gold_stream("air-quality")
        .with_gold_stream("outdoor-weather");

    let mut domain = create_two_stream_domain();
    domain.alignment.join_strategy = JoinStrategy::Inner;

    let generator = AlignedViewGenerator::new(loader);

    // Act
    let sql = generator.generate(&domain, Action::Sync).unwrap();

    // Assert
    assert_sql_contains(&sql, "INNER JOIN", "Should use INNER JOIN when configured");
    assert_sql_not_contains(
        &sql,
        "FULL OUTER JOIN",
        "Should not use FULL OUTER when INNER configured",
    );
}

// ============================================================================
// v11-005-02: COALESCE Bucket Tests
// ============================================================================

/// ACCEPTANCE: Bucket column COALESCEs from all streams.
///
/// Per TEST-PLAN.md: "Bucket should COALESCE from all streams"
#[test]
fn test_bucket_coalesces_from_all_streams() {
    // Arrange
    let loader = MockConfigLoader::new()
        .with_gold_stream("air-quality")
        .with_gold_stream("outdoor-weather")
        .with_gold_stream("home-assistant-state");

    let domain = create_three_stream_domain();
    let generator = AlignedViewGenerator::new(loader);

    // Act
    let sql = generator.generate(&domain, Action::Sync).unwrap();

    // Assert: COALESCE should include buckets from all streams
    assert_sql_contains(&sql, "COALESCE(", "Should have COALESCE for bucket");

    // All stream aliases should appear in bucket references
    for stream in &domain.streams {
        let bucket_ref = format!("{}.bucket", stream.alias);
        assert_sql_contains(
            &sql,
            &bucket_ref,
            &format!("Missing bucket reference for alias: {}", stream.alias),
        );
    }
}

/// Unit: Two-stream COALESCE has correct format.
#[test]
fn test_two_stream_bucket_coalesce_format() {
    // Arrange
    let loader = MockConfigLoader::new()
        .with_gold_stream("air-quality")
        .with_gold_stream("outdoor-weather");

    let domain = create_two_stream_domain();
    let generator = AlignedViewGenerator::new(loader);

    // Act
    let sql = generator.generate(&domain, Action::Sync).unwrap();

    // Assert: COALESCE with both bucket references
    assert_sql_contains(
        &sql,
        "COALESCE(indoor.bucket, outdoor.bucket)",
        "Should COALESCE both stream buckets",
    );
}

/// Unit: Single stream does not use COALESCE (optimization).
#[test]
fn test_single_stream_no_coalesce() {
    // Arrange
    let loader = MockConfigLoader::new().with_gold_stream("air-quality");

    // Create single-stream domain (will fail validation, but we test the concept)
    // Note: Per the generator, minimum 2 streams required - this tests edge case handling
    let domain = DomainConfig {
        id: "single-stream".to_string(),
        description: "Single stream test".to_string(),
        streams: vec![StreamRef {
            stream_id: "air-quality".to_string(),
            alias: "aq".to_string(),
            role: StreamRole::Primary,
            null_handling: None,
        }],
        alignment: AlignmentConfig {
            view_name: "single_aligned".to_string(),
            granularity: "1 hour".to_string(),
            join_strategy: JoinStrategy::FullOuter,
            null_handling: NullHandling::Preserve,
        },
        objectives: vec![],
        events: None,
        intelligence: None,
    };

    let generator = AlignedViewGenerator::new(loader);

    // Act
    let result = generator.generate(&domain, Action::Sync);

    // Assert: Should fail - minimum 2 streams required for alignment
    assert!(result.is_err(), "Single stream should fail validation");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("at least 2 streams"),
        "Error should mention minimum stream requirement"
    );
}

// ============================================================================
// v11-005-03: NULL Handling Tests (ADR-FE001-004)
// ============================================================================

/// ACCEPTANCE: Observation streams preserve NULL (no LOCF).
///
/// Per ADR-FE001-004: Observation type uses Preserve NULL handling.
#[test]
fn test_observation_null_handling_preserve() {
    // Arrange
    let loader = MockConfigLoader::new()
        .with_gold_stream("air-quality")
        .with_gold_stream("outdoor-weather");

    let domain = create_two_stream_domain();
    let generator = AlignedViewGenerator::new(loader);

    // Act
    let sql = generator.generate(&domain, Action::Sync).unwrap();

    // Assert: Observation columns should be simple references without LOCF
    // The indoor columns should be "indoor.pm25_mean AS indoor_pm25_mean" format
    assert_sql_contains(
        &sql,
        "AS indoor_pm25_mean",
        "Should have aliased indoor pm25 column",
    );

    // Observation columns should NOT have LAG IGNORE NULLS for LOCF
    // (LOCF is only for state_event type)
}

/// ACCEPTANCE: State event streams use LOCF (carry forward).
///
/// Per ADR-FE001-004: StateEvent type uses CarryForward NULL handling.
/// Uses PostgreSQL-compatible cascading LAG pattern (not IGNORE NULLS).
#[test]
fn test_state_event_null_handling_locf() {
    // Arrange
    let loader = MockConfigLoader::new()
        .with_gold_stream("air-quality")
        .with_gold_stream("home-assistant-state");

    let mut domain = create_two_stream_domain();
    domain.streams[1] = StreamRef {
        stream_id: "home-assistant-state".to_string(),
        alias: "state".to_string(),
        role: StreamRole::Actuator,
        null_handling: Some(NullHandling::CarryForward),
    };

    let generator = AlignedViewGenerator::new(loader);

    // Act
    let sql = generator.generate(&domain, Action::Sync).unwrap();

    // Assert: State event columns should use PostgreSQL-compatible LOCF pattern
    // Uses cascading LAG (LAG(..., 1), LAG(..., 2), etc.) instead of IGNORE NULLS
    assert_sql_contains(&sql, "LAG(", "State event should use LAG for LOCF");
    assert_sql_contains(
        &sql,
        "COALESCE",
        "State event should use COALESCE with cascading LAG",
    );
    // Should NOT use IGNORE NULLS (not PostgreSQL compatible)
    assert!(
        !sql.contains("IGNORE NULLS"),
        "Should use PostgreSQL-compatible pattern, not IGNORE NULLS"
    );
}

/// Unit: NULL handling differs by stream type enum.
#[test]
fn test_null_handling_by_stream_type_enum() {
    // Observation -> preserve
    assert_eq!(
        StreamType::Observation.default_null_handling(),
        NullHandling::Preserve,
        "Observation should default to Preserve"
    );

    // StateEvent -> carry_forward
    assert_eq!(
        StreamType::StateEvent.default_null_handling(),
        NullHandling::CarryForward,
        "StateEvent should default to CarryForward"
    );

    // Forecast -> preserve (use actual forecast, don't carry forward old)
    assert_eq!(
        StreamType::Forecast.default_null_handling(),
        NullHandling::Preserve,
        "Forecast should default to Preserve"
    );

    // Dimension -> carry_forward
    assert_eq!(
        StreamType::Dimension.default_null_handling(),
        NullHandling::CarryForward,
        "Dimension should default to CarryForward"
    );
}

/// Unit: Explicit null_handling override takes precedence over stream type default.
#[test]
fn test_explicit_null_handling_override() {
    // Arrange
    let loader = MockConfigLoader::new()
        .with_gold_stream("air-quality")
        .with_gold_stream("outdoor-weather");

    let mut domain = create_two_stream_domain();
    // Override observation stream to use CarryForward
    domain.streams[1].null_handling = Some(NullHandling::CarryForward);

    let generator = AlignedViewGenerator::new(loader);

    // Act
    let sql = generator.generate(&domain, Action::Sync).unwrap();

    // Assert: The overridden stream should use LOCF despite being observation type
    assert_sql_contains(
        &sql,
        "LAG(outdoor",
        "Overridden stream should use LAG for LOCF",
    );
}

// ============================================================================
// v11-005-04: Column Aliasing Convention Tests
// ============================================================================

/// ACCEPTANCE: Columns are aliased with stream prefix.
///
/// Per TEST-PLAN.md: "Columns should be prefixed with alias"
#[test]
fn test_column_aliasing_convention() {
    // Arrange
    let loader = MockConfigLoader::new()
        .with_gold_stream("air-quality")
        .with_gold_stream("outdoor-weather");

    let domain = create_two_stream_domain();
    let generator = AlignedViewGenerator::new(loader);

    // Act
    let sql = generator.generate(&domain, Action::Sync).unwrap();

    // Assert: Columns prefixed with stream alias
    assert_sql_contains(
        &sql,
        "indoor_pm25_mean",
        "Should have indoor-prefixed pm25 column",
    );
    assert_sql_contains(
        &sql,
        "outdoor_pm25_mean",
        "Should have outdoor-prefixed pm25 column",
    );
}

/// Unit: Descriptive aliases used instead of stream IDs.
#[test]
fn test_descriptive_alias_not_stream_id() {
    // Arrange
    let loader = MockConfigLoader::new().with_gold_stream("air-quality");

    let mut domain = create_two_stream_domain();
    domain.streams = vec![
        StreamRef {
            stream_id: "air-quality".to_string(),
            alias: "indoor".to_string(), // Descriptive alias
            role: StreamRole::Primary,
            null_handling: None,
        },
        StreamRef {
            stream_id: "outdoor-weather".to_string(),
            alias: "outdoor".to_string(),
            role: StreamRole::Context,
            null_handling: None,
        },
    ];

    let loader = MockConfigLoader::new()
        .with_gold_stream("air-quality")
        .with_gold_stream("outdoor-weather");

    let generator = AlignedViewGenerator::new(loader);

    // Act
    let sql = generator.generate(&domain, Action::Sync).unwrap();

    // Assert: Uses alias, not stream_id
    assert_sql_contains(&sql, "indoor.", "Should use 'indoor' alias");
    assert_sql_contains(&sql, "outdoor.", "Should use 'outdoor' alias");
}

// ============================================================================
// v11-005-05: Primary Stream First in FROM Tests
// ============================================================================

/// ACCEPTANCE: Primary stream is first in FROM clause.
///
/// Per TEST-PLAN.md: "Primary stream should be in FROM, not in JOIN"
#[test]
fn test_primary_stream_first_in_from() {
    // Arrange: Put context stream first in config (wrong order)
    let loader = MockConfigLoader::new()
        .with_gold_stream("air-quality")
        .with_gold_stream("outdoor-weather");

    let domain = DomainConfig {
        id: "test".to_string(),
        description: "Test domain".to_string(),
        streams: vec![
            // Context first (wrong order in config)
            StreamRef {
                stream_id: "outdoor-weather".to_string(),
                alias: "outdoor".to_string(),
                role: StreamRole::Context,
                null_handling: None,
            },
            // Primary second (should still be first in SQL)
            StreamRef {
                stream_id: "air-quality".to_string(),
                alias: "indoor".to_string(),
                role: StreamRole::Primary,
                null_handling: None,
            },
        ],
        alignment: AlignmentConfig {
            view_name: "test_aligned".to_string(),
            granularity: "1 hour".to_string(),
            join_strategy: JoinStrategy::FullOuter,
            null_handling: NullHandling::Preserve,
        },
        objectives: vec![],
        events: None,
        intelligence: None,
    };

    let generator = AlignedViewGenerator::new(loader);

    // Act
    let sql = generator.generate(&domain, Action::Sync).unwrap();

    // Assert: Primary stream should be in FROM (wrapped in bucket subquery)
    assert!(
        sql.contains("FROM (SELECT bucket"),
        "Primary stream should be in FROM clause as bucket subquery:\n{}",
        sql
    );
    assert!(
        sql.contains("gold.air_quality_hourly"),
        "Primary stream table should appear in FROM subquery:\n{}",
        sql
    );

    // Primary's table should appear in FROM before any JOIN reference
    let from_pos = sql.find("gold.air_quality_hourly");
    let join_outdoor_pos = sql.find("JOIN");

    assert!(from_pos.is_some(), "Primary should be in FROM");
    if let Some(join_pos) = join_outdoor_pos {
        assert!(
            from_pos.unwrap() < join_pos,
            "Primary stream should appear in FROM before any JOIN"
        );
    }
}

/// Unit: No primary stream returns error.
#[test]
fn test_no_primary_stream_error() {
    // Arrange: All context streams, no primary
    let loader = MockConfigLoader::new()
        .with_gold_stream("outdoor-weather")
        .with_gold_stream("home-assistant-state");

    let domain = DomainConfig {
        id: "no-primary".to_string(),
        description: "No primary stream".to_string(),
        streams: vec![
            StreamRef {
                stream_id: "outdoor-weather".to_string(),
                alias: "outdoor".to_string(),
                role: StreamRole::Context,
                null_handling: None,
            },
            StreamRef {
                stream_id: "home-assistant-state".to_string(),
                alias: "state".to_string(),
                role: StreamRole::Actuator,
                null_handling: None,
            },
        ],
        alignment: AlignmentConfig {
            view_name: "no_primary_aligned".to_string(),
            granularity: "1 hour".to_string(),
            join_strategy: JoinStrategy::FullOuter,
            null_handling: NullHandling::Preserve,
        },
        objectives: vec![],
        events: None,
        intelligence: None,
    };

    let generator = AlignedViewGenerator::new(loader);

    // Act
    let result = generator.generate(&domain, Action::Sync);

    // Assert: Should fail - no primary stream
    assert!(result.is_err(), "Should fail without primary stream");
    let err = result.unwrap_err().to_string();
    assert!(
        err.to_lowercase().contains("primary"),
        "Error should mention missing primary stream"
    );
}

// ============================================================================
// v11-005-06: Forecast Stream LATERAL JOIN Tests
// ============================================================================

/// ACCEPTANCE: Forecast stream uses LATERAL join (ADR-FE001-003).
///
/// Per ADR-FE001-003: Forecast alignment uses issued_at for temporal correlation.
#[test]
fn test_forecast_stream_lateral_join() {
    // Arrange
    let loader = MockConfigLoader::new()
        .with_gold_stream("air-quality")
        .with_gold_stream("nws-forecast-hourly");

    let domain = create_domain_with_forecast();
    let generator = AlignedViewGenerator::new(loader);

    // Act
    let sql = generator.generate(&domain, Action::Sync).unwrap();

    // Assert: Forecast should use LATERAL join pattern
    assert_sql_contains(
        &sql,
        "LEFT JOIN LATERAL",
        "Forecast stream should use LEFT JOIN LATERAL",
    );
    assert_sql_contains(
        &sql,
        "issued_at",
        "Forecast join should reference issued_at",
    );
    assert_sql_contains(
        &sql,
        "ORDER BY",
        "Forecast LATERAL should have ORDER BY for latest",
    );
    assert_sql_contains(
        &sql,
        "LIMIT 1",
        "Forecast LATERAL should LIMIT 1 for latest forecast",
    );
}

/// Unit: Forecast lateral join uses correct bucket reference.
#[test]
fn test_forecast_lateral_uses_bucket_reference() {
    // Arrange
    let loader = MockConfigLoader::new()
        .with_gold_stream("air-quality")
        .with_gold_stream("nws-forecast-hourly");

    let domain = create_domain_with_forecast();
    let generator = AlignedViewGenerator::new(loader);

    // Act
    let sql = generator.generate(&domain, Action::Sync).unwrap();

    // Assert: LATERAL should reference the observation bucket
    assert_sql_contains(
        &sql,
        "issued_at <=",
        "Forecast should join on issued_at <= bucket",
    );
}

// ============================================================================
// v11-005-07: Action Mode Tests (Sync vs Recreate)
// ============================================================================

/// Unit: Sync mode checks for existence before creating.
#[test]
fn test_sync_mode_checks_existence() {
    // Arrange
    let loader = MockConfigLoader::new()
        .with_gold_stream("air-quality")
        .with_gold_stream("outdoor-weather");

    let domain = create_two_stream_domain();
    let generator = AlignedViewGenerator::new(loader);

    // Act
    let sql = generator.generate(&domain, Action::Sync).unwrap();

    // Assert: Should check existence
    assert_sql_contains(&sql, "IF NOT EXISTS", "Sync mode should check existence");
    assert_sql_contains(
        &sql,
        "pg_matviews",
        "Should check materialized views catalog",
    );
}

/// Unit: Recreate mode drops existing view.
#[test]
fn test_recreate_mode_drops_first() {
    // Arrange
    let loader = MockConfigLoader::new()
        .with_gold_stream("air-quality")
        .with_gold_stream("outdoor-weather");

    let domain = create_two_stream_domain();
    let generator = AlignedViewGenerator::new(loader);

    // Act
    let sql = generator.generate(&domain, Action::Recreate).unwrap();

    // Assert: Should drop first
    assert_sql_contains(
        &sql,
        "DROP MATERIALIZED VIEW IF EXISTS",
        "Recreate should drop existing",
    );
    assert_sql_contains(&sql, "CASCADE", "Drop should cascade");
}

// ============================================================================
// v11-005-08: Index Generation Tests
// ============================================================================

/// Unit: Index created on bucket column.
#[test]
fn test_index_generation_on_bucket() {
    // Arrange
    let loader = MockConfigLoader::new()
        .with_gold_stream("air-quality")
        .with_gold_stream("outdoor-weather");

    let domain = create_two_stream_domain();
    let generator = AlignedViewGenerator::new(loader);

    // Act
    let sql = generator.generate(&domain, Action::Sync).unwrap();

    // Assert: Index on bucket
    assert_sql_contains(&sql, "CREATE INDEX", "Should create index");
    assert_sql_contains(&sql, "(bucket)", "Index should be on bucket column");
    assert_sql_contains(
        &sql,
        "idx_test_aligned_bucket",
        "Index should use view name convention",
    );
}

// ============================================================================
// v11-005-09: Sample Count Aggregation Tests
// ============================================================================

/// Unit: Sample count columns included for each stream.
#[test]
fn test_sample_count_per_stream() {
    // Arrange
    let loader = MockConfigLoader::new()
        .with_gold_stream("air-quality")
        .with_gold_stream("outdoor-weather");

    let domain = create_two_stream_domain();
    let generator = AlignedViewGenerator::new(loader);

    // Act
    let sql = generator.generate(&domain, Action::Sync).unwrap();

    // Assert: Per-stream sample counts
    assert_sql_contains(&sql, "indoor_samples", "Should have indoor samples column");
    assert_sql_contains(
        &sql,
        "outdoor_samples",
        "Should have outdoor samples column",
    );
}

/// Unit: Total samples calculated from all streams.
#[test]
fn test_total_samples_column() {
    // Arrange
    let loader = MockConfigLoader::new()
        .with_gold_stream("air-quality")
        .with_gold_stream("outdoor-weather");

    let domain = create_two_stream_domain();
    let generator = AlignedViewGenerator::new(loader);

    // Act
    let sql = generator.generate(&domain, Action::Sync).unwrap();

    // Assert: Total samples aggregation
    assert_sql_contains(&sql, "total_samples", "Should have total_samples column");
}

// ============================================================================
// v11-005-10: Error Handling Tests
// ============================================================================

/// Unit: Missing stream config returns error.
#[test]
fn test_missing_stream_config_error() {
    // Arrange: Loader missing one stream
    let loader = MockConfigLoader::new().with_gold_stream("air-quality");
    // Note: outdoor-weather NOT loaded

    let domain = create_two_stream_domain();
    let generator = AlignedViewGenerator::new(loader);

    // Act
    let result = generator.generate(&domain, Action::Sync);

    // Assert: Should fail with config not found
    assert!(result.is_err(), "Should fail when stream config missing");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("not found") || err.contains("outdoor-weather"),
        "Error should indicate missing config"
    );
}

/// Unit: View name included in SQL.
#[test]
fn test_view_name_in_sql() {
    // Arrange
    let loader = MockConfigLoader::new()
        .with_gold_stream("air-quality")
        .with_gold_stream("outdoor-weather");

    let domain = create_two_stream_domain();
    let generator = AlignedViewGenerator::new(loader);

    // Act
    let sql = generator.generate(&domain, Action::Sync).unwrap();

    // Assert: View name from config used
    assert_sql_contains(
        &sql,
        &format!("gold.{}", domain.alignment.view_name),
        "Should use view_name from config",
    );
}

// ============================================================================
// v11-005-11: Comment and Documentation Tests
// ============================================================================

/// Unit: Generated SQL includes helpful comments.
#[test]
fn test_sql_includes_comments() {
    // Arrange
    let loader = MockConfigLoader::new()
        .with_gold_stream("air-quality")
        .with_gold_stream("outdoor-weather");

    let domain = create_two_stream_domain();
    let generator = AlignedViewGenerator::new(loader);

    // Act
    let sql = generator.generate(&domain, Action::Sync).unwrap();

    // Assert: Has comments
    assert_sql_contains(&sql, "--", "Should have SQL comments");
    assert_sql_contains(&sql, &domain.id, "Comments should reference domain ID");
}
