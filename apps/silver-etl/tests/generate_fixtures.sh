#!/bin/bash
# Generate Sample Parquet Fixtures for Silver ETL Tests
# apps/silver-etl/tests/generate_fixtures.sh
#
# This script creates sample Parquet files with Bronze layer schema for testing.
# Uses DuckDB CLI if available, otherwise falls back to Python with pyarrow.
#
# Usage:
#   ./generate_fixtures.sh           # Generate all fixtures
#   ./generate_fixtures.sh clean     # Remove existing fixtures
#   ./generate_fixtures.sh air       # Generate only air-quality fixtures
#   ./generate_fixtures.sh weather   # Generate only weather fixtures

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIXTURES_DIR="${SCRIPT_DIR}/fixtures"
PARQUET_DIR="${FIXTURES_DIR}/parquet"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check for required tools
check_duckdb() {
    if command -v duckdb &> /dev/null; then
        log_info "DuckDB CLI found"
        return 0
    else
        log_warn "DuckDB CLI not found, will try Python fallback"
        return 1
    fi
}

check_python() {
    if command -v python3 &> /dev/null; then
        if python3 -c "import pyarrow" 2>/dev/null; then
            log_info "Python with pyarrow found"
            return 0
        else
            log_warn "Python pyarrow not available"
            return 1
        fi
    fi
    return 1
}

# Clean existing fixtures
clean_fixtures() {
    log_info "Cleaning existing Parquet fixtures..."
    rm -rf "${PARQUET_DIR}"
    log_info "Done"
}

# Create directory structure
create_directories() {
    log_info "Creating directory structure..."

    # Air quality directories
    mkdir -p "${PARQUET_DIR}/air-quality/valid"
    mkdir -p "${PARQUET_DIR}/air-quality/out-of-range"
    mkdir -p "${PARQUET_DIR}/air-quality/nulls"
    mkdir -p "${PARQUET_DIR}/air-quality/duplicates"

    # Weather directories
    mkdir -p "${PARQUET_DIR}/outdoor-weather/valid"
    mkdir -p "${PARQUET_DIR}/outdoor-weather/unit-conversion"

    # Special test directories
    mkdir -p "${PARQUET_DIR}/invalid"
    mkdir -p "${PARQUET_DIR}/empty"

    log_info "Directory structure created"
}

