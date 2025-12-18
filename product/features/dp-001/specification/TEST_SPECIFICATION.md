# Test Specification: DP-001 - DuckDB Analytics + Grafana

**Feature**: Virtual Silver Layer using DuckDB with Grafana Visualization
**Version**: 1.0
**Date**: 2025-12-18
**Status**: Draft

---

## 1. Test Strategy Overview

### 1.1 Testing Pyramid

```
                    /\
                   /E2E\          5% - End-to-End (Grafana → DuckDB → Parquet)
                  /------\
                 /  Integ \       25% - Integration (DuckDB + Parquet)
                /----------\
               /    Unit    \     70% - Unit (SQL views, data quality)
              /--------------\
```

### 1.2 Test Categories

| Category | Scope | Environment | Execution |
|----------|-------|-------------|-----------|
| **Unit Tests** | SQL view logic, DQ rules | In-memory DuckDB | `cargo test` |
| **Integration Tests** | DuckDB + Parquet files | Local filesystem | `cargo test --ignored` |
| **E2E Tests** | Full stack (Grafana + DuckDB + Parquet) | Docker Compose | Manual or CI |
| **Performance Tests** | Query latency, memory usage | Pi 5 or equivalent | Benchmarks |
| **Deployment Tests** | Container orchestration | Docker Compose | Deployment script |

### 1.3 Test Execution Strategy

- **Unit Tests**: Fast, deterministic, run on every commit
- **Integration Tests**: Run before PR merge
- **E2E Tests**: Run before release deployment
- **Performance Tests**: Run weekly, establish baselines
- **Deployment Tests**: Run during deployment automation

### 1.4 Success Criteria

- Unit test coverage ≥ 70%
- All integration tests pass
- E2E tests validate user workflows
- Performance tests meet latency targets
- Zero critical bugs in production

---

## 2. DuckDB Tests

### T-DB-001: Parquet File Discovery and Loading

**Objective**: Verify DuckDB can discover and load Parquet files from Bronze layer

**Test Cases**:

```rust
#[test]
fn test_parquet_file_discovery() {
    // Arrange
    let bronze_path = "/data/bronze/indoor-air-pm2_5/*.parquet";
    let conn = Connection::open_in_memory().unwrap();

    // Act
    let result = conn.execute(
        &format!("SELECT COUNT(*) FROM read_parquet('{}')", bronze_path)
    );

    // Assert
    assert!(result.is_ok());
    assert!(result.unwrap() > 0, "Should find at least one file");
}

#[test]
fn test_parquet_loading_empty_directory() {
    let bronze_path = "/data/bronze/nonexistent/*.parquet";
    let conn = Connection::open_in_memory().unwrap();

    let result = conn.execute(
        &format!("SELECT * FROM read_parquet('{}')", bronze_path)
    );

    assert!(result.is_err(), "Should error on empty directory");
}

#[test]
fn test_parquet_wildcard_expansion() {
    // Test multiple files are loaded
    let bronze_path = "/data/bronze/indoor-air-pm2_5/*.parquet";
    let conn = Connection::open_in_memory().unwrap();

    let count: i64 = conn.query_row(
        &format!("SELECT COUNT(DISTINCT filename) FROM read_parquet('{}', filename=true)", bronze_path),
        []
    ).unwrap();

    assert!(count > 1, "Should load multiple Parquet files");
}
```

**Expected Behavior**:
- Successfully loads all `.parquet` files matching pattern
- Returns empty result set if no files found (no error)
- Handles multiple files via wildcard expansion

---

### T-DB-002: Schema Inference Correctness

**Objective**: Verify DuckDB correctly infers schemas from Parquet metadata

**Test Cases**:

```rust
#[test]
fn test_schema_inference_types() {
    let conn = setup_test_db_with_sample_data();

    // Act: Query schema information
    let schema = conn.execute("DESCRIBE SELECT * FROM read_parquet('/data/bronze/indoor-air-pm2_5/*.parquet')").unwrap();

    // Assert: Expected columns and types
    assert_eq!(schema.column("timestamp").data_type(), DataType::Timestamp);
    assert_eq!(schema.column("stream_id").data_type(), DataType::Utf8);
    assert_eq!(schema.column("pm2_5").data_type(), DataType::Float64);
    assert_eq!(schema.column("tags").data_type(), DataType::Map);
}

#[test]
fn test_schema_consistency_across_files() {
    // Verify all Parquet files have consistent schema
    let conn = setup_test_db();

    let schemas: Vec<Schema> = conn.execute("
        SELECT DISTINCT typeof(pm2_5), typeof(timestamp)
        FROM read_parquet('/data/bronze/indoor-air-pm2_5/*.parquet', filename=true)
        GROUP BY filename
    ").unwrap();

    assert_eq!(schemas.len(), 1, "All files should have same schema");
}
```

**Expected Behavior**:
- Correctly infers timestamp, string, numeric, and map types
- Handles schema evolution gracefully
- Consistent schema across multiple Parquet files

---

### T-DB-003: Virtual View Creation

**Objective**: Verify SQL views are created correctly with DQ logic

**Test Cases**:

```rust
#[test]
fn test_view_creation() {
    let conn = setup_test_db();

    // Act: Create virtual view
    conn.execute("
        CREATE VIEW silver_indoor_air_pm2_5 AS
        SELECT
            timestamp,
            stream_id,
            pm2_5,
            tags
        FROM read_parquet('/data/bronze/indoor-air-pm2_5/*.parquet')
        WHERE pm2_5 IS NOT NULL
          AND pm2_5 BETWEEN 0 AND 500
    ").unwrap();

    // Assert: View exists
    let views: Vec<String> = conn.query_row("SELECT name FROM duckdb_views()", []).unwrap();
    assert!(views.contains(&"silver_indoor_air_pm2_5".to_string()));
}

#[test]
fn test_view_query_execution() {
    let conn = setup_test_db_with_views();

    // Act: Query view
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM silver_indoor_air_pm2_5", []).unwrap();

    // Assert: Returns results
    assert!(count >= 0);
}
```

**Expected Behavior**:
- Views are created without errors
- Views are queryable
- Views return expected columns and types

---

### T-DB-004: NULL Handling in Views

