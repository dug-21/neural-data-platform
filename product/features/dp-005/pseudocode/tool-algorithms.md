# Tool Algorithms - Bronze MCP Server

## Overview

This document defines the core algorithms for each of the 4 MCP tools in the Bronze MCP Server. Each algorithm is designed to be implementation-ready with clear data flows, complexity analysis, and error handling.

---

## Data Flow Diagram

```
                                   +-----------------+
                                   |   MCP Client    |
                                   |  (Claude Code)  |
                                   +-----------------+
                                          |
                                          | HTTP POST /mcp
                                          v
+-------------------------------------------------------------------------+
|                          MCP Server (axum)                               |
|                                                                          |
|  +------------------+    +-------------------+    +------------------+  |
|  | list_streams     |    | describe_schema   |    | validate_config  |  |
|  +------------------+    +-------------------+    +------------------+  |
|           |                       |                        |            |
|           v                       v                        v            |
|  +------------------+    +-------------------+    +------------------+  |
|  | sample_data      |    | EtcdConfigStore   |    | BronzeStorage    |  |
|  +------------------+    +-------------------+    +------------------+  |
|                                   |                        |            |
+-----------------------------------|------------------------|------------+
                                    |                        |
                                    v                        v
                          +-----------------+      +-------------------+
                          |      etcd       |      |  /data/raw/       |
                          |  /streams/*     |      |  {stream_id}/     |
                          +-----------------+      |  year=YYYY/...    |
                                                   +-------------------+
```

---

## 1. list_streams

### Purpose
Enumerate all available Bronze layer streams with metadata from etcd and storage statistics from the filesystem.

### Algorithm

```
ALGORITHM: ListStreams
INPUT: none
OUTPUT: StreamListResponse { streams: Array<StreamInfo> }

BEGIN
    // Phase 1: Query etcd for all stream configurations
    stream_ids <- EtcdClient.list_keys_with_prefix("/streams/")

    // Extract unique stream_ids from key paths
    unique_streams <- SET()
    FOR EACH key IN stream_ids DO
        // Key format: /streams/{stream_id}/{path}
        parts <- key.split("/")
        IF parts.length >= 3 THEN
            unique_streams.add(parts[2])
        END IF
    END FOR

    // Phase 2: Build stream info for each stream
    results <- []

    FOR EACH stream_id IN unique_streams DO
        stream_info <- BuildStreamInfo(stream_id)
        results.append(stream_info)
    END FOR

    // Phase 3: Sort by stream_id for consistent output
    results.sort_by(stream_id)

    RETURN StreamListResponse { success: true, streams: results }
END

SUBROUTINE: BuildStreamInfo
INPUT: stream_id (string)
OUTPUT: StreamInfo

BEGIN
    // Fetch metadata from etcd (parallel queries for efficiency)
    metadata <- PARALLEL {
        description <- EtcdClient.get("/streams/{stream_id}/description")
        enabled <- EtcdClient.get("/streams/{stream_id}/enabled")
        version <- EtcdClient.get("/streams/{stream_id}/version")
        sources <- EtcdClient.list_keys("/streams/{stream_id}/sources/")
    }

    // Extract source types from sources array
    source_types <- []
    FOR EACH source_key IN sources DO
        IF source_key.ends_with("/type") THEN
            type_value <- EtcdClient.get(source_key)
            source_types.append(type_value)
        END IF
    END FOR

    // Scan filesystem for storage stats
    storage_stats <- ScanBronzeStorage(stream_id)

    RETURN StreamInfo {
        stream_id: stream_id,
        description: metadata.description OR "No description",
        enabled: parse_bool(metadata.enabled) OR false,
        version: metadata.version OR "unknown",
        sources: source_types,
        storage: storage_stats  // null if no data exists
    }
END

SUBROUTINE: ScanBronzeStorage
INPUT: stream_id (string)
OUTPUT: StorageStats? (nullable)

BEGIN
    base_path <- NDP_RAW_PATH / stream_id

    IF NOT base_path.exists() THEN
        RETURN null
    END IF

    // Find latest partition using reverse date order scan
    latest_partition <- null
    latest_file <- null

    // Walk year directories in reverse order (most recent first)
    year_dirs <- list_directories(base_path).sort_descending()

    FOR EACH year_dir IN year_dirs DO
        month_dirs <- list_directories(year_dir).sort_descending()
        FOR EACH month_dir IN month_dirs DO
            day_dirs <- list_directories(month_dir).sort_descending()
            FOR EACH day_dir IN day_dirs DO
                parquet_file <- day_dir / "data.parquet"
                IF parquet_file.exists() THEN
                    latest_partition <- extract_partition_path(day_dir)
                    latest_file <- parquet_file
                    BREAK outer  // Found most recent, exit all loops
                END IF
            END FOR
        END FOR
    END FOR

    IF latest_file IS null THEN
        RETURN null
    END IF

    // Get file stats
    file_stats <- filesystem.stat(latest_file)

    RETURN StorageStats {
        latest_partition: latest_partition,  // "year=2026/month=01/day=03"
        file_size_bytes: file_stats.size,
        file_modified: file_stats.modified_time
    }
END
```

