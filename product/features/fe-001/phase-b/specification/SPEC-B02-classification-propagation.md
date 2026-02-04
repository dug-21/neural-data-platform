# SPEC-B02: Classification Propagation

> **Feature ID:** v11-002
> **Priority:** High
> **Status:** Specification
> **Dependencies:** v11-001 (Stream Type Classification)
> **Blocks:** Phase C (Alignment Interpreter), V1.2 (Correlation Analysis)

---

## User Story

**As a** correlation analysis engine (V1.2),
**I want** stream type classifications to be queryable via the data dictionary and MCP tools,
**So that** I can determine which streams are potential causes vs effects for pattern detection.

---

## Goal

Propagate the `stream_type` classification from config to:
1. Data dictionary tables (queryable metadata)
2. Silver table metadata (lineage tracking)
3. Gold table generation (feature selection)
4. MCP tools (runtime queryability)

---

## Background: Classification Flow

```
┌─────────────────────────────────────────────────────────────────────────┐
│                   CLASSIFICATION PROPAGATION FLOW                        │
│                                                                          │
│  CONFIG                    DATA DICTIONARY              GOLD LAYER       │
│  ──────                    ───────────────              ──────────       │
│                                                                          │
│  stream config             stream_classification        aligned view     │
│  ┌────────────┐           ┌──────────────────┐        join strategy     │
│  │stream_type:│ ─────────►│ stream_id        │ ──────►depends on type   │
│  │observation │  sync     │ stream_type      │                          │
│  └────────────┘           │ correlation_role │        NULL handling     │
│                           └──────────────────┘        depends on type   │
│                                    │                                     │
│                                    ▼                                     │
│                           MCP tool query                                │
│                           ┌──────────────────┐                          │
│                           │ list_streams     │                          │
│                           │  --by-type       │                          │
│                           │  --by-role       │                          │
│                           └──────────────────┘                          │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Functional Requirements

### FR-B02-001: Stream Classification Table

The data dictionary SHALL include a `stream_classification` table:

```sql
CREATE TABLE data_dictionary.stream_classification (
    stream_id TEXT PRIMARY KEY
        REFERENCES data_dictionary.streams(stream_id),
    stream_type TEXT NOT NULL
        CHECK (stream_type IN ('observation', 'state_event', 'forecast', 'dimension')),
    correlation_role TEXT NOT NULL
        CHECK (correlation_role IN ('effect', 'cause', 'context', 'metadata')),
    null_handling TEXT NOT NULL
        CHECK (null_handling IN ('preserve', 'carry_forward')),
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_stream_classification_type ON data_dictionary.stream_classification(stream_type);
CREATE INDEX idx_stream_classification_role ON data_dictionary.stream_classification(correlation_role);
```

### FR-B02-002: Automatic Role Derivation

The correlation_role SHALL be automatically derived from stream_type:

| stream_type | correlation_role | null_handling |
|-------------|------------------|---------------|
| observation | effect | preserve |
| state_event | cause | carry_forward |
| forecast | context | preserve |
| dimension | metadata | carry_forward |

### FR-B02-003: Sync to Data Dictionary

The `sync_to_data_dictionary()` function in deploy.sh SHALL be extended:

```bash
# In sync_to_data_dictionary()
local stream_type=$(yaml_get "$config_file" "stream_type" "")

if [ -n "$stream_type" ]; then
    local correlation_role=$(derive_correlation_role "$stream_type")
    local null_handling=$(derive_null_handling "$stream_type")

    echo "INSERT INTO data_dictionary.stream_classification
          (stream_id, stream_type, correlation_role, null_handling)
          VALUES ('$stream_id', '$stream_type', '$correlation_role', '$null_handling')
          ON CONFLICT (stream_id) DO UPDATE SET
              stream_type = EXCLUDED.stream_type,
              correlation_role = EXCLUDED.correlation_role,
              null_handling = EXCLUDED.null_handling,
              updated_at = NOW();"
fi
```

### FR-B02-004: MCP Tool Enhancement

The existing MCP data dictionary tools SHALL support classification queries:

```bash
# List streams by type
mcp query "SELECT * FROM data_dictionary.stream_classification WHERE stream_type = 'observation'"

# List streams by correlation role
mcp query "SELECT * FROM data_dictionary.stream_classification WHERE correlation_role = 'effect'"
```

### FR-B02-005: Gold Layer Metadata

When generating Gold DDL, the classification SHALL be included in gold_tables:

```sql
INSERT INTO data_dictionary.gold_tables
    (table_name, object_type, source_silver_table, source_stream_type)
VALUES
    ('gold.air_quality_hourly', 'continuous_aggregate',
     'silver.air_quality_observations', 'observation');
```

---

## Non-Functional Requirements

### NFR-B02-001: Sync Idempotency

The classification sync SHALL be idempotent (safe to run multiple times).

### NFR-B02-002: Query Performance

Classification lookup queries SHALL complete in < 10ms.

### NFR-B02-003: Consistency

The classification in data_dictionary SHALL always match the stream config. Any discrepancy indicates a sync failure.

---

## Acceptance Criteria

### AC-B02-001: Air-Quality Classification Synced

```gherkin
Scenario: Air-quality classification is in data dictionary
  Given the air-quality stream config has stream_type = "observation"
  When deploy.sh apply is executed with air-quality
  Then data_dictionary.stream_classification SHALL contain:
    | stream_id   | stream_type  | correlation_role | null_handling  |
    | air-quality | observation  | effect           | preserve       |
```

### AC-B02-002: Role Correctly Derived

```gherkin
Scenario: Correlation role is derived from stream type
  Given a stream config with stream_type = "state_event"
  When the classification is synced to data dictionary
  Then correlation_role SHALL equal "cause"
  And null_handling SHALL equal "carry_forward"
```

### AC-B02-003: MCP Query Returns Classification

```gherkin
Scenario: MCP tool can query stream classifications
  Given the data dictionary is populated with stream classifications
  When I query for streams with stream_type = "observation"
  Then air-quality SHALL be in the result set
  And the correlation_role SHALL be "effect"
```

### AC-B02-004: Gold Table Includes Stream Type

```gherkin
Scenario: Gold table metadata includes source stream type
  Given air-quality Gold layer is deployed
  When I query data_dictionary.gold_tables
  Then gold.air_quality_hourly SHALL have source_stream_type = "observation"
```

### AC-B02-005: Sync Is Idempotent

```gherkin
Scenario: Running sync twice produces same result
  Given air-quality classification is already synced
  When deploy.sh apply is executed again
  Then data_dictionary.stream_classification SHALL have exactly one row for air-quality
  And updated_at SHALL be updated
```

---

## Data Dictionary DDL

### Table Creation

Execute during init-timescaledb.sql:

```sql
-- Stream classification metadata for correlation analysis
CREATE TABLE IF NOT EXISTS data_dictionary.stream_classification (
    stream_id TEXT PRIMARY KEY
        REFERENCES data_dictionary.streams(stream_id) ON DELETE CASCADE,
    stream_type TEXT NOT NULL
        CHECK (stream_type IN ('observation', 'state_event', 'forecast', 'dimension')),
    correlation_role TEXT NOT NULL
        CHECK (correlation_role IN ('effect', 'cause', 'context', 'metadata')),
    null_handling TEXT NOT NULL
        CHECK (null_handling IN ('preserve', 'carry_forward')),
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_stream_classification_type
    ON data_dictionary.stream_classification(stream_type);

CREATE INDEX IF NOT EXISTS idx_stream_classification_role
    ON data_dictionary.stream_classification(correlation_role);

COMMENT ON TABLE data_dictionary.stream_classification IS
    'Stream type classifications for Gold layer correlation analysis (V1.1+)';
```

### Extend gold_tables

```sql
-- Add source_stream_type to gold_tables
ALTER TABLE data_dictionary.gold_tables
ADD COLUMN IF NOT EXISTS source_stream_type TEXT
    REFERENCES data_dictionary.stream_classification(stream_type);
```

---

## Deploy.sh Extension

### Helper Functions

```bash
# Derive correlation role from stream type
derive_correlation_role() {
    local stream_type="$1"
    case "$stream_type" in
        observation) echo "effect" ;;
        state_event) echo "cause" ;;
        forecast)    echo "context" ;;
        dimension)   echo "metadata" ;;
        *)           echo "unknown" ;;
    esac
}

