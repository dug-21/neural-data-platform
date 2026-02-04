# ALGO-aligned-view-gen: Cross-Stream FULL OUTER JOIN Generation

> **Algorithm ID:** C01
> **Feature:** v11-005 (Cross-Stream Aligned View)
> **Phase:** C (Cross-Stream + Alignment)
> **Created:** 2026-02-04

---

## Purpose

Generate SQL for a domain-aligned materialized view that JOINs multiple Gold layer continuous aggregates using FULL OUTER JOIN. This algorithm produces the `gold.indoor_air_quality_aligned` view that correlates air-quality, outdoor-weather, and home-assistant-state streams on hourly buckets with NULL handling per stream type.

---

## Algorithm: GenerateAlignedView

```
ALGORITHM: GenerateAlignedView
INPUT:
    domain_config: DomainConfig
    config_loader: ConfigLoader
    action: Action
OUTPUT: Result<String, GeneratorError>
REQUIRES:
    - domain_config.streams.len() >= 2
    - All referenced streams have Gold layer enabled
    - Primary stream is defined

BEGIN
    alignment <- domain_config.alignment

    // 1. Validate minimum streams
    IF domain_config.streams.len() < 2 THEN
        RETURN Err(GeneratorError::InsufficientStreams {
            domain_id: domain_config.id,
            count: domain_config.streams.len(),
            minimum: 2
        })
    END IF

    // 2. Sort streams by role (primary first)
    sorted_streams <- SortStreamsByRole(domain_config.streams)?

    // 3. Load stream configurations and build aligned stream metadata
    aligned_streams <- Vec::new()

    FOR EACH stream_ref IN sorted_streams DO
        stream_config <- config_loader.load_stream_config(stream_ref.stream_id)?

        // Validate Gold layer is enabled
        IF stream_config.gold_etl IS None OR NOT stream_config.gold_etl.enabled THEN
            RETURN Err(GeneratorError::StreamNoGoldLayer {
                stream_id: stream_ref.stream_id,
                suggestion: "Enable gold_etl in stream config"
            })
        END IF

        // Build aligned stream metadata
        gold_table <- DeriveGoldTableName(stream_ref.stream_id, alignment.granularity)
        columns <- DeriveGoldColumns(stream_config.gold_etl)
        null_handling <- ResolveNullHandling(stream_ref, stream_config.stream_type)

        aligned_streams.push(AlignedStream {
            stream_id: stream_ref.stream_id,
            alias: stream_ref.alias,
            role: stream_ref.role,
            stream_type: stream_config.stream_type,
            gold_table: gold_table,
            columns: columns,
            null_handling: null_handling
        })
    END FOR

    // 4. Generate SQL components
    column_exprs <- GenerateColumnExpressions(aligned_streams)
    join_clauses <- GenerateJoinClauses(aligned_streams, alignment.join_strategy)

    // 5. Generate complete DDL based on action
    sql <- MATCH action WITH
        | Sync => GenerateSyncAlignedViewSQL(alignment, aligned_streams, column_exprs, join_clauses)
        | Recreate => GenerateRecreateAlignedViewSQL(alignment, aligned_streams, column_exprs, join_clauses)

    RETURN Ok(sql)
END
```

---

## Algorithm: SortStreamsByRole

```
ALGORITHM: SortStreamsByRole
INPUT: streams: Vec<StreamReference>
OUTPUT: Result<Vec<StreamReference>, GeneratorError>

ROLE_PRIORITY:
    primary   = 0
    context   = 1
    actuator  = 2
    constraint = 3

BEGIN
    sorted <- streams.clone()
    sorted.sort_by_key(|s| ROLE_PRIORITY.get(s.role).unwrap_or(99))

    // Validate primary stream exists
    IF sorted.is_empty() OR sorted[0].role != "primary" THEN
        RETURN Err(GeneratorError::NoPrimaryStream {
            domain_id: "unknown"
        })
    END IF

    RETURN Ok(sorted)
END
```

---

## Algorithm: GenerateColumnExpressions

