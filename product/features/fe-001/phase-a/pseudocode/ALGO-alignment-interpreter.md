# ALGO-alignment-interpreter: Cross-Stream Aligned View Generator

> **Algorithm ID:** A03
> **Feature:** v11-A04 (Alignment Interpreter)
> **Created:** 2026-02-04

---

## Purpose

Generate SQL for domain-aligned materialized views that JOIN multiple Gold layer streams. This algorithm implements the cross-stream correlation foundation required for V1.2 pattern detection, handling NULL strategies per stream type and forecast alignment on `issued_at`.

---

## Key Design Decisions Referenced

- **ADR-FE001-003**: Forecast streams align on `issued_at`, not `valid_time` (causal validity)
- **ADR-FE001-004**: NULL handling varies by stream_type (preserve vs carry-forward)

---

## Algorithm: Generate Aligned View

```
ALGORITHM: GenerateAlignedView
INPUT:
    domain_config: DomainConfig
    action: Action
OUTPUT: Result<String, GeneratorError>
REQUIRES:
    - All referenced streams have Gold layer enabled
    - Primary stream is defined
    - At least 2 streams for alignment

BEGIN
    alignment <- domain_config.alignment

    // 1. Sort streams to ensure primary is first
    streams <- sort_streams_by_role(domain_config.streams)
    primary_stream <- streams[0]

    IF streams.len() < 2 THEN
        RETURN Err(GeneratorError::InsufficientStreams(domain_config.id))
    END IF

    // 2. Load stream configurations to get Gold table info
    stream_configs <- Map::new()
    FOR EACH stream_ref IN streams DO
        config <- config_loader.load_stream_config(stream_ref.stream_id)?
        stream_configs.insert(stream_ref.stream_id, config)
    END FOR

    // 3. Build aligned stream metadata
    aligned_streams <- Vec::new()
    FOR EACH stream_ref IN streams DO
        config <- stream_configs.get(stream_ref.stream_id)
        gold_table <- derive_gold_table_name(stream_ref.stream_id, alignment.granularity)
        columns <- derive_gold_columns(config.gold_etl)

        aligned_streams.push(AlignedStream {
            stream_id: stream_ref.stream_id,
            alias: stream_ref.alias,
            role: stream_ref.role,
            stream_type: config.stream_type,
            gold_table: gold_table,
            columns: columns,
            null_handling: stream_ref.null_handling.or(default_null_handling(config.stream_type))
        })
    END FOR

    // 4. Generate SQL based on action
    sql <- MATCH action WITH
        | Sync => generate_sync_aligned_view(alignment, aligned_streams)
        | Recreate => generate_recreate_aligned_view(alignment, aligned_streams)

    RETURN Ok(sql)
END
```

---

## Algorithm: Sort Streams by Role

```
ALGORITHM: SortStreamsByRole
INPUT: streams: Vec<StreamReference>
OUTPUT: Vec<StreamReference> (sorted with primary first)

ROLE_ORDER:
    primary   = 0
    context   = 1
    actuator  = 2
    constraint = 3

BEGIN
    // Stable sort to maintain order within same role
    sorted <- streams.clone()
    sorted.sort_by_key(|s| ROLE_ORDER.get(s.role).unwrap_or(99))

    // Verify primary exists
    IF sorted[0].role != "primary" THEN
        RETURN Err(GeneratorError::NoPrimaryStream)
    END IF

    RETURN sorted
END
```

---

## Algorithm: Generate Column Expressions

```
ALGORITHM: GenerateColumnExpressions
INPUT:
    aligned_streams: Vec<AlignedStream>
    null_handlers: Map<String, Box<dyn NullHandler>>
OUTPUT: Vec<String> (SQL column expressions)

BEGIN
    expressions <- Vec::new()

    // 1. Generate bucket coalesce expression
    bucket_aliases <- aligned_streams.iter().map(|s| format!("{}.bucket", s.alias)).collect()
    expressions.push(format!("COALESCE({}) AS bucket", bucket_aliases.join(", ")))

    // 2. Generate columns for each stream
    FOR EACH stream IN aligned_streams DO
        // Get appropriate null handler for this stream
        null_handler <- null_handlers.get(stream.stream_type)?

        FOR EACH column IN stream.columns DO
            // Skip bucket column (already handled above)
            IF column == "bucket" THEN
                CONTINUE
            END IF

            // Generate aliased column with null handling
            source_expr <- format!("{}.{}", stream.alias, column)
            target_alias <- format!("{}_{}", stream.alias, column)

            wrapped_expr <- null_handler.wrap_column(source_expr, target_alias, stream.alias)
            expressions.push(wrapped_expr)
        END FOR

        // Add sample count with stream prefix
        expressions.push(format!(
            "COALESCE({}.sample_count, 0) AS {}_samples",
            stream.alias, stream.alias
        ))
    END FOR

    // 3. Add total samples column
    sample_exprs <- aligned_streams.iter()
        .map(|s| format!("COALESCE({}.sample_count, 0)", s.alias))
        .collect()
    expressions.push(format!("{} AS total_samples", sample_exprs.join(" + ")))

    RETURN expressions
END
```