### Complexity Analysis

| Operation | Time | Space |
|-----------|------|-------|
| etcd list keys | O(n) where n = total keys | O(n) |
| Extract unique streams | O(n) | O(m) where m = unique streams |
| Build stream info | O(m * k) where k = keys per stream | O(m) |
| Scan storage per stream | O(p) where p = partition depth | O(1) |
| **Total** | O(n + m * (k + p)) | O(n) |

### Notes

- etcd queries are batched where possible to minimize round trips
- Storage scan uses reverse chronological order to find latest partition quickly
- Gracefully handles missing etcd keys or storage directories

---

## 2. describe_schema

### Purpose
Get schema information for a stream with three modes: source (raw payload structure), target (entity schemas), or all (complete ETL picture).

### Algorithm

```
ALGORITHM: DescribeSchema
INPUT: stream_id (string), mode (string: "all" | "source" | "target")
OUTPUT: SchemaResponse

BEGIN
    // Validate stream exists
    stream_exists <- EtcdClient.key_exists("/streams/{stream_id}/stream_id")
    IF NOT stream_exists THEN
        RETURN ErrorResponse("Stream not found: {stream_id}")
    END IF

    // Branch based on mode
    SWITCH mode:
        CASE "source":
            RETURN BuildSourceSchema(stream_id)
        CASE "target":
            RETURN BuildTargetSchema(stream_id)
        CASE "all":
            RETURN BuildAllSchema(stream_id)
        DEFAULT:
            mode <- "all"
            RETURN BuildAllSchema(stream_id)
    END SWITCH
END

SUBROUTINE: BuildSourceSchema
INPUT: stream_id (string)
OUTPUT: SourceSchemaResponse

BEGIN
    // Step 1: Find latest Parquet file and extract raw_payload samples
    parquet_file <- FindLatestParquetFile(stream_id)

    IF parquet_file IS null THEN
        RETURN ErrorResponse("No Bronze data found for stream: {stream_id}")
    END IF

    // Step 2: Read Parquet and extract raw_payload JSON structure
    payload_structure <- AnalyzeRawPayloads(parquet_file)

    // Step 3: Get field mappings from etcd parser config
    field_mappings <- GetFieldMappings(stream_id)

    // Step 4: Compute unmapped source fields
    mapped_source_paths <- SET()
    FOR EACH mapping IN field_mappings DO
        mapped_source_paths.add(mapping.source_path)
    END FOR

    all_source_paths <- FlattenJsonPaths(payload_structure)
    unmapped_fields <- all_source_paths.difference(mapped_source_paths)

    RETURN SourceSchemaResponse {
        success: true,
        stream_id: stream_id,
        mode: "source",
        raw_payload_structure: payload_structure,
        parser_type: GetParserType(stream_id),
        field_mappings: field_mappings,
        unmapped_source_fields: unmapped_fields.to_array(),
        file_analyzed: parquet_file.path
    }
END

SUBROUTINE: BuildTargetSchema
INPUT: stream_id (string)
OUTPUT: TargetSchemaResponse

BEGIN
    // Get entity_schemas from etcd
    entity_schemas <- GetEntitySchemas(stream_id)

    IF entity_schemas.is_empty() THEN
        RETURN ErrorResponse("No entity_schemas found for stream: {stream_id}")
    END IF

    // Use first schema (typically only one per stream)
    primary_schema <- entity_schemas[0]

    RETURN TargetSchemaResponse {
        success: true,
        stream_id: stream_id,
        mode: "target",
        entity_schema: primary_schema.schema_name,
        attributes: primary_schema.attributes
    }
END

SUBROUTINE: BuildAllSchema
INPUT: stream_id (string)
OUTPUT: AllSchemaResponse

BEGIN
    source_info <- BuildSourceSchemaInternal(stream_id)
    target_info <- BuildTargetSchemaInternal(stream_id)

    // Compute gap analysis
    gap_analysis <- ComputeGapAnalysis(source_info, target_info)

    RETURN AllSchemaResponse {
        success: true,
        stream_id: stream_id,
        mode: "all",
        source: {
            raw_payload_structure: source_info.payload_structure,
            field_mappings: source_info.field_mappings
        },
        target: {
            entity_schema: target_info.schema_name,
            attributes: target_info.attributes
        },
        gap_analysis: gap_analysis
    }
END

SUBROUTINE: AnalyzeRawPayloads
INPUT: parquet_file (Path)
OUTPUT: JsonStructure { keys: Array<string>, nested: Map<string, Array<string>> }

BEGIN
    // Read Parquet file using arrow/parquet crate
    reader <- ParquetReader.open(parquet_file)

    // Sample up to 10 rows for structure analysis
    sample_size <- MIN(10, reader.row_count())

    // Extract raw_payload column
    raw_payloads <- reader.read_column("raw_payload", limit=sample_size)

    // Merge JSON structures from all samples
    merged_keys <- SET()
    nested_structure <- MAP()

    FOR EACH payload_json IN raw_payloads DO
        payload <- parse_json(payload_json)

        FOR EACH (key, value) IN payload DO
            merged_keys.add(key)

            // Track nested object keys
            IF value.is_object() THEN
                IF key NOT IN nested_structure THEN
                    nested_structure[key] <- SET()
                END IF
                FOR EACH nested_key IN value.keys() DO
                    nested_structure[key].add(nested_key)
                END FOR
            END IF
        END FOR
    END FOR

    // Convert sets to sorted arrays
    RETURN JsonStructure {
        keys: merged_keys.to_sorted_array(),
        nested: nested_structure.map_values(v -> v.to_sorted_array())
    }
END

SUBROUTINE: GetFieldMappings
INPUT: stream_id (string)
OUTPUT: Array<FieldMapping>

BEGIN
    // Query etcd for parser field_mappings
    // Key pattern: /streams/{stream_id}/sources/{n}/parser/field_mappings/{m}/*

    mappings_prefix <- "/streams/{stream_id}/sources/"
    source_keys <- EtcdClient.list_keys(mappings_prefix)

    result <- []

    // Find first enabled source with parser config
    FOR EACH source_idx IN 0..10 DO  // Max 10 sources
        mapping_base <- "{mappings_prefix}{source_idx}/parser/field_mappings/"
        mapping_keys <- EtcdClient.list_keys(mapping_base)

        IF mapping_keys.is_empty() THEN
            CONTINUE
        END IF

        // Parse each field mapping
        FOR EACH idx IN 0..mapping_keys.length DO
            path <- EtcdClient.get("{mapping_base}{idx}/path")
            metric_name <- EtcdClient.get("{mapping_base}{idx}/metric_name")
            unit <- EtcdClient.get("{mapping_base}{idx}/unit")

            IF path IS NOT null AND metric_name IS NOT null THEN
                result.append(FieldMapping {
                    source_path: path,
                    target_field: metric_name,
                    unit: unit
                })
            END IF
        END FOR

        IF NOT result.is_empty() THEN
            BREAK  // Use first source with mappings
        END IF
    END FOR

    RETURN result
END

SUBROUTINE: GetEntitySchemas
INPUT: stream_id (string)
OUTPUT: Array<EntitySchema>

BEGIN
    // Key pattern: /streams/{stream_id}/entity_schemas/{n}/*
    schemas_prefix <- "/streams/{stream_id}/entity_schemas/"

    schemas <- []

    FOR EACH idx IN 0..10 DO  // Max 10 schemas per stream
        schema_base <- "{schemas_prefix}{idx}/"

        schema_name <- EtcdClient.get("{schema_base}schema_name")
        IF schema_name IS null THEN
            BREAK  // No more schemas
        END IF

        description <- EtcdClient.get("{schema_base}description")

        // Get attributes array
        attributes <- []
        FOR EACH attr_idx IN 0..50 DO  // Max 50 attributes
            attr_base <- "{schema_base}attributes/{attr_idx}/"

            name <- EtcdClient.get("{attr_base}name")
            IF name IS null THEN
                BREAK
            END IF

            attributes.append(Attribute {
                name: name,
                type: EtcdClient.get("{attr_base}type"),
                unit: EtcdClient.get("{attr_base}unit"),
                description: EtcdClient.get("{attr_base}description"),
                nullable: parse_bool(EtcdClient.get("{attr_base}nullable"))
            })
        END FOR

        schemas.append(EntitySchema {
            schema_name: schema_name,
            description: description,
            attributes: attributes
        })
    END FOR

    RETURN schemas
END

SUBROUTINE: ComputeGapAnalysis
INPUT: source_info, target_info
OUTPUT: GapAnalysis

BEGIN
    // Get all target field names
    target_fields <- SET()
    FOR EACH attr IN target_info.attributes DO
        target_fields.add(attr.name)
    END FOR

    // Get all mapped target fields
    mapped_targets <- SET()
    FOR EACH mapping IN source_info.field_mappings DO
        mapped_targets.add(mapping.target_field)
    END FOR

    // Compute gaps
    unmapped_source <- source_info.unmapped_source_fields
    target_without_mapping <- target_fields.difference(mapped_targets)

    RETURN GapAnalysis {
        unmapped_source_fields: unmapped_source,
        target_fields_without_mapping: target_without_mapping.to_array()
    }
END
```

