# GRAFANA_QUERY_GENERATION.md - Dynamic Grafana Query Generation

## Overview

This document defines the pseudocode for generating dynamic SQL queries for Grafana panels in the Data Quality dashboard. The system generates queries that automatically reflect the current state of the data dictionary, eliminating the need for manual dashboard updates when schemas change.

---

## Dashboard Architecture

### Panel Structure

```
DASHBOARD: HomeAssistant Data Quality
├── ROW: Coverage Overview
│   ├── Panel: Schema Coverage Summary (Stat)
│   ├── Panel: Entities by Status (Pie Chart)
│   └── Panel: Coverage Trend (Time Series)
├── ROW: Unknown Entities
│   ├── Panel: Unknown Entities List (Table)
│   └── Panel: Unknown by Domain (Bar Chart)
├── ROW: Schema Analysis
│   ├── Panel: Attribute Heatmap (Heatmap)
│   └── Panel: Incomplete Schemas (Table)
└── ROW: Data Explorer
    ├── Panel: Raw Event Browser (Table)
    └── Panel: Entity Timeline (Time Series)
```

### Data Sources

| Data Source | Type | Purpose |
|-------------|------|---------|
| TimescaleDB | PostgreSQL | Data dictionary queries |
| DuckDB Plugin | Parquet | Bronze layer queries |
| Grafana Variables | Dashboard | User selections |

---

## Dashboard Variables

```
VARIABLE: $stream_id
  Type: Query
  Query: SELECT DISTINCT stream_id FROM data_dictionary ORDER BY stream_id
  Multi: true
  Default: All

VARIABLE: $time_window
  Type: Interval
  Options: [1h, 6h, 24h, 7d, 30d]
  Default: 24h

VARIABLE: $domain
  Type: Query
  Query: SELECT DISTINCT split_part(schema_name, '.', 1) AS domain
         FROM data_dictionary
         WHERE stream_id IN ($stream_id)
  Multi: true
  Default: All

VARIABLE: $severity
  Type: Custom
  Options: [all, error, warning, info]
  Default: all
```

---

## Algorithm 1: Schema Coverage Summary Query

