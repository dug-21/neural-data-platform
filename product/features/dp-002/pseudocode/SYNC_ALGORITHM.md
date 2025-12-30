# SYNC_ALGORITHM.md - Data Dictionary Synchronization

## Overview

This document defines the pseudocode for synchronizing entity_schemas from etcd configuration store to the TimescaleDB data_dictionary table. The sync mechanism supports full sync (initial population), incremental sync (updates only), and orphan detection (deleted schemas).

---

## Data Structures

### TimescaleDB Schema

```
TABLE: data_dictionary
COLUMNS:
  - id:            SERIAL PRIMARY KEY
  - stream_id:     VARCHAR(100) NOT NULL
  - schema_name:   VARCHAR(200) NOT NULL
  - attribute:     VARCHAR(100) NOT NULL
  - type:          VARCHAR(50) NOT NULL
  - unit:          VARCHAR(50)
  - description:   TEXT
  - nullable:      BOOLEAN DEFAULT true
  - metadata:      JSONB                    -- Extensible metadata
  - created_at:    TIMESTAMP DEFAULT NOW()
  - updated_at:    TIMESTAMP DEFAULT NOW()
  - sync_hash:     VARCHAR(64)              -- SHA256 for change detection

UNIQUE CONSTRAINT: (stream_id, schema_name, attribute)
INDEX: (stream_id)
INDEX: (schema_name)
INDEX: (sync_hash)
```

### etcd Configuration Structure

```
PATH: /streams/{stream_id}/entity_schemas

YAML Structure:
  entity_schemas:
    - schema_name: "string"
      description: "string"
      device_class: "string"       -- Optional, for HomeAssistant
      pattern: "string"            -- Optional, glob pattern for entity matching
      attributes:
        - name: "string"
          type: "string"
          unit: "string"           -- Optional
          description: "string"    -- Optional
          nullable: boolean        -- Optional, default true
```

---

## Algorithm 1: Full Sync (Initial Population)