### Complexity Analysis

| Operation | Time | Space |
|-----------|------|-------|
| Find latest Parquet | O(p) partition depth | O(1) |
| Read Parquet sample | O(s) where s = sample size | O(s) |
| Analyze JSON structure | O(s * k) where k = avg keys | O(k) |
| Get field mappings | O(m) where m = mappings count | O(m) |
| Get entity schemas | O(a) where a = attributes count | O(a) |
| **Total** | O(p + s * k + m + a) | O(k + m + a) |

---

## 3. validate_config

### Purpose
Compare entity_schemas attributes against raw_payload keys to detect schema mismatches.

### Algorithm

```
ALGORITHM: ValidateConfig
INPUT: stream_id (string)
OUTPUT: ValidationResponse

BEGIN
    // Step 1: Validate stream exists
    stream_exists <- EtcdClient.key_exists("/streams/{stream_id}/stream_id")
    IF NOT stream_exists THEN
        RETURN ErrorResponse("Stream not found: {stream_id}")
    END IF

    // Step 2: Get entity_schemas from etcd
    entity_schemas <- GetEntitySchemas(stream_id)

    IF entity_schemas.is_empty() THEN
        RETURN ErrorResponse("No entity_schemas found for stream: {stream_id}")
    END IF

    // Step 3: Find and analyze Bronze data
    parquet_file <- FindLatestParquetFile(stream_id)

    IF parquet_file IS null THEN
        RETURN ValidationResponse {
            success: true,
            stream_id: stream_id,
            entity_schema: entity_schemas[0].schema_name,
            validation: {
                status: "no_data",
                message: "No Bronze data exists for comparison"
            }
        }
    END IF

    // Step 4: Extract raw_payload field keys
    raw_payload_fields <- ExtractPayloadFields(parquet_file)

    // Step 5: Extract config fields from entity_schemas
    config_fields <- SET()
    FOR EACH attr IN entity_schemas[0].attributes DO
        config_fields.add(attr.name)
    END FOR

    // Step 6: Compute field comparison
    config_array <- config_fields.to_sorted_array()
    payload_array <- raw_payload_fields.to_sorted_array()

    in_config_not_payload <- config_fields.difference(raw_payload_fields)
    in_payload_not_config <- raw_payload_fields.difference(config_fields)
    matching <- config_fields.intersection(raw_payload_fields)

    // Step 7: Determine status
    IF in_config_not_payload.is_empty() AND in_payload_not_config.is_empty() THEN
        status <- "match"
    ELSE
        status <- "mismatch"
    END IF

    RETURN ValidationResponse {
        success: true,
        stream_id: stream_id,
        entity_schema: entity_schemas[0].schema_name,
        validation: {
            status: status,
            config_fields: config_array,
            raw_payload_fields: payload_array,
            analysis: {
                in_config_not_in_payload: in_config_not_payload.to_sorted_array(),
                in_payload_not_in_config: in_payload_not_config.to_sorted_array(),
                matching: matching.to_sorted_array()
            },
            notes: GenerateValidationNotes(status, stream_id)
        }
    }
END

SUBROUTINE: ExtractPayloadFields
INPUT: parquet_file (Path)
OUTPUT: SET<string>

BEGIN
    reader <- ParquetReader.open(parquet_file)

    // Sample multiple rows to capture field variations
    sample_size <- MIN(20, reader.row_count())
    raw_payloads <- reader.read_column("raw_payload", limit=sample_size)

    all_fields <- SET()

    FOR EACH payload_json IN raw_payloads DO
        payload <- parse_json(payload_json)
        fields <- ExtractAllFieldNames(payload, prefix="")
        all_fields <- all_fields.union(fields)
    END FOR

    RETURN all_fields
END

SUBROUTINE: ExtractAllFieldNames
INPUT: json_value, prefix (string)
OUTPUT: SET<string>

BEGIN
    fields <- SET()

    IF json_value.is_object() THEN
        FOR EACH (key, value) IN json_value DO
            full_key <- IF prefix.is_empty() THEN key ELSE "{prefix}.{key}"

            // Add top-level key
            fields.add(key)

            // Recursively extract nested keys
            IF value.is_object() THEN
                nested <- ExtractAllFieldNames(value, full_key)
                fields <- fields.union(nested)
            END IF
        END FOR
    END IF

    RETURN fields
END

SUBROUTINE: GenerateValidationNotes
INPUT: status (string), stream_id (string)
OUTPUT: string

BEGIN
    IF status == "match" THEN
        RETURN "All config fields are present in raw_payload"
    ELSE
        RETURN "Config uses flattened field names (e.g., temperature); " +
               "raw_payload preserves source structure (e.g., main.temp). " +
               "Mapping happens in Silver layer ETL."
    END IF
END
```

