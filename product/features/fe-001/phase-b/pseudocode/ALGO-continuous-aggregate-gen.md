# ALGO-continuous-aggregate-gen: Single Stream Continuous Aggregate Generation

> **Algorithm ID:** B02
> **Feature:** v11-003, v11-004 (Per-Stream Continuous Aggregates)
> **Phase:** B (First Stream - Reference Implementation)
> **Created:** 2026-02-04

---

## Purpose

Generate TimescaleDB continuous aggregate DDL for a single stream (air-quality reference implementation). This algorithm extends Phase A's generic aggregate generator with air-quality-specific validation and demonstrates the complete config-to-DDL flow that will be replicated for all subsequent streams.

---

## Algorithm: GenerateSingleStreamAggregate

```
ALGORITHM: GenerateSingleStreamAggregate
INPUT:
    stream_id: String                    // e.g., "air-quality"
    config_loader: ConfigLoader
    action: Action                       // Sync | Recreate
OUTPUT: Result<GeneratedArtifacts, GeneratorError>
REQUIRES:
    - Stream config exists with gold_etl section
    - gold_etl.enabled = true
    - gold_etl.aggregates is defined

BEGIN
    // 1. Load stream configuration
    stream_config <- config_loader.load_stream_config(stream_id)?

    // 2. Validate gold_etl configuration
    validation_result <- ValidateGoldEtlConfig(stream_id, stream_config)
    IF validation_result.has_errors() THEN
        RETURN Err(GeneratorError::ValidationFailed(validation_result.errors))
    END IF

    gold_etl <- stream_config.gold_etl.unwrap()

    // 3. Resolve source table from silver_etl
    source_table <- ResolveSilverSourceTable(stream_id, stream_config)

    // 4. Extract timestamp column for time_bucket
    timestamp_column <- ExtractTimestampColumn(stream_config)

    // 5. Generate DDL for each granularity
    artifacts <- GeneratedArtifacts::new()

    FOR EACH granularity IN gold_etl.aggregates.granularities DO
        // 5a. Generate continuous aggregate view
        view_ddl <- GenerateContinuousAggregateView(
            stream_id,
            gold_etl,
            source_table,
            timestamp_column,
            granularity,
            action
        )?
        artifacts.add_ddl(view_ddl)

        // 5b. Generate refresh policy
        policy_ddl <- GenerateRefreshPolicy(
            stream_id,
            granularity,
            gold_etl.refresh_policy
        )?
        artifacts.add_ddl(policy_ddl)

        // 5c. Generate data dictionary metadata
        metadata_ddl <- GenerateMetadataInserts(
            stream_id,
            granularity,
            gold_etl
        )?
        artifacts.add_ddl(metadata_ddl)
    END FOR

    RETURN Ok(artifacts)
END
```

---

## Algorithm: ValidateGoldEtlConfig

```
ALGORITHM: ValidateGoldEtlConfig
INPUT:
    stream_id: String
    stream_config: StreamConfig
OUTPUT: ValidationResult

BEGIN
    errors <- Vec::new()

    // 1. Check gold_etl exists
    IF stream_config.gold_etl IS None THEN
        errors.push(ValidationError::Code400_NoGoldConfig { stream_id })
        RETURN ValidationResult::new(errors)
    END IF

    gold_etl <- stream_config.gold_etl.unwrap()

    // 2. Check enabled flag
    IF NOT gold_etl.enabled THEN
        errors.push(ValidationError::GoldDisabled { stream_id })
        RETURN ValidationResult::new(errors)
    END IF

    // 3. Check aggregates section exists
    IF gold_etl.aggregates IS None THEN
        errors.push(ValidationError::NoAggregatesConfig { stream_id })
        RETURN ValidationResult::new(errors)
    END IF

    aggregates <- gold_etl.aggregates.unwrap()

    // 4. Validate granularities
    FOR EACH granularity IN aggregates.granularities DO
        IF NOT IsValidGranularity(granularity) THEN
            errors.push(ValidationError::Code406_InvalidGranularity {
                stream_id,
                value: granularity,
                examples: ["1 hour", "1 day", "15 minutes"]
            })
        END IF
    END FOR

    // 5. Validate field references against Silver schema
    silver_columns <- GetSilverColumnNames(stream_config)

    FOR EACH (field_name, field_config) IN aggregates.fields DO
        IF field_name NOT IN silver_columns THEN
            errors.push(ValidationError::Code400_InvalidGoldField {
                stream_id,
                field: field_name,
                available: silver_columns
            })
        END IF

        // 6. Validate metrics for each field
        FOR EACH metric IN field_config.metrics DO
            IF NOT IsValidMetric(metric) THEN
                errors.push(ValidationError::Code403_InvalidMetric {
                    stream_id,
                    field: field_name,
                    metric: metric,
                    valid: ["mean", "std", "min", "max", "count", "sum", "p95", "p99", "first", "last"]
                })
            END IF
        END FOR
    END FOR

    // 7. Stream type specific validation
    IF stream_config.stream_type == "state_event" THEN
        // state_event streams should have transitions, not just aggregates
        IF gold_etl.transitions IS None THEN
            errors.push(ValidationError::Warning {
                message: "state_event stream may benefit from transitions config"
            })
        END IF
    END IF

    RETURN ValidationResult::new(errors)
END
```

