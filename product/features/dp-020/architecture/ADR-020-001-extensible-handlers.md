# ADR-020-001: Extensible Handler Architecture

**Status**: Proposed
**Date**: 2026-02-02
**Decision Makers**: NDP Architecture Team
**Feature**: dp-020 Declarative Deploy

---

## Context

dp-020 introduces a manifest-driven deployment where each declaration type (stream, silver-table, migration, dimensions, dictionary) requires a handler to execute the appropriate actions. The handler architecture must support:

1. **Extensibility** - Adding new declaration types without major refactoring
2. **Testability** - Each handler testable in isolation
3. **Simplicity** - Easy to understand, debug, and maintain
4. **Consistency** - All handlers follow the same patterns

### Current deploy.sh Patterns

The existing `deploy.sh` uses shell functions for each operation:

```bash
sync_config() { ... }
sync_to_data_dictionary() { ... }
sync_dimensions() { ... }
```

This pattern works well and is familiar to operators managing Pi deployments.

---

## Decision

**Use shell functions as handlers within deploy.sh, with a dispatch mechanism for manifest-driven execution.**

### Architecture

```
deploy.sh
    |
    +-- apply()                    # Entry point for manifest deployment
    |       |
    |       +-- parse_manifest()   # Load and parse manifest.json
    |       |
    |       +-- dispatch()         # Route declarations to handlers
    |               |
    |               +-- handle_container_build()   # Shell function (Phase 2)
    |               +-- handle_stream()            # Shell function
    |               +-- handle_silver_table()      # Shell function (calls DDL generator)
    |               +-- handle_migration()         # Shell function
    |               +-- handle_dimensions()        # Shell function
    |               +-- handle_dictionary()        # Shell function
    |               +-- handle_container_restart() # Shell function (Phase 8)
    |
    +-- (existing functions preserved)
```

### Handler Contract

Each handler is a shell function with a consistent interface:

```bash
# Handler function signature:
#   handle_<type>() {
#       local declaration_json="$1"
#       # ... implementation ...
#       return 0  # success
#       return 1  # failure (stops deployment)
#   }

# Example:
handle_stream() {
    local json="$1"

    # Extract fields from JSON
    local stream_id=$(echo "$json" | jq -r '.id')
    local action=$(echo "$json" | jq -r '.action // "update"')
    local reload=$(echo "$json" | jq -r '.reload // "none"')

    log "Processing stream: $stream_id (action=$action, reload=$reload)"

    # Execute actions
    case "$action" in
        create|update)
            sync_stream_to_etcd "$stream_id"
            ;;
        delete)
            delete_stream_from_etcd "$stream_id"
            ;;
        *)
            error "Unknown action: $action"
            return 1
            ;;
    esac

    # Track reload requirement for later phase
    if [ "$reload" != "none" ]; then
        RELOAD_REQUIRED["$stream_id"]="$reload"
    fi

    return 0
}
```

### Dispatch Mechanism

```bash
dispatch_declaration() {
    local declaration="$1"
    local type=$(echo "$declaration" | jq -r '.type')

    case "$type" in
        container-build)
            handle_container_build "$declaration"
            ;;
        container-restart)
            handle_container_restart "$declaration"
            ;;
        stream)
            handle_stream "$declaration"
            ;;
        silver-table)
            handle_silver_table "$declaration"
            ;;
        migration)
            handle_migration "$declaration"
            ;;
        dimensions)
            handle_dimensions "$declaration"
            ;;
        dictionary)
            handle_dictionary "$declaration"
            ;;
        *)
            warn "Unknown declaration type: $type (skipping)"
            return 0  # Don't fail on unknown types for forward compatibility
            ;;
    esac
}
```

### Handler Implementations

#### Container Build Handler

```bash
handle_container_build() {
    local json="$1"
    local target=$(echo "$json" | jq -r '.target')
    local no_cache=$(echo "$json" | jq -r '.no_cache // "false"')

    local service=$(map_container_target "$target")

    if [ -z "$service" ]; then
        error "Unknown container target: $target"
        return 1
    fi

    log "Building container: $service (no_cache=$no_cache)"

    if [ "$no_cache" = "true" ]; then
        if ! dc build --no-cache "$service"; then
            error "Container build failed: $service"
            return 1
        fi
    else
        if ! dc build "$service"; then
            error "Container build failed: $service"
            return 1
        fi
    fi

    log "Container built successfully: $service"
    return 0
}

# Map declaration targets to docker-compose service names
map_container_target() {
    local target="$1"
    case "$target" in
        air-quality|air-quality-app)
            echo "air-quality-app"
            ;;
        etl|silver-etl)
            echo "silver-etl"
            ;;
        *)
            echo "$target"  # Pass through unknown targets
            ;;
    esac
}
```

#### Container Restart Handler