```
ALGORITHM: GenerateColumnExpressions
INPUT: aligned_streams: Vec<AlignedStream>
OUTPUT: Vec<String>

BEGIN
    expressions <- Vec::new()

    // 1. Generate COALESCE bucket expression (FULL OUTER JOIN requires this)
    bucket_refs <- aligned_streams.iter()
        .map(|s| format!("{}.bucket", s.alias))
        .collect()

    expressions.push(format!(
        "COALESCE({}) AS bucket",
        bucket_refs.join(", ")
    ))

    // 2. Generate columns for each stream
    FOR EACH stream IN aligned_streams DO
        FOR EACH column IN stream.columns DO
            // Skip bucket (handled above)
            IF column == "bucket" THEN
                CONTINUE
            END IF

            // Generate source expression
            source_expr <- format!("{}.{}", stream.alias, column)

            // Generate target alias
            target_alias <- format!("{}_{}", stream.alias, column)

            // Apply NULL handling based on stream type
            wrapped_expr <- ApplyNullHandling(source_expr, target_alias, stream)

            expressions.push(wrapped_expr)
        END FOR

        // Add sample count with COALESCE to handle NULLs
        expressions.push(format!(
            "COALESCE({}.sample_count, 0) AS {}_samples",
            stream.alias, stream.alias
        ))
    END FOR

    // 3. Add total samples column
    sample_parts <- aligned_streams.iter()
        .map(|s| format!("COALESCE({}.sample_count, 0)", s.alias))
        .collect()

    expressions.push(format!(
        "{} AS total_samples",
        sample_parts.join(" + ")
    ))

    RETURN expressions
END
```

---

## Algorithm: ApplyNullHandling

```
ALGORITHM: ApplyNullHandling
INPUT:
    source_expr: String       // e.g., "indoor.pm25_mean"
    target_alias: String      // e.g., "indoor_pm25_mean"
    stream: AlignedStream
OUTPUT: String

BEGIN
    RETURN MATCH stream.null_handling WITH
        | NullHandling::Preserve =>
            // Observation streams: pass through NULL
            format!("{} AS {}", source_expr, target_alias)

        | NullHandling::CarryForward =>
            // State streams: use LOCF (Last Observation Carried Forward)
            format!(r#"
COALESCE(
    {},
    LAG({}) IGNORE NULLS OVER (ORDER BY COALESCE({bucket_coalesce}))
) AS {}"#,
                source_expr,
                source_expr,
                bucket_coalesce = GenerateBucketCoalesce(stream.all_streams),
                target_alias
            )

        | NullHandling::Interpolate =>
            // Linear interpolation (rarely used)
            format!(r#"
CASE
    WHEN {} IS NOT NULL THEN {}
    ELSE (
        LAG({}) IGNORE NULLS OVER (ORDER BY bucket) +
        LEAD({}) IGNORE NULLS OVER (ORDER BY bucket)
    ) / 2.0
END AS {}"#,
                source_expr, source_expr,
                source_expr, source_expr,
                target_alias
            )
END
```

---

## Algorithm: GenerateJoinClauses

