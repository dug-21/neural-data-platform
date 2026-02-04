# ALGO-continuous-aggregate: TimescaleDB Continuous Aggregate Generator

> **Algorithm ID:** A02
> **Feature:** v11-A02 (Gold DDL Tool)
> **Created:** 2026-02-04

---

## Purpose

Generate TimescaleDB continuous aggregate DDL from stream configuration. This algorithm transforms declarative `gold_etl.aggregates` config into executable SQL that creates materialized views with automatic refresh policies.

---

## Algorithm: Generate Continuous Aggregate

```
ALGORITHM: GenerateContinuousAggregate
INPUT:
    stream_id: String
    gold_etl: GoldEtlConfig
    source_table: String           // e.g., "silver.air_quality_observations"
    granularity: String            // e.g., "1 hour"
    action: Action
    feature_registry: FeatureRegistry
OUTPUT: Result<String, GeneratorError>
REQUIRES:
    - gold_etl.enabled = true
    - gold_etl.aggregates is Some
    - All field references are valid

BEGIN
    aggregates <- gold_etl.aggregates.unwrap()

    // 1. Generate view name from stream and granularity
    granularity_suffix <- granularity_to_suffix(granularity)
    view_name <- format!("gold.{}_{}", stream_id.replace("-", "_"), granularity_suffix)

    // 2. Build column expressions
    columns <- Vec::new()

    // 2a. Add time bucket column
    timestamp_column <- extract_timestamp_column(source_table)
    bucket_expr <- format!("time_bucket('{}', {}) AS bucket", granularity, timestamp_column)
    columns.push(bucket_expr)

    // 2b. Add entity column (for partitioning by device/sensor)
    entity_column <- aggregates.entity_column  // e.g., "ndp_id"
    columns.push(entity_column.clone())

    // 2c. Generate aggregate expressions for each field
    FOR EACH (field_name, field_metrics) IN aggregates.fields DO
        FOR EACH metric IN field_metrics.metrics DO
            agg_expr <- generate_aggregate_expression(field_name, metric, timestamp_column)
            columns.push(agg_expr)
        END FOR
    END FOR

    // 2d. Add sample count for data quality tracking
    columns.push("COUNT(*) AS sample_count")

    // 2e. Generate feature columns if features configured
    IF gold_etl.features IS Some(features) THEN
        feature_columns <- generate_feature_columns(features, aggregates.fields.keys(), feature_registry)
        columns.extend(feature_columns)
    END IF

    // 3. Build the CREATE statement based on action
    sql <- MATCH action WITH
        | Sync => generate_sync_create(view_name, source_table, entity_column, columns)
        | Recreate => generate_recreate_create(view_name, source_table, entity_column, columns)

    RETURN Ok(sql)
END
```

---

## Algorithm: Generate Aggregate Expression

```
ALGORITHM: GenerateAggregateExpression
INPUT:
    field_name: String       // e.g., "pm25"
    metric: String           // e.g., "mean"
    timestamp_column: String // e.g., "observation_time"
OUTPUT: String (SQL expression)
REQUIRES: metric is a valid aggregate metric type

BEGIN
    // Generate the aggregate SQL expression
    expression <- MATCH metric.to_lowercase() WITH
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

        | unknown =>
            RETURN Err(GeneratorError::InvalidMetric(unknown))

    RETURN expression
END
```

---

## Algorithm: Generate Sync Create (Idempotent)

```
ALGORITHM: GenerateSyncCreate
INPUT:
    view_name: String
    source_table: String
    entity_column: String
    columns: Vec<String>
OUTPUT: String (SQL DDL)

BEGIN
    // Use PL/pgSQL DO block for idempotent creation
    sql <- format!(r#"
-- Continuous aggregate: {view_name}
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
        view_short_name = extract_short_name(view_name),
        column_list = columns.join(",\n            "),
        source_table = source_table,
        entity_column = entity_column
    )

    RETURN sql
END
```

---

## Algorithm: Generate Recreate Create