---

## Algorithm: Generate Join Clauses

```
ALGORITHM: GenerateJoinClauses
INPUT:
    aligned_streams: Vec<AlignedStream>
    join_strategy: JoinStrategy
OUTPUT: String (SQL JOIN clauses)

BEGIN
    primary <- aligned_streams[0]
    other_streams <- aligned_streams[1..]

    joins <- Vec::new()

    // First line: FROM clause with primary stream
    from_clause <- format!("FROM {} {}", primary.gold_table, primary.alias)

    // Build join for each subsequent stream
    FOR i, stream IN other_streams.enumerate() DO
        // Determine join type
        join_type <- MATCH join_strategy WITH
            | FullOuter => "FULL OUTER JOIN"
            | Left => "LEFT JOIN"
            | Inner => "INNER JOIN"

        // Build join condition
        join_condition <- MATCH join_strategy WITH
            | FullOuter =>
                // For full outer, coalesce all previous buckets
                IF i == 0 THEN
                    format!("{}.bucket = {}.bucket", primary.alias, stream.alias)
                ELSE
                    previous_aliases <- aligned_streams[0..i+1].iter()
                        .map(|s| format!("{}.bucket", s.alias))
                        .collect()
                    format!("COALESCE({}) = {}.bucket",
                        previous_aliases.join(", "),
                        stream.alias
                    )
                END IF

            | Left | Inner =>
                format!("{}.bucket = {}.bucket", primary.alias, stream.alias)

        // Handle forecast streams specially (ADR-FE001-003)
        IF stream.stream_type == "forecast" THEN
            join_clause <- generate_forecast_lateral_join(stream, primary, aligned_streams[0..i+1])
        ELSE
            join_clause <- format!(
                "{} {} {}\n    ON {}",
                join_type,
                stream.gold_table,
                stream.alias,
                join_condition
            )
        END IF

        joins.push(join_clause)
    END FOR

    RETURN from_clause + "\n" + joins.join("\n")
END
```

---

## Algorithm: Generate Forecast LATERAL Join

```
ALGORITHM: GenerateForecastLateralJoin
INPUT:
    forecast_stream: AlignedStream
    primary: AlignedStream
    previous_streams: Vec<AlignedStream>
OUTPUT: String (SQL LATERAL join clause)
REQUIRES: forecast_stream.stream_type == "forecast"

BEGIN
    // Build bucket reference from coalesced previous streams
    bucket_expr <- IF previous_streams.len() == 1 THEN
        format!("{}.bucket", previous_streams[0].alias)
    ELSE
        aliases <- previous_streams.iter().map(|s| format!("{}.bucket", s.alias)).collect()
        format!("COALESCE({})", aliases.join(", "))
    END IF

    // Generate LATERAL join that selects the most recent forecast
    // available at the time of each bucket
    join_sql <- format!(r#"
LEFT JOIN LATERAL (
    SELECT * FROM {gold_table} f
    WHERE f.issued_at <= {bucket_expr}
    ORDER BY f.issued_at DESC
    LIMIT 1
) {alias} ON TRUE"#,
        gold_table = forecast_stream.gold_table,
        bucket_expr = bucket_expr,
        alias = forecast_stream.alias
    )

    RETURN join_sql
END
```

---

## Algorithm: Default NULL Handling by Stream Type

```
ALGORITHM: DefaultNullHandling
INPUT: stream_type: String
OUTPUT: NullHandling

BEGIN
    RETURN MATCH stream_type WITH
        | "observation" => NullHandling::Preserve
        | "forecast" => NullHandling::Preserve
        | "state_event" => NullHandling::CarryForward
        | "dimension" => NullHandling::CarryForward
        | _ => NullHandling::Preserve  // Default: don't fabricate data
END
```

---

## Algorithm: NULL Handler - Preserve