```
ALGORITHM: GenerateJoinClauses
INPUT:
    aligned_streams: Vec<AlignedStream>
    join_strategy: JoinStrategy
OUTPUT: String

BEGIN
    primary <- aligned_streams[0]
    other_streams <- aligned_streams[1..]

    // Start with FROM clause
    from_clause <- format!("FROM {} {}", primary.gold_table, primary.alias)

    joins <- Vec::new()

    FOR i, stream IN other_streams.enumerate() DO
        // Determine join type
        join_type <- MATCH join_strategy WITH
            | FullOuter => "FULL OUTER JOIN"
            | Left => "LEFT JOIN"
            | Inner => "INNER JOIN"

        // Build join condition
        // For FULL OUTER JOIN, use COALESCE of all previous buckets
        join_condition <- IF join_strategy == FullOuter THEN
            IF i == 0 THEN
                // First join: just primary bucket
                format!("{}.bucket = {}.bucket", primary.alias, stream.alias)
            ELSE
                // Subsequent joins: COALESCE all previous buckets
                previous_buckets <- aligned_streams[0..i+1].iter()
                    .map(|s| format!("{}.bucket", s.alias))
                    .collect()
                format!(
                    "COALESCE({}) = {}.bucket",
                    previous_buckets.join(", "),
                    stream.alias
                )
            END IF
        ELSE
            // LEFT or INNER JOIN: simple bucket equality
            format!("{}.bucket = {}.bucket", primary.alias, stream.alias)
        END IF

        // Handle forecast streams with LATERAL join (ADR-FE001-003)
        IF stream.stream_type == "forecast" THEN
            join_clause <- GenerateForecastLateralJoin(stream, primary, aligned_streams[0..i+1])
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

## Algorithm: GenerateForecastLateralJoin

```
ALGORITHM: GenerateForecastLateralJoin
INPUT:
    forecast_stream: AlignedStream
    primary: AlignedStream
    previous_streams: Vec<AlignedStream>
OUTPUT: String
REQUIRES: forecast_stream.stream_type == "forecast"

BEGIN
    // Build bucket reference from previous streams
    bucket_expr <- IF previous_streams.len() == 1 THEN
        format!("{}.bucket", previous_streams[0].alias)
    ELSE
        bucket_refs <- previous_streams.iter()
            .map(|s| format!("{}.bucket", s.alias))
            .collect()
        format!("COALESCE({})", bucket_refs.join(", "))
    END IF

    // LATERAL join selects the most recent forecast available at query time
    // This ensures causal validity (we only see forecasts issued BEFORE the bucket)
    RETURN format!(r#"
LEFT JOIN LATERAL (
    SELECT * FROM {} f
    WHERE f.issued_at <= {}
    ORDER BY f.issued_at DESC
    LIMIT 1
) {} ON TRUE"#,
        forecast_stream.gold_table,
        bucket_expr,
        forecast_stream.alias
    )
END
```

---

## Algorithm: GenerateSyncAlignedViewSQL

```
ALGORITHM: GenerateSyncAlignedViewSQL
INPUT:
    alignment: AlignmentConfig
    aligned_streams: Vec<AlignedStream>
    column_exprs: Vec<String>
    join_clauses: String
OUTPUT: String

BEGIN
    // Build bucket coalesce for WHERE clause
    bucket_coalesce <- aligned_streams.iter()
        .map(|s| format!("{}.bucket", s.alias))
        .join(", ")

    // Build window definition for LOCF if needed
    window_def <- IF AnyStreamUsesLocf(aligned_streams) THEN
        format!("\nWINDOW w AS (ORDER BY COALESCE({}))", bucket_coalesce)
    ELSE
        ""
    END IF

    sql <- format!(r#"
-- Aligned view for domain: {domain_id}
-- Streams: {stream_list}
-- Generated by ndp-gold-ddl
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
        {join_clauses}{window_def}
        WHERE COALESCE({bucket_coalesce}) >= NOW() - INTERVAL '90 days';

        -- Create index for time-based queries
        CREATE INDEX IF NOT EXISTS idx_{view_name}_bucket
            ON gold.{view_name} (bucket DESC);

        RAISE NOTICE 'Created aligned view: gold.{view_name}';
    ELSE
        RAISE NOTICE 'gold.{view_name} already exists, skipping';
    END IF;
END $$;
"#,
        domain_id = alignment.domain_id,
        stream_list = aligned_streams.iter().map(|s| s.alias.as_str()).join(", "),
        view_name = alignment.view_name,
        column_list = column_exprs.join(",\n            "),
        join_clauses = join_clauses,
        window_def = window_def,
        bucket_coalesce = bucket_coalesce
    )

    RETURN sql
END
```

---

## Algorithm: GenerateRecreateAlignedViewSQL

```
ALGORITHM: GenerateRecreateAlignedViewSQL
INPUT:
    alignment: AlignmentConfig
    aligned_streams: Vec<AlignedStream>
    column_exprs: Vec<String>
    join_clauses: String
OUTPUT: String