---

## Algorithm: GenerateContinuousAggregateView

```
ALGORITHM: GenerateContinuousAggregateView
INPUT:
    stream_id: String
    gold_etl: GoldEtlConfig
    source_table: String
    timestamp_column: String
    granularity: String
    action: Action
OUTPUT: Result<String, GeneratorError>

BEGIN
    aggregates <- gold_etl.aggregates.unwrap()

    // 1. Derive view name
    view_name <- DeriveGoldViewName(stream_id, granularity)

    // 2. Build column expressions
    columns <- Vec::new()

    // 2a. Time bucket column
    bucket_expr <- format!(
        "time_bucket('{}', {}) AS bucket",
        granularity,
        timestamp_column
    )
    columns.push(bucket_expr)

    // 2b. Entity column
    entity_column <- aggregates.entity_column  // e.g., "ndp_id"
    columns.push(entity_column.clone())

    // 2c. Aggregate columns for each field
    FOR EACH (field_name, field_config) IN aggregates.fields DO
        FOR EACH metric IN field_config.metrics DO
            agg_expr <- GenerateAggregateExpression(field_name, metric, timestamp_column)
            columns.push(agg_expr)
        END FOR
    END FOR

    // 2d. Sample count for data quality
    columns.push("COUNT(*) AS sample_count")

    // 3. Generate SQL based on action
    sql <- MATCH action WITH
        | Sync => GenerateSyncModeSQL(view_name, source_table, entity_column, columns)
        | Recreate => GenerateRecreateModeSQL(view_name, source_table, entity_column, columns)

    RETURN Ok(sql)
END
```

---

## Algorithm: GenerateAggregateExpression

```
ALGORITHM: GenerateAggregateExpression
INPUT:
    field_name: String
    metric: String
    timestamp_column: String
OUTPUT: String

BEGIN
    RETURN MATCH metric.to_lowercase() WITH
        | "mean" | "avg" =>
            format!("AVG({}) AS {}_mean", field_name, field_name)

        | "std" | "stddev" =>
            format!("STDDEV({}) AS {}_std", field_name, field_name)

        | "min" =>
            format!("MIN({}) AS {}_min", field_name, field_name)

        | "max" =>
            format!("MAX({}) AS {}_max", field_name, field_name)

        | "count" =>
            format!("COUNT({}) AS {}_count", field_name, field_name)

        | "sum" =>
            format!("SUM({}) AS {}_sum", field_name, field_name)

        | "p95" =>
            format!(
                "PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY {}) AS {}_p95",
                field_name, field_name
            )

        | "p99" =>
            format!(
                "PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY {}) AS {}_p99",
                field_name, field_name
            )

        | "first" =>
            format!(
                "FIRST({}, {}) AS {}_first",
                field_name, timestamp_column, field_name
            )

        | "last" =>
            format!(
                "LAST({}, {}) AS {}_last",
                field_name, timestamp_column, field_name
            )
END
```

---

## Algorithm: GenerateSyncModeSQL