# Derive NULL handling from stream type
derive_null_handling() {
    local stream_type="$1"
    case "$stream_type" in
        observation) echo "preserve" ;;
        state_event) echo "carry_forward" ;;
        forecast)    echo "preserve" ;;
        dimension)   echo "carry_forward" ;;
        *)           echo "preserve" ;;
    esac
}
```

### Sync Function Extension

```bash
# Add to sync_to_data_dictionary() after streams INSERT
sync_stream_classification() {
    local config_file="$1"
    local stream_id="$2"

    local stream_type=$(yaml_get "$config_file" "stream_type" "")

    if [ -z "$stream_type" ]; then
        log "  WARNING: No stream_type for $stream_id (V1.0 config)"
        return 0
    fi

    local correlation_role=$(derive_correlation_role "$stream_type")
    local null_handling=$(derive_null_handling "$stream_type")

    log "  Classification: $stream_id -> $stream_type ($correlation_role)"

    cat >> "$SQL_FILE" << EOF
INSERT INTO data_dictionary.stream_classification
    (stream_id, stream_type, correlation_role, null_handling)
VALUES
    ('$stream_id', '$stream_type', '$correlation_role', '$null_handling')
ON CONFLICT (stream_id) DO UPDATE SET
    stream_type = EXCLUDED.stream_type,
    correlation_role = EXCLUDED.correlation_role,
    null_handling = EXCLUDED.null_handling,
    updated_at = NOW();
EOF
}
```

---

## Integration Test Requirements

### Test: Classification Sync

```bash
# Deploy air-quality
deploy.sh apply .deploy/test/phase-b-air-quality.manifest.json