```
ALGORITHM: FullDataDictionarySync
PURPOSE: Populate data_dictionary from all stream configurations in etcd

INPUT:
  - etcd_connection: Connection to etcd (endpoint, credentials)
  - timescale_connection: Connection to TimescaleDB
  - stream_prefix: etcd key prefix for streams (default: "/streams/")

OUTPUT:
  - sync_report: {
      streams_processed: integer,
      entries_inserted: integer,
      entries_updated: integer,
      errors: List<SyncError>
    }

CONSTANTS:
  BATCH_SIZE = 100  -- Rows per transaction batch

BEGIN
    // Initialize report
    report ← {
        streams_processed: 0,
        entries_inserted: 0,
        entries_updated: 0,
        errors: []
    }

    // Step 1: Get list of all stream configurations from etcd
    stream_keys ← etcd_connection.get_keys_with_prefix(stream_prefix)

    // Filter to only config.yaml files
    config_keys ← FILTER stream_keys WHERE key ENDS WITH "/config.yaml"

    IF config_keys IS EMPTY THEN
        RETURN report WITH warning "No stream configurations found"
    END IF

    // Step 2: Process each stream configuration
    pending_rows ← []

    FOR EACH config_key IN config_keys DO
        TRY
            // Extract stream_id from path: /streams/{stream_id}/config.yaml
            stream_id ← ExtractStreamId(config_key)

            // Get configuration content from etcd
            config_yaml ← etcd_connection.get(config_key)

            // Parse and validate configuration
            config ← ParseYAML(config_yaml)

            // Check if entity_schemas section exists
            IF config.entity_schemas IS NULL OR config.entity_schemas IS EMPTY THEN
                report.errors.append({
                    stream_id: stream_id,
                    error: "No entity_schemas section found",
                    severity: "warning"
                })
                CONTINUE  -- Skip this stream
            END IF

            // Step 3: Transform each entity_schema to dictionary rows
            FOR EACH schema IN config.entity_schemas DO
                rows ← TransformSchemaToRows(stream_id, schema)
                pending_rows ← pending_rows + rows

                // Batch insert when threshold reached
                IF LENGTH(pending_rows) >= BATCH_SIZE THEN
                    BatchUpsert(timescale_connection, pending_rows, report)
                    pending_rows ← []
                END IF
            END FOR

            report.streams_processed ← report.streams_processed + 1

        CATCH ParseError AS e
            report.errors.append({
                stream_id: stream_id,
                error: "YAML parse error: " + e.message,
                severity: "error"
            })
        CATCH ConnectionError AS e
            report.errors.append({
                stream_id: stream_id,
                error: "etcd connection error: " + e.message,
                severity: "critical"
            })
            BREAK  -- Stop processing on connection failure
        END TRY
    END FOR

    // Step 4: Insert remaining rows
    IF pending_rows IS NOT EMPTY THEN
        BatchUpsert(timescale_connection, pending_rows, report)
    END IF

    // Step 5: Log summary
    LogSyncSummary(report)

    RETURN report
END


SUBROUTINE: ExtractStreamId
INPUT: config_key (string, e.g., "/streams/air-quality/config.yaml")
OUTPUT: stream_id (string, e.g., "air-quality")

BEGIN
    parts ← SPLIT(config_key, "/")
    // Path format: ["", "streams", "{stream_id}", "config.yaml"]
    IF LENGTH(parts) >= 3 THEN
        RETURN parts[2]
    ELSE
        RAISE InvalidPathError("Cannot extract stream_id from: " + config_key)
    END IF
END


SUBROUTINE: TransformSchemaToRows
INPUT: stream_id (string), schema (EntitySchema object)
OUTPUT: rows (List of dictionary row objects)

BEGIN
    rows ← []
    schema_name ← schema.schema_name
    schema_description ← schema.description OR ""
    device_class ← schema.device_class OR NULL
    pattern ← schema.pattern OR NULL

    // Calculate hash for entire schema (for change detection)
    schema_hash ← SHA256(serialize(schema))

    FOR EACH attr IN schema.attributes DO
        row ← {
            stream_id: stream_id,
            schema_name: schema_name,
            attribute: attr.name,
            type: attr.type,
            unit: attr.unit OR NULL,
            description: attr.description OR "",
            nullable: attr.nullable OR true,
            metadata: {
                schema_description: schema_description,
                device_class: device_class,
                pattern: pattern
            },
            sync_hash: schema_hash
        }
        rows.append(row)
    END FOR

    RETURN rows
END


SUBROUTINE: BatchUpsert
INPUT: connection, rows (List), report (mutable reference)
OUTPUT: None (modifies report in place)

BEGIN
    // Start transaction
    transaction ← connection.begin_transaction()

    TRY
        FOR EACH row IN rows DO
            // Use UPSERT (INSERT ... ON CONFLICT UPDATE)
            result ← transaction.execute(
                """
                INSERT INTO data_dictionary
                    (stream_id, schema_name, attribute, type, unit, description, nullable, metadata, sync_hash, updated_at)
                VALUES
                    ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW())
                ON CONFLICT (stream_id, schema_name, attribute)
                DO UPDATE SET
                    type = EXCLUDED.type,
                    unit = EXCLUDED.unit,
                    description = EXCLUDED.description,
                    nullable = EXCLUDED.nullable,
                    metadata = EXCLUDED.metadata,
                    sync_hash = EXCLUDED.sync_hash,
                    updated_at = NOW()
                WHERE data_dictionary.sync_hash != EXCLUDED.sync_hash
                RETURNING (xmax = 0) AS inserted
                """,
                row.values()
            )

            IF result.inserted THEN
                report.entries_inserted ← report.entries_inserted + 1
            ELSE
                report.entries_updated ← report.entries_updated + 1
            END IF
        END FOR

        transaction.commit()

    CATCH Error AS e
        transaction.rollback()
        RAISE SyncError("Batch upsert failed: " + e.message)
    END TRY
END
```

---

## Algorithm 2: Incremental Sync (Watch-Based Updates)