```
ALGORITHM: GenerateSyncModeSQL
INPUT:
    view_name: String
    source_table: String
    entity_column: String
    columns: Vec<String>
OUTPUT: String

BEGIN
    view_short_name <- view_name.split(".").last()

    sql <- format!(r#"
-- Continuous aggregate: {view_name}
-- Generated by ndp-gold-ddl (air-quality reference implementation)
-- Mode: SYNC (create if not exists)

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM timescaledb_information.continuous_aggregates
        WHERE view_schema = 'gold'
          AND view_name = '{view_short_name}'
    ) THEN
        CREATE MATERIALIZED VIEW {view_name}
        WITH (timescaledb.continuous) AS
        SELECT
            {column_list}
        FROM {source_table}
        GROUP BY bucket, {entity_column};

        RAISE NOTICE 'Created continuous aggregate: {view_name}';
    ELSE
        RAISE NOTICE '{view_name} already exists, skipping';
    END IF;
END $$;
"#,
        view_name = view_name,
        view_short_name = view_short_name,
        column_list = columns.join(",\n            "),
        source_table = source_table,
        entity_column = entity_column
    )

    RETURN sql
END
```

---

## Algorithm: GenerateRecreateModeSQL

```
ALGORITHM: GenerateRecreateModeSQL
INPUT:
    view_name: String
    source_table: String
    entity_column: String
    columns: Vec<String>
OUTPUT: String

BEGIN
    sql <- format!(r#"
-- Continuous aggregate: {view_name}
-- Generated by ndp-gold-ddl (air-quality reference implementation)
-- Mode: RECREATE (drop and recreate)

-- Drop existing view and dependent policies
DROP MATERIALIZED VIEW IF EXISTS {view_name} CASCADE;

-- Create continuous aggregate
CREATE MATERIALIZED VIEW {view_name}
WITH (timescaledb.continuous) AS
SELECT
    {column_list}
FROM {source_table}
GROUP BY bucket, {entity_column};
"#,
        view_name = view_name,
        column_list = columns.join(",\n    "),
        source_table = source_table,
        entity_column = entity_column
    )

    RETURN sql
END
```

---

## Algorithm: GenerateRefreshPolicy

```
ALGORITHM: GenerateRefreshPolicy
INPUT:
    stream_id: String
    granularity: String
    refresh_policy: RefreshPolicyConfig
OUTPUT: Result<String, GeneratorError>

BEGIN
    view_name <- DeriveGoldViewName(stream_id, granularity)

    // Extract policy parameters with defaults
    schedule_interval <- refresh_policy.schedule_interval.unwrap_or("15 minutes")
    start_offset <- refresh_policy.start_offset.unwrap_or("4 hours")
    end_offset <- refresh_policy.end_offset.unwrap_or("15 minutes")

    sql <- format!(r#"
-- Refresh policy for {view_name}
-- Runs every {schedule_interval}
-- Processes buckets from {start_offset} ago to {end_offset} ago

DO $$
BEGIN
    -- Remove existing policy if any
    BEGIN
        PERFORM remove_continuous_aggregate_policy('{view_name}', if_exists => TRUE);
    EXCEPTION WHEN OTHERS THEN
        NULL;  -- Policy didn't exist, continue
    END;

    -- Add refresh policy
    PERFORM add_continuous_aggregate_policy('{view_name}',
        start_offset => INTERVAL '{start_offset}',
        end_offset => INTERVAL '{end_offset}',
        schedule_interval => INTERVAL '{schedule_interval}'
    );

    RAISE NOTICE 'Added refresh policy to {view_name}';
END $$;
"#,
        view_name = view_name,
        schedule_interval = schedule_interval,
        start_offset = start_offset,
        end_offset = end_offset
    )

    RETURN Ok(sql)
END
```

---

## Algorithm: GenerateMetadataInserts