BEGIN
    bucket_coalesce <- aligned_streams.iter()
        .map(|s| format!("{}.bucket", s.alias))
        .join(", ")

    window_def <- IF AnyStreamUsesLocf(aligned_streams) THEN
        format!("\nWINDOW w AS (ORDER BY COALESCE({}))", bucket_coalesce)
    ELSE
        ""
    END IF

    sql <- format!(r#"
-- Aligned view for domain: {domain_id}
-- Streams: {stream_list}
-- Generated by ndp-gold-ddl
-- Mode: RECREATE (drop and create)

-- Drop existing view
DROP MATERIALIZED VIEW IF EXISTS gold.{view_name} CASCADE;

-- Create aligned view
CREATE MATERIALIZED VIEW gold.{view_name} AS
SELECT
    {column_list}
{join_clauses}{window_def}
WHERE COALESCE({bucket_coalesce}) >= NOW() - INTERVAL '90 days';

-- Create index for time-based queries
CREATE INDEX idx_{view_name}_bucket
    ON gold.{view_name} (bucket DESC);
"#,
        domain_id = alignment.domain_id,
        stream_list = aligned_streams.iter().map(|s| s.alias.as_str()).join(", "),
        view_name = alignment.view_name,
        column_list = column_exprs.join(",\n    "),
        join_clauses = join_clauses,
        window_def = window_def,
        bucket_coalesce = bucket_coalesce
    )

    RETURN sql
END
```

---

## SQL Template: Indoor Air Quality Aligned View

Given domain config with 3 streams:

```sql
-- Generated aligned view for indoor-air-quality domain
CREATE MATERIALIZED VIEW gold.indoor_air_quality_aligned AS
SELECT
    -- Bucket (COALESCE for FULL OUTER JOIN)
    COALESCE(indoor.bucket, outdoor.bucket, state.bucket) AS bucket,

    -- Indoor Air Quality (observation - preserve NULL)
    indoor.pm25_mean AS indoor_pm25_mean,
    indoor.pm25_std AS indoor_pm25_std,
    indoor.co2_mean AS indoor_co2_mean,
    indoor.co2_std AS indoor_co2_std,
    indoor.temperature_c_mean AS indoor_temp_mean,
    indoor.humidity_pct_mean AS indoor_humidity_mean,
    COALESCE(indoor.sample_count, 0) AS indoor_samples,

    -- Outdoor Weather (observation - preserve NULL)
    outdoor.temperature_c_mean AS outdoor_temp_mean,
    outdoor.humidity_pct_mean AS outdoor_humidity_mean,
    outdoor.wind_speed_kmh_mean AS outdoor_wind_mean,
    outdoor.pressure_pa_mean AS outdoor_pressure_mean,
    COALESCE(outdoor.sample_count, 0) AS outdoor_samples,

    -- State Events (state_event - carry forward NULL)
    COALESCE(
        state.window_open_count,
        LAG(state.window_open_count) IGNORE NULLS OVER w
    ) AS window_opens,
    COALESCE(
        state.door_open_count,
        LAG(state.door_open_count) IGNORE NULLS OVER w
    ) AS door_opens,
    COALESCE(
        state.last_window_state,
        LAG(state.last_window_state) IGNORE NULLS OVER w
    ) AS last_window_state,

    -- Total samples
    COALESCE(indoor.sample_count, 0) +
    COALESCE(outdoor.sample_count, 0) AS total_samples

FROM gold.air_quality_hourly indoor
FULL OUTER JOIN gold.outdoor_weather_hourly outdoor
    ON indoor.bucket = outdoor.bucket
FULL OUTER JOIN gold.state_events_hourly state
    ON COALESCE(indoor.bucket, outdoor.bucket) = state.bucket

WINDOW w AS (ORDER BY COALESCE(indoor.bucket, outdoor.bucket, state.bucket))

WHERE COALESCE(indoor.bucket, outdoor.bucket, state.bucket) >= NOW() - INTERVAL '90 days';

