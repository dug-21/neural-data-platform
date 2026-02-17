# dp-023: deploy-sh Pseudocode

## Component: deploy/pi/ddl-generator.sh

### Verification: map_type() handles text/jsonb

**File**: `deploy/pi/ddl-generator.sh`, line 29-70

The existing `map_type()` function already handles:
- `string|text` -> `TEXT` (line 47-48)
- `varchar` -> `VARCHAR` (line 50-51)
- `json|jsonb` -> `JSONB` (line 59-60)
- `"text[]"|"TEXT[]"` -> `TEXT[]` (line 62-63)

**No changes needed** to `map_type()`.

### Verification: generate_silver_ddl() does not assume numeric types

Need to verify that `generate_silver_ddl()` (line 598) does not add:
- `DEFAULT 0` or other numeric defaults to data columns
- `NOT NULL` constraints to data columns (text fields should be nullable)
- Numeric-specific check constraints

**Expected**: Data columns use the type from `map_type()` directly with no default or constraint unless explicitly configured. Nullable is determined from config.

## Component: deploy/pi/deploy.sh

### Change: Add Gold text view generation to Phase 6

**File**: `deploy/pi/deploy.sh`
**Location**: Phase 6 (Gold DDL generation section)

```bash
# Pseudocode for Gold text view integration in deploy.sh Phase 6

handle_gold_text_view() {
    local domain_id="$1"
    local mode="${2:-full}"

    log "Generating Gold text view for domain: $domain_id"

    # Use ndp CLI to generate text view DDL
    local text_view_sql
    text_view_sql=$("$ndp_tool" gold text-view --domain "$domain_id" --action "$mode" 2>&1)

    if [ $? -ne 0 ]; then
        warn "Gold text view generation failed for $domain_id: $text_view_sql"
        return 1
    fi

    # Skip if no text fields found (generator returns comment-only)
    if echo "$text_view_sql" | grep -q "^-- No text fields"; then
        log "No text fields in domain $domain_id, skipping text view"
        return 0
    fi

    # Execute the DDL
    execute_sql "$text_view_sql"
    log "Gold text view created for domain: $domain_id"
}

# In Phase 6, after continuous aggregates and aligned views:
# handle_gold_text_view "$domain_id" "$mode"
```

### Verification: Data dictionary sync handles text/jsonb

**File**: `deploy/pi/deploy.sh`, line 669-681

The type mapping in `_sync_to_data_dictionary_bash()` already includes:
- `text` -> `TEXT` (line 676)
- `jsonb` -> `JSONB` (line 679)
- Default `*` -> `TEXT` (line 680)

**No changes needed** for dictionary sync.

The sync correctly:
1. Reads `silver_etl.field_mappings` from config
2. Maps `type` to PostgreSQL type via case statement
3. UPSERTs into `data_dictionary.silver_columns`
4. Creates `silver_lineage` entries mapping source_path -> target_column
5. Handles nullable and description from config

## Component: deploy/pi/ddl-generator.sh (generate_silver_ddl)

### Verification Points

```bash
# In generate_silver_ddl(), verify these behaviors for text/jsonb:

# 1. Column definition uses map_type() result directly
#    Expected: "short_forecast TEXT" not "short_forecast DOUBLE PRECISION"
pg_type=$(map_type "$col_type")   # line 291 or 502
# $pg_type should be "TEXT" for type="text", "JSONB" for type="jsonb"

# 2. No DEFAULT clause added for data columns
#    Expected: "short_forecast TEXT" not "short_forecast TEXT DEFAULT ''"

# 3. Nullable from config respected
#    Expected: No NOT NULL for nullable=true text fields

# 4. No numeric-specific constraints
#    Expected: No CHECK constraints assuming numeric values
```

## Summary of Changes

| File | Action | Description |
|------|--------|-------------|
| `deploy/pi/ddl-generator.sh` | Verify only | map_type() and generate_silver_ddl() already handle text/jsonb |
| `deploy/pi/deploy.sh` | Modify | Add `handle_gold_text_view()` to Phase 6 |
| `deploy/pi/deploy.sh` | Verify only | Dictionary sync already maps text/jsonb types |