# Generate fixtures using DuckDB
generate_with_duckdb() {
    log_info "Generating fixtures with DuckDB..."

    duckdb :memory: << 'EOF'
-- Air Quality: Valid data
COPY (
    SELECT
        1704931200000000::BIGINT AS timestamp,         -- 2026-01-10T12:00:00 UTC
        'sensor-office'::VARCHAR AS ndp_id,
        'mqtt://airgradient/sensor-office'::VARCHAR AS source_id,
        '{"location": {"path": "office/main", "floor": 1}}'::VARCHAR AS context,
        '{"pm02": 25.5, "pm10": 35.0, "rco2": 850, "atmp": 22.5, "rhum": 45.0, "tvoc": 150, "nox": 25}'::VARCHAR AS raw_payload
    UNION ALL
    SELECT
        1704934800000000::BIGINT,                      -- 2026-01-10T13:00:00 UTC
        'sensor-office'::VARCHAR,
        'mqtt://airgradient/sensor-office'::VARCHAR,
        '{"location": {"path": "office/main", "floor": 1}}'::VARCHAR,
        '{"pm02": 28.0, "pm10": 38.0, "rco2": 920, "atmp": 23.0, "rhum": 43.0, "tvoc": 160, "nox": 28}'::VARCHAR
    UNION ALL
    SELECT
        1704938400000000::BIGINT,                      -- 2026-01-10T14:00:00 UTC
        'sensor-basement'::VARCHAR,
        'mqtt://airgradient/sensor-basement'::VARCHAR,
        '{"location": {"path": "basement/workshop", "floor": -1}}'::VARCHAR,
        '{"pm02": 15.0, "pm10": 22.0, "rco2": 720, "atmp": 18.5, "rhum": 55.0, "tvoc": 80, "nox": 12}'::VARCHAR
) TO 'tests/fixtures/parquet/air-quality/valid/data.parquet' (FORMAT PARQUET);

-- Air Quality: Out of range data (DQ violations)
COPY (
    SELECT
        1704931200000000::BIGINT AS timestamp,
        'sensor-faulty'::VARCHAR AS ndp_id,
        'mqtt://airgradient/sensor-faulty'::VARCHAR AS source_id,
        '{"location": {"path": "garage", "floor": 0}}'::VARCHAR AS context,
        '{"pm02": 1500.0, "pm10": 2000.0, "rco2": 15000, "atmp": 95.0, "rhum": 120.0}'::VARCHAR AS raw_payload
    UNION ALL
    SELECT
        1704934800000000::BIGINT,
        'sensor-faulty'::VARCHAR,
        'mqtt://airgradient/sensor-faulty'::VARCHAR,
        '{"location": {"path": "garage", "floor": 0}}'::VARCHAR,
        '{"pm02": -10.0, "pm10": 5.0, "rco2": 200, "atmp": -50.0, "rhum": -5.0}'::VARCHAR
) TO 'tests/fixtures/parquet/air-quality/out-of-range/data.parquet' (FORMAT PARQUET);

-- Air Quality: Data with NULL values
COPY (
    SELECT
        1704931200000000::BIGINT AS timestamp,
        'sensor-partial'::VARCHAR AS ndp_id,
        'mqtt://airgradient/sensor-partial'::VARCHAR AS source_id,
        '{"location": {"path": "attic", "floor": 2}}'::VARCHAR AS context,
        '{"pm02": 30.0}'::VARCHAR AS raw_payload
    UNION ALL
    SELECT
        1704934800000000::BIGINT,
        'sensor-partial'::VARCHAR,
        'mqtt://airgradient/sensor-partial'::VARCHAR,
        '{"location": {"path": "attic", "floor": 2}}'::VARCHAR,
        '{"rco2": 900, "atmp": 25.0}'::VARCHAR
) TO 'tests/fixtures/parquet/air-quality/nulls/data.parquet' (FORMAT PARQUET);

-- Air Quality: Duplicate keys (same timestamp + ndp_id)
COPY (
    SELECT
        1704931200000000::BIGINT AS timestamp,
        'sensor-dup'::VARCHAR AS ndp_id,
        'mqtt://airgradient/sensor-dup'::VARCHAR AS source_id,
        '{"location": {"path": "kitchen", "floor": 1}}'::VARCHAR AS context,
        '{"pm02": 20.0, "rco2": 800}'::VARCHAR AS raw_payload
    UNION ALL
    SELECT
        1704931200000000::BIGINT,                      -- Same timestamp
        'sensor-dup'::VARCHAR,                          -- Same ndp_id
        'mqtt://airgradient/sensor-dup'::VARCHAR,
        '{"location": {"path": "kitchen", "floor": 1}}'::VARCHAR,
        '{"pm02": 22.0, "rco2": 810}'::VARCHAR          -- Different values (later should win)
) TO 'tests/fixtures/parquet/air-quality/duplicates/data.parquet' (FORMAT PARQUET);

-- Outdoor Weather: Valid data (with values needing unit conversion)
COPY (
    SELECT
        1704931200000000::BIGINT AS timestamp,
        'owm-home'::VARCHAR AS ndp_id,
        'http://api.openweathermap.org'::VARCHAR AS source_id,
        '{"coordinates": {"lat": 47.6062, "lon": -122.3321}}'::VARCHAR AS context,
        '{"main": {"temp": 288.15, "feels_like": 286.15, "humidity": 72, "pressure": 1015}, "wind": {"speed": 5.5, "deg": 270, "gust": 8.2}, "clouds": {"all": 40}, "visibility": 10000, "weather": [{"id": 803, "description": "broken clouds"}]}'::VARCHAR AS raw_payload
    UNION ALL
    SELECT
        1704934800000000::BIGINT,
        'owm-home'::VARCHAR,
        'http://api.openweathermap.org'::VARCHAR,
        '{"coordinates": {"lat": 47.6062, "lon": -122.3321}}'::VARCHAR,
        '{"main": {"temp": 290.15, "feels_like": 288.15, "humidity": 65, "pressure": 1013}, "wind": {"speed": 7.2, "deg": 245, "gust": 10.5}, "clouds": {"all": 25}, "visibility": 15000, "weather": [{"id": 801, "description": "few clouds"}]}'::VARCHAR
) TO 'tests/fixtures/parquet/outdoor-weather/valid/data.parquet' (FORMAT PARQUET);

-- Outdoor Weather: Edge case temperatures for unit conversion testing
COPY (
    SELECT
        1704931200000000::BIGINT AS timestamp,
        'owm-test'::VARCHAR AS ndp_id,
        'http://api.openweathermap.org'::VARCHAR AS source_id,
        '{"coordinates": {"lat": 0.0, "lon": 0.0}}'::VARCHAR AS context,
        '{"main": {"temp": 273.15, "feels_like": 273.15, "humidity": 50, "pressure": 1013}, "wind": {"speed": 0.0, "deg": 0}}'::VARCHAR AS raw_payload
    UNION ALL
    SELECT
        1704934800000000::BIGINT,
        'owm-test'::VARCHAR,
        'http://api.openweathermap.org'::VARCHAR,
        '{"coordinates": {"lat": 0.0, "lon": 0.0}}'::VARCHAR,
        '{"main": {"temp": 373.15, "feels_like": 373.15, "humidity": 100, "pressure": 900}, "wind": {"speed": 27.78, "deg": 180}}'::VARCHAR
) TO 'tests/fixtures/parquet/outdoor-weather/unit-conversion/data.parquet' (FORMAT PARQUET);

-- Empty file (zero rows but valid schema)
COPY (
    SELECT
        1::BIGINT AS timestamp,
        ''::VARCHAR AS ndp_id,
        ''::VARCHAR AS source_id,
        ''::VARCHAR AS context,
        ''::VARCHAR AS raw_payload
    WHERE false
) TO 'tests/fixtures/parquet/empty/empty.parquet' (FORMAT PARQUET);

.quit
EOF

    log_info "DuckDB fixture generation complete"
}