```
ALGORITHM: GenerateMetadataInserts
INPUT:
    stream_id: String
    granularity: String
    gold_etl: GoldEtlConfig
OUTPUT: Result<String, GeneratorError>

BEGIN
    view_name <- DeriveGoldViewName(stream_id, granularity)
    view_short_name <- view_name.split(".").last()
    aggregates <- gold_etl.aggregates.unwrap()

    // 1. Generate gold_tables insert
    table_insert <- format!(r#"
-- Data dictionary: gold_tables entry
INSERT INTO data_dictionary.gold_tables (
    table_name, table_schema, object_type, source_silver_table,
    source_stream_id, bucket_interval, description, updated_at
) VALUES (
    '{view_short_name}',
    'gold',
    'continuous_aggregate',
    'silver.{silver_table}',
    '{stream_id}',
    INTERVAL '{granularity}',
    '{description}',
    NOW()
)
ON CONFLICT (table_name) DO UPDATE SET
    source_silver_table = EXCLUDED.source_silver_table,
    bucket_interval = EXCLUDED.bucket_interval,
    description = EXCLUDED.description,
    updated_at = NOW();
"#,
        view_short_name = view_short_name,
        silver_table = stream_id.replace("-", "_") + "_observations",
        stream_id = stream_id,
        granularity = granularity,
        description = gold_etl.description.unwrap_or("")
    )

    // 2. Generate gold_columns inserts
    column_inserts <- Vec::new()

    // Bucket column
    column_inserts.push(GenerateColumnMetadata(
        view_short_name, "bucket", "timestamptz", "dimension", None, None
    ))

    // Entity column
    column_inserts.push(GenerateColumnMetadata(
        view_short_name, aggregates.entity_column, "text", "identity", None, None
    ))

    // Aggregate columns
    FOR EACH (field_name, field_config) IN aggregates.fields DO
        FOR EACH metric IN field_config.metrics DO
            column_name <- format!("{}_{}", field_name, metric)
            source_expr <- format!("{}({})", metric.to_uppercase(), field_name)
            column_inserts.push(GenerateColumnMetadata(
                view_short_name, column_name, "double precision", "aggregate",
                Some(field_name), Some(source_expr)
            ))
        END FOR
    END FOR

    // Sample count
    column_inserts.push(GenerateColumnMetadata(
        view_short_name, "sample_count", "bigint", "aggregate", None, Some("COUNT(*)")
    ))

    RETURN Ok(table_insert + "\n" + column_inserts.join("\n"))
END
```

---

## Algorithm: DeriveGoldViewName

```
ALGORITHM: DeriveGoldViewName
INPUT:
    stream_id: String
    granularity: String
OUTPUT: String

BEGIN
    // Convert stream-id to stream_id
    normalized_id <- stream_id.replace("-", "_")

    // Convert granularity to suffix
    suffix <- MATCH granularity WITH
        | "1 hour" | "1h" => "hourly"
        | "1 day" | "1d" => "daily"
        | "15 minutes" | "15min" => "15min"
        | "5 minutes" | "5min" => "5min"
        | _ => granularity.replace(" ", "_")

    RETURN format!("gold.{}_{}", normalized_id, suffix)
END
```

---

## Air Quality Reference Implementation

### Example Configuration

```yaml
# config/base/streams/air-quality/config.yaml
stream_id: "air-quality"
stream_type: "observation"

gold_etl:
  enabled: true
  description: "Hourly and daily aggregates for indoor air quality"

  aggregates:
    granularities: ["1 hour", "1 day"]
    entity_column: "ndp_id"
    fields:
      pm25:
        metrics: ["mean", "std", "min", "max", "p95"]
      pm10:
        metrics: ["mean", "min", "max"]
      co2:
        metrics: ["mean", "std", "min", "max"]
      temperature_c:
        metrics: ["mean", "min", "max"]
      humidity_pct:
        metrics: ["mean", "min", "max"]
      tvoc_index:
        metrics: ["mean", "max"]
      nox_index:
        metrics: ["mean", "max"]

  refresh_policy:
    schedule_interval: "15 minutes"
    start_offset: "4 hours"
    end_offset: "15 minutes"
```

### Generated DDL (Hourly)

```sql
CREATE MATERIALIZED VIEW gold.air_quality_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', observation_time) AS bucket,
    ndp_id,
    AVG(pm25) AS pm25_mean,
    STDDEV(pm25) AS pm25_std,
    MIN(pm25) AS pm25_min,
    MAX(pm25) AS pm25_max,
    PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY pm25) AS pm25_p95,
    AVG(pm10) AS pm10_mean,
    MIN(pm10) AS pm10_min,
    MAX(pm10) AS pm10_max,
    AVG(co2) AS co2_mean,
    STDDEV(co2) AS co2_std,
    MIN(co2) AS co2_min,
    MAX(co2) AS co2_max,
    AVG(temperature_c) AS temperature_c_mean,
    MIN(temperature_c) AS temperature_c_min,
    MAX(temperature_c) AS temperature_c_max,
    AVG(humidity_pct) AS humidity_pct_mean,
    MIN(humidity_pct) AS humidity_pct_min,
    MAX(humidity_pct) AS humidity_pct_max,
    AVG(tvoc_index) AS tvoc_index_mean,
    MAX(tvoc_index) AS tvoc_index_max,
    AVG(nox_index) AS nox_index_mean,
    MAX(nox_index) AS nox_index_max,
    COUNT(*) AS sample_count
FROM silver.air_quality_observations
GROUP BY bucket, ndp_id;
```