```
ALGORITHM: IncrementalDataDictionarySync
PURPOSE: React to etcd watch events and update only changed configurations

INPUT:
  - etcd_watch_event: {
      event_type: "PUT" | "DELETE",
      key: string,
      value: string (YAML content, for PUT only),
      mod_revision: integer
    }
  - timescale_connection: Connection to TimescaleDB

OUTPUT:
  - sync_result: {
      action: "insert" | "update" | "delete" | "skip",
      stream_id: string,
      changes: integer
    }

BEGIN
    // Extract stream_id from key
    IF NOT key MATCHES "/streams/*/config.yaml" THEN
        RETURN {action: "skip", stream_id: NULL, changes: 0}
    END IF

    stream_id ← ExtractStreamId(event.key)

    SWITCH event.event_type
        CASE "PUT":
            // Configuration added or modified
            TRY
                config ← ParseYAML(event.value)

                IF config.entity_schemas IS NULL THEN
                    // No entity_schemas - might need to delete existing
                    DeleteStreamFromDictionary(timescale_connection, stream_id)
                    RETURN {action: "delete", stream_id: stream_id, changes: 0}
                END IF

                // Get current entries for this stream
                current_hashes ← GetCurrentSyncHashes(timescale_connection, stream_id)

                // Transform new schemas to rows
                new_rows ← []
                FOR EACH schema IN config.entity_schemas DO
                    new_rows ← new_rows + TransformSchemaToRows(stream_id, schema)
                END FOR

                // Identify changes
                new_keys ← SET(row.stream_id + row.schema_name + row.attribute FOR row IN new_rows)
                current_keys ← SET(current_hashes.keys())

                to_delete ← current_keys - new_keys  // Entries to remove
                to_upsert ← new_rows WHERE hash changed OR key is new

                // Apply changes in transaction
                transaction ← timescale_connection.begin_transaction()

                TRY
                    // Delete removed entries
                    FOR EACH key IN to_delete DO
                        (stream_id, schema_name, attribute) ← ParseCompositeKey(key)
                        transaction.execute(
                            "DELETE FROM data_dictionary WHERE stream_id = $1 AND schema_name = $2 AND attribute = $3",
                            [stream_id, schema_name, attribute]
                        )
                    END FOR

                    // Upsert changed/new entries
                    BatchUpsert(transaction, to_upsert)

                    transaction.commit()

                CATCH Error AS e
                    transaction.rollback()
                    RAISE e
                END TRY

                RETURN {
                    action: "update",
                    stream_id: stream_id,
                    changes: LENGTH(to_delete) + LENGTH(to_upsert)
                }

            CATCH ParseError AS e
                Log.error("Failed to parse config for " + stream_id + ": " + e.message)
                RETURN {action: "skip", stream_id: stream_id, changes: 0}
            END TRY

        CASE "DELETE":
            // Configuration deleted - remove all entries for this stream
            deleted_count ← DeleteStreamFromDictionary(timescale_connection, stream_id)
            RETURN {action: "delete", stream_id: stream_id, changes: deleted_count}

    END SWITCH
END


SUBROUTINE: GetCurrentSyncHashes
INPUT: connection, stream_id
OUTPUT: Map<composite_key, sync_hash>

BEGIN
    results ← connection.query(
        """
        SELECT stream_id || '::' || schema_name || '::' || attribute AS key, sync_hash
        FROM data_dictionary
        WHERE stream_id = $1
        """,
        [stream_id]
    )

    RETURN MAP(row.key → row.sync_hash FOR row IN results)
END


SUBROUTINE: DeleteStreamFromDictionary
INPUT: connection, stream_id
OUTPUT: deleted_count (integer)

BEGIN
    result ← connection.execute(
        "DELETE FROM data_dictionary WHERE stream_id = $1 RETURNING 1",
        [stream_id]
    )
    RETURN result.row_count
END
```

---

## Algorithm 3: Orphan Detection and Cleanup