```bash
handle_container_restart() {
    local json="$1"
    local target=$(echo "$json" | jq -r '.target')
    local timeout=$(echo "$json" | jq -r '.health_timeout // 60')

    local service=$(map_container_target "$target")

    if [ -z "$service" ]; then
        error "Unknown container target: $target"
        return 1
    fi

    log "Restarting container: $service"

    # Bring up the service (recreates if config changed)
    if ! dc up -d "$service"; then
        error "Container restart failed: $service"
        return 1
    fi

    # Wait for health check to pass
    if ! wait_for_health "$service" "$timeout"; then
        error "Container health check failed: $service (timeout=${timeout}s)"
        return 1
    fi

    log "Container restarted and healthy: $service"
    return 0
}

# Wait for container health check to pass
wait_for_health() {
    local service="$1"
    local timeout="${2:-60}"
    local elapsed=0

    log "Waiting for $service to become healthy (timeout=${timeout}s)..."

    while [ $elapsed -lt $timeout ]; do
        local health=$(docker inspect --format='{{.State.Health.Status}}' "$(dc ps -q $service)" 2>/dev/null)

        case "$health" in
            healthy)
                return 0
                ;;
            unhealthy)
                error "Container reported unhealthy: $service"
                return 1
                ;;
            *)
                # starting or no health check defined
                sleep 2
                elapsed=$((elapsed + 2))
                ;;
        esac
    done

    return 1  # Timeout
}
```

#### Stream Handler

```bash
handle_stream() {
    local json="$1"
    local stream_id=$(echo "$json" | jq -r '.id')
    local action=$(echo "$json" | jq -r '.action // "update"')

    local config_file="$REPO_ROOT/config/base/streams/$stream_id/config.json"

    if [ ! -f "$config_file" ]; then
        error "Stream config not found: $config_file"
        return 1
    fi

    # Validate before sync (uses dp-019 validator)
    if ! validate_stream_config "$config_file"; then
        error "Stream config validation failed: $stream_id"
        return 1
    fi

    # Sync to etcd
    log "Syncing stream $stream_id to etcd..."
    dcx etcd etcdctl put "/streams/$stream_id/config" "$(cat $config_file)"

    return 0
}
```

#### Silver Table Handler

```bash
handle_silver_table() {
    local json="$1"
    local stream_id=$(echo "$json" | jq -r '.stream_id')
    local action=$(echo "$json" | jq -r '.action // "sync"')

    local config_file="$REPO_ROOT/config/base/streams/$stream_id/config.json"

    if [ ! -f "$config_file" ]; then
        error "Stream config not found: $config_file"
        return 1
    fi

    case "$action" in
        sync|create)
            # Generate DDL from config
            local ddl_file="/tmp/ddl_${stream_id}_$$.sql"
            if ! generate_ddl "$config_file" > "$ddl_file"; then
                error "DDL generation failed for stream: $stream_id"
                return 1
            fi

            # Apply DDL to TimescaleDB
            log "Applying DDL for $stream_id..."
            if ! dcx timescaledb psql -U postgres -d ndp < "$ddl_file"; then
                error "DDL execution failed for stream: $stream_id"
                rm -f "$ddl_file"
                return 1
            fi

            rm -f "$ddl_file"
            log "Silver table synced for stream: $stream_id"
            ;;
        validate-only)
            # Just check table exists
            local target_table=$(jq -r '.silver_etl.target_table // empty' "$config_file")
            if ! table_exists "$target_table"; then
                error "Silver table does not exist: $target_table"
                return 1
            fi
            ;;
        *)
            error "Unknown silver-table action: $action"
            return 1
            ;;
    esac

    return 0
}
```

#### Migration Handler

```bash
handle_migration() {
    local json="$1"
    local file=$(echo "$json" | jq -r '.file')

    local migration_path="$REPO_ROOT/deploy/migrations/$file"

    if [ ! -f "$migration_path" ]; then
        error "Migration file not found: $migration_path"
        return 1
    fi

    # Check if already applied (using migration tracking table)
    local migration_name=$(basename "$file")
    if migration_already_applied "$migration_name"; then
        log "Migration already applied: $migration_name (skipping)"
        return 0
    fi

    # Apply migration
    log "Applying migration: $migration_name"
    if ! dcx timescaledb psql -U postgres -d ndp < "$migration_path"; then
        error "Migration failed: $migration_name"
        return 1
    fi

    # Record migration as applied
    record_migration_applied "$migration_name"

    return 0
}
```

#### Dimensions Handler

```bash
handle_dimensions() {
    local json="$1"
    local action=$(echo "$json" | jq -r '.action // "sync"')
    local id=$(echo "$json" | jq -r '.id // empty')

    case "$action" in
        sync)
            if [ -n "$id" ] && [ "$id" != "null" ]; then
                # Sync specific dimension
                sync_single_dimension "$id"
            else
                # Sync all dimensions
                sync_dimensions
            fi
            ;;
        *)
            error "Unknown dimensions action: $action"
            return 1
            ;;
    esac

    return 0
}
```