### Complexity Analysis

| Operation | Time | Space |
|-----------|------|-------|
| Get entity schemas | O(a) attributes | O(a) |
| Read Parquet sample | O(s) sample size | O(s) |
| Extract field names | O(s * f) where f = fields per payload | O(f) |
| Set operations | O(a + f) | O(a + f) |
| **Total** | O(a + s * f) | O(a + f) |

---

## 4. sample_data

### Purpose
Retrieve the most recent N rows from a stream's latest partition.

### Algorithm

```
ALGORITHM: SampleData
INPUT: stream_id (string), n (integer, default=10, max=100)
OUTPUT: SampleDataResponse

BEGIN
    // Step 1: Validate and clamp n
    n <- CLAMP(n, 1, 100)

    // Step 2: Find latest Parquet file
    parquet_file <- FindLatestParquetFile(stream_id)

    IF parquet_file IS null THEN
        RETURN ErrorResponse("No Bronze data found for stream: {stream_id}")
    END IF

    // Step 3: Read Parquet file
    reader <- ParquetReader.open(parquet_file)
    total_rows <- reader.row_count()

    // Step 4: Read most recent rows (they're at the end due to append-only writes)
    // Calculate offset to read last N rows
    offset <- MAX(0, total_rows - n)

    // Read all columns for selected rows
    rows <- ReadParquetRows(reader, offset, n)

    // Step 5: Convert to JSON format
    json_rows <- []
    FOR EACH row IN rows DO
        json_row <- {
            timestamp: row.timestamp,  // Keep as INT64 microseconds
            source_id: row.source_id,
            ndp_id: row.ndp_id,  // nullable
            context: parse_json(row.context),  // nullable JSON
            raw_payload: parse_json(row.raw_payload)  // JSON
        }
        json_rows.append(json_row)
    END FOR

    // Step 6: Sort by timestamp descending (most recent first)
    json_rows.sort_by_descending(row -> row.timestamp)

    RETURN SampleDataResponse {
        success: true,
        stream_id: stream_id,
        row_count: json_rows.length,
        rows: json_rows,
        source_file: parquet_file.path
    }
END

SUBROUTINE: FindLatestParquetFile
INPUT: stream_id (string)
OUTPUT: Path? (nullable)

BEGIN
    base_path <- NDP_RAW_PATH / stream_id

    IF NOT base_path.exists() THEN
        RETURN null
    END IF

    // Traverse Hive-style partitions in reverse chronological order
    // Structure: {base}/year=YYYY/month=MM/day=DD/data.parquet

    year_dirs <- list_directories(base_path)
                    .filter(d -> d.name.starts_with("year="))
                    .sort_descending_by(d -> d.name)

    FOR EACH year_dir IN year_dirs DO
        month_dirs <- list_directories(year_dir)
                        .filter(d -> d.name.starts_with("month="))
                        .sort_descending_by(d -> d.name)

        FOR EACH month_dir IN month_dirs DO
            day_dirs <- list_directories(month_dir)
                          .filter(d -> d.name.starts_with("day="))
                          .sort_descending_by(d -> d.name)

            FOR EACH day_dir IN day_dirs DO
                parquet_file <- day_dir / "data.parquet"
                IF parquet_file.exists() THEN
                    RETURN parquet_file
                END IF
            END FOR
        END FOR
    END FOR

    RETURN null
END

SUBROUTINE: ReadParquetRows
INPUT: reader (ParquetReader), offset (integer), limit (integer)
OUTPUT: Array<BronzeRow>

BEGIN
    rows <- []

    // Read schema first
    schema <- reader.schema()

    // Verify required columns exist
    required_columns <- ["timestamp", "source_id", "raw_payload"]
    FOR EACH col IN required_columns DO
        IF col NOT IN schema.column_names() THEN
            THROW Error("Missing required column: {col}")
        END IF
    END FOR

    // Read row group(s) containing our offset range
    // Parquet stores data in row groups, so we may need to skip some
    current_offset <- 0
    rows_remaining <- limit

    FOR EACH row_group IN reader.row_groups() DO
        group_rows <- row_group.row_count()

        // Skip row groups before our offset
        IF current_offset + group_rows <= offset THEN
            current_offset <- current_offset + group_rows
            CONTINUE
        END IF

        // Calculate read range within this row group
        start_in_group <- MAX(0, offset - current_offset)
        rows_to_read <- MIN(rows_remaining, group_rows - start_in_group)

        // Read columns for this range
        timestamp_col <- row_group.read_column("timestamp")[start_in_group : start_in_group + rows_to_read]
        source_id_col <- row_group.read_column("source_id")[start_in_group : start_in_group + rows_to_read]
        ndp_id_col <- row_group.read_column_optional("ndp_id")[start_in_group : start_in_group + rows_to_read]
        context_col <- row_group.read_column_optional("context")[start_in_group : start_in_group + rows_to_read]
        raw_payload_col <- row_group.read_column("raw_payload")[start_in_group : start_in_group + rows_to_read]

        // Combine into rows
        FOR i IN 0..rows_to_read DO
            rows.append(BronzeRow {
                timestamp: timestamp_col[i],
                source_id: source_id_col[i],
                ndp_id: ndp_id_col[i] IF EXISTS ELSE null,
                context: context_col[i] IF EXISTS ELSE null,
                raw_payload: raw_payload_col[i]
            })
        END FOR

        rows_remaining <- rows_remaining - rows_to_read
        current_offset <- current_offset + group_rows

        IF rows_remaining <= 0 THEN
            BREAK
        END IF
    END FOR

    RETURN rows
END
```