---

## Complexity Analysis

| Operation | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| Load config | O(1) | O(c) where c = config size |
| Validation | O(f * m) | O(1) |
| Generate view DDL | O(f * m) | O(f * m) |
| Generate refresh policy | O(1) | O(1) |
| Generate metadata | O(f * m) | O(f * m) |
| Total per granularity | O(f * m) | O(f * m) |

Where: f = fields, m = metrics per field

---

## Error Handling

```
ENUM GeneratorError:
    Code400_InvalidGoldField { stream_id, field, available }
    Code403_InvalidMetric { stream_id, field, metric, valid }
    Code406_InvalidGranularity { stream_id, value, examples }
    NoGoldConfig { stream_id }
    GoldDisabled { stream_id }
    NoAggregatesConfig { stream_id }
    ValidationFailed { errors: Vec<ValidationError> }
```

---

## Invariants

1. **View Naming Convention**: `gold.{stream_id}_{granularity_suffix}`
2. **Bucket First**: Time bucket column always first in SELECT
3. **Entity Second**: Entity column always second
4. **Sample Count Included**: Always include COUNT(*) AS sample_count
5. **Idempotent Sync**: Sync mode never fails on existing views
6. **Cascade Drop**: Recreate mode drops with CASCADE

---

## Test Cases (London TDD)

```
TRAITS TO MOCK:
    - ConfigLoader: Return test stream configs
    - DatabaseExecutor: Capture generated SQL

TEST: GenerateAirQualityHourlyAggregate
    GIVEN air-quality config with gold_etl.aggregates
    AND granularities = ["1 hour"]
    WHEN GenerateSingleStreamAggregate() is called
    THEN output contains "CREATE MATERIALIZED VIEW gold.air_quality_hourly"
    AND output contains "AVG(pm25) AS pm25_mean"
    AND output contains "AVG(co2) AS co2_mean"
    AND output contains "COUNT(*) AS sample_count"

TEST: GenerateDailyAlongsideHourly
    GIVEN air-quality config with granularities = ["1 hour", "1 day"]
    WHEN GenerateSingleStreamAggregate() is called
    THEN output contains "gold.air_quality_hourly"
    AND output contains "gold.air_quality_daily"
    AND two refresh policies are generated

TEST: ValidateInvalidFieldReference
    GIVEN gold_etl.aggregates.fields contains "nonexistent_field"
    WHEN ValidateGoldEtlConfig() is called
    THEN ValidationError::Code400_InvalidGoldField is returned

TEST: ValidateInvalidMetricType
    GIVEN field_config.metrics contains "invalid_metric"
    WHEN ValidateGoldEtlConfig() is called
    THEN ValidationError::Code403_InvalidMetric is returned

TEST: SyncModeUsesIfNotExists
    GIVEN action = Sync
    WHEN GenerateContinuousAggregateView() is called
    THEN output contains "IF NOT EXISTS"

TEST: RecreateModeDropsFirst
    GIVEN action = Recreate
    WHEN GenerateContinuousAggregateView() is called
    THEN output contains "DROP MATERIALIZED VIEW IF EXISTS"
    AND DROP appears before CREATE

TEST: RefreshPolicyConfigurable
    GIVEN refresh_policy.schedule_interval = "30 minutes"
    WHEN GenerateRefreshPolicy() is called
    THEN output contains "schedule_interval => INTERVAL '30 minutes'"

TEST: MetadataInsertsGeneratedCorrectly
    GIVEN air-quality config
    WHEN GenerateMetadataInserts() is called
    THEN output contains "INSERT INTO data_dictionary.gold_tables"
    AND output contains "INSERT INTO data_dictionary.gold_columns"
```

---

## References

- [SPEC-B03-continuous-aggregates.md](../specification/SPEC-B03-continuous-aggregates.md)
- [SPEC-B04-refresh-policy.md](../specification/SPEC-B04-refresh-policy.md)
- [ALGO-continuous-aggregate.md](../../phase-a/pseudocode/ALGO-continuous-aggregate.md) - Phase A generic algorithm
- [TimescaleDB Continuous Aggregates](https://docs.timescale.com/use-timescale/latest/continuous-aggregates/)