```
ALGORITHM: GenerateCoverageSummaryQuery
PURPOSE: Generate query for schema coverage stat panel

INPUT:
  - panel_type: "percentage" | "counts" | "trend"
  - stream_filter: List<stream_id> or "*"
  - time_window: interval string

OUTPUT:
  - sql_query: string (PostgreSQL/TimescaleDB compatible)

BEGIN
    SWITCH panel_type
        CASE "percentage":
            query ← GenerateCoveragePercentageQuery(stream_filter, time_window)

        CASE "counts":
            query ← GenerateCoverageCountsQuery(stream_filter, time_window)

        CASE "trend":
            query ← GenerateCoverageTrendQuery(stream_filter, time_window)
    END SWITCH

    RETURN query
END


SUBROUTINE: GenerateCoveragePercentageQuery
INPUT: stream_filter, time_window
OUTPUT: SQL query string

BEGIN
    query ← """
    WITH bronze_entities AS (
        -- Query unique entities from Bronze Parquet via DuckDB
        SELECT DISTINCT entity_id
        FROM read_parquet('/data/parquet/$stream_filter/**/*.parquet')
        WHERE timestamp >= NOW() - INTERVAL '$time_window'
    ),
    dictionary_patterns AS (
        -- Get patterns from data dictionary
        SELECT DISTINCT schema_name,
               metadata->>'pattern' AS pattern
        FROM data_dictionary
        WHERE stream_id = ANY($stream_filter_array)
          AND metadata->>'pattern' IS NOT NULL
    ),
    matched AS (
        -- Count entities matching any pattern
        SELECT COUNT(DISTINCT be.entity_id) AS matched_count
        FROM bronze_entities be
        JOIN dictionary_patterns dp ON
            be.entity_id ~ dp.pattern  -- PostgreSQL regex match
    ),
    total AS (
        SELECT COUNT(*) AS total_count FROM bronze_entities
    )
    SELECT
        ROUND(
            (matched.matched_count::DECIMAL / NULLIF(total.total_count, 0)) * 100,
            1
        ) AS coverage_percentage,
        matched.matched_count,
        total.total_count
    FROM matched, total
    """

    RETURN InterpolateVariables(query, stream_filter, time_window)
END


SUBROUTINE: GenerateCoverageCountsQuery
INPUT: stream_filter, time_window
OUTPUT: SQL query string

BEGIN
    query ← """
    SELECT
        'Known Entities' AS category,
        COUNT(DISTINCT entity_id) FILTER (WHERE matched = true) AS count
    FROM entity_match_cache
    WHERE last_seen >= NOW() - INTERVAL '$time_window'
      AND stream_id = ANY($stream_filter_array)

    UNION ALL

    SELECT
        'Unknown Entities' AS category,
        COUNT(DISTINCT entity_id) FILTER (WHERE matched = false) AS count
    FROM entity_match_cache
    WHERE last_seen >= NOW() - INTERVAL '$time_window'
      AND stream_id = ANY($stream_filter_array)
    """

    RETURN InterpolateVariables(query, stream_filter, time_window)
END


SUBROUTINE: GenerateCoverageTrendQuery
INPUT: stream_filter, time_window
OUTPUT: SQL query string for time series

BEGIN
    query ← """
    SELECT
        time_bucket('1 hour', timestamp) AS time,
        COUNT(DISTINCT entity_id) FILTER (WHERE matched = true) AS known,
        COUNT(DISTINCT entity_id) FILTER (WHERE matched = false) AS unknown,
        ROUND(
            COUNT(DISTINCT entity_id) FILTER (WHERE matched = true)::DECIMAL /
            NULLIF(COUNT(DISTINCT entity_id), 0) * 100,
            1
        ) AS coverage_pct
    FROM entity_observations
    WHERE timestamp >= NOW() - INTERVAL '$time_window'
      AND stream_id = ANY($stream_filter_array)
    GROUP BY time_bucket('1 hour', timestamp)
    ORDER BY time
    """

    RETURN InterpolateVariables(query, stream_filter, time_window)
END
```

---

## Algorithm 2: Unknown Entities Query

```
ALGORITHM: GenerateUnknownEntitiesQuery
PURPOSE: Generate query for unknown entities table panel

INPUT:
  - format: "table" | "grouped" | "chart"
  - stream_filter: List<stream_id>
  - time_window: interval
  - limit: integer (default: 100)

OUTPUT:
  - sql_query: string

BEGIN
    SWITCH format
        CASE "table":
            query ← """
            SELECT
                entity_id,
                split_part(entity_id, '.', 1) AS domain,
                COUNT(*) AS observation_count,
                MIN(timestamp) AS first_seen,
                MAX(timestamp) AS last_seen,
                EXTRACT(EPOCH FROM (MAX(timestamp) - MIN(timestamp))) / 3600 AS active_hours,
                -- Sample attributes (first non-null)
                (SELECT attributes
                 FROM bronze_observations bo
                 WHERE bo.entity_id = unk.entity_id
                 ORDER BY timestamp DESC LIMIT 1
                ) AS sample_attributes
            FROM unknown_entities_view unk
            WHERE last_seen >= NOW() - INTERVAL '$time_window'
              AND stream_id = ANY($stream_filter_array)
            GROUP BY entity_id
            ORDER BY observation_count DESC
            LIMIT $limit
            """

        CASE "grouped":
            query ← """
            WITH unknown AS (
                SELECT
                    entity_id,
                    split_part(entity_id, '.', 1) AS domain,
                    -- Extract common prefix (up to first number or last underscore)
                    regexp_replace(
                        split_part(entity_id, '.', 2),
                        '_[0-9]+.*$|_[^_]+$',
                        '_*'
                    ) AS pattern_hint
                FROM unknown_entities_view
                WHERE last_seen >= NOW() - INTERVAL '$time_window'
                  AND stream_id = ANY($stream_filter_array)
            )
            SELECT
                domain,
                domain || '.' || pattern_hint AS suggested_pattern,
                COUNT(DISTINCT entity_id) AS entity_count,
                array_agg(DISTINCT entity_id ORDER BY entity_id) FILTER (WHERE entity_id IS NOT NULL)[1:5] AS examples
            FROM unknown
            GROUP BY domain, pattern_hint
            HAVING COUNT(DISTINCT entity_id) >= 2
            ORDER BY entity_count DESC
            LIMIT $limit
            """

        CASE "chart":
            query ← """
            SELECT
                split_part(entity_id, '.', 1) AS domain,
                COUNT(DISTINCT entity_id) AS count
            FROM unknown_entities_view
            WHERE last_seen >= NOW() - INTERVAL '$time_window'
              AND stream_id = ANY($stream_filter_array)
            GROUP BY domain
            ORDER BY count DESC
            """
    END SWITCH

    RETURN InterpolateVariables(query, stream_filter, time_window, limit)
END
```

