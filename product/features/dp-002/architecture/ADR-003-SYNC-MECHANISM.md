# ADR-003: etcd to TimescaleDB Sync Mechanism

**Status**: Proposed
**Date**: 2025-12-30
**Decision Makers**: NDP Architecture Team
**Context**: DP-002 Online Data Dictionary and HomeAssistant Stream Preparation
**Supersedes**: None

---

## Context

The Data Dictionary (ADR-001) stores stream metadata in TimescaleDB. However, the **source of truth** for stream configuration remains:

1. **YAML files**: `config/base/streams/{stream-id}/config.yaml` (version controlled)
2. **etcd**: `/streams/{stream-id}/config` (runtime configuration)

We need a **synchronization mechanism** to populate TimescaleDB from these sources. This sync must:

1. Handle initial population (full sync)
2. Support incremental updates (when configs change)
3. Integrate with existing `deploy.sh` workflow
4. Handle add/update/delete operations correctly
5. Be robust to failures (atomic or rollback)

### Current Deployment Flow

```
YAML Files --> deploy.sh sync --> etcd --> air-quality-app watches
                                        --> config-client reads
```

### Proposed Flow with Data Dictionary

```
YAML Files --> deploy.sh sync --> etcd --> TimescaleDB (data dictionary)
                                        --> air-quality-app watches
                                        --> Grafana queries TimescaleDB
```

---

## Decision

**Implement sync as a shell script integrated into `deploy.sh`, using direct SQL for atomic operations.**

### Sync Architecture

```
deploy.sh sync
    |
    +-- 1. Read YAML files from config/base/streams/
    |
    +-- 2. Sync to etcd (existing behavior)
    |
    +-- 3. NEW: Sync to TimescaleDB
            |
            +-- Generate SQL from YAML
            +-- Execute in transaction
            +-- Log sync status
```

### Sync Command Integration

```bash
# deploy/pi/deploy.sh

sync() {
    echo "Syncing configuration..."

    # Step 1: Sync to etcd (existing)
    sync_to_etcd

    # Step 2: NEW - Sync to TimescaleDB Data Dictionary
    sync_to_data_dictionary

    echo "Sync complete"
}

sync_to_data_dictionary() {
    echo "Syncing Data Dictionary to TimescaleDB..."

    # Generate SQL from YAML configs
    local SQL_FILE="/tmp/data_dictionary_sync_$$.sql"
    generate_sync_sql > "$SQL_FILE"

    # Execute in TimescaleDB container
    docker exec -i pi5-timescaledb psql -U postgres -d ndp -f - < "$SQL_FILE"

    if [ $? -eq 0 ]; then
        echo "Data Dictionary sync successful"
        rm "$SQL_FILE"
    else
        echo "ERROR: Data Dictionary sync failed"
        rm "$SQL_FILE"
        return 1
    fi
}
```

### SQL Generation Strategy

#### Full Sync (Initial or Reset)

```bash
generate_sync_sql() {
    cat <<'EOF'
-- Data Dictionary Full Sync
-- Generated: $(date -Iseconds)
-- Source: YAML configuration files

BEGIN;

-- Record sync start
INSERT INTO data_dictionary.sync_status (sync_type, status)
VALUES ('full', 'running')
RETURNING id AS sync_id \gset

-- Clear existing data (preserving sync_status history)
TRUNCATE data_dictionary.streams CASCADE;

EOF

    # Generate INSERT statements for each stream
    for config_file in config/base/streams/*/config.yaml; do
        if [ -f "$config_file" ]; then
            generate_stream_sql "$config_file"
        fi
    done

    cat <<'EOF'

-- Update sync status to success
UPDATE data_dictionary.sync_status
SET completed_at = NOW(),
    status = 'success',
    streams_synced = (SELECT COUNT(*) FROM data_dictionary.streams),
    fields_synced = (SELECT COUNT(*) FROM data_dictionary.fields),
    entities_synced = (SELECT COUNT(*) FROM data_dictionary.entity_schemas)
WHERE id = :'sync_id';

COMMIT;
EOF
}
```

#### Stream SQL Generation