**Objective**: Verify NULL values are excluded from virtual views

**Test Cases**:

```rust
#[test]
fn test_null_exclusion() {
    let conn = setup_test_db();

    // Arrange: Insert test data with NULLs
    insert_test_data_with_nulls(&conn);

    // Act: Query view
    let null_count: i64 = conn.query_row("
        SELECT COUNT(*) FROM silver_indoor_air_pm2_5 WHERE pm2_5 IS NULL
    ", []).unwrap();

    // Assert: No NULLs in view
    assert_eq!(null_count, 0, "View should exclude NULL pm2_5 values");
}

#[test]
fn test_null_vs_raw_count() {
    let conn = setup_test_db();
    insert_test_data_with_nulls(&conn);

    let raw_count: i64 = conn.query_row("
        SELECT COUNT(*) FROM read_parquet('/data/bronze/indoor-air-pm2_5/*.parquet')
    ", []).unwrap();

    let view_count: i64 = conn.query_row("
        SELECT COUNT(*) FROM silver_indoor_air_pm2_5
    ", []).unwrap();

    assert!(view_count < raw_count, "View should filter out rows");
}
```

**Expected Behavior**:
- NULL values excluded from views
- View count < raw Parquet count if NULLs present
- Other columns with NULL retained if primary field valid

---

### T-DB-005: Range Filtering Logic

**Objective**: Verify data quality range filters work correctly

**Test Cases**:

```rust
#[test]
fn test_pm2_5_range_filter() {
    let conn = setup_test_db();

    // Arrange: Insert out-of-range data
    insert_test_data(&conn, vec![
        ("indoor-air-pm2_5", -10.0),  // Below range
        ("indoor-air-pm2_5", 250.0),  // Valid
        ("indoor-air-pm2_5", 600.0),  // Above range
    ]);

    // Act: Query view
    let results: Vec<f64> = conn.query("SELECT pm2_5 FROM silver_indoor_air_pm2_5", []).unwrap();

    // Assert: Only valid range
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], 250.0);
}

#[test]
fn test_temperature_range_filter() {
    let conn = setup_test_db();

    insert_test_data(&conn, vec![
        ("indoor-air-temperature", -60.0),  // Below range
        ("indoor-air-temperature", 22.5),   // Valid
        ("indoor-air-temperature", 70.0),   // Above range
    ]);

    let results: Vec<f64> = conn.query("SELECT temperature FROM silver_indoor_air_temperature", []).unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0], 22.5);
}

#[test]
fn test_boundary_values_included() {
    let conn = setup_test_db();

    // Test boundary values are INCLUDED
    insert_test_data(&conn, vec![
        ("indoor-air-pm2_5", 0.0),    // Lower boundary
        ("indoor-air-pm2_5", 500.0),  // Upper boundary
    ]);

    let count: i64 = conn.query_row("SELECT COUNT(*) FROM silver_indoor_air_pm2_5", []).unwrap();
    assert_eq!(count, 2, "Boundary values should be included");
}
```

**Expected Behavior**:
- Out-of-range values excluded
- Boundary values included (BETWEEN is inclusive)
- Multiple range filters applied independently

**Data Quality Ranges**:
| Stream | Field | Min | Max |
|--------|-------|-----|-----|
| indoor-air-pm2_5 | pm2_5 | 0 | 500 |
| indoor-air-temperature | temperature | -50 | 60 |
| indoor-air-humidity | humidity | 0 | 100 |
| outdoor-air-aqi | aqi | 1 | 5 |

---

### T-DB-006: Cross-Stream JOIN Correctness

**Objective**: Verify JOINs between virtual views work correctly

**Test Cases**:

```rust
#[test]
fn test_join_pm2_5_and_temperature() {
    let conn = setup_test_db_with_views();

    // Act: JOIN on timestamp (assuming same collection time)
    let results = conn.query("
        SELECT
            pm.timestamp,
            pm.pm2_5,
            temp.temperature
        FROM silver_indoor_air_pm2_5 pm
        INNER JOIN silver_indoor_air_temperature temp
          ON pm.timestamp = temp.timestamp
        LIMIT 10
    ", []).unwrap();

    // Assert: Results have both fields
    assert!(results.len() > 0);
    for row in results {
        assert!(row.pm2_5.is_some());
        assert!(row.temperature.is_some());
    }
}

#[test]
fn test_time_range_join() {
    let conn = setup_test_db_with_views();

    // Act: JOIN with time window (within 5 minutes)
    let results = conn.query("
        SELECT
            pm.timestamp,
            pm.pm2_5,
            temp.temperature
        FROM silver_indoor_air_pm2_5 pm
        LEFT JOIN silver_indoor_air_temperature temp
          ON pm.timestamp BETWEEN temp.timestamp - INTERVAL 5 MINUTE
                               AND temp.timestamp + INTERVAL 5 MINUTE
        WHERE pm.timestamp > NOW() - INTERVAL 7 DAY
    ", []).unwrap();

    assert!(results.len() > 0);
}
```

**Expected Behavior**:
- Exact timestamp JOINs work
- Time range JOINs work with BETWEEN
- LEFT JOIN preserves rows from left table

---

### T-DB-007: Query Performance Benchmarks

**Objective**: Establish baseline query performance metrics

**Test Cases**:

```rust
#[test]
#[ignore] // Run with --ignored
fn bench_7_day_query() {
    let conn = setup_test_db_with_real_data();

    let start = Instant::now();
    let _ = conn.query("
        SELECT * FROM silver_indoor_air_pm2_5
        WHERE timestamp > NOW() - INTERVAL 7 DAY
    ", []).unwrap();
    let duration = start.elapsed();

    assert!(duration < Duration::from_secs(5), "7-day query should complete in < 5s");
}

#[test]
#[ignore]
fn bench_aggregation_query() {
    let conn = setup_test_db_with_real_data();

    let start = Instant::now();
    let _ = conn.query("
        SELECT
            DATE_TRUNC('hour', timestamp) as hour,
            AVG(pm2_5) as avg_pm2_5,
            MAX(pm2_5) as max_pm2_5
        FROM silver_indoor_air_pm2_5
        WHERE timestamp > NOW() - INTERVAL 30 DAY
        GROUP BY hour
        ORDER BY hour
    ", []).unwrap();
    let duration = start.elapsed();

    assert!(duration < Duration::from_secs(15), "30-day aggregation should complete in < 15s");
}

#[test]
#[ignore]
fn bench_join_query() {
    let conn = setup_test_db_with_real_data();

    let start = Instant::now();
    let _ = conn.query("
        SELECT
            pm.timestamp,
            pm.pm2_5,
            temp.temperature,
            hum.humidity
        FROM silver_indoor_air_pm2_5 pm
        LEFT JOIN silver_indoor_air_temperature temp ON pm.timestamp = temp.timestamp
        LEFT JOIN silver_indoor_air_humidity hum ON pm.timestamp = hum.timestamp
        WHERE pm.timestamp > NOW() - INTERVAL 7 DAY
    ", []).unwrap();
    let duration = start.elapsed();

    assert!(duration < Duration::from_secs(10), "Multi-join should complete in < 10s");
}
```

**Performance Targets**:
| Query Type | Dataset | Target Latency |
|------------|---------|----------------|
| Single stream, 7 days | ~10k rows | < 5 seconds |
| Single stream, 30 days | ~40k rows | < 15 seconds |
| Multi-stream JOIN, 7 days | ~30k rows | < 10 seconds |
| Aggregation (hourly), 30 days | ~720 groups | < 15 seconds |

---

## 3. Grafana Tests

### T-GF-001: Container Startup and Health

**Objective**: Verify Grafana container starts successfully and reports healthy

**Test Cases**:

```bash
#!/bin/bash
# test_grafana_startup.sh

test_container_starts() {
    docker-compose up -d grafana
    sleep 5

    status=$(docker-compose ps grafana | grep "Up")
    assert_not_empty "$status" "Grafana container should be running"
}

test_health_endpoint() {
    response=$(curl -s http://localhost:3001/api/health)
    echo "$response" | grep -q '"database":"ok"'
    assert_success "Health endpoint should return ok"
}

test_ready_endpoint() {
    response=$(curl -s http://localhost:3001/api/ready)
    assert_equals "$?" "0" "Ready endpoint should return 200"
}
```

**Expected Behavior**:
- Container starts within 10 seconds
- Health endpoint returns `{"database":"ok"}`
- Ready endpoint returns 200 OK

---

### T-GF-002: Datasource Connection

**Objective**: Verify Grafana can connect to DuckDB datasource

**Test Cases**:

```bash
test_datasource_exists() {
    response=$(curl -s -u admin:admin http://localhost:3001/api/datasources)
    echo "$response" | grep -q '"name":"DuckDB"'
    assert_success "DuckDB datasource should exist"
}

test_datasource_connection() {
    # Test datasource connectivity
    response=$(curl -s -u admin:admin -X POST \
        http://localhost:3001/api/datasources/1/resources/test \
        -H "Content-Type: application/json")

    echo "$response" | grep -q '"status":"success"'
    assert_success "Datasource connection should succeed"
}

test_query_execution() {
    # Execute test query via datasource
    response=$(curl -s -u admin:admin -X POST \
        http://localhost:3001/api/ds/query \
        -H "Content-Type: application/json" \
        -d '{
            "queries": [{
                "datasourceId": 1,
                "rawSql": "SELECT COUNT(*) FROM silver_indoor_air_pm2_5"
            }]
        }')

    echo "$response" | grep -q '"refId"'
    assert_success "Query should execute successfully"
}
```

**Expected Behavior**:
- Datasource appears in datasource list
- Connection test succeeds
- Can execute queries via API

---

### T-GF-003: Dashboard Provisioning

**Objective**: Verify dashboards are provisioned automatically on startup

**Test Cases**:

```bash
test_dashboard_exists() {
    response=$(curl -s -u admin:admin http://localhost:3001/api/search?query=Air%20Quality)
    echo "$response" | grep -q '"title":"Air Quality Dashboard"'
    assert_success "Air Quality dashboard should be provisioned"
}

test_dashboard_panels() {
    # Get dashboard JSON
    response=$(curl -s -u admin:admin http://localhost:3001/api/dashboards/uid/air-quality-dashboard)

    # Check for expected panels
    echo "$response" | grep -q '"title":"Indoor PM2.5"'
    echo "$response" | grep -q '"title":"Temperature"'
    echo "$response" | grep -q '"title":"Humidity"'
    assert_success "Dashboard should have all panels"
}

test_dashboard_variables() {
    response=$(curl -s -u admin:admin http://localhost:3001/api/dashboards/uid/air-quality-dashboard)

    # Check for time range variable
    echo "$response" | grep -q '"name":"time_range"'
    assert_success "Dashboard should have time_range variable"
}
```

**Expected Behavior**:
- Dashboard appears in search results
- All panels present in dashboard JSON
- Dashboard variables configured

---

### T-GF-004: Panel Query Execution

**Objective**: Verify dashboard panels execute queries and return data

**Test Cases**:

```bash
test_pm2_5_panel_query() {
    # Query panel data via API
    response=$(curl -s -u admin:admin -X POST \
        http://localhost:3001/api/ds/query \
        -H "Content-Type: application/json" \
        -d '{
            "queries": [{
                "datasourceId": 1,
                "rawSql": "SELECT timestamp, pm2_5 FROM silver_indoor_air_pm2_5 WHERE timestamp > NOW() - INTERVAL 7 DAY ORDER BY timestamp",
                "refId": "A",
                "format": "time_series"
            }],
            "from": "now-7d",
            "to": "now"
        }')

    echo "$response" | grep -q '"fields"'
    assert_success "Panel query should return time series data"
}

test_aggregation_panel_query() {
    response=$(curl -s -u admin:admin -X POST \
        http://localhost:3001/api/ds/query \
        -H "Content-Type: application/json" \
        -d '{
            "queries": [{
                "datasourceId": 1,
                "rawSql": "SELECT DATE_TRUNC('"'"'hour'"'"', timestamp) as time, AVG(pm2_5) as value FROM silver_indoor_air_pm2_5 WHERE timestamp > NOW() - INTERVAL 7 DAY GROUP BY time ORDER BY time",
                "refId": "A"
            }]
        }')

    echo "$response" | grep -q '"value"'
    assert_success "Aggregation query should return results"
}
```