# Verify classification exists
dcx timescaledb psql -U postgres -d ndp -c "
SELECT stream_id, stream_type, correlation_role, null_handling
FROM data_dictionary.stream_classification
WHERE stream_id = 'air-quality';
"
# Expected:
# stream_id   | stream_type | correlation_role | null_handling
# ------------+-------------+------------------+---------------
# air-quality | observation | effect           | preserve
```

### Test: Idempotency

```bash
# Deploy twice
deploy.sh apply .deploy/test/phase-b-air-quality.manifest.json
deploy.sh apply .deploy/test/phase-b-air-quality.manifest.json

# Verify only one row
dcx timescaledb psql -U postgres -d ndp -c "
SELECT COUNT(*) FROM data_dictionary.stream_classification
WHERE stream_id = 'air-quality';
"
# Expected: 1
```

### Test: Gold Table Metadata

```bash
# Verify Gold table includes stream type
dcx timescaledb psql -U postgres -d ndp -c "
SELECT table_name, source_stream_type
FROM data_dictionary.gold_tables
WHERE table_name = 'gold.air_quality_hourly';
"
# Expected: source_stream_type = 'observation'
```

---

## London TDD Interfaces

### Trait: ClassificationSyncer

```rust
/// Syncs stream classification to data dictionary
trait ClassificationSyncer {
    /// Sync classification for a single stream
    fn sync_classification(&self, stream: &StreamConfig) -> Result<(), SyncError>;

    /// Get classification from data dictionary
    fn get_classification(&self, stream_id: &str) -> Result<StreamClassification, SyncError>;

    /// List streams by type
    fn list_by_type(&self, stream_type: StreamType) -> Result<Vec<StreamClassification>, SyncError>;

    /// List streams by correlation role
    fn list_by_role(&self, role: CorrelationRole) -> Result<Vec<StreamClassification>, SyncError>;
}
```

### Struct: StreamClassification

```rust
#[derive(Debug, Clone)]
pub struct StreamClassification {
    pub stream_id: String,
    pub stream_type: StreamType,
    pub correlation_role: CorrelationRole,
    pub null_handling: NullHandling,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl StreamClassification {
    /// Create classification from stream config
    pub fn from_config(config: &StreamConfig) -> Self {
        let correlation_role = match config.stream_type {
            StreamType::Observation => CorrelationRole::Effect,
            StreamType::StateEvent => CorrelationRole::Cause,
            StreamType::Forecast => CorrelationRole::Context,
            StreamType::Dimension => CorrelationRole::Metadata,
        };

        let null_handling = match config.stream_type {
            StreamType::Observation | StreamType::Forecast => NullHandling::Preserve,
            StreamType::StateEvent | StreamType::Dimension => NullHandling::CarryForward,
        };

        Self {
            stream_id: config.stream_id.clone(),
            stream_type: config.stream_type,
            correlation_role,
            null_handling,
            description: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}
```

### Mock: ClassificationSyncer

```rust
mock! {
    pub ClassificationSyncer {}

    impl ClassificationSyncer for ClassificationSyncer {
        fn sync_classification(&self, stream: &StreamConfig) -> Result<(), SyncError>;
        fn get_classification(&self, stream_id: &str) -> Result<StreamClassification, SyncError>;
        fn list_by_type(&self, stream_type: StreamType) -> Result<Vec<StreamClassification>, SyncError>;
        fn list_by_role(&self, role: CorrelationRole) -> Result<Vec<StreamClassification>, SyncError>;
    }
}
```

---

## MCP Tool Extension

### Query Examples

```bash
# List all observation streams (potential effects)
mcp__ndp__query_dictionary --query "
SELECT stream_id, correlation_role
FROM data_dictionary.stream_classification
WHERE stream_type = 'observation'"

# List all state_event streams (potential causes)
mcp__ndp__query_dictionary --query "
SELECT stream_id, correlation_role
FROM data_dictionary.stream_classification
WHERE stream_type = 'state_event'"

# Get NULL handling for a stream
mcp__ndp__query_dictionary --query "
SELECT null_handling
FROM data_dictionary.stream_classification
WHERE stream_id = 'air-quality'"
```

---

## References

- [SCOPE.md](../../SCOPE.md) - v11-002 Classification Propagation
- [DECISIONS.md](../../architecture/DECISIONS.md) - Decision 3: Data Dictionary Extension
- [SPEC-B01](./SPEC-B01-stream-type-classification.md) - Stream Type Classification
- [CONFIG-DEPLOYMENT-FLOW.md](../../architecture/CONFIG-DEPLOYMENT-FLOW.md) - Phase 3: Data Dictionary Sync

---

*Specification created: 2026-02-04*