```
ALGORITHM: GenerateRecreateCreate
INPUT:
    view_name: String
    source_table: String
    entity_column: String
    columns: Vec<String>
OUTPUT: String (SQL DDL)

BEGIN
    sql <- format!(r#"
-- Continuous aggregate: {view_name}
-- Mode: RECREATE (drop and create)

-- Drop existing view and policies (CASCADE removes dependent policies)
DROP MATERIALIZED VIEW IF EXISTS {view_name} CASCADE;

-- Create new continuous aggregate
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

## Algorithm: Generate Refresh Policy

```
ALGORITHM: GenerateRefreshPolicy
INPUT:
    stream_id: String
    granularity: String
    refresh_interval: String     // e.g., "15 minutes"
    start_offset: String         // e.g., "4 hours"
    end_offset: String           // e.g., "15 minutes"
OUTPUT: String (SQL DDL)

BEGIN
    granularity_suffix <- granularity_to_suffix(granularity)
    view_name <- format!("gold.{}_{}", stream_id.replace("-", "_"), granularity_suffix)

    sql <- format!(r#"
-- Refresh policy for {view_name}
-- Refreshes every {refresh_interval}
-- Processes data from {start_offset} ago to {end_offset} ago

DO $$
BEGIN
    -- Remove existing policy if any (for recreate mode)
    BEGIN
        PERFORM remove_continuous_aggregate_policy('{view_name}', if_exists => TRUE);
    EXCEPTION WHEN OTHERS THEN
        -- Policy doesn't exist, continue
        NULL;
    END;

    -- Add new policy
    PERFORM add_continuous_aggregate_policy('{view_name}',
        start_offset => INTERVAL '{start_offset}',
        end_offset => INTERVAL '{end_offset}',
        schedule_interval => INTERVAL '{refresh_interval}'
    );

    RAISE NOTICE 'Added refresh policy to {view_name}: every {refresh_interval}';
END $$;
"#,
        view_name = view_name,
        refresh_interval = refresh_interval,
        start_offset = start_offset,
        end_offset = end_offset
    )

    RETURN sql
END
```

---

## Algorithm: Generate Feature Columns

```
ALGORITHM: GenerateFeatureColumns
INPUT:
    features: FeaturesConfig
    aggregate_fields: Set<String>     // Fields that have aggregates defined
    feature_registry: FeatureRegistry
OUTPUT: Result<Vec<String>, FeatureError>

BEGIN
    columns <- Vec::new()

    // Process lag features
    IF features.lag IS Some(lag) AND lag.enabled THEN
        lag_generator <- feature_registry.get("lag")?

        FOR EACH field IN lag.fields DO
            // Features operate on aggregate columns (e.g., pm25_mean, not raw pm25)
            base_column <- format!("{}_mean", field)

            lag_columns <- lag_generator.generate_columns(features, base_column)?
            FOR EACH col IN lag_columns DO
                columns.push(col.expression + " AS " + col.alias)
            END FOR
        END FOR
    END IF

    // Process rolling features
    IF features.rolling IS Some(rolling) AND rolling.enabled THEN
        rolling_generator <- feature_registry.get("rolling")?

        FOR EACH field IN rolling.fields DO
            base_column <- format!("{}_mean", field)

            rolling_columns <- rolling_generator.generate_columns(features, base_column)?
            FOR EACH col IN rolling_columns DO
                columns.push(col.expression + " AS " + col.alias)
            END FOR
        END FOR
    END IF

    // Process trend features
    IF features.trend IS Some(trend) AND trend.enabled THEN
        trend_generator <- feature_registry.get("trend")?

        FOR EACH field IN trend.fields DO
            base_column <- format!("{}_mean", field)

            trend_columns <- trend_generator.generate_columns(features, base_column)?
            FOR EACH col IN trend_columns DO
                columns.push(col.expression + " AS " + col.alias)
            END FOR
        END FOR
    END IF

    RETURN Ok(columns)
END
```

---

## Algorithm: Extract Timestamp Column

```
ALGORITHM: ExtractTimestampColumn
INPUT: source_table: String
OUTPUT: String

BEGIN
    // Standard convention: Silver tables use observation_time
    // Forecasts use issue_time as primary timestamp
    // This could be extended to read from stream config if needed

    RETURN "observation_time"
END
```

---

## Algorithm: Multiple Granularities

```
ALGORITHM: GenerateMultipleGranularities
INPUT:
    stream_id: String
    gold_etl: GoldEtlConfig
    source_table: String
    action: Action
    feature_registry: FeatureRegistry