**Expected Behavior**:
- Panel queries execute successfully
- Time series data returned in correct format
- Aggregations calculated correctly

---

### T-GF-005: Time Range Filtering

**Objective**: Verify time range picker filters data correctly

**Test Cases**:

```bash
test_time_range_last_7_days() {
    response=$(curl -s -u admin:admin -X POST \
        http://localhost:3001/api/ds/query \
        -H "Content-Type: application/json" \
        -d '{
            "queries": [{
                "datasourceId": 1,
                "rawSql": "SELECT timestamp FROM silver_indoor_air_pm2_5 WHERE timestamp > NOW() - INTERVAL 7 DAY"
            }],
            "from": "now-7d",
            "to": "now"
        }')

    # Parse response and verify timestamps
    # All timestamps should be within last 7 days
    assert_success "Time range filter should work"
}

test_time_range_custom() {
    response=$(curl -s -u admin:admin -X POST \
        http://localhost:3001/api/ds/query \
        -H "Content-Type: application/json" \
        -d '{
            "queries": [{
                "datasourceId": 1,
                "rawSql": "SELECT timestamp FROM silver_indoor_air_pm2_5 WHERE timestamp BETWEEN '"'"'2025-12-01'"'"' AND '"'"'2025-12-07'"'"'"
            }],
            "from": "2025-12-01T00:00:00Z",
            "to": "2025-12-07T23:59:59Z"
        }')

    assert_success "Custom time range should filter correctly"
}
```

**Expected Behavior**:
- Last 7 days filter returns correct data
- Custom time ranges work
- Time range variables passed to SQL queries

---

### T-GF-006: Dashboard Edit Persistence

**Objective**: Verify dashboard edits are saved (if not provisioned)

**Test Cases**:

```bash
test_dashboard_modification() {
    # Create non-provisioned dashboard
    dashboard_json='{
        "dashboard": {
            "title": "Test Dashboard",
            "panels": []
        },
        "overwrite": false
    }'

    response=$(curl -s -u admin:admin -X POST \
        http://localhost:3001/api/dashboards/db \
        -H "Content-Type: application/json" \
        -d "$dashboard_json")

    uid=$(echo "$response" | jq -r '.uid')

    # Verify dashboard exists
    response=$(curl -s -u admin:admin http://localhost:3001/api/dashboards/uid/$uid)
    echo "$response" | grep -q '"title":"Test Dashboard"'
    assert_success "Dashboard should be saved"
}

test_provisioned_dashboard_readonly() {
    # Provisioned dashboards should not allow edits via API
    response=$(curl -s -u admin:admin -X POST \
        http://localhost:3001/api/dashboards/db \
        -H "Content-Type: application/json" \
        -d '{
            "dashboard": {
                "uid": "air-quality-dashboard",
                "title": "Modified Title"
            },
            "overwrite": true
        }')

    # Should return error or ignore changes
    echo "$response" | grep -q '"message":"Cannot save provisioned dashboard"'
    assert_success "Provisioned dashboards should be read-only"
}
```

**Expected Behavior**:
- Non-provisioned dashboards can be edited
- Provisioned dashboards are read-only
- Changes persist after container restart (if using volume)

---

### T-GF-007: Refresh Functionality

**Objective**: Verify dashboard auto-refresh works correctly

**Test Cases**:

```bash
test_manual_refresh() {
    # Query panel, wait, query again
    response1=$(curl -s -u admin:admin -X POST http://localhost:3001/api/ds/query \
        -H "Content-Type: application/json" \
        -d '{"queries": [{"datasourceId": 1, "rawSql": "SELECT COUNT(*) FROM silver_indoor_air_pm2_5"}]}')

    sleep 2

    response2=$(curl -s -u admin:admin -X POST http://localhost:3001/api/ds/query \
        -H "Content-Type: application/json" \
        -d '{"queries": [{"datasourceId": 1, "rawSql": "SELECT COUNT(*) FROM silver_indoor_air_pm2_5"}]}')

    # Both queries should succeed
    assert_success "Manual refresh should work"
}

test_auto_refresh_setting() {
    response=$(curl -s -u admin:admin http://localhost:3001/api/dashboards/uid/air-quality-dashboard)

    # Check refresh interval setting
    echo "$response" | grep -q '"refresh"'
    assert_success "Dashboard should have refresh setting"
}
```

**Expected Behavior**:
- Manual refresh updates data
- Auto-refresh setting configurable
- Refresh intervals: 5s, 10s, 30s, 1m, 5m, 15m, 30m, 1h

---

## 4. Data Quality Tests

### T-DQ-001: Indoor Air PM2.5 Valid Ranges (0-500)

**Test Cases**:

```rust
#[test]
fn test_pm2_5_valid_range() {
    let conn = setup_test_db();

    insert_test_data(&conn, vec![
        ("indoor-air-pm2_5", -1.0),    // Invalid
        ("indoor-air-pm2_5", 0.0),     // Valid (boundary)
        ("indoor-air-pm2_5", 250.0),   // Valid (mid-range)
        ("indoor-air-pm2_5", 500.0),   // Valid (boundary)
        ("indoor-air-pm2_5", 501.0),   // Invalid
    ]);

    let valid_count: i64 = conn.query_row("SELECT COUNT(*) FROM silver_indoor_air_pm2_5", []).unwrap();
    assert_eq!(valid_count, 3, "Should have 3 valid readings");
}

#[test]
fn test_pm2_5_extreme_values() {
    let conn = setup_test_db();

    insert_test_data(&conn, vec![
        ("indoor-air-pm2_5", -999.0),
        ("indoor-air-pm2_5", 9999.0),
        ("indoor-air-pm2_5", f64::NAN),
        ("indoor-air-pm2_5", f64::INFINITY),
    ]);

    let count: i64 = conn.query_row("SELECT COUNT(*) FROM silver_indoor_air_pm2_5", []).unwrap();
    assert_eq!(count, 0, "Should reject all extreme values");
}
```