---

## Algorithm 3: Attribute Heatmap Query

```
ALGORITHM: GenerateAttributeHeatmapQuery
PURPOSE: Generate query showing attribute presence across schemas

INPUT:
  - stream_filter: List<stream_id>
  - group_by: "schema" | "device_class" | "domain"

OUTPUT:
  - sql_query: string (for heatmap visualization)

BEGIN
    // Determine grouping column
    SWITCH group_by
        CASE "schema":
            group_column ← "schema_name"
        CASE "device_class":
            group_column ← "metadata->>'device_class'"
        CASE "domain":
            group_column ← "split_part(schema_name, '.', 1)"
    END SWITCH

    query ← """
    WITH schema_attributes AS (
        -- Get defined attributes from dictionary
        SELECT
            $group_column AS group_name,
            attribute,
            1 AS defined
        FROM data_dictionary
        WHERE stream_id = ANY($stream_filter_array)
    ),
    observed_attributes AS (
        -- Get actually observed attributes from Bronze data
        SELECT
            dd.$group_column AS group_name,
            jsonb_object_keys(bo.attributes) AS attribute,
            COUNT(*) AS observation_count
        FROM bronze_observations bo
        JOIN data_dictionary dd ON
            bo.entity_id ~ (dd.metadata->>'pattern')
        WHERE bo.timestamp >= NOW() - INTERVAL '$time_window'
          AND dd.stream_id = ANY($stream_filter_array)
        GROUP BY dd.$group_column, jsonb_object_keys(bo.attributes)
    ),
    combined AS (
        SELECT
            COALESCE(sa.group_name, oa.group_name) AS group_name,
            COALESCE(sa.attribute, oa.attribute) AS attribute,
            COALESCE(sa.defined, 0) AS is_defined,
            COALESCE(oa.observation_count, 0) AS observations
        FROM schema_attributes sa
        FULL OUTER JOIN observed_attributes oa
            ON sa.group_name = oa.group_name
            AND sa.attribute = oa.attribute
    )
    SELECT
        group_name,
        attribute,
        CASE
            WHEN is_defined = 1 AND observations > 0 THEN 'present'
            WHEN is_defined = 1 AND observations = 0 THEN 'missing'
            WHEN is_defined = 0 AND observations > 0 THEN 'extra'
            ELSE 'undefined'
        END AS status,
        observations,
        -- Numeric value for heatmap
        CASE
            WHEN is_defined = 1 AND observations > 0 THEN 3  -- Green
            WHEN is_defined = 1 AND observations = 0 THEN 1  -- Red
            WHEN is_defined = 0 AND observations > 0 THEN 2  -- Yellow
            ELSE 0
        END AS heatmap_value
    FROM combined
    ORDER BY group_name, attribute
    """

    RETURN InterpolateVariables(query, stream_filter, group_column)
END
```

---

## Algorithm 4: Incomplete Schemas Query