# Generate fixtures using Python (fallback)
generate_with_python() {
    log_info "Generating fixtures with Python/pyarrow..."

    python3 << 'PYTHON_EOF'
import pyarrow as pa
import pyarrow.parquet as pq
import json
import os

fixtures_dir = "tests/fixtures/parquet"

# Define Bronze schema
bronze_schema = pa.schema([
    pa.field("timestamp", pa.int64()),
    pa.field("ndp_id", pa.string()),
    pa.field("source_id", pa.string()),
    pa.field("context", pa.string()),
    pa.field("raw_payload", pa.string()),
])

def write_parquet(path, data):
    """Write data to parquet file"""
    os.makedirs(os.path.dirname(path), exist_ok=True)
    table = pa.table(data, schema=bronze_schema)
    pq.write_table(table, path)
    print(f"  Created: {path}")

# Air Quality: Valid data
write_parquet(f"{fixtures_dir}/air-quality/valid/data.parquet", {
    "timestamp": [1704931200000000, 1704934800000000, 1704938400000000],
    "ndp_id": ["sensor-office", "sensor-office", "sensor-basement"],
    "source_id": ["mqtt://airgradient/sensor-office", "mqtt://airgradient/sensor-office", "mqtt://airgradient/sensor-basement"],
    "context": [
        '{"location": {"path": "office/main", "floor": 1}}',
        '{"location": {"path": "office/main", "floor": 1}}',
        '{"location": {"path": "basement/workshop", "floor": -1}}'
    ],
    "raw_payload": [
        '{"pm02": 25.5, "pm10": 35.0, "rco2": 850, "atmp": 22.5, "rhum": 45.0, "tvoc": 150, "nox": 25}',
        '{"pm02": 28.0, "pm10": 38.0, "rco2": 920, "atmp": 23.0, "rhum": 43.0, "tvoc": 160, "nox": 28}',
        '{"pm02": 15.0, "pm10": 22.0, "rco2": 720, "atmp": 18.5, "rhum": 55.0, "tvoc": 80, "nox": 12}'
    ]
})

# Air Quality: Out of range data
write_parquet(f"{fixtures_dir}/air-quality/out-of-range/data.parquet", {
    "timestamp": [1704931200000000, 1704934800000000],
    "ndp_id": ["sensor-faulty", "sensor-faulty"],
    "source_id": ["mqtt://airgradient/sensor-faulty", "mqtt://airgradient/sensor-faulty"],
    "context": [
        '{"location": {"path": "garage", "floor": 0}}',
        '{"location": {"path": "garage", "floor": 0}}'
    ],
    "raw_payload": [
        '{"pm02": 1500.0, "pm10": 2000.0, "rco2": 15000, "atmp": 95.0, "rhum": 120.0}',
        '{"pm02": -10.0, "pm10": 5.0, "rco2": 200, "atmp": -50.0, "rhum": -5.0}'
    ]
})

# Air Quality: NULL values
write_parquet(f"{fixtures_dir}/air-quality/nulls/data.parquet", {
    "timestamp": [1704931200000000, 1704934800000000],
    "ndp_id": ["sensor-partial", "sensor-partial"],
    "source_id": ["mqtt://airgradient/sensor-partial", "mqtt://airgradient/sensor-partial"],
    "context": [
        '{"location": {"path": "attic", "floor": 2}}',
        '{"location": {"path": "attic", "floor": 2}}'
    ],
    "raw_payload": [
        '{"pm02": 30.0}',
        '{"rco2": 900, "atmp": 25.0}'
    ]
})

# Air Quality: Duplicates
write_parquet(f"{fixtures_dir}/air-quality/duplicates/data.parquet", {
    "timestamp": [1704931200000000, 1704931200000000],
    "ndp_id": ["sensor-dup", "sensor-dup"],
    "source_id": ["mqtt://airgradient/sensor-dup", "mqtt://airgradient/sensor-dup"],
    "context": [
        '{"location": {"path": "kitchen", "floor": 1}}',
        '{"location": {"path": "kitchen", "floor": 1}}'
    ],
    "raw_payload": [
        '{"pm02": 20.0, "rco2": 800}',
        '{"pm02": 22.0, "rco2": 810}'
    ]
})

# Outdoor Weather: Valid data
write_parquet(f"{fixtures_dir}/outdoor-weather/valid/data.parquet", {
    "timestamp": [1704931200000000, 1704934800000000],
    "ndp_id": ["owm-home", "owm-home"],
    "source_id": ["http://api.openweathermap.org", "http://api.openweathermap.org"],
    "context": [
        '{"coordinates": {"lat": 47.6062, "lon": -122.3321}}',
        '{"coordinates": {"lat": 47.6062, "lon": -122.3321}}'
    ],
    "raw_payload": [
        '{"main": {"temp": 288.15, "feels_like": 286.15, "humidity": 72, "pressure": 1015}, "wind": {"speed": 5.5, "deg": 270, "gust": 8.2}, "clouds": {"all": 40}, "visibility": 10000, "weather": [{"id": 803, "description": "broken clouds"}]}',
        '{"main": {"temp": 290.15, "feels_like": 288.15, "humidity": 65, "pressure": 1013}, "wind": {"speed": 7.2, "deg": 245, "gust": 10.5}, "clouds": {"all": 25}, "visibility": 15000, "weather": [{"id": 801, "description": "few clouds"}]}'
    ]
})

# Outdoor Weather: Unit conversion edge cases
write_parquet(f"{fixtures_dir}/outdoor-weather/unit-conversion/data.parquet", {
    "timestamp": [1704931200000000, 1704934800000000],
    "ndp_id": ["owm-test", "owm-test"],
    "source_id": ["http://api.openweathermap.org", "http://api.openweathermap.org"],
    "context": [
        '{"coordinates": {"lat": 0.0, "lon": 0.0}}',
        '{"coordinates": {"lat": 0.0, "lon": 0.0}}'
    ],
    "raw_payload": [
        '{"main": {"temp": 273.15, "feels_like": 273.15, "humidity": 50, "pressure": 1013}, "wind": {"speed": 0.0, "deg": 0}}',
        '{"main": {"temp": 373.15, "feels_like": 373.15, "humidity": 100, "pressure": 900}, "wind": {"speed": 27.78, "deg": 180}}'
    ]
})

# Empty file
write_parquet(f"{fixtures_dir}/empty/empty.parquet", {
    "timestamp": [],
    "ndp_id": [],
    "source_id": [],
    "context": [],
    "raw_payload": []
})

print("Python fixture generation complete")
PYTHON_EOF

    log_info "Python fixture generation complete"
}