**EPA AQI Reference**:
- 0-50: Good
- 51-100: Moderate
- 101-150: Unhealthy for Sensitive Groups
- 151-200: Unhealthy
- 201-300: Very Unhealthy
- 301-500: Hazardous

---

### T-DQ-002: Temperature Valid Ranges (-50 to 60)

**Test Cases**:

```rust
#[test]
fn test_temperature_valid_range() {
    let conn = setup_test_db();

    insert_test_data(&conn, vec![
        ("indoor-air-temperature", -51.0),   // Invalid
        ("indoor-air-temperature", -50.0),   // Valid (boundary)
        ("indoor-air-temperature", 22.5),    // Valid (typical)
        ("indoor-air-temperature", 60.0),    // Valid (boundary)
        ("indoor-air-temperature", 61.0),    // Invalid
    ]);

    let valid_count: i64 = conn.query_row("SELECT COUNT(*) FROM silver_indoor_air_temperature", []).unwrap();
    assert_eq!(valid_count, 3);
}
```

**Range Justification**:
- Lower bound: -50°C (coldest inhabited place on Earth)
- Upper bound: 60°C (extreme heat, sensor upper limit)

---

### T-DQ-003: Humidity Valid Ranges (0-100)

**Test Cases**:

```rust
#[test]
fn test_humidity_valid_range() {
    let conn = setup_test_db();

    insert_test_data(&conn, vec![
        ("indoor-air-humidity", -1.0),    // Invalid
        ("indoor-air-humidity", 0.0),     // Valid (boundary)
        ("indoor-air-humidity", 50.0),    // Valid (typical)
        ("indoor-air-humidity", 100.0),   // Valid (boundary)
        ("indoor-air-humidity", 101.0),   // Invalid
    ]);

    let valid_count: i64 = conn.query_row("SELECT COUNT(*) FROM silver_indoor_air_humidity", []).unwrap();
    assert_eq!(valid_count, 3);
}
```

**Range Justification**:
- Humidity is a percentage (0-100%)
- Values outside this range are physically impossible

---

### T-DQ-004: AQI Valid Ranges (1-5)

**Test Cases**:

```rust
#[test]
fn test_aqi_valid_range() {
    let conn = setup_test_db();

    insert_test_data(&conn, vec![
        ("outdoor-air-aqi", 0),    // Invalid
        ("outdoor-air-aqi", 1),    // Valid (Good)
        ("outdoor-air-aqi", 3),    // Valid (Moderate)
        ("outdoor-air-aqi", 5),    // Valid (Hazardous)
        ("outdoor-air-aqi", 6),    // Invalid
    ]);

    let valid_count: i64 = conn.query_row("SELECT COUNT(*) FROM silver_outdoor_air_aqi", []).unwrap();
    assert_eq!(valid_count, 3);
}
```

**OpenWeather AQI Scale**:
- 1: Good
- 2: Fair
- 3: Moderate
- 4: Poor
- 5: Very Poor

---

### T-DQ-005: NULL Exclusion Verification

**Test Cases**:

```rust
#[test]
fn test_null_exclusion_all_streams() {
    let conn = setup_test_db();

    // Insert data with NULLs
    insert_test_data_with_nulls(&conn);

    // Check each view
    let streams = vec![
        "silver_indoor_air_pm2_5",
        "silver_indoor_air_temperature",
        "silver_indoor_air_humidity",
        "silver_outdoor_air_aqi",
    ];

    for stream in streams {
        let null_count: i64 = conn.query_row(
            &format!("SELECT COUNT(*) FROM {} WHERE value IS NULL", stream),
            []
        ).unwrap();

        assert_eq!(null_count, 0, "Stream {} should have no NULLs", stream);
    }
}

#[test]
fn test_partial_null_handling() {
    let conn = setup_test_db();

    // Insert row with NULL primary field but valid tags
    conn.execute("
        INSERT INTO bronze_indoor_air_pm2_5 (timestamp, stream_id, pm2_5, tags)
        VALUES (NOW(), 'test-stream', NULL, '{\"location\": \"living-room\"}')
    ").unwrap();

    // Should be excluded from view
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM silver_indoor_air_pm2_5", []).unwrap();
    assert_eq!(count, 0, "Row with NULL pm2_5 should be excluded");
}
```

**Expected Behavior**:
- Primary measurement fields with NULL are excluded
- Tags with NULL are retained (not primary measurement)

---

## 5. Performance Tests

### T-PERF-001: 7-Day Query < 5 Seconds

**Objective**: Verify single-stream queries complete quickly

**Test Setup**:
- Dataset: ~10,080 rows (7 days × 24 hours × 60 minutes, 1-minute interval)
- Environment: Raspberry Pi 5 (4GB RAM)

**Test Cases**:

```rust
#[test]
#[ignore]
fn perf_7_day_single_stream() {
    let conn = setup_test_db_with_real_data();

    let start = Instant::now();
    let _: Vec<Row> = conn.query("
        SELECT timestamp, pm2_5
        FROM silver_indoor_air_pm2_5
        WHERE timestamp > NOW() - INTERVAL 7 DAY
        ORDER BY timestamp
    ", []).unwrap();
    let duration = start.elapsed();

    println!("7-day query took: {:?}", duration);
    assert!(duration < Duration::from_secs(5), "Query too slow: {:?}", duration);
}
```

**Performance Target**: < 5 seconds
**Acceptable Range**: 2-5 seconds
**Failure Threshold**: > 10 seconds

---

### T-PERF-002: 30-Day Query < 15 Seconds

**Objective**: Verify monthly queries with aggregations complete in reasonable time

**Test Setup**:
- Dataset: ~43,200 rows (30 days × 24 hours × 60 minutes)
- Aggregation: Hourly averages (720 groups)

**Test Cases**:

```rust
#[test]
#[ignore]
fn perf_30_day_aggregation() {
    let conn = setup_test_db_with_real_data();

    let start = Instant::now();
    let _: Vec<Row> = conn.query("
        SELECT
            DATE_TRUNC('hour', timestamp) as hour,
            AVG(pm2_5) as avg_pm2_5,
            MIN(pm2_5) as min_pm2_5,
            MAX(pm2_5) as max_pm2_5,
            COUNT(*) as count
        FROM silver_indoor_air_pm2_5
        WHERE timestamp > NOW() - INTERVAL 30 DAY
        GROUP BY hour
        ORDER BY hour
    ", []).unwrap();
    let duration = start.elapsed();

    println!("30-day aggregation took: {:?}", duration);
    assert!(duration < Duration::from_secs(15), "Query too slow: {:?}", duration);
}
```