```bash
generate_stream_sql() {
    local config_file="$1"
    local stream_id=$(yq eval '.stream_id' "$config_file")
    local description=$(yq eval '.description // ""' "$config_file" | sql_escape)
    local version=$(yq eval '.version // "1.0.0"' "$config_file")
    local enabled=$(yq eval '.enabled // true' "$config_file")
    local retention_days=$(yq eval '.retention_days // 90' "$config_file")

    cat <<EOF
-- Stream: $stream_id
INSERT INTO data_dictionary.streams (stream_id, description, version, enabled, retention_days)
VALUES ('$stream_id', '$description', '$version', $enabled, $retention_days);

EOF

    # Generate field inserts
    generate_fields_sql "$config_file" "$stream_id"

    # Generate entity schema inserts (if present)
    generate_entity_schemas_sql "$config_file" "$stream_id"

    # Generate source inserts
    generate_sources_sql "$config_file" "$stream_id"
}

generate_fields_sql() {
    local config_file="$1"
    local stream_id="$2"

    local field_count=$(yq eval '.fields | length' "$config_file")
    for i in $(seq 0 $((field_count - 1))); do
        local name=$(yq eval ".fields[$i].name" "$config_file")
        local field_type=$(yq eval ".fields[$i].field_type // .fields[$i].type // 'String'" "$config_file")
        local nullable=$(yq eval ".fields[$i].nullable // true" "$config_file")
        local unit=$(yq eval ".fields[$i].unit // null" "$config_file")
        local description=$(yq eval ".fields[$i].description // ''" "$config_file" | sql_escape)

        cat <<EOF
INSERT INTO data_dictionary.fields (stream_id, field_name, field_type, nullable, unit, description, sort_order)
VALUES ('$stream_id', '$name', '$field_type', $nullable, $(sql_null "$unit"), '$description', $i);
EOF
    done
}

generate_entity_schemas_sql() {
    local config_file="$1"
    local stream_id="$2"

    local schema_count=$(yq eval '.entity_schemas | length // 0' "$config_file")
    if [ "$schema_count" -eq 0 ]; then
        return
    fi

    for i in $(seq 0 $((schema_count - 1))); do
        local pattern=$(yq eval ".entity_schemas[$i].pattern" "$config_file")
        local domain=$(yq eval ".entity_schemas[$i].domain" "$config_file")
        local device_class=$(yq eval ".entity_schemas[$i].device_class // null" "$config_file")
        local description=$(yq eval ".entity_schemas[$i].description // ''" "$config_file" | sql_escape)
        local protocol=$(yq eval ".entity_schemas[$i].protocol // null" "$config_file")
        local enabled=$(yq eval ".entity_schemas[$i].enabled // true" "$config_file")
        local priority=$(yq eval ".entity_schemas[$i].priority // 0" "$config_file")
        local state_mapping=$(yq eval ".entity_schemas[$i].state_mapping // {}" "$config_file" -o=json)

        cat <<EOF
INSERT INTO data_dictionary.entity_schemas
    (stream_id, entity_pattern, entity_domain, device_class, description, protocol, enabled, priority, state_mapping)
VALUES
    ('$stream_id', '$pattern', '$domain', $(sql_null "$device_class"), '$description', $(sql_null "$protocol"), $enabled, $priority, '$state_mapping'::jsonb);
EOF
    done
}
```

### Incremental Sync (Future Enhancement)

For incremental sync, track etcd revision and only sync changed streams:

```sql
-- Check last sync revision
SELECT etcd_revision FROM data_dictionary.sync_status
WHERE status = 'success'
ORDER BY completed_at DESC
LIMIT 1;

-- If current etcd revision > last sync revision, sync changed keys
```

**Decision**: Start with full sync; incremental sync is a future optimization.

### Error Handling and Rollback

```bash
sync_to_data_dictionary() {
    local SQL_FILE="/tmp/data_dictionary_sync_$$.sql"
    local ERROR_FILE="/tmp/data_dictionary_error_$$.log"

    generate_sync_sql > "$SQL_FILE"

    # Execute with error capture
    docker exec -i pi5-timescaledb psql -U postgres -d ndp \
        -v ON_ERROR_STOP=1 \
        -f - < "$SQL_FILE" 2> "$ERROR_FILE"

    local exit_code=$?

    if [ $exit_code -ne 0 ]; then
        echo "ERROR: Data Dictionary sync failed"
        echo "Error details:"
        cat "$ERROR_FILE"

        # Record failed sync (outside transaction, so always succeeds)
        docker exec -i pi5-timescaledb psql -U postgres -d ndp <<EOF
INSERT INTO data_dictionary.sync_status (sync_type, status, error_message)
VALUES ('full', 'failed', '$(cat "$ERROR_FILE" | sql_escape)');
EOF

        rm -f "$SQL_FILE" "$ERROR_FILE"
        return 1
    fi

    rm -f "$SQL_FILE" "$ERROR_FILE"
    echo "Data Dictionary sync successful"
}
```