```
ALGORITHM: DetectAndCleanOrphanedEntries
PURPOSE: Identify data dictionary entries that no longer exist in etcd configuration

INPUT:
  - etcd_connection: Connection to etcd
  - timescale_connection: Connection to TimescaleDB
  - dry_run: boolean (if true, only report, don't delete)

OUTPUT:
  - orphan_report: {
      orphaned_streams: List<stream_id>,
      orphaned_entries: List<{stream_id, schema_name, attribute}>,
      deleted_count: integer (0 if dry_run)
    }

BEGIN
    report ← {
        orphaned_streams: [],
        orphaned_entries: [],
        deleted_count: 0
    }

    // Step 1: Get all stream_ids from data dictionary
    db_streams ← SET(
        connection.query("SELECT DISTINCT stream_id FROM data_dictionary")
    )

    // Step 2: Get all stream_ids from etcd
    config_keys ← etcd_connection.get_keys_with_prefix("/streams/")
    etcd_streams ← SET()

    FOR EACH key IN config_keys DO
        IF key MATCHES "/streams/*/config.yaml" THEN
            etcd_streams.add(ExtractStreamId(key))
        END IF
    END FOR

    // Step 3: Identify orphaned streams (in DB but not in etcd)
    orphaned_streams ← db_streams - etcd_streams
    report.orphaned_streams ← LIST(orphaned_streams)

    // Step 4: For streams that exist in both, check for orphaned schemas
    common_streams ← db_streams INTERSECTION etcd_streams

    FOR EACH stream_id IN common_streams DO
        // Get schemas from database
        db_entries ← connection.query(
            """
            SELECT DISTINCT schema_name, attribute
            FROM data_dictionary
            WHERE stream_id = $1
            """,
            [stream_id]
        )
        db_schema_attrs ← SET((entry.schema_name, entry.attribute) FOR entry IN db_entries)

        // Get schemas from etcd config
        config_yaml ← etcd_connection.get("/streams/" + stream_id + "/config.yaml")
        config ← ParseYAML(config_yaml)

        etcd_schema_attrs ← SET()
        IF config.entity_schemas IS NOT NULL THEN
            FOR EACH schema IN config.entity_schemas DO
                FOR EACH attr IN schema.attributes DO
                    etcd_schema_attrs.add((schema.schema_name, attr.name))
                END FOR
            END FOR
        END IF

        // Find orphaned entries
        orphaned ← db_schema_attrs - etcd_schema_attrs
        FOR EACH (schema_name, attribute) IN orphaned DO
            report.orphaned_entries.append({
                stream_id: stream_id,
                schema_name: schema_name,
                attribute: attribute
            })
        END FOR
    END FOR

    // Step 5: Clean up orphaned entries (unless dry_run)
    IF NOT dry_run THEN
        transaction ← timescale_connection.begin_transaction()

        TRY
            // Delete orphaned streams entirely
            FOR EACH stream_id IN orphaned_streams DO
                transaction.execute(
                    "DELETE FROM data_dictionary WHERE stream_id = $1",
                    [stream_id]
                )
            END FOR

            // Delete individual orphaned entries
            FOR EACH entry IN report.orphaned_entries DO
                transaction.execute(
                    """
                    DELETE FROM data_dictionary
                    WHERE stream_id = $1 AND schema_name = $2 AND attribute = $3
                    """,
                    [entry.stream_id, entry.schema_name, entry.attribute]
                )
            END FOR

            report.deleted_count ← LENGTH(report.orphaned_entries) +
                SumEntriesInStreams(orphaned_streams)

            transaction.commit()

        CATCH Error AS e
            transaction.rollback()
            RAISE CleanupError("Orphan cleanup failed: " + e.message)
        END TRY
    END IF

    RETURN report
END
```

---

## Algorithm 4: Deploy Script Integration