```
ALGORITHM: GenerateIncompleteSchemaQuery
PURPOSE: Generate query for schemas with missing or extra attributes

INPUT:
  - stream_filter: List<stream_id>
  - issue_type: "missing" | "extra" | "both"
  - time_window: interval

OUTPUT:
  - sql_query: string

BEGIN
    base_cte ← """
    WITH expected AS (
        -- Attributes defined in dictionary
        SELECT
            stream_id,
            schema_name,
            attribute,
            type,
            metadata->>'pattern' AS pattern,
            COALESCE((metadata->>'nullable')::boolean, true) AS nullable
        FROM data_dictionary
        WHERE stream_id = ANY($stream_filter_array)
    ),
    observed AS (
        -- Attributes actually seen in Bronze data
        SELECT DISTINCT
            dd.stream_id,
            dd.schema_name,
            jsonb_object_keys(bo.attributes) AS attribute,
            dd.metadata->>'pattern' AS pattern
        FROM bronze_observations bo
        JOIN data_dictionary dd ON
            bo.entity_id ~ (dd.metadata->>'pattern')
        WHERE bo.timestamp >= NOW() - INTERVAL '$time_window'
          AND dd.stream_id = ANY($stream_filter_array)
    )
    """

    SWITCH issue_type
        CASE "missing":
            query ← base_cte + """
            SELECT
                e.stream_id,
                e.schema_name,
                e.attribute AS missing_attribute,
                e.type AS expected_type,
                e.nullable,
                'Missing' AS issue_type
            FROM expected e
            LEFT JOIN observed o ON
                e.stream_id = o.stream_id
                AND e.schema_name = o.schema_name
                AND e.attribute = o.attribute
            WHERE o.attribute IS NULL
              AND e.nullable = false  -- Only report non-nullable missing
            ORDER BY e.stream_id, e.schema_name, e.attribute
            """

        CASE "extra":
            query ← base_cte + """
            SELECT
                o.stream_id,
                o.schema_name,
                o.attribute AS extra_attribute,
                'unknown' AS actual_type,
                NULL AS nullable,
                'Extra' AS issue_type
            FROM observed o
            LEFT JOIN expected e ON
                o.stream_id = e.stream_id
                AND o.schema_name = e.schema_name
                AND o.attribute = e.attribute
            WHERE e.attribute IS NULL
            ORDER BY o.stream_id, o.schema_name, o.attribute
            """

        CASE "both":
            query ← base_cte + """
            SELECT
                COALESCE(e.stream_id, o.stream_id) AS stream_id,
                COALESCE(e.schema_name, o.schema_name) AS schema_name,
                COALESCE(e.attribute, o.attribute) AS attribute,
                CASE
                    WHEN o.attribute IS NULL THEN 'Missing'
                    WHEN e.attribute IS NULL THEN 'Extra'
                    ELSE 'OK'
                END AS issue_type,
                e.type AS expected_type,
                e.nullable
            FROM expected e
            FULL OUTER JOIN observed o ON
                e.stream_id = o.stream_id
                AND e.schema_name = o.schema_name
                AND e.attribute = o.attribute
            WHERE o.attribute IS NULL OR e.attribute IS NULL
            ORDER BY stream_id, schema_name,
                     CASE WHEN o.attribute IS NULL THEN 0 ELSE 1 END,
                     attribute
            """
    END SWITCH

    RETURN InterpolateVariables(query, stream_filter, time_window)
END
```

---

## Algorithm 5: Raw Event Browser Query

```
ALGORITHM: GenerateRawEventBrowserQuery
PURPOSE: Generate query for browsing raw Bronze data with dictionary context

INPUT:
  - entity_filter: string (pattern or exact)
  - time_window: interval
  - limit: integer
  - include_dictionary_context: boolean

OUTPUT:
  - sql_query: string

BEGIN
    IF include_dictionary_context THEN
        query ← """
        SELECT
            bo.timestamp,
            bo.entity_id,
            bo.state,
            bo.attributes,
            dd.schema_name AS matched_schema,
            dd.metadata->>'device_class' AS device_class,
            CASE
                WHEN dd.schema_name IS NOT NULL THEN 'Known'
                ELSE 'Unknown'
            END AS status
        FROM bronze_observations bo
        LEFT JOIN data_dictionary dd ON
            bo.entity_id ~ (dd.metadata->>'pattern')
            AND dd.stream_id = bo.stream_id
        WHERE bo.timestamp >= NOW() - INTERVAL '$time_window'
          AND ($entity_filter = '*' OR bo.entity_id LIKE '$entity_filter')
        ORDER BY bo.timestamp DESC
        LIMIT $limit
        """
    ELSE
        query ← """
        SELECT
            timestamp,
            entity_id,
            state,
            attributes,
            stream_id
        FROM bronze_observations
        WHERE timestamp >= NOW() - INTERVAL '$time_window'
          AND ($entity_filter = '*' OR entity_id LIKE '$entity_filter')
        ORDER BY timestamp DESC
        LIMIT $limit
        """
    END IF

    RETURN InterpolateVariables(query, entity_filter, time_window, limit)
END
```