### Add/Update/Delete Semantics

| Operation | Detection | SQL Action |
|-----------|-----------|------------|
| **Add Stream** | New YAML file | INSERT new rows |
| **Update Stream** | Modified YAML | TRUNCATE + INSERT (full sync) |
| **Delete Stream** | YAML removed | CASCADE DELETE (stream missing after TRUNCATE) |

**Decision**: Use TRUNCATE + INSERT (idempotent full sync) rather than complex MERGE/UPSERT logic. This is simpler and correct for the expected update frequency (rare schema changes).

---

## Rationale

### Why Shell Script Over Rust Binary

| Criterion | Shell Script | Rust Binary |
|-----------|--------------|-------------|
| **Development Speed** | Fast | Slower |
| **Dependencies** | yq, psql (already available) | Compile, link, deploy |
| **Debugging** | Easy (echo, set -x) | Requires logging setup |
| **Deployment** | No additional binary | New container or sidecar |
| **Maintenance** | deploy.sh already maintained | New codebase |

**Decision**: Shell script for initial implementation. Consider Rust if performance becomes an issue (unlikely for schema sync).

### Why Full Sync Over Incremental

| Criterion | Full Sync | Incremental Sync |
|-----------|-----------|------------------|
| **Correctness** | Always matches YAML | Risk of drift |
| **Complexity** | Simple (TRUNCATE + INSERT) | Complex (diff, merge) |
| **Performance** | 10-50 streams in <1s | Marginal improvement |
| **Recovery** | Idempotent (run again) | May need manual fix |

**Decision**: Full sync is sufficient for expected scale (10-50 streams). Incremental sync adds complexity without significant benefit.

### Why Direct SQL Over ORM/Migration Tool

| Criterion | Direct SQL | Migration Tool (e.g., Flyway) |
|-----------|------------|-------------------------------|
| **Deployment Complexity** | Low (psql available) | Requires Java/migration runner |
| **Data Sync** | Natural fit | Designed for schema, not data |
| **Rollback** | Transaction rollback | Complex for data changes |
| **Integration** | deploy.sh already exists | New workflow |

**Decision**: Direct SQL in transactions provides atomic operations without additional tooling.

---

## Consequences

### Positive

1. **Atomic Updates**: SQL transaction ensures all-or-nothing sync
2. **Idempotent**: Running sync twice produces same result
3. **Simple Recovery**: On failure, re-run sync
4. **Audit Trail**: sync_status table records history
5. **No New Dependencies**: Uses existing yq, psql

### Negative

1. **Full Sync Overhead**: Clears and repopulates all data each time
2. **Shell Complexity**: SQL generation in shell can be fragile
3. **No Real-time Watch**: Updates require explicit sync command

### Risks

1. **SQL Injection**: Improperly escaped YAML values
   - **Mitigation**: sql_escape helper function for all string values
2. **Partial Failure**: Transaction rollback may leave stale data
   - **Mitigation**: TRUNCATE before INSERT ensures clean state
3. **Timing Window**: Brief period where data dictionary is empty during sync
   - **Mitigation**: Acceptable for schema changes (not hot path)

---

## Alternatives Considered

### Alternative 1: Rust Sync Binary

Create a dedicated Rust binary for sync operations.

```rust
// sync-data-dictionary/src/main.rs
async fn main() {
    let configs = load_yaml_configs();
    let pool = PgPool::connect(&database_url).await?;
    sync_to_database(&pool, configs).await?;
}
```

**Pros**:
- Type-safe
- Better error handling
- Could integrate with air-quality-app

**Cons**:
- New binary to build, deploy, maintain
- Overkill for simple sync operation
- Deployment complexity

**Verdict**: Deferred - Shell script sufficient for initial implementation

### Alternative 2: etcd Watch + Real-time Sync

Have a service watch etcd and sync changes immediately to TimescaleDB.

```rust
// In air-quality-app or separate service
async fn watch_and_sync() {
    let mut watcher = etcd_client.watch("/streams/").await?;
    while let Some(event) = watcher.next().await {
        sync_stream_to_db(&event.key, &event.value).await?;
    }
}
```

**Pros**:
- Real-time updates
- No manual sync step

**Cons**:
- Complex failure handling (what if DB down?)
- Additional service to manage
- Race conditions with deploy.sh

**Verdict**: Rejected - Over-engineering for rare schema changes

### Alternative 3: Database Polling of etcd

TimescaleDB extension or function that queries etcd directly.

**Pros**:
- Single source query

