# JSON Schema Design: Stream Config v1.1

**Feature**: dp-018 JSON Config Foundation
**Date**: 2026-02-01

---

## Overview

This document defines the v1.1 JSON Schema for stream configuration. The v1.1 schema:

1. **Supports enriched fields** with `description` and `device_class`
2. **Deprecates entity_schemas** (still accepted for backward compatibility)
3. **Enables migration path** from v1.0 (YAML with entity_schemas) to v2.0 (JSON without entity_schemas)

---

## Schema Versioning Strategy

| Version | Format | entity_schemas | Enriched fields | Status |
|---------|--------|----------------|-----------------|--------|
| v1.0 | YAML | Required | Not supported | Current (legacy) |
| **v1.1** | **JSON** | **Deprecated (optional)** | **Supported** | **This feature** |
| v2.0 | JSON | Forbidden | Required | Future (dp-021) |

### Migration Path

```
v1.0 (YAML)                    v1.1 (JSON)                    v2.0 (JSON)
+------------------+           +------------------+            +------------------+
| fields: []       |   copy    | fields: []       |   remove   | fields: []       |
| entity_schemas:  | --------> | entity_schemas:  | ---------> |                  |
|   (descriptions) |           |   (deprecated)   |            | (descriptions    |
+------------------+           +------------------+            |  in fields only) |
                                        |                      +------------------+
                                        |
                                        v
                               +------------------+
                               | fields: []       |
                               |   description:   |
                               |   device_class:  |
                               +------------------+
```

---

## v1.1 Schema Structure

### Top-Level Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://neural-data-platform.local/schemas/stream-config.v1.1.schema.json",
  "title": "Stream Configuration v1.1",
  "description": "Configuration for a data stream in the Neural Data Platform",
  "type": "object",
  "required": ["stream_id", "description", "fields", "sources"],
  "additionalProperties": false,
  "properties": {
    "config_version": {
      "type": "string",
      "const": "1.1",
      "description": "Schema version identifier"
    },
    "stream_id": {
      "type": "string",
      "pattern": "^[a-z][a-z0-9-]{2,63}$",
      "description": "Unique stream identifier (kebab-case, 3-64 chars)"
    },
    "description": {
      "type": "string",
      "description": "Human-readable description of the stream"
    },
    "version": {
      "type": "string",
      "default": "1.0.0",
      "description": "Stream schema version (semver)"
    },
    "enabled": {
      "type": "boolean",
      "default": true,
      "description": "Whether stream is active"
    },
    "retention_days": {
      "type": "integer",
      "minimum": 0,
      "default": 0,
      "description": "Days to retain data (0 = infinite)"
    },
    "compression_after_days": {
      "type": "integer",
      "minimum": 0,
      "default": 0,
      "description": "Days before compression"
    },
    "partitioning_strategy": {
      "type": "string",
      "enum": ["daily", "hourly", "monthly"],
      "default": "daily"
    },
    "fields": {
      "type": "array",
      "minItems": 1,
      "items": { "$ref": "#/$defs/field" },
      "description": "Field definitions with optional enriched metadata"
    },
    "sources": {
      "type": "array",
      "minItems": 1,
      "items": { "$ref": "#/$defs/source" }
    },
    "storage": { "$ref": "#/$defs/storage" },
    "entity_schemas": {
      "type": "array",
      "items": { "$ref": "#/$defs/entity_schema" },
      "deprecated": true,
      "description": "DEPRECATED in v1.1. Use fields[].description instead."
    },
    "silver_etl": { "$ref": "#/$defs/silver_etl" }
  },
  "$defs": {
    "field": { },
    "source": { },
    "storage": { },
    "entity_schema": { },
    "silver_etl": { }
  }
}
```

### Field Definition ($defs/field)

The key v1.1 enhancement: fields now support `description` and `device_class`.

```json
{
  "field": {
    "type": "object",
    "required": ["name", "type"],
    "additionalProperties": false,
    "properties": {
      "name": {
        "type": "string",
        "pattern": "^[a-z][a-z0-9_]{0,63}$",
        "description": "Field name (snake_case)"
      },
      "type": {
        "type": "string",
        "enum": ["float", "int", "string", "bool", "json"],
        "description": "Field data type"
      },
      "unit": {
        "type": "string",
        "description": "Physical unit (e.g., 'ug/m3', 'celsius', 'percent')"
      },
      "description": {
        "type": "string",
        "description": "Human-readable description for data dictionary"
      },
      "device_class": {
        "type": "string",
        "description": "Device/sensor class (e.g., 'air_quality', 'temperature', 'weather')"
      },
      "range": {
        "type": "array",
        "items": { "type": "number" },
        "minItems": 2,
        "maxItems": 2,
        "description": "Expected range [min, max] for numeric types"
      },
      "display_precision": {
        "type": "integer",
        "minimum": 0,
        "description": "Decimal places for display"
      },
      "nullable": {
        "type": "boolean",
        "default": true,
        "description": "Whether field can be null"
      },
      "default": {
        "description": "Default value if not provided"
      }
    }
  }
}
```

### Entity Schema (Deprecated in v1.1)

Still accepted for backward compatibility, but should be migrated to fields.

```json
{
  "entity_schema": {
    "type": "object",
    "deprecated": true,
    "description": "DEPRECATED: Use fields[].description instead",
    "required": ["schema_name", "attributes"],
    "properties": {
      "schema_name": { "type": "string" },
      "description": { "type": "string" },
      "device_class": { "type": "string" },
      "attributes": {
        "type": "array",
        "items": {
          "type": "object",
          "required": ["name", "type"],
          "properties": {
            "name": { "type": "string" },
            "type": { "type": "string" },
            "unit": { "type": "string" },
            "description": { "type": "string" },
            "nullable": { "type": "boolean" },
            "range": {
              "type": "array",
              "items": { "type": "number" }
            }
          }
        }
      }
    }
  }
}
```

### Silver ETL Configuration ($defs/silver_etl)

```json
{
  "silver_etl": {
    "type": "object",
    "required": ["enabled", "target_table", "timestamp"],
    "additionalProperties": false,
    "properties": {
      "enabled": { "type": "boolean" },
      "target_table": {
        "type": "string",
        "pattern": "^silver\\.[a-z_]+$",
        "description": "Target table (must start with 'silver.')"
      },
      "target_schema": { "type": "string" },
      "description": { "type": "string" },
      "grain": { "type": "string" },
      "timestamp": { "$ref": "#/$defs/timestamp_mapping" },
      "valid_timestamp": { "$ref": "#/$defs/valid_timestamp_mapping" },
      "pre_transform": { "$ref": "#/$defs/pre_transform" },
      "identity_fields": {
        "type": "array",
        "items": { "$ref": "#/$defs/identity_field" }
      },
      "field_mappings": {
        "type": "array",
        "items": { "$ref": "#/$defs/field_mapping" }
      },
      "dq_rules": {
        "type": "array",
        "items": { "$ref": "#/$defs/dq_rule" }
      },
      "dq_output": { "$ref": "#/$defs/dq_output" },
      "deduplication": { "$ref": "#/$defs/deduplication" },
      "incremental": { "$ref": "#/$defs/incremental" }
    }
  }
}
```

---

## Example: v1.1 Stream Config

### Before (v1.0 YAML with entity_schemas)

```yaml
stream_id: air-quality
description: AirGradient sensor readings