# Generate using Rust (second fallback)
generate_with_rust() {
    log_info "Generating fixtures with cargo test helper..."

    # Create a temporary Rust test file to generate fixtures
    cat > "${FIXTURES_DIR}/generate_fixtures_test.rs" << 'RUST_EOF'
// Temporary test file to generate fixtures
// This will be run via: cargo test --test generate_fixtures_test -- --ignored

use polars::prelude::*;
use std::path::Path;

fn create_bronze_df(
    timestamps: &[i64],
    ndp_ids: &[&str],
    source_ids: &[&str],
    contexts: &[&str],
    payloads: &[&str],
) -> DataFrame {
    df! {
        "timestamp" => timestamps,
        "ndp_id" => ndp_ids,
        "source_id" => source_ids,
        "context" => contexts,
        "raw_payload" => payloads
    }.unwrap()
}

fn write_parquet(df: &mut DataFrame, path: &str) {
    let path = Path::new(path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let file = std::fs::File::create(path).unwrap();
    ParquetWriter::new(file).finish(df).unwrap();
    println!("Created: {}", path.display());
}

#[test]
#[ignore]
fn generate_all_fixtures() {
    let base = "tests/fixtures/parquet";

    // Air Quality: Valid
    let mut df = create_bronze_df(
        &[1704931200000000, 1704934800000000, 1704938400000000],
        &["sensor-office", "sensor-office", "sensor-basement"],
        &["mqtt://airgradient/sensor-office", "mqtt://airgradient/sensor-office", "mqtt://airgradient/sensor-basement"],
        &[
            r#"{"location": {"path": "office/main", "floor": 1}}"#,
            r#"{"location": {"path": "office/main", "floor": 1}}"#,
            r#"{"location": {"path": "basement/workshop", "floor": -1}}"#,
        ],
        &[
            r#"{"pm02": 25.5, "pm10": 35.0, "rco2": 850, "atmp": 22.5, "rhum": 45.0}"#,
            r#"{"pm02": 28.0, "pm10": 38.0, "rco2": 920, "atmp": 23.0, "rhum": 43.0}"#,
            r#"{"pm02": 15.0, "pm10": 22.0, "rco2": 720, "atmp": 18.5, "rhum": 55.0}"#,
        ],
    );
    write_parquet(&mut df, &format!("{}/air-quality/valid/data.parquet", base));

    println!("Fixture generation complete!");
}
RUST_EOF

    log_info "Created Rust fixture generation test"
    log_info "Run with: cargo test -p silver-etl generate_all_fixtures -- --ignored"
}

# Create corrupt parquet file for error testing
create_corrupt_fixture() {
    log_info "Creating corrupt parquet file for error testing..."
    echo "This is not a valid parquet file" > "${PARQUET_DIR}/invalid/corrupt.parquet"
}

# Main execution
main() {
    local cmd="${1:-all}"

    case "$cmd" in
        clean)
            clean_fixtures
            ;;
        air)
            create_directories
            if check_duckdb; then
                generate_with_duckdb
            elif check_python; then
                generate_with_python
            else
                generate_with_rust
            fi
            ;;
        weather)
            create_directories
            if check_duckdb; then
                generate_with_duckdb
            elif check_python; then
                generate_with_python
            else
                generate_with_rust
            fi
            ;;
        all)
            clean_fixtures
            create_directories
            if check_duckdb; then
                generate_with_duckdb
            elif check_python; then
                generate_with_python
            else
                generate_with_rust
            fi
            create_corrupt_fixture
            log_info "All fixtures generated successfully!"
            ;;
        *)
            echo "Usage: $0 [clean|air|weather|all]"
            exit 1
            ;;
    esac
}

# Change to the script's parent directory (apps/silver-etl)
cd "${SCRIPT_DIR}/.."

main "$@"