```
ALGORITHM: SyncDictionaryCommand
PURPOSE: Integration with deploy.sh as "sync-dictionary" command

INPUT:
  - mode: "full" | "incremental" | "cleanup" | "status"
  - dry_run: boolean (default: false)
  - verbose: boolean (default: false)

OUTPUT:
  - exit_code: 0 (success) | 1 (error) | 2 (warning)

BEGIN
    // Step 1: Validate prerequisites
    IF NOT CheckTimescaleDBRunning() THEN
        Log.error("TimescaleDB is not running")
        RETURN 1
    END IF

    IF NOT CheckEtcdRunning() THEN
        Log.error("etcd is not running")
        RETURN 1
    END IF

    // Step 2: Establish connections
    etcd_conn ← ConnectToEtcd("etcd:2379")
    timescale_conn ← ConnectToTimescaleDB("timescaledb:5432", "ndp", "ndp_password")

    // Step 3: Ensure schema exists
    EnsureDataDictionaryTableExists(timescale_conn)

    // Step 4: Execute requested mode
    SWITCH mode
        CASE "full":
            Log.info("Starting full data dictionary sync...")
            report ← FullDataDictionarySync(etcd_conn, timescale_conn, "/streams/")

            IF report.errors HAS severity "critical" THEN
                Log.error("Sync completed with critical errors")
                RETURN 1
            ELSE IF report.errors HAS severity "error" THEN
                Log.warn("Sync completed with errors")
                PrintSyncReport(report, verbose)
                RETURN 2
            ELSE
                Log.info("Sync completed successfully")
                PrintSyncReport(report, verbose)
                RETURN 0
            END IF

        CASE "incremental":
            Log.info("Starting incremental sync (watch mode)...")
            // This would be a long-running process
            WatchAndSync(etcd_conn, timescale_conn)
            // Never returns unless interrupted

        CASE "cleanup":
            Log.info("Detecting orphaned entries...")
            report ← DetectAndCleanOrphanedEntries(etcd_conn, timescale_conn, dry_run)

            IF dry_run THEN
                Log.info("DRY RUN - No changes made")
                Log.info("Would delete " + LENGTH(report.orphaned_entries) + " orphaned entries")
            ELSE
                Log.info("Deleted " + report.deleted_count + " orphaned entries")
            END IF

            PrintOrphanReport(report, verbose)
            RETURN 0

        CASE "status":
            stats ← GetDictionaryStats(timescale_conn)
            PrintDictionaryStatus(stats)
            RETURN 0

    END SWITCH
END


SUBROUTINE: EnsureDataDictionaryTableExists
INPUT: connection
OUTPUT: None

BEGIN
    connection.execute("""
        CREATE TABLE IF NOT EXISTS data_dictionary (
            id SERIAL PRIMARY KEY,
            stream_id VARCHAR(100) NOT NULL,
            schema_name VARCHAR(200) NOT NULL,
            attribute VARCHAR(100) NOT NULL,
            type VARCHAR(50) NOT NULL,
            unit VARCHAR(50),
            description TEXT,
            nullable BOOLEAN DEFAULT true,
            metadata JSONB,
            created_at TIMESTAMP DEFAULT NOW(),
            updated_at TIMESTAMP DEFAULT NOW(),
            sync_hash VARCHAR(64),
            UNIQUE(stream_id, schema_name, attribute)
        );

        CREATE INDEX IF NOT EXISTS idx_dd_stream_id ON data_dictionary(stream_id);
        CREATE INDEX IF NOT EXISTS idx_dd_schema_name ON data_dictionary(schema_name);
        CREATE INDEX IF NOT EXISTS idx_dd_sync_hash ON data_dictionary(sync_hash);
    """)
END


SUBROUTINE: GetDictionaryStats
INPUT: connection
OUTPUT: stats object

BEGIN
    RETURN connection.query("""
        SELECT
            COUNT(DISTINCT stream_id) AS stream_count,
            COUNT(DISTINCT schema_name) AS schema_count,
            COUNT(*) AS attribute_count,
            MIN(updated_at) AS oldest_update,
            MAX(updated_at) AS latest_update
        FROM data_dictionary
    """)[0]
END
```

---

## Complexity Analysis

### FullDataDictionarySync