**Performance Target**: < 15 seconds
**Acceptable Range**: 5-15 seconds
**Failure Threshold**: > 30 seconds

---

### T-PERF-003: Dashboard Load Time < 3 Seconds

**Objective**: Verify dashboard loads quickly with all panels

**Test Setup**:
- Dashboard: 6 panels (PM2.5, Temperature, Humidity, AQI, Multi-line, Aggregation)
- Time range: Last 7 days

**Test Cases**:

```bash
#!/bin/bash
test_dashboard_load_time() {
    start=$(date +%s%N)

    # Simulate dashboard load (all panel queries)
    curl -s -u admin:admin -X POST http://localhost:3001/api/ds/query \
        -H "Content-Type: application/json" \
        -d '{
            "queries": [
                {"datasourceId": 1, "rawSql": "SELECT timestamp, pm2_5 FROM silver_indoor_air_pm2_5 WHERE timestamp > NOW() - INTERVAL 7 DAY"},
                {"datasourceId": 1, "rawSql": "SELECT timestamp, temperature FROM silver_indoor_air_temperature WHERE timestamp > NOW() - INTERVAL 7 DAY"},
                {"datasourceId": 1, "rawSql": "SELECT timestamp, humidity FROM silver_indoor_air_humidity WHERE timestamp > NOW() - INTERVAL 7 DAY"},
                {"datasourceId": 1, "rawSql": "SELECT timestamp, aqi FROM silver_outdoor_air_aqi WHERE timestamp > NOW() - INTERVAL 7 DAY"}
            ]
        }' > /dev/null

    end=$(date +%s%N)
    duration=$(( (end - start) / 1000000 )) # Convert to milliseconds

    echo "Dashboard load time: ${duration}ms"
    [ $duration -lt 3000 ] || exit 1
}
```

**Performance Target**: < 3 seconds
**Acceptable Range**: 1-3 seconds
**Failure Threshold**: > 5 seconds

---

### T-PERF-004: Memory Usage Within Limits

**Objective**: Verify DuckDB memory usage stays within Pi 5 limits

**Test Setup**:
- Pi 5: 4GB RAM total
- Target: DuckDB < 1GB RAM under normal load

**Test Cases**:

```bash
#!/bin/bash
test_memory_usage() {
    # Start DuckDB container
    docker-compose up -d duckdb
    sleep 5

    # Run queries to load data
    for i in {1..10}; do
        curl -s -X POST http://localhost:8080/query \
            -d "SELECT * FROM silver_indoor_air_pm2_5 WHERE timestamp > NOW() - INTERVAL 7 DAY" > /dev/null
    done

    # Check memory usage
    mem_usage=$(docker stats duckdb --no-stream --format "{{.MemUsage}}" | awk '{print $1}')
    mem_mb=$(echo $mem_usage | sed 's/MiB//')

    echo "DuckDB memory usage: ${mem_mb}MB"
    [ $mem_mb -lt 1024 ] || exit 1
}

test_memory_leak() {
    # Run continuous queries for 5 minutes
    initial_mem=$(get_container_memory duckdb)

    for i in {1..300}; do
        run_test_query
        sleep 1
    done

    final_mem=$(get_container_memory duckdb)
    mem_growth=$((final_mem - initial_mem))

    echo "Memory growth: ${mem_growth}MB"
    [ $mem_growth -lt 100 ] || exit 1  # Should grow < 100MB
}
```

**Performance Target**: < 1GB RAM
**Acceptable Range**: 512MB - 1GB
**Failure Threshold**: > 1.5GB

---

## 6. Deployment Tests

### T-DEP-001: docker-compose up Succeeds

**Objective**: Verify Docker Compose stack starts successfully

**Test Cases**:

```bash
#!/bin/bash
test_docker_compose_up() {
    cd /workspaces/neural-data-platform/deploy/docker

    # Start stack
    docker-compose up -d

    # Check exit code
    assert_equals $? 0 "docker-compose up should succeed"

    # Verify containers created
    container_count=$(docker-compose ps -q | wc -l)
    assert_equals $container_count 2 "Should have 2 containers (DuckDB + Grafana)"
}

test_stack_startup_time() {
    start=$(date +%s)
    docker-compose up -d

    # Wait for all containers healthy
    docker-compose ps | grep -q "Up"

    end=$(date +%s)
    duration=$((end - start))

    echo "Stack startup time: ${duration}s"
    [ $duration -lt 30 ] || exit 1  # Should start in < 30 seconds
}
```

**Expected Behavior**:
- `docker-compose up -d` exits with code 0
- All containers created
- Stack ready within 30 seconds

---

### T-DEP-002: All Containers Healthy

**Objective**: Verify all containers report healthy status

**Test Cases**:

```bash
#!/bin/bash
test_container_health() {
    docker-compose up -d
    sleep 10

    # Check DuckDB container
    duckdb_status=$(docker-compose ps duckdb | grep "Up" | grep "healthy")
    assert_not_empty "$duckdb_status" "DuckDB should be healthy"

    # Check Grafana container
    grafana_status=$(docker-compose ps grafana | grep "Up" | grep "healthy")
    assert_not_empty "$grafana_status" "Grafana should be healthy"
}

test_health_checks() {
    # DuckDB health check
    response=$(curl -s http://localhost:8080/health)
    assert_equals "$response" "OK" "DuckDB health check should return OK"

    # Grafana health check
    response=$(curl -s http://localhost:3001/api/health)
    echo "$response" | grep -q '"database":"ok"'
    assert_success "Grafana health check should succeed"
}
```

**Expected Behavior**:
- All containers report "Up" status
- Health checks pass within 10 seconds
- Containers stay healthy over time

---

### T-DEP-003: Network Connectivity

**Objective**: Verify containers can communicate

**Test Cases**:

```bash
#!/bin/bash
test_grafana_to_duckdb_connectivity() {
    # Test Grafana can reach DuckDB
    docker exec grafana curl -s http://duckdb:8080/health
    assert_success "Grafana should reach DuckDB"
}

test_host_to_container_connectivity() {
    # Test host can reach containers
    curl -s http://localhost:8080/health > /dev/null
    assert_success "Host should reach DuckDB"

    curl -s http://localhost:3001/api/health > /dev/null
    assert_success "Host should reach Grafana"
}

test_dns_resolution() {
    # Test DNS resolution within Docker network
    docker exec grafana nslookup duckdb
    assert_success "DNS resolution should work"
}
```

**Expected Behavior**:
- Containers communicate via Docker network
- Host reaches containers via published ports
- DNS resolution works for service names

---

### T-DEP-004: Volume Mounts Correct

**Objective**: Verify volume mounts are configured correctly

**Test Cases**:

```bash
#!/bin/bash
test_bronze_data_mount() {
    # Check Bronze data accessible from DuckDB
    docker exec duckdb ls -la /data/bronze/indoor-air-pm2_5/
    assert_success "Bronze data should be mounted"
}

test_grafana_data_persistence() {
    # Create test dashboard
    create_test_dashboard

    # Restart Grafana
    docker-compose restart grafana
    sleep 5

    # Verify dashboard still exists
    dashboard_exists
    assert_success "Grafana data should persist"
}

test_readonly_bronze_mount() {
    # Verify Bronze data is read-only (if configured)
    docker exec duckdb touch /data/bronze/test.txt 2>&1 | grep -q "Read-only file system"
    assert_success "Bronze mount should be read-only"
}
```

**Expected Behavior**:
- Bronze data accessible from DuckDB container
- Grafana data persists across restarts
- Read-only mounts enforced (if configured)

**Volume Configuration**:
```yaml
volumes:
  - ../../data/bronze:/data/bronze:ro  # Read-only
  - grafana-data:/var/lib/grafana       # Persistent
```

---

### T-DEP-005: Graceful Shutdown

**Objective**: Verify containers shut down cleanly

**Test Cases**:

```bash
#!/bin/bash
test_graceful_shutdown() {
    docker-compose up -d
    sleep 5

    # Stop stack
    start=$(date +%s)
    docker-compose down
    end=$(date +%s)
    duration=$((end - start))

    # Should stop quickly
    echo "Shutdown time: ${duration}s"
    [ $duration -lt 15 ] || exit 1

    # Verify all containers stopped
    running=$(docker-compose ps -q | wc -l)
    assert_equals $running 0 "All containers should be stopped"
}

test_sigterm_handling() {
    docker-compose up -d
    sleep 5

    # Send SIGTERM to container
    docker kill --signal=SIGTERM duckdb

    # Verify graceful shutdown (not killed)
    sleep 2
    exit_code=$(docker inspect duckdb --format='{{.State.ExitCode}}')
    assert_equals $exit_code 0 "Container should exit cleanly"
}
```

**Expected Behavior**:
- Containers stop within 10 seconds
- No force-kill required
- Clean exit codes (0)

---

## 7. Test Data Requirements

### 7.1 Sample Parquet Files

**Required Test Datasets**:

| Dataset | Description | Rows | Time Range |
|---------|-------------|------|------------|
| `test-7day-indoor-pm2_5.parquet` | 7 days of PM2.5 data | ~10k | Last 7 days |
| `test-30day-indoor-pm2_5.parquet` | 30 days of PM2.5 data | ~43k | Last 30 days |
| `test-mixed-quality.parquet` | Data with NULLs and out-of-range | 1k | Various |
| `test-edge-cases.parquet` | Boundary values, extreme values | 100 | N/A |

**Test Data Generation Script**:

```rust
// tests/fixtures/generate_test_data.rs
use chrono::{Duration, Utc};
use parquet::record::RecordWriter;

pub fn generate_7_day_dataset() -> PathBuf {
    let mut writer = create_parquet_writer("test-7day-indoor-pm2_5.parquet");

    let end = Utc::now();
    let start = end - Duration::days(7);

    let mut timestamp = start;
    while timestamp <= end {
        writer.write(TimeSeriesPoint {
            timestamp,
            stream_id: "indoor-air-pm2_5".to_string(),
            fields: HashMap::from([
                ("pm2_5".to_string(), json!(rand::random::<f64>() * 100.0)),
            ]),
            tags: HashMap::from([
                ("location".to_string(), "test-location".to_string()),
            ]),
        }).unwrap();

        timestamp = timestamp + Duration::minutes(1);
    }

    writer.close().unwrap();
    PathBuf::from("test-7day-indoor-pm2_5.parquet")
}

pub fn generate_edge_case_dataset() -> PathBuf {
    let mut writer = create_parquet_writer("test-edge-cases.parquet");

    // Boundary values
    writer.write(point_with_value(0.0)).unwrap();
    writer.write(point_with_value(500.0)).unwrap();

    // Out-of-range values
    writer.write(point_with_value(-1.0)).unwrap();
    writer.write(point_with_value(501.0)).unwrap();

    // NULL values
    writer.write(point_with_null()).unwrap();

    // Extreme values
    writer.write(point_with_value(f64::MAX)).unwrap();
    writer.write(point_with_value(f64::MIN)).unwrap();

    writer.close().unwrap();
    PathBuf::from("test-edge-cases.parquet")
}
```

---

### 7.2 Known-Good Data for Assertions

**Test Fixtures**:

```rust
// tests/fixtures/known_good_data.rs

pub fn known_good_pm2_5_reading() -> TimeSeriesPoint {
    TimeSeriesPoint {
        timestamp: Utc.ymd(2025, 12, 18).and_hms(12, 0, 0),
        stream_id: "indoor-air-pm2_5".to_string(),
        fields: HashMap::from([
            ("pm2_5".to_string(), json!(35.5)),
        ]),
        tags: HashMap::from([
            ("location".to_string(), "living-room".to_string()),
            ("sensor".to_string(), "sds011".to_string()),
        ]),
    }
}

pub fn expected_hourly_aggregation() -> Vec<AggregationResult> {
    vec![
        AggregationResult {
            hour: Utc.ymd(2025, 12, 18).and_hms(10, 0, 0),
            avg: 32.5,
            min: 28.0,
            max: 38.0,
            count: 60,
        },
        AggregationResult {
            hour: Utc.ymd(2025, 12, 18).and_hms(11, 0, 0),
            avg: 35.2,
            min: 30.0,
            max: 42.0,
            count: 60,
        },
    ]
}
```