### Complexity Analysis

| Operation | Time | Space |
|-----------|------|-------|
| Find latest Parquet | O(p) partition depth | O(1) |
| Calculate offset | O(1) | O(1) |
| Read Parquet rows | O(n) where n = requested rows | O(n) |
| Parse JSON payloads | O(n * k) where k = avg payload size | O(n * k) |
| Sort by timestamp | O(n log n) | O(1) |
| **Total** | O(p + n * k + n log n) | O(n * k) |

---

## Shared Utility Algorithms

### FindLatestParquetFile

This subroutine is shared across multiple tools. See implementation in `sample_data` algorithm above.

### Key Design Decisions

1. **Reverse chronological traversal**: By sorting directories in descending order (2026 before 2025), we find the most recent data with minimal directory traversal.

2. **Daily partitions**: The storage uses `year=YYYY/month=MM/day=DD/data.parquet` structure (no hourly subdivision), which reduces small file proliferation.

3. **Sample-based structure analysis**: `describe_schema` samples multiple rows (10-20) to capture field variations in the JSON payloads.

4. **Fail-fast validation**: All tools validate input parameters and existence of required resources before performing work.

---

## Data Structures

### Bronze Row Schema

| Column | Parquet Type | Arrow Type | Description |
|--------|--------------|------------|-------------|
| timestamp | INT64 | Timestamp(Microsecond) | Ingestion timestamp |
| source_id | BYTE_ARRAY | Utf8 | Source identifier |
| ndp_id | BYTE_ARRAY | Utf8 (nullable) | Platform-assigned ID |
| context | BYTE_ARRAY | Utf8 (nullable) | JSON metadata |
| raw_payload | BYTE_ARRAY | Utf8 | JSON source data |

### etcd Key Patterns

| Pattern | Example | Description |
|---------|---------|-------------|
| `/streams/{stream_id}/stream_id` | `/streams/air-quality/stream_id` | Stream identifier |
| `/streams/{stream_id}/enabled` | `/streams/air-quality/enabled` | Enabled flag |
| `/streams/{stream_id}/sources/{n}/type` | `/streams/air-quality/sources/0/type` | Source type |
| `/streams/{stream_id}/entity_schemas/{n}/schema_name` | `/streams/air-quality/entity_schemas/0/schema_name` | Schema name |
| `/streams/{stream_id}/entity_schemas/{n}/attributes/{m}/name` | `/streams/air-quality/entity_schemas/0/attributes/0/name` | Attribute name |

---

## Error Handling

See `error-handling.md` for comprehensive error flow pseudocode.

Key error cases handled:
- Stream not found in etcd
- No Bronze data exists
- Parquet read failures
- JSON parse failures in raw_payload
- etcd connection failures

---

*Pseudocode ready for implementation in Rust with axum, etcd-client, and arrow/parquet crates.*