| Operation | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| Get stream keys from etcd | O(s) | O(s) |
| Parse YAML config | O(c) per stream | O(c) |
| Transform schemas to rows | O(a) per schema | O(a) |
| Batch upsert | O(n) total rows | O(b) batch size |
| **Total** | **O(s * c * a)** | **O(s + b)** |

Where:
- s = number of streams
- c = size of config YAML
- a = number of attributes per schema
- n = total dictionary entries
- b = batch size (constant 100)

### IncrementalSync

| Operation | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| Parse single config | O(c) | O(c) |
| Get current hashes | O(e) | O(e) |
| Compute diff | O(e) | O(e) |
| Apply changes | O(d + u) | O(1) |
| **Total** | **O(c + e)** | **O(c + e)** |

Where:
- e = existing entries for stream
- d = entries to delete
- u = entries to upsert

### OrphanDetection

| Operation | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| Get DB streams | O(s) | O(s) |
| Get etcd streams | O(s) | O(s) |
| Compare schemas per stream | O(a) | O(a) |
| Delete orphans | O(o) | O(1) |
| **Total** | **O(s * a + o)** | **O(s + a)** |

Where:
- o = orphaned entries count

---

## Error Handling

### Error Categories

1. **Connection Errors** (Critical)
   - etcd unreachable
   - TimescaleDB unreachable
   - Network timeout

2. **Parse Errors** (Error)
   - Invalid YAML syntax
   - Missing required fields
   - Type mismatches

3. **Sync Errors** (Error)
   - Transaction failures
   - Constraint violations
   - Deadlocks

4. **Validation Warnings** (Warning)
   - Missing entity_schemas section
   - Empty attributes list
   - Unknown type values

### Rollback Strategy

```
STRATEGY: Transaction Rollback

1. All upserts within a single stream are wrapped in ONE transaction
2. If any row fails, entire stream transaction rolls back
3. Other streams continue processing
4. Failed streams are logged to error report
5. Retry logic: 3 attempts with exponential backoff for transient errors

RETRY_CONFIG:
  max_attempts: 3
  initial_delay: 1s
  max_delay: 10s
  backoff_factor: 2
```

---

## Worked Example

### Input Configuration (air-quality/config.yaml)

```yaml
stream_id: "air-quality"
entity_schemas:
  - schema_name: "airgradient"
    description: "AirGradient indoor air quality sensors"
    device_class: "air_quality"
    attributes:
      - name: "pm25"
        type: "float"
        unit: "ug/m3"
        description: "Particulate Matter 2.5 micrometers"
      - name: "co2"
        type: "int"
        unit: "ppm"
        description: "Carbon Dioxide concentration"
```

### Transformed Rows

| stream_id | schema_name | attribute | type | unit | description | metadata |
|-----------|-------------|-----------|------|------|-------------|----------|
| air-quality | airgradient | pm25 | float | ug/m3 | Particulate Matter 2.5 micrometers | {"schema_description": "AirGradient indoor...", "device_class": "air_quality"} |
| air-quality | airgradient | co2 | int | ppm | Carbon Dioxide concentration | {"schema_description": "AirGradient indoor...", "device_class": "air_quality"} |

### Sync Report Output

```
[SYNC] Data Dictionary Sync Complete
  Streams processed: 7
  Entries inserted: 42
  Entries updated: 0
  Errors: 0
  Warnings: 1 (homeassistant stream pending entity_schemas)
```

---

## Integration Points

### Deploy Script Command

```bash
# Add to deploy.sh case statement:

sync-dictionary)
    log "Syncing data dictionary..."
    # Execute sync via docker
    docker exec timescaledb psql -U ndp -d ndp -f /scripts/sync-dictionary.sql
    # Or via a sync utility container
    docker run --rm --network=pi_default ndp-sync-tool full
    ;;
```

### Watch Mode (Future Enhancement)

```bash
# Long-running watch process
./deploy.sh sync-dictionary --watch

# This would:
# 1. Do initial full sync
# 2. Start etcd watch on /streams/ prefix
# 3. Process PUT/DELETE events incrementally
# 4. Run orphan cleanup every 24 hours
```