fields:
  pm25:
    type: float
    unit: ug/m3
    # No description here in v1.0

entity_schemas:
  - schema_name: airgradient
    description: AirGradient indoor air quality sensors
    device_class: air_quality
    attributes:
      - name: pm25
        type: float
        unit: ug/m3
        description: Particulate Matter 2.5 micrometers
        range: [0, 1000]
```

### After (v1.1 JSON with enriched fields)

```json
{
  "config_version": "1.1",
  "stream_id": "air-quality",
  "description": "AirGradient sensor readings",
  "fields": [
    {
      "name": "pm25",
      "type": "float",
      "unit": "ug/m3",
      "description": "Particulate Matter 2.5 micrometers",
      "device_class": "air_quality",
      "range": [0, 1000],
      "nullable": false
    },
    {
      "name": "pm10",
      "type": "float",
      "unit": "ug/m3",
      "description": "Particulate Matter 10 micrometers",
      "device_class": "air_quality",
      "range": [0, 2000],
      "nullable": true
    },
    {
      "name": "temperature",
      "type": "float",
      "unit": "celsius",
      "description": "Ambient temperature",
      "device_class": "temperature",
      "range": [-40, 85],
      "nullable": true
    }
  ],
  "sources": [
    {
      "type": "mqtt",
      "enabled": true,
      "ndp_id": "aq_airgradient_1",
      "broker_url": "mosquitto",
      "port": 1883
    }
  ],
  "entity_schemas": [
    {
      "_comment": "DEPRECATED - kept for backward compatibility during migration"
    }
  ],
  "silver_etl": {
    "enabled": true,
    "target_table": "silver.air_quality_observations",
    "description": "Indoor air quality measurements",
    "grain": "One row per sensor reading"
  }
}
```

---

## Field Metadata Resolution

The ConfigLoader resolves field metadata using this priority:

```
1. fields[name].description        (v1.1 enriched - preferred)
2. entity_schemas[*].attributes[name].description  (v1.0 fallback)
```

### Resolution Algorithm

```rust
fn get_field_description(config: &Value, field_name: &str) -> Option<String> {
    // Priority 1: Check fields array (v1.1 enriched)
    if let Some(fields) = config.get("fields").and_then(|v| v.as_array()) {
        for field in fields {
            if field.get("name").and_then(|n| n.as_str()) == Some(field_name) {
                if let Some(desc) = field.get("description").and_then(|d| d.as_str()) {
                    return Some(desc.to_string());
                }
            }
        }
    }

    // Priority 2: Fallback to entity_schemas (v1.0 deprecated)
    if let Some(schemas) = config.get("entity_schemas").and_then(|v| v.as_array()) {
        for schema in schemas {
            if let Some(attrs) = schema.get("attributes").and_then(|a| a.as_array()) {
                for attr in attrs {
                    if attr.get("name").and_then(|n| n.as_str()) == Some(field_name) {
                        if let Some(desc) = attr.get("description").and_then(|d| d.as_str()) {
                            return Some(desc.to_string());
                        }
                    }
                }
            }
        }
    }

    None
}
```

---

## Migration Script Design

The migration script (`scripts/migrate-yaml-to-json.sh`) will:

1. **Convert YAML to JSON** using yq/jq
2. **Add config_version** field set to "1.1"
3. **Copy descriptions from entity_schemas to fields**
4. **Keep entity_schemas** (for backward compatibility during transition)
5. **Validate against JSON Schema**

### Script Outline

```bash
#!/bin/bash
# migrate-yaml-to-json.sh
# Converts stream config.yaml files to config.json (v1.1 format)