OUTPUT: Result<String, GeneratorError>
REQUIRES: gold_etl.aggregates.granularities is non-empty

BEGIN
    aggregates <- gold_etl.aggregates.unwrap()
    output <- StringWriter::new()

    FOR EACH granularity IN aggregates.granularities DO
        // 1. Generate the continuous aggregate view
        view_sql <- generate_continuous_aggregate(
            stream_id, gold_etl, source_table, granularity, action, feature_registry
        )?
        output.write_statement(view_sql)
        output.write_blank_line()

        // 2. Generate the refresh policy
        policy_sql <- generate_refresh_policy(
            stream_id,
            granularity,
            aggregates.refresh_interval,
            aggregates.start_offset,
            aggregates.end_offset
        )?
        output.write_statement(policy_sql)
        output.write_blank_line()
    END FOR

    RETURN output.finish()
END
```

---

## SQL Template Examples

### Example: Air Quality Hourly Aggregate

Given config:
```yaml
gold_etl:
  enabled: true
  aggregates:
    granularities: ["1 hour"]
    entity_column: ndp_id
    fields:
      pm25: { metrics: [mean, std, max] }
      co2: { metrics: [mean, std, min, max] }
    refresh_interval: 15 minutes
    start_offset: 4 hours
    end_offset: 15 minutes
```

Generated SQL (sync mode):
```sql
-- Continuous aggregate: gold.air_quality_hourly
-- Mode: SYNC (create if not exists)

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM timescaledb_information.continuous_aggregates
        WHERE view_schema = 'gold'
          AND view_name = 'air_quality_hourly'
    ) THEN
        CREATE MATERIALIZED VIEW gold.air_quality_hourly
        WITH (timescaledb.continuous) AS
        SELECT
            time_bucket('1 hour', observation_time) AS bucket,
            ndp_id,
            AVG(pm25) AS pm25_mean,
            STDDEV(pm25) AS pm25_std,
            MAX(pm25) AS pm25_max,
            AVG(co2) AS co2_mean,
            STDDEV(co2) AS co2_std,
            MIN(co2) AS co2_min,
            MAX(co2) AS co2_max,
            COUNT(*) AS sample_count
        FROM silver.air_quality_observations
        GROUP BY bucket, ndp_id;

        RAISE NOTICE 'Created continuous aggregate: gold.air_quality_hourly';
    ELSE
        RAISE NOTICE 'gold.air_quality_hourly already exists, skipping';
    END IF;
END $$;

-- Refresh policy for gold.air_quality_hourly
DO $$
BEGIN
    BEGIN
        PERFORM remove_continuous_aggregate_policy('gold.air_quality_hourly', if_exists => TRUE);
    EXCEPTION WHEN OTHERS THEN
        NULL;
    END;

    PERFORM add_continuous_aggregate_policy('gold.air_quality_hourly',
        start_offset => INTERVAL '4 hours',
        end_offset => INTERVAL '15 minutes',
        schedule_interval => INTERVAL '15 minutes'
    );

    RAISE NOTICE 'Added refresh policy to gold.air_quality_hourly';
END $$;
```

### Example: With Features (Lag)

Given additional config:
```yaml
  features:
    lag:
      enabled: true
      lags_hours: [1, 6, 24]
      fields: [pm25]
```

Additional columns in SELECT:
```sql
    LAG(AVG(pm25), 1) OVER (PARTITION BY ndp_id ORDER BY bucket) AS pm25_mean_lag_1h,
    LAG(AVG(pm25), 6) OVER (PARTITION BY ndp_id ORDER BY bucket) AS pm25_mean_lag_6h,
    LAG(AVG(pm25), 24) OVER (PARTITION BY ndp_id ORDER BY bucket) AS pm25_mean_lag_24h,