```
ALGORITHM: PreserveNullHandler
IMPLEMENTS: NullHandler

METHOD wrap_column(source_expr, target_alias, table_alias):
    // Pass through without transformation
    RETURN format!("{} AS {}", source_expr, target_alias)
```

---

## Algorithm: NULL Handler - Carry Forward (LOCF)

```
ALGORITHM: CarryForwardNullHandler
IMPLEMENTS: NullHandler

METHOD wrap_column(source_expr, target_alias, table_alias):
    // Use LAG with IGNORE NULLS to carry forward last known value
    // Requires window frame over ordered buckets
    RETURN format!(r#"
COALESCE(
    {source_expr},
    LAG({source_expr}) IGNORE NULLS OVER (ORDER BY bucket)
) AS {target_alias}"#,
        source_expr = source_expr,
        target_alias = target_alias
    )
```

---

## Algorithm: NULL Handler - Interpolate

```
ALGORITHM: InterpolateNullHandler
IMPLEMENTS: NullHandler

METHOD wrap_column(source_expr, target_alias, table_alias):
    // Linear interpolation between surrounding non-NULL values
    // Note: This is more computationally expensive
    RETURN format!(r#"
CASE
    WHEN {source_expr} IS NOT NULL THEN {source_expr}
    ELSE (
        LAG({source_expr}) IGNORE NULLS OVER (ORDER BY bucket) +
        LEAD({source_expr}) IGNORE NULLS OVER (ORDER BY bucket)
    ) / 2.0
END AS {target_alias}"#,
        source_expr = source_expr,
        target_alias = target_alias
    )
```

---

## Algorithm: Generate Sync Aligned View

```
ALGORITHM: GenerateSyncAlignedView
INPUT:
    alignment: AlignmentConfig
    aligned_streams: Vec<AlignedStream>
OUTPUT: String (SQL DDL)

BEGIN
    columns <- generate_column_expressions(aligned_streams, null_handlers)
    joins <- generate_join_clauses(aligned_streams, alignment.join_strategy)

    sql <- format!(r#"
-- Aligned view for domain: {domain_id}
-- Streams: {stream_list}
-- Mode: SYNC (create if not exists)

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_matviews
        WHERE schemaname = 'gold'
          AND matviewname = '{view_name}'
    ) THEN
        CREATE MATERIALIZED VIEW gold.{view_name} AS
        SELECT
            {column_list}
        {join_clauses}
        WHERE COALESCE({bucket_coalesce}) >= NOW() - INTERVAL '90 days';

        RAISE NOTICE 'Created aligned view: gold.{view_name}';
    ELSE
        RAISE NOTICE 'gold.{view_name} already exists, skipping';
    END IF;
END $$;
"#,
        domain_id = alignment.domain_id,
        stream_list = aligned_streams.iter().map(|s| s.alias).join(", "),
        view_name = alignment.view_name,
        column_list = columns.join(",\n            "),
        join_clauses = joins,
        bucket_coalesce = aligned_streams.iter()
            .map(|s| format!("{}.bucket", s.alias))
            .join(", ")
    )

    RETURN sql
END
```

---

## Algorithm: Generate Recreate Aligned View

```
ALGORITHM: GenerateRecreateAlignedView
INPUT:
    alignment: AlignmentConfig
    aligned_streams: Vec<AlignedStream>
OUTPUT: String (SQL DDL)

BEGIN
    columns <- generate_column_expressions(aligned_streams, null_handlers)
    joins <- generate_join_clauses(aligned_streams, alignment.join_strategy)

    sql <- format!(r#"
-- Aligned view for domain: {domain_id}
-- Streams: {stream_list}
-- Mode: RECREATE (drop and create)

-- Drop existing view
DROP MATERIALIZED VIEW IF EXISTS gold.{view_name} CASCADE;

-- Create aligned view
CREATE MATERIALIZED VIEW gold.{view_name} AS
SELECT
    {column_list}
{join_clauses}
WHERE COALESCE({bucket_coalesce}) >= NOW() - INTERVAL '90 days';
"#,
        domain_id = alignment.domain_id,
        stream_list = aligned_streams.iter().map(|s| s.alias).join(", "),
        view_name = alignment.view_name,
        column_list = columns.join(",\n    "),
        join_clauses = joins,
        bucket_coalesce = aligned_streams.iter()
            .map(|s| format!("{}.bucket", s.alias))
            .join(", ")
    )

    RETURN sql
END
```

---

## Algorithm: Derive Gold Table Name