set -euo pipefail

CONFIG_DIR="${1:-config/base/streams}"
SCHEMA_FILE="schemas/stream-config.v1.1.schema.json"

for yaml_file in "$CONFIG_DIR"/*/config.yaml; do
    stream_dir=$(dirname "$yaml_file")
    stream_id=$(basename "$stream_dir")
    json_file="$stream_dir/config.json"

    echo "Migrating $stream_id..."

    # Step 1: Convert YAML to JSON
    yq -o=json "$yaml_file" > "$json_file.tmp"

    # Step 2: Add config_version
    jq '. + {"config_version": "1.1"}' "$json_file.tmp" > "$json_file.tmp2"
    mv "$json_file.tmp2" "$json_file.tmp"

    # Step 3: Enrich fields with descriptions from entity_schemas
    jq '
      # Build lookup from entity_schemas
      (.entity_schemas // []) as $schemas |
      ($schemas | map(.attributes // []) | flatten | map({key: .name, value: .}) | from_entries) as $attr_lookup |

      # Enrich fields
      .fields = (
        if .fields | type == "array" then
          .fields
        else
          # Convert object-style fields to array
          [.fields | to_entries[] | {name: .key} + .value]
        end
        | map(
            . + (
              if .description then {} else
                {description: ($attr_lookup[.name].description // null)}
              end
            ) + (
              if .device_class then {} else
                {device_class: (
                  $schemas | map(select(.attributes | any(.name == .name))) | .[0].device_class // null
                )}
              end
            )
          )
      )
    ' "$json_file.tmp" > "$json_file"

    rm -f "$json_file.tmp"

    # Step 4: Validate against schema
    if command -v ajv &> /dev/null; then
        ajv validate -s "$SCHEMA_FILE" -d "$json_file" --strict=false
    fi

    echo "  Created $json_file"
done

echo "Migration complete."
```

---

## Data Dictionary Integration

The data dictionary sync reads field descriptions via ConfigLoader:

```rust
async fn sync_data_dictionary(
    config_loader: &dyn ConfigLoader,
    db_pool: &PgPool,
) -> Result<(), Error> {
    let streams = config_loader.list_streams().await?;

    for stream_id in streams {
        let field_descs = config_loader.get_field_descriptions(&stream_id).await?;

        for (field_name, metadata) in field_descs.fields {
            sqlx::query!(
                r#"
                INSERT INTO silver.data_dictionary (stream_id, column_name, description, unit, device_class)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (stream_id, column_name) DO UPDATE SET
                    description = EXCLUDED.description,
                    unit = EXCLUDED.unit,
                    device_class = EXCLUDED.device_class
                "#,
                stream_id,
                field_name,
                metadata.description,
                metadata.unit,
                metadata.device_class
            )
            .execute(db_pool)
            .await?;
        }
    }

    Ok(())
}
```

---

## Validation Rules

### Semantic Validation (beyond JSON Schema)

1. **Field name consistency**: silver_etl.field_mappings[].target_column should match fields[].name
2. **DQ rule field references**: DQ rules must reference existing fields
3. **Source parameter validation**: MQTT sources must have broker_url, HTTP sources must have url
4. **No duplicate field names**: fields[].name must be unique

### Schema Validation Command

```bash
# Using ajv-cli
npm install -g ajv-cli
ajv validate -s schemas/stream-config.v1.1.schema.json -d config/base/streams/*/config.json

# Using check-jsonschema (Python)
pip install check-jsonschema
check-jsonschema --schemafile schemas/stream-config.v1.1.schema.json config/base/streams/*/config.json
```

---

## File Locations

| Artifact | Location |
|----------|----------|
| JSON Schema (v1.1) | `schemas/stream-config.v1.1.schema.json` |
| Dimension Schema | `schemas/dimension-config.schema.json` |
| Manifest Schema | `schemas/manifest.schema.json` |
| Migration Script | `scripts/migrate-yaml-to-json.sh` |
| Validation Script | `scripts/validate-configs.sh` |

---

## References

- [ADR-018-001: ConfigLoader Design](./ADR-018-001-config-loader-design.md)
- [ADR-016-001: Config Source of Truth](../../dp-016/architecture/ADR-016-001-config-source-of-truth.md)
- [JSON Schema Specification](https://json-schema.org/specification)