---

### 7.3 Edge Cases

**Test Cases to Cover**:

```rust
pub fn edge_case_data() -> Vec<TestCase> {
    vec![
        // Boundary values (should be INCLUDED)
        TestCase { value: 0.0, expected: true },
        TestCase { value: 500.0, expected: true },

        // Just outside boundaries (should be EXCLUDED)
        TestCase { value: -0.1, expected: false },
        TestCase { value: 500.1, expected: false },

        // NULL values (should be EXCLUDED)
        TestCase { value: None, expected: false },

        // Extreme values (should be EXCLUDED)
        TestCase { value: f64::INFINITY, expected: false },
        TestCase { value: f64::NEG_INFINITY, expected: false },
        TestCase { value: f64::NAN, expected: false },

        // Very large valid values
        TestCase { value: 499.9, expected: true },
        TestCase { value: 0.1, expected: true },

        // Floating point precision
        TestCase { value: 35.123456789, expected: true },
    ]
}
```

---

## 8. Test Execution Plan

### 8.1 Development Phase

```bash
# Run unit tests frequently
cargo test

# Run integration tests before commit
cargo test --ignored

# Run specific test
cargo test test_pm2_5_range_filter -- --nocapture
```

---

### 8.2 Pre-Deployment Phase

```bash
# Run all tests
cargo test --all

# Run performance benchmarks
cargo test --ignored bench_

# Run deployment tests
cd tests/deployment
./test_docker_compose.sh
./test_grafana.sh
```

---

### 8.3 Post-Deployment Verification

```bash
# Health checks
curl http://localhost:8080/health
curl http://localhost:3001/api/health

# Smoke tests
./tests/deployment/smoke_test.sh

# Load dashboard and verify
open http://localhost:3001/d/air-quality-dashboard
```

---

## 9. Test Maintenance

### 9.1 Adding New Tests

1. **Identify test category**: Unit, Integration, E2E, Performance
2. **Write test following templates** in this document
3. **Add test data** if needed (fixtures, sample Parquet)
4. **Update this document** with new test ID
5. **Run tests** to verify

---

### 9.2 Test Data Refresh

- Regenerate test Parquet files monthly
- Update known-good data if schema changes
- Archive old test datasets

---

### 9.3 Performance Baseline Updates

- Re-run performance tests after major changes
- Update targets if hardware changes (e.g., Pi 4 → Pi 5)
- Document performance regression investigations

---

## 10. Test Reporting

### 10.1 Test Coverage

Target coverage by component:

| Component | Target Coverage | Current |
|-----------|----------------|---------|
| SQL view generation | 90% | TBD |
| Data quality filters | 90% | TBD |
| Query execution | 80% | TBD |
| Grafana integration | 70% | TBD |
| Deployment | 80% | TBD |

---

### 10.2 Test Results Format

```markdown
## Test Results - DP-001

**Date**: 2025-12-18
**Environment**: Docker on Raspberry Pi 5
**Test Suite Version**: 1.0

### Summary
- **Total Tests**: 52
- **Passed**: 50
- **Failed**: 2
- **Skipped**: 0

### Failed Tests
- T-PERF-002: 30-day query took 18s (target: <15s)
- T-DEP-004: Volume mount permissions incorrect

### Performance Metrics
- 7-day query: 3.2s (target: <5s) ✅
- 30-day query: 18s (target: <15s) ❌
- Dashboard load: 2.1s (target: <3s) ✅
- Memory usage: 850MB (target: <1GB) ✅

### Recommendations
1. Optimize 30-day aggregation query with index
2. Fix volume mount permissions in docker-compose.yml
```

---

## 11. Appendix

### 11.1 Test Helper Functions

```rust
// tests/helpers/mod.rs

pub fn setup_test_db() -> Connection {
    Connection::open_in_memory().unwrap()
}

pub fn setup_test_db_with_views() -> Connection {
    let conn = setup_test_db();
    create_silver_views(&conn);
    conn
}

pub fn insert_test_data(conn: &Connection, data: Vec<(&str, f64)>) {
    for (stream_id, value) in data {
        conn.execute("
            INSERT INTO bronze_data (timestamp, stream_id, value)
            VALUES (NOW(), ?, ?)
        ", params![stream_id, value]).unwrap();
    }
}

pub fn assert_query_time<F>(f: F, max_duration: Duration, description: &str)
where
    F: FnOnce(),
{
    let start = Instant::now();
    f();
    let duration = start.elapsed();

    assert!(
        duration < max_duration,
        "{} took {:?}, expected < {:?}",
        description, duration, max_duration
    );
}
```

---

### 11.2 CI/CD Integration

```yaml
# .github/workflows/test-dp-001.yml
name: DP-001 Tests

on:
  pull_request:
    paths:
      - 'product/features/dp-001/**'
      - 'deploy/docker/**'

jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Run unit tests
        run: cargo test

  integration-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Generate test data
        run: cargo run --bin generate_test_data
      - name: Run integration tests
        run: cargo test --ignored

  deployment-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Start Docker stack
        run: |
          cd deploy/docker
          docker-compose up -d
      - name: Run deployment tests
        run: ./tests/deployment/test_all.sh
```

---

## Summary

This test specification covers:
- ✅ 52 test cases across 7 categories
- ✅ Unit, integration, E2E, and performance tests
- ✅ DuckDB, Grafana, data quality, and deployment testing
- ✅ Clear acceptance criteria and performance targets
- ✅ Test data generation and fixtures
- ✅ CI/CD integration guidelines

**Next Steps**:
1. Implement Rust unit tests for SQL view logic
2. Create integration tests for DuckDB + Parquet
3. Write Bash scripts for Grafana E2E tests
4. Generate test Parquet datasets
5. Set up CI/CD workflow for automated testing