```
ALGORITHM: DeriveGoldTableName
INPUT:
    stream_id: String
    granularity: String
OUTPUT: String (fully qualified table name)

BEGIN
    // Convert stream-id to stream_id (hyphen to underscore)
    normalized_id <- stream_id.replace("-", "_")

    // Convert granularity to suffix
    suffix <- granularity_to_suffix(granularity)

    RETURN format!("gold.{}_{}", normalized_id, suffix)
END
```

---

## Algorithm: Derive Gold Columns

```
ALGORITHM: DeriveGoldColumns
INPUT: gold_etl: GoldEtlConfig
OUTPUT: Vec<String> (list of column names)

BEGIN
    columns <- Vec::new()

    // Always include bucket
    columns.push("bucket")

    IF gold_etl.aggregates IS Some(aggregates) THEN
        // Add aggregate columns
        FOR EACH (field_name, field_metrics) IN aggregates.fields DO
            FOR EACH metric IN field_metrics.metrics DO
                columns.push(format!("{}_{}", field_name, metric))
            END FOR
        END FOR

        // Always include sample_count
        columns.push("sample_count")
    END IF

    // Add feature columns if configured
    IF gold_etl.features IS Some(features) THEN
        columns.extend(derive_feature_column_names(features))
    END IF

    // Add transition columns for state_event streams
    IF gold_etl.transitions IS Some(transitions) THEN
        columns.push("transition_count")
        FOR EACH state_field IN transitions.fields DO
            columns.push(format!("{}_state", state_field))
        END FOR
    END IF

    RETURN columns
END
```

---

## SQL Template Example

### Complete Aligned View (Indoor Air Quality Domain)

Given domain config:
```yaml
domain:
  id: indoor-air-quality
  streams:
    - stream_id: air-quality
      alias: indoor
      role: primary
    - stream_id: outdoor-weather
      alias: outdoor
      role: context
    - stream_id: home-assistant-state
      alias: state
      role: actuator
  alignment:
    view_name: indoor_air_quality_aligned
    granularity: "1 hour"
    join_strategy: full_outer
```

Generated SQL:
```sql
-- Aligned view for domain: indoor-air-quality
-- Streams: indoor, outdoor, state
-- Mode: SYNC (create if not exists)

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_matviews
        WHERE schemaname = 'gold'
          AND matviewname = 'indoor_air_quality_aligned'
    ) THEN
        CREATE MATERIALIZED VIEW gold.indoor_air_quality_aligned AS
        SELECT
            -- Bucket column (coalesced from all streams)
            COALESCE(indoor.bucket, outdoor.bucket, state.bucket) AS bucket,

            -- Indoor Air Quality (observation - primary)
            indoor.pm25_mean AS indoor_pm25_mean,
            indoor.pm25_std AS indoor_pm25_std,
            indoor.pm25_max AS indoor_pm25_max,
            indoor.co2_mean AS indoor_co2_mean,
            indoor.co2_std AS indoor_co2_std,
            indoor.temp_mean AS indoor_temp_mean,
            indoor.humidity_mean AS indoor_humidity_mean,
            COALESCE(indoor.sample_count, 0) AS indoor_samples,

            -- Outdoor Weather (observation - context)
            outdoor.temp_mean AS outdoor_temp_mean,
            outdoor.humidity_mean AS outdoor_humidity_mean,
            outdoor.wind_speed_mean AS outdoor_wind_speed_mean,
            outdoor.pressure_mean AS outdoor_pressure_mean,
            COALESCE(outdoor.sample_count, 0) AS outdoor_samples,

            -- Home Assistant State (state_event - actuator)
            -- NULL handling: carry_forward (state persists)
            COALESCE(
                state.window_state,
                LAG(state.window_state) IGNORE NULLS OVER (ORDER BY COALESCE(indoor.bucket, outdoor.bucket, state.bucket))
            ) AS state_window_state,
            state.transition_count AS state_transitions,

            -- Total sample count
            COALESCE(indoor.sample_count, 0) + COALESCE(outdoor.sample_count, 0) AS total_samples

        FROM gold.air_quality_hourly indoor
        FULL OUTER JOIN gold.outdoor_weather_hourly outdoor
            ON indoor.bucket = outdoor.bucket
        FULL OUTER JOIN gold.home_assistant_state_hourly state
            ON COALESCE(indoor.bucket, outdoor.bucket) = state.bucket

        WHERE COALESCE(indoor.bucket, outdoor.bucket, state.bucket) >= NOW() - INTERVAL '90 days';

        RAISE NOTICE 'Created aligned view: gold.indoor_air_quality_aligned';
    ELSE
        RAISE NOTICE 'gold.indoor_air_quality_aligned already exists, skipping';
    END IF;
END $$;

-- Index for efficient bucket queries
CREATE INDEX IF NOT EXISTS idx_indoor_air_quality_aligned_bucket
    ON gold.indoor_air_quality_aligned (bucket);
```