```

**Note:** Window functions in continuous aggregates require TimescaleDB 2.10+. For older versions, features are computed in a separate view on top of the base aggregate.

---

## Complexity Analysis

| Operation | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| Parse granularity | O(1) | O(1) |
| Generate aggregate expr | O(1) per metric | O(1) |
| Generate all columns | O(f * m) | O(f * m) |
| Build SQL string | O(c) where c = total columns | O(c) |
| Total per view | O(f * m) | O(f * m) |

Where:
- f = number of fields
- m = average metrics per field

---

## Error Handling

```
ENUM GeneratorError:
    // Invalid metric type (code 403)
    InvalidMetric {
        metric: String,
        valid_metrics: Vec<String>
    }

    // Missing gold_etl configuration
    NoGoldConfig {
        stream_id: String
    }

    // Gold layer is disabled
    GoldDisabled {
        stream_id: String
    }

    // Invalid granularity format (code 406)
    InvalidGranularity {
        value: String,
        examples: Vec<String>
    }

    // Feature generation failed
    FeatureError {
        feature_type: String,
        message: String
    }
```

---

## Invariants

1. **View Naming**: View name follows pattern `gold.{stream_id}_{suffix}` where underscores replace hyphens
2. **Bucket Column**: First column is always `bucket` from `time_bucket()`
3. **Entity Column**: Second column is always the entity (e.g., `ndp_id`)
4. **Sample Count**: `sample_count` column is always included for DQ
5. **Idempotency**: Sync mode never fails on existing views
6. **Policy Removal**: Recreate mode removes existing policies before recreating

---

## Test Cases (London TDD)

```
TEST: GenerateBasicAggregate
    GIVEN gold_etl with aggregates.fields.pm25.metrics = [mean, std]
    AND granularity = "1 hour"
    WHEN generate_continuous_aggregate() is called
    THEN SQL contains "AVG(pm25) AS pm25_mean"
    AND SQL contains "STDDEV(pm25) AS pm25_std"
    AND SQL contains "time_bucket('1 hour'"

TEST: GenerateSyncModeIsIdempotent
    GIVEN action = Sync
    WHEN generate_sync_create() is called
    THEN SQL contains "IF NOT EXISTS"
    AND SQL contains "timescaledb_information.continuous_aggregates"

TEST: GenerateRecreateModeDropsFirst
    GIVEN action = Recreate
    WHEN generate_recreate_create() is called
    THEN SQL contains "DROP MATERIALIZED VIEW IF EXISTS"
    AND "DROP" appears before "CREATE"

TEST: GeneratePercentileMetrics
    GIVEN metrics = [p95, p99]
    WHEN generate_aggregate_expression() is called
    THEN SQL contains "PERCENTILE_CONT(0.95)"
    AND SQL contains "PERCENTILE_CONT(0.99)"

TEST: GenerateFirstLastMetrics
    GIVEN metrics = [first, last]
    WHEN generate_aggregate_expression() is called
    THEN SQL contains "FIRST(pm25, observation_time)"
    AND SQL contains "LAST(pm25, observation_time)"

TEST: GenerateMultipleGranularities
    GIVEN granularities = ["1 hour", "1 day"]
    WHEN generate_multiple_granularities() is called
    THEN output contains "gold.air_quality_hourly"
    AND output contains "gold.air_quality_daily"
    AND two refresh policies are generated

TEST: RefreshPolicyConfiguration
    GIVEN refresh_interval = "15 minutes"
    AND start_offset = "4 hours"
    AND end_offset = "15 minutes"
    WHEN generate_refresh_policy() is called
    THEN SQL contains "schedule_interval => INTERVAL '15 minutes'"
    AND SQL contains "start_offset => INTERVAL '4 hours'"
    AND SQL contains "end_offset => INTERVAL '15 minutes'"
```

---

## Integration with Feature Registry

The continuous aggregate generator collaborates with the Feature Registry (A04) for generating feature columns. The workflow:

1. Generator creates base aggregate columns (mean, std, etc.)
2. If `features` config exists, generator calls `feature_registry.generate_all()`
3. Feature generators return SQL expressions for their column type
4. Generator appends feature columns to the SELECT clause

**Constraint**: Some window functions may not be compatible with continuous aggregates. In such cases, features are computed in a secondary view that references the base aggregate.

---

## References

- [SPEC-A02](../specification/SPEC-A02-gold-ddl-tool.md) - Gold DDL Tool specification
- [TimescaleDB Continuous Aggregates](https://docs.timescale.com/use-timescale/latest/continuous-aggregates/)
- [Silver ETL schema_gen.rs](/workspaces/neural-data-platform/apps/silver-etl/src/schema_gen.rs) - Pattern reference
