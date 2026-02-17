# dp-023: ndp-validate Pseudocode

## Component: tools/ndp-validate/

### Change: Verify schema accepts text/jsonb field_mapping types

**File**: `config/schemas/stream.schema.json` (if external) or embedded in `tools/ndp-validate/src/schema.rs`

```
# Pseudocode: Verify the field_mappings type enum

# In the JSON schema for stream config, find the enum for field_mappings[].type:
# Expected enum values should include:
#   "double_precision", "real", "integer", "bigint", "smallint",
#   "text", "varchar", "jsonb", "boolean", "timestamptz", "text[]"

# If "text", "jsonb" etc. are missing from the enum, add them.
# If validation uses pattern matching instead of enum, verify patterns accept these types.
```

### Verification: DQ rule validation skips range_check for non-numeric

**File**: `tools/ndp-validate/src/semantic/dq_rules.rs`

```
# The validate_range_check() function (line 320) validates:
#   - min/max are numbers
#   - min < max
# It does NOT check the field type -- it only validates the rule structure.

# This means:
# - A text field WITH a range_check rule: validates the rule structure (min/max),
#   the rule itself is valid even though it's semantically wrong for text.
# - A text field WITHOUT range_check: no issue.

# For dp-023, NWS text fields have no DQ rules, so no validation issue arises.
# No changes needed to dq_rules.rs.
```

### Verification: ndp validate CLI behavior

```bash
# Test command:
ndp validate --config config/base/streams/nws-forecast-hourly/config.json

# Expected output: PASS (0 errors, 0 warnings)
# If validation fails on text/jsonb types, investigate schema.rs embedded schema.
```

## Summary of Changes

| File | Action | Description |
|------|--------|-------------|
| `config/schemas/stream.schema.json` or `tools/ndp-validate/src/schema.rs` | Verify/modify | Ensure type enum includes text, jsonb, varchar, boolean, text[] |
| `tools/ndp-validate/src/semantic/dq_rules.rs` | Verify only | range_check validation is type-independent |