---

## Complexity Analysis

| Operation | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| Sort streams | O(s log s) | O(s) |
| Generate column exprs | O(s * c) | O(s * c) |
| Generate join clauses | O(s) | O(s) |
| Build SQL string | O(total chars) | O(total chars) |
| Total | O(s * c) | O(s * c) |

Where:
- s = number of streams
- c = average columns per stream

---

## Error Handling

```
ENUM AlignmentError:
    // No primary stream defined (fatal)
    NoPrimaryStream {
        domain_id: String
    }

    // Stream not found (code 404)
    StreamNotFound {
        stream_id: String,
        domain_id: String
    }

    // Stream has no Gold layer (code 402)
    StreamNoGoldLayer {
        stream_id: String,
        suggestion: "Enable gold_etl in stream config"
    }

    // Unknown stream type for NULL handling
    UnknownStreamType {
        stream_id: String,
        stream_type: String
    }

    // Insufficient streams for alignment
    InsufficientStreams {
        domain_id: String,
        count: usize,
        minimum: 2
    }
```

---

## Invariants

1. **Primary First**: Primary stream is always first in FROM clause
2. **Bucket Coalesce**: All bucket references use COALESCE for FULL OUTER JOIN
3. **Column Aliasing**: All columns prefixed with stream alias to avoid conflicts
4. **Forecast LATERAL**: Forecast streams always use LATERAL join on `issued_at`
5. **NULL by Type**: NULL handling defaults to stream_type if not explicitly set
6. **90-Day Window**: WHERE clause limits to 90 days for query performance

---

## Test Cases (London TDD)

```
TEST: PrimaryStreamFirst
    GIVEN streams with roles [context, primary, actuator]
    WHEN sort_streams_by_role() is called
    THEN first stream has role = primary

TEST: FullOuterJoinCoalesce
    GIVEN join_strategy = FullOuter
    AND 3 streams
    WHEN generate_join_clauses() is called
    THEN third join uses COALESCE of first two buckets

TEST: ForecastUsesLateralJoin
    GIVEN stream with stream_type = forecast
    WHEN generate_join_clauses() is called
    THEN output contains "LEFT JOIN LATERAL"
    AND output contains "WHERE f.issued_at <="
    AND output contains "ORDER BY f.issued_at DESC"

TEST: StateEventCarriesForward
    GIVEN stream with stream_type = state_event
    AND null_handling not explicitly set
    WHEN generate_column_expressions() is called
    THEN output contains "LAG(...) IGNORE NULLS"

TEST: ObservationPreservesNull
    GIVEN stream with stream_type = observation
    WHEN generate_column_expressions() is called
    THEN column has no COALESCE or LAG wrapper

TEST: LeftJoinPreservesPrimary
    GIVEN join_strategy = Left
    WHEN generate_join_clauses() is called
    THEN all joins are "LEFT JOIN"
    AND primary stream is in FROM clause

TEST: SyncModeChecksExistence
    GIVEN action = Sync
    WHEN generate_sync_aligned_view() is called
    THEN SQL contains "IF NOT EXISTS"
    AND SQL contains "pg_matviews"

TEST: TotalSamplesColumn
    GIVEN 3 streams
    WHEN generate_column_expressions() is called
    THEN output contains "total_samples"
    AND expression is sum of all sample counts
```

---

## Performance Considerations

1. **Materialized View**: Aligned view is MATERIALIZED, not a regular view
   - Requires explicit REFRESH
   - Better query performance
   - Storage cost for denormalized data

2. **90-Day Window**: WHERE clause limits data to recent 90 days
   - Reduces materialization size
   - Can be configured per domain

3. **LATERAL Join Cost**: Forecast LATERAL joins are more expensive
   - Consider limiting forecast streams per domain
   - Index on `issued_at` in forecast tables is critical

4. **Index on Bucket**: Index created automatically for efficient time-based queries

---

## References

- [SPEC-A04](../specification/SPEC-A04-alignment-interpreter.md) - Full specification
- [ADR-FE001-003](../../architecture/DECISIONS.md) - Forecast alignment decision
- [ADR-FE001-004](../../architecture/DECISIONS.md) - NULL handling decision