**Cons**:
- No PostgreSQL-to-etcd extension exists
- Would need custom development
- Violates separation of concerns

**Verdict**: Rejected - Not feasible without significant development

---

## Implementation Impact

### Files to Modify

- `deploy/pi/deploy.sh` - Add sync_to_data_dictionary function

### Files to Create

- `deploy/pi/scripts/sync-data-dictionary.sh` - SQL generation helpers
- `deploy/pi/init-scripts/01-create-data-dictionary.sql` - DDL (from ADR-001)

### Dependencies

- `yq` - YAML processor (already used by deploy.sh)
- `psql` - PostgreSQL client (available in TimescaleDB container)

### Testing

```bash
# Dry run - generate SQL without executing
./deploy.sh sync --dry-run

# Full sync
./deploy.sh sync

# Verify sync
docker exec pi5-timescaledb psql -U postgres -d ndp -c \
    "SELECT * FROM data_dictionary.stream_overview;"
```

---

## Sync Command Specification

```bash
# deploy/pi/deploy.sh

sync_to_data_dictionary() {
    local DRY_RUN="${1:-false}"

    echo "Generating Data Dictionary SQL..."

    # Ensure yq is available
    if ! command -v yq &> /dev/null; then
        echo "ERROR: yq is required but not installed"
        return 1
    fi

    # Generate SQL
    local SQL_FILE="/tmp/data_dictionary_sync_$$.sql"

    {
        echo "-- Data Dictionary Sync"
        echo "-- Generated: $(date -Iseconds)"
        echo "-- Mode: ${DRY_RUN:+DRY RUN}"
        echo ""
        echo "BEGIN;"
        echo ""
        echo "-- Record sync start"
        echo "INSERT INTO data_dictionary.sync_status (sync_type, status)"
        echo "VALUES ('full', 'running');"
        echo ""
        echo "-- Clear existing data"
        echo "TRUNCATE data_dictionary.streams CASCADE;"
        echo ""

        # Process each stream config
        for config_dir in "$CONFIG_DIR"/base/streams/*/; do
            if [ -f "$config_dir/config.yaml" ]; then
                local stream_id=$(basename "$config_dir")
                echo "-- Stream: $stream_id"
                generate_stream_sql "$config_dir/config.yaml"
                echo ""
            fi
        done

        echo "-- Update sync status"
        echo "UPDATE data_dictionary.sync_status"
        echo "SET completed_at = NOW(),"
        echo "    status = 'success',"
        echo "    streams_synced = (SELECT COUNT(*) FROM data_dictionary.streams),"
        echo "    fields_synced = (SELECT COUNT(*) FROM data_dictionary.fields),"
        echo "    entities_synced = (SELECT COUNT(*) FROM data_dictionary.entity_schemas)"
        echo "WHERE status = 'running'"
        echo "  AND completed_at IS NULL;"
        echo ""
        echo "COMMIT;"

    } > "$SQL_FILE"

    if [ "$DRY_RUN" = "true" ]; then
        echo "=== DRY RUN - Generated SQL ==="
        cat "$SQL_FILE"
        rm "$SQL_FILE"
        return 0
    fi

    # Execute sync
    echo "Executing sync..."
    docker exec -i pi5-timescaledb psql -U postgres -d ndp \
        -v ON_ERROR_STOP=1 < "$SQL_FILE"

    local exit_code=$?
    rm "$SQL_FILE"

    if [ $exit_code -eq 0 ]; then
        echo "Data Dictionary sync successful"
    else
        echo "ERROR: Data Dictionary sync failed (exit code: $exit_code)"
        return 1
    fi
}

# Helper: Escape string for SQL
sql_escape() {
    sed "s/'/''/g"
}

# Helper: Generate NULL or quoted string
sql_null() {
    local value="$1"
    if [ "$value" = "null" ] || [ -z "$value" ]; then
        echo "NULL"
    else
        echo "'$value'"
    fi
}
```

---

## Related Decisions

- **ADR-001 (DP-002)**: TimescaleDB Schema Design (target schema)
- **ADR-002 (DP-002)**: Entity Schema Format (source YAML structure)
- **ADR-004 (DP-002)**: DQ Dashboard (consumes synced data)

---

## References

- [PostgreSQL Transaction Management](https://www.postgresql.org/docs/current/tutorial-transactions.html)
- [yq YAML Processor](https://github.com/mikefarah/yq)
- [Existing deploy.sh](../../../deploy/pi/deploy.sh)

---

**Last Updated**: 2025-12-30
**Next Review**: After initial sync implementation and testing