CREATE INDEX idx_indoor_air_quality_aligned_bucket
    ON gold.indoor_air_quality_aligned (bucket DESC);
```

---

## Complexity Analysis

| Operation | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| Sort streams by role | O(s log s) | O(s) |
| Load stream configs | O(s) | O(s * c) |
| Generate column exprs | O(s * cols) | O(s * cols) |
| Generate join clauses | O(s) | O(s) |
| Build SQL string | O(total chars) | O(total chars) |
| Total | O(s * cols) | O(s * cols) |

Where: s = streams, cols = columns per stream

---

## Error Handling

```
ENUM GeneratorError:
    InsufficientStreams { domain_id, count, minimum }
    NoPrimaryStream { domain_id }
    StreamNotFound { stream_id, domain_id }
    StreamNoGoldLayer { stream_id, suggestion }
    UnknownStreamType { stream_id, stream_type }
```

---

## Invariants

1. **Primary First**: Primary stream always in FROM clause
2. **COALESCE Bucket**: FULL OUTER JOIN requires COALESCE on all buckets
3. **Column Aliasing**: All columns prefixed with stream alias
4. **NULL by Type**: Observations preserve NULL, state_events carry forward
5. **90-Day Window**: WHERE clause limits to 90 days for performance
6. **Index Created**: Bucket index always created for time queries

---

## Test Cases (London TDD)

```
TRAITS TO MOCK:
    - ConfigLoader: Return test stream configs
    - DatabaseExecutor: Capture generated SQL

TEST: GenerateThreeStreamAlignedView
    GIVEN domain with air-quality, outdoor-weather, home-assistant-state
    WHEN GenerateAlignedView() is called
    THEN SQL contains "FULL OUTER JOIN gold.air_quality_hourly"
    AND SQL contains "FULL OUTER JOIN gold.outdoor_weather_hourly"
    AND SQL contains "FULL OUTER JOIN gold.state_events_hourly"

TEST: PrimaryStreamFirst
    GIVEN streams with roles [context, actuator, primary]
    WHEN SortStreamsByRole() is called
    THEN first stream has role = "primary"

TEST: BucketCoalesceForFullOuterJoin
    GIVEN join_strategy = FullOuter
    WHEN GenerateJoinClauses() is called
    THEN third join uses "COALESCE(indoor.bucket, outdoor.bucket)"

TEST: ObservationPreservesNull
    GIVEN stream with stream_type = "observation"
    WHEN ApplyNullHandling() is called
    THEN output has no COALESCE or LAG wrapper

TEST: StateEventCarriesForward
    GIVEN stream with stream_type = "state_event"
    WHEN ApplyNullHandling() is called
    THEN output contains "LAG(...) IGNORE NULLS"

TEST: SyncModeChecksExistence
    GIVEN action = Sync
    WHEN GenerateSyncAlignedViewSQL() is called
    THEN SQL contains "IF NOT EXISTS"
    AND SQL contains "pg_matviews"

TEST: RecreateDropsFirst
    GIVEN action = Recreate
    WHEN GenerateRecreateAlignedViewSQL() is called
    THEN SQL contains "DROP MATERIALIZED VIEW IF EXISTS"
    AND DROP appears before CREATE

TEST: WindowDefinitionForLocf
    GIVEN domain with state_event stream
    WHEN GenerateAlignedViewSQL() is called
    THEN SQL contains "WINDOW w AS (ORDER BY COALESCE"

TEST: IndexCreatedOnBucket
    GIVEN any domain config
    WHEN GenerateAlignedViewSQL() is called
    THEN SQL contains "CREATE INDEX"
    AND SQL contains "(bucket DESC)"
```

---

## References

- [SPEC-C01-aligned-view.md](../specification/SPEC-C01-aligned-view.md)
- [ADR-FE001-003](../../architecture/DECISIONS.md) - Forecast alignment
- [ADR-FE001-004](../../architecture/DECISIONS.md) - NULL handling by stream type
- [ALGO-alignment-interpreter.md](../../phase-a/pseudocode/ALGO-alignment-interpreter.md) - Phase A generic algorithm