#### Dictionary Handler

```bash
handle_dictionary() {
    local json="$1"
    local action=$(echo "$json" | jq -r '.action // "sync"')

    case "$action" in
        sync)
            sync_to_data_dictionary
            ;;
        *)
            error "Unknown dictionary action: $action"
            return 1
            ;;
    esac

    return 0
}
```

---

## Extensibility Pattern

To add a new declaration type:

### 1. Define the declaration schema

Add to `schemas/manifest.schema.json`:

```json
{
  "type": "object",
  "properties": {
    "type": { "const": "new-type" },
    "param1": { "type": "string" },
    "param2": { "type": "integer" }
  },
  "required": ["type", "param1"]
}
```

### 2. Implement the handler

Add to `deploy.sh`:

```bash
handle_new_type() {
    local json="$1"
    local param1=$(echo "$json" | jq -r '.param1')
    local param2=$(echo "$json" | jq -r '.param2 // 0')

    log "Processing new-type: $param1"

    # Implementation
    # ...

    return 0
}
```

### 3. Register in dispatch

Add case in `dispatch_declaration()`:

```bash
case "$type" in
    # ... existing cases ...
    new-type)
        handle_new_type "$declaration"
        ;;
    # ...
esac
```

---

## Consequences

### Positive

1. **Familiar patterns** - Operators already know shell functions in deploy.sh
2. **Simple to extend** - Add a function, add a case, done
3. **Easy debugging** - Shell scripts with logging, can run handlers manually
4. **No new binaries** - Works with existing Pi deployment infrastructure
5. **Testable** - Source deploy.sh, call handler with test JSON
6. **Low overhead** - No compilation step, no new dependencies

### Negative

1. **Shell limitations** - Complex JSON parsing requires jq
2. **No type safety** - Must validate manually
3. **Error handling** - Shell error handling is verbose
4. **Reuse across projects** - Shell functions not easily portable

### Neutral

1. **DDL generation** - Could be shell templates or call external tool (see ADR-020-002)
2. **Validation** - Depends on dp-019 validator binary

---

## Alternatives Considered

### Alternative 1: Rust Binary per Handler

Each handler is a separate Rust binary (e.g., `handle-stream`, `handle-silver-table`).

**Rejected because**:
- Many small binaries increase build time
- Cross-compilation for Pi adds complexity
- deploy.sh would still need dispatch logic
- Overkill for shell-friendly operations (etcdctl, psql)

### Alternative 2: Single Rust Orchestrator Binary

One Rust binary (`ndp-deploy`) handles all orchestration and handlers.

**Rejected because**:
- Current deploy.sh works well for Pi operations
- Rust binary adds compilation step to deployment changes
- Would duplicate existing shell logic (docker compose, etc.)
- Higher barrier for operators to modify

### Alternative 3: Plugin Architecture

Handlers as loadable plugins (shared libraries or scripts in a directory).

**Rejected because**:
- Adds complexity without clear benefit for 5 handler types
- Shell function pattern is simpler and adequate
- Plugin discovery and loading adds failure modes
- YAGNI - we know the handler types now

### Alternative 4: Hybrid - Shell Dispatch, Rust Complex Handlers

Shell for dispatch and simple handlers, Rust for complex ones (DDL generation).

**Selected variant**: This is partially adopted. The `generate_ddl` function may call an external tool (see ADR-020-002), but the handler dispatch remains shell-based.

---

## Implementation Notes

### Testing Strategy

```bash
# Unit test a handler
source deploy.sh  # Load functions

# Mock infrastructure
dcx() { echo "MOCK: $@"; }

# Test with sample JSON
test_json='{"type": "stream", "id": "test-stream", "action": "update"}'
handle_stream "$test_json"
echo "Exit code: $?"
```

### Integration with deploy.sh

The new `apply` command integrates with existing commands:

```bash
case "${1:-deploy}" in
    apply)
        check_prereqs
        apply
        ;;
    # ... existing cases ...
esac
```

### Dependencies

| Dependency | Required | Purpose |
|------------|----------|---------|
| `jq` | Yes | JSON parsing |
| `docker` | Yes | Container operations (build, inspect, health checks) |
| `docker-compose` / `dc` | Yes | Container orchestration (build, up -d) |
| `etcdctl` | Yes (in container) | etcd operations |
| `psql` | Yes (in container) | TimescaleDB operations |

---

## Related Decisions

- **ADR-020-002**: DDL Generation Strategy (how silver-table handler generates DDL)
- **ADR-020-003**: Manifest Schema Versioning (how manifest evolves)
- **ADR-016-002**: Declarative Deploy (parent decision)

---

## References

- `/workspaces/neural-data-platform/deploy/pi/deploy.sh` - Current implementation
- `/workspaces/neural-data-platform/product/features/dp-020/SCOPE.md` - Feature requirements

---

*ADR created: 2026-02-02*
*Feature: dp-020 Declarative Deploy*