---

## Query Optimization Patterns

### Pattern 1: Materialized View for Entity Matching

```
ALGORITHM: CreateEntityMatchCache
PURPOSE: Pre-compute entity-to-schema matches for faster queries

BEGIN
    sql ← """
    CREATE MATERIALIZED VIEW entity_match_cache AS
    WITH latest_patterns AS (
        SELECT DISTINCT ON (schema_name)
            stream_id,
            schema_name,
            metadata->>'pattern' AS pattern,
            metadata->>'device_class' AS device_class
        FROM data_dictionary
        ORDER BY schema_name, updated_at DESC
    )
    SELECT
        bo.entity_id,
        bo.stream_id,
        lp.schema_name AS matched_schema,
        lp.device_class,
        lp.pattern AS matched_pattern,
        CASE WHEN lp.schema_name IS NOT NULL THEN true ELSE false END AS matched,
        MIN(bo.timestamp) AS first_seen,
        MAX(bo.timestamp) AS last_seen,
        COUNT(*) AS observation_count
    FROM bronze_observations bo
    LEFT JOIN latest_patterns lp ON
        bo.entity_id ~ lp.pattern
        AND bo.stream_id = lp.stream_id
    GROUP BY bo.entity_id, bo.stream_id, lp.schema_name, lp.device_class, lp.pattern

    CREATE UNIQUE INDEX ON entity_match_cache(entity_id, stream_id);
    CREATE INDEX ON entity_match_cache(matched);
    CREATE INDEX ON entity_match_cache(last_seen);
    """

    EXECUTE(sql)
END


ALGORITHM: RefreshEntityMatchCache
PURPOSE: Update materialized view after dictionary changes
SCHEDULE: On dictionary sync or every 15 minutes

BEGIN
    EXECUTE("REFRESH MATERIALIZED VIEW CONCURRENTLY entity_match_cache")
END
```

### Pattern 2: Query Caching Layer

```
ALGORITHM: CacheableQueryWrapper
PURPOSE: Cache expensive query results in Grafana

INPUT:
  - base_query: string
  - cache_key: string (based on query params)
  - ttl: interval (default: 5 minutes)

OUTPUT:
  - cached_query: string with caching hints

BEGIN
    // Add TimescaleDB continuous aggregate hints if applicable
    IF base_query USES time_bucket THEN
        cached_query ← AddContinuousAggregateHint(base_query)
    END IF

    // Add query comments for debugging
    cached_query ← """
    -- Cache Key: $cache_key
    -- TTL: $ttl
    -- Generated: $timestamp
    """ + cached_query

    RETURN cached_query
END
```

---

## Grafana Panel Configuration

### Panel: Schema Coverage Summary

```
PANEL_CONFIG:
  type: stat
  title: Schema Coverage
  datasource: TimescaleDB
  query: |
    ${GenerateCoverageSummaryQuery("percentage", "$stream_id", "$time_window")}
  fieldConfig:
    defaults:
      unit: percent
      thresholds:
        steps:
          - value: 0
            color: red
          - value: 80
            color: yellow
          - value: 95
            color: green
```

### Panel: Unknown Entities Table

```
PANEL_CONFIG:
  type: table
  title: Unknown Entities
  datasource: TimescaleDB
  query: |
    ${GenerateUnknownEntitiesQuery("table", "$stream_id", "$time_window", 100)}
  transformations:
    - id: organize
      options:
        indexByName:
          entity_id: 0
          domain: 1
          observation_count: 2
          first_seen: 3
          last_seen: 4
  fieldConfig:
    overrides:
      - matcher:
          id: byName
          options: entity_id
        properties:
          - id: links
            value:
              - title: View Details
                url: /d/entity-detail?entity=${__value.text}
```

### Panel: Attribute Heatmap

```
PANEL_CONFIG:
  type: heatmap
  title: Attribute Status by Schema
  datasource: TimescaleDB
  query: |
    ${GenerateAttributeHeatmapQuery("$stream_id", "schema")}
  options:
    yAxis:
      axisPlacement: left
    cellGap: 1
    color:
      scheme: RdYlGn
      min: 0
      max: 3
```

---

## Complexity Analysis

### Query Generation

| Algorithm | Time Complexity | Notes |
|-----------|-----------------|-------|
| GenerateCoverageSummaryQuery | O(1) | Template interpolation |
| GenerateUnknownEntitiesQuery | O(1) | Template interpolation |
| GenerateAttributeHeatmapQuery | O(1) | Template interpolation |
| InterpolateVariables | O(n) | n = number of variables |

### Query Execution (Database Side)

| Query Type | Time Complexity | Optimization |
|------------|-----------------|--------------|
| Coverage Summary | O(e + p) | Use materialized view |
| Unknown Entities | O(e * p) | Index on entity_id |
| Attribute Heatmap | O(s * a) | Index on stream_id |
| Raw Event Browser | O(n) | Index on timestamp |

Where:
- e = number of entities
- p = number of patterns
- s = number of schemas
- a = average attributes per schema
- n = number of rows in time window

---

## Dynamic Dashboard Updates

### Auto-Refresh on Dictionary Changes

```
ALGORITHM: SetupDictionaryChangeNotifications
PURPOSE: Refresh Grafana panels when data dictionary changes

BEGIN
    // Create PostgreSQL trigger
    CREATE TRIGGER dict_change_notify
    AFTER INSERT OR UPDATE OR DELETE ON data_dictionary
    FOR EACH STATEMENT
    EXECUTE FUNCTION pg_notify('dict_changed', 'refresh');

    // Grafana annotation query for change markers
    annotation_query ← """
    SELECT
        updated_at AS time,
        'Dictionary Update' AS title,
        schema_name || ': ' || attribute AS text,
        'dict-update' AS tags
    FROM data_dictionary
    WHERE updated_at >= $__timeFrom()
      AND updated_at <= $__timeTo()
    ORDER BY updated_at DESC
    """

    RETURN annotation_query
END
```

---

## Worked Example

### Input Variables

```
$stream_id = ['homeassistant']
$time_window = '24h'
$domain = ['sensor', 'binary_sensor']
```

### Generated Coverage Query

```sql
-- Cache Key: coverage_homeassistant_24h
-- TTL: 5m
-- Generated: 2024-01-15T10:00:00Z

WITH bronze_entities AS (
    SELECT DISTINCT entity_id
    FROM read_parquet('/data/parquet/homeassistant/**/*.parquet')
    WHERE timestamp >= NOW() - INTERVAL '24 hours'
),
dictionary_patterns AS (
    SELECT DISTINCT schema_name,
           metadata->>'pattern' AS pattern
    FROM data_dictionary
    WHERE stream_id = 'homeassistant'
      AND metadata->>'pattern' IS NOT NULL
),
matched AS (
    SELECT COUNT(DISTINCT be.entity_id) AS matched_count
    FROM bronze_entities be
    JOIN dictionary_patterns dp ON
        be.entity_id ~ dp.pattern
),
total AS (
    SELECT COUNT(*) AS total_count FROM bronze_entities
)
SELECT
    ROUND(
        (matched.matched_count::DECIMAL / NULLIF(total.total_count, 0)) * 100,
        1
    ) AS coverage_percentage,
    matched.matched_count,
    total.total_count
FROM matched, total
```

### Expected Output

```
| coverage_percentage | matched_count | total_count |
|---------------------|---------------|-------------|
| 87.5                | 35            | 40          |
```
