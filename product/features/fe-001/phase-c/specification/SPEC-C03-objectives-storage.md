# SPEC-C03: Objectives Storage (v11-007)

> **Feature ID:** v11-007
> **Feature Name:** Objectives Storage
> **Phase:** C (Cross-Stream + Alignment)
> **Priority:** Medium
> **Created:** 2026-02-04

---

## User Story

**As a** pattern detection system,
**I want** objectives stored in a queryable format,
**So that** I can evaluate whether observations meet targets and generate threshold crossing events.

---

## Goal

Store domain objectives in the data dictionary with:
1. Queryable metadata for target thresholds
2. Support for all condition types (<, >, <=, >=, ==, !=)
3. Link to source domain configuration
4. Enable threshold crossing detection (Phase E)

---

## Background

### What Are Objectives?

Objectives are declarative targets that define "good" states for metrics:

```yaml
objectives:
  - id: healthy_co2
    target:
      stream: air-quality
      metric: co2
      condition: "<"
      threshold: 800
      unit: ppm
    priority: high
```

This says: "CO2 should be less than 800 ppm".

### Why Store Objectives?

1. **V1.2 Pattern Detection**: Needs thresholds to find patterns that affect objective compliance
2. **Threshold Crossing Events**: Generate events when metrics cross objectives
3. **Dashboard Visualization**: Show objective lines on charts
4. **MCP Queryability**: Allow agents to query objectives programmatically

---

## Functional Requirements

### FR-C03-001: Objectives Table Creation

**Description:** Create `data_dictionary.objectives` table to store objective metadata.

**Acceptance Criteria:**
- Table created in `data_dictionary` schema
- Supports all objective fields from domain config
- Foreign key to `data_dictionary.domains`
- Unique constraint on (domain_id, objective_id)

**Table Schema:**
```sql
CREATE TABLE data_dictionary.objectives (
    objective_id TEXT NOT NULL,
    domain_id TEXT NOT NULL REFERENCES data_dictionary.domains(domain_id),
    description TEXT,
    target_stream TEXT NOT NULL,
    target_metric TEXT NOT NULL,
    condition TEXT NOT NULL CHECK (condition IN ('<', '>', '<=', '>=', '==', '!=')),
    threshold NUMERIC NOT NULL,
    unit TEXT,
    priority TEXT DEFAULT 'medium' CHECK (priority IN ('low', 'medium', 'high', 'critical')),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (domain_id, objective_id)
);
```

---

### FR-C03-002: Objectives Sync from Config

**Description:** Sync objectives from domain.yaml to data dictionary during deployment.

**Acceptance Criteria:**
- `deploy.sh` syncs objectives after domain config is applied
- Existing objectives are updated (upsert behavior)
- Removed objectives are deleted (full sync)
- Sync is idempotent

**Sync Flow:**
1. Read domain.yaml
2. Parse objectives array
3. DELETE existing objectives for domain (if full sync)
4. INSERT/UPDATE objectives from config

---

### FR-C03-003: Constraints Table Creation

**Description:** Create `data_dictionary.constraints` table for action constraints.

**Acceptance Criteria:**
- Table created in `data_dictionary` schema
- Supports constraint fields from domain config
- Foreign key to `data_dictionary.domains`
- Used by V1.3+ action framework

**Table Schema:**
```sql
CREATE TABLE data_dictionary.constraints (
    constraint_id TEXT NOT NULL,
    domain_id TEXT NOT NULL REFERENCES data_dictionary.domains(domain_id),
    description TEXT,
    constraint_stream TEXT NOT NULL,
    constraint_metric TEXT NOT NULL,
    condition TEXT NOT NULL CHECK (condition IN ('<', '>', '<=', '>=', '==', '!=')),
    threshold NUMERIC NOT NULL,
    unit TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (domain_id, constraint_id)
);
```

---

### FR-C03-004: Domain Table Creation

**Description:** Create `data_dictionary.domains` table as parent for objectives.

**Acceptance Criteria:**
- Table created in `data_dictionary` schema
- Stores domain metadata
- Referenced by objectives and constraints tables

**Table Schema:**
```sql
CREATE TABLE data_dictionary.domains (
    domain_id TEXT PRIMARY KEY,
    description TEXT,
    stream_count INTEGER,
    config_path TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE data_dictionary.domain_streams (
    domain_id TEXT NOT NULL REFERENCES data_dictionary.domains(domain_id),
    stream_id TEXT NOT NULL REFERENCES data_dictionary.streams(stream_id),
    alias TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('primary', 'context', 'actuator', 'constraint')),
    PRIMARY KEY (domain_id, stream_id)
);
```

---

### FR-C03-005: MCP Tool Query Support

**Description:** Objectives queryable via MCP tools.

**Acceptance Criteria:**
- MCP query returns objectives for a domain
- Supports filtering by priority, stream, metric
- Returns threshold values for dashboard annotations

**MCP Query Pattern:**
```sql
SELECT
    objective_id,
    target_stream,
    target_metric,
    condition,
    threshold,
    unit,
    priority
FROM data_dictionary.objectives
WHERE domain_id = 'indoor-air-quality'
AND priority IN ('high', 'critical');
```

---

### FR-C03-006: Objective Condition Types

**Description:** Support all comparison operators for objective conditions.

**Acceptance Criteria:**
- `<` : metric should be less than threshold
- `>` : metric should be greater than threshold
- `<=` : metric should be less than or equal to threshold
- `>=` : metric should be greater than or equal to threshold
- `==` : metric should equal threshold
- `!=` : metric should not equal threshold

**Condition SQL Generation (for threshold crossing):**
```rust
fn condition_to_sql(condition: &str, metric: &str, threshold: f64) -> String {
    match condition {
        "<" => format!("{} >= {}", metric, threshold),  // Crossing is when NOT meeting
        ">" => format!("{} <= {}", metric, threshold),
        "<=" => format!("{} > {}", metric, threshold),
        ">=" => format!("{} < {}", metric, threshold),
        "==" => format!("{} != {}", metric, threshold),
        "!=" => format!("{} == {}", metric, threshold),
        _ => panic!("Invalid condition"),
    }
}
```

---

## Non-Functional Requirements

### NFR-C03-001: Sync Performance

**Description:** Objective sync must be fast.

**Acceptance Criteria:**
- Sync of 10 objectives completes in < 500ms
- Does not block other deployment steps

---

### NFR-C03-002: Query Performance

**Description:** Objective queries must be efficient.

**Acceptance Criteria:**
- Query all objectives for a domain < 10ms
- Index on domain_id exists

---

## Domain Configuration Example

**File:** `config/domains/indoor-air-quality/domain.yaml`

```yaml
domain:
  id: indoor-air-quality
  description: "Maintain healthy indoor air quality"

  streams:
    - stream_id: air-quality
      alias: indoor
      role: primary
    - stream_id: outdoor-weather
      alias: outdoor
      role: context
    - stream_id: home-assistant-state
      alias: state
      role: actuator
    # outdoor-air-quality reserved for Phase D

  alignment:
    view_name: indoor_air_quality_aligned
    granularity: "1 hour"
    join_strategy: full_outer
    null_handling: by_stream_type

  objectives:
    - id: healthy_co2
      description: "Keep CO2 below 800 ppm for cognitive performance"
      target:
        stream: air-quality
        metric: co2
        condition: "<"
        threshold: 800
        unit: ppm
      priority: high

    - id: healthy_pm25
      description: "Keep PM2.5 below WHO guideline of 12 ug/m3"
      target:
        stream: air-quality
        metric: pm25
        condition: "<"
        threshold: 12
        unit: ug/m3
      priority: high

    - id: comfortable_temp
      description: "Maintain temperature between 20-24C"
      target:
        stream: air-quality
        metric: temperature_c
        condition: ">="
        threshold: 20
        unit: celsius
      priority: medium

    - id: comfortable_temp_upper
      description: "Maintain temperature below 24C"
      target:
        stream: air-quality
        metric: temperature_c
        condition: "<="
        threshold: 24
        unit: celsius
      priority: medium

  constraints:
    - id: outdoor_air_safe
      description: "Don't open windows if outdoor PM2.5 is high"
      stream: outdoor-air-quality  # Future stream
      metric: pm25
      condition: "<"
      threshold: 35
      unit: ug/m3
```

---

## Sync Script Example

**Location:** `deploy/pi/deploy.sh` (extended)

```bash
sync_objectives_to_data_dictionary() {
    local domain_id="$1"
    local config_file="$CONFIG_DIR/domains/$domain_id/domain.yaml"

    if [ ! -f "$config_file" ]; then
        warn "Domain config not found: $config_file"
        return 1
    fi

    log "Syncing objectives for domain: $domain_id"

    # Parse YAML and generate SQL
    local sql=$(cat <<EOF
-- Upsert domain
INSERT INTO data_dictionary.domains (domain_id, description, config_path, updated_at)
VALUES (
    '$(yq -r '.domain.id' "$config_file")',
    '$(yq -r '.domain.description // ""' "$config_file")',
    '$config_file',
    NOW()
)
ON CONFLICT (domain_id) DO UPDATE SET
    description = EXCLUDED.description,
    config_path = EXCLUDED.config_path,
    updated_at = NOW();

-- Clear existing objectives for this domain
DELETE FROM data_dictionary.objectives WHERE domain_id = '$domain_id';

-- Insert objectives from config
$(yq -r '.domain.objectives[]? | "INSERT INTO data_dictionary.objectives (domain_id, objective_id, description, target_stream, target_metric, condition, threshold, unit, priority) VALUES (\x27'$domain_id'\x27, \x27\(.id)\x27, \x27\(.description // "")\x27, \x27\(.target.stream)\x27, \x27\(.target.metric)\x27, \x27\(.target.condition)\x27, \(.target.threshold), \x27\(.target.unit // "")\x27, \x27\(.priority // "medium")\x27);"' "$config_file")

-- Clear existing constraints for this domain
DELETE FROM data_dictionary.constraints WHERE domain_id = '$domain_id';

-- Insert constraints from config
$(yq -r '.domain.constraints[]? | "INSERT INTO data_dictionary.constraints (domain_id, constraint_id, description, constraint_stream, constraint_metric, condition, threshold, unit) VALUES (\x27'$domain_id'\x27, \x27\(.id)\x27, \x27\(.description // "")\x27, \x27\(.stream)\x27, \x27\(.metric)\x27, \x27\(.condition)\x27, \(.threshold), \x27\(.unit // "")\x27);"' "$config_file")

-- Sync domain streams
DELETE FROM data_dictionary.domain_streams WHERE domain_id = '$domain_id';

$(yq -r '.domain.streams[]? | "INSERT INTO data_dictionary.domain_streams (domain_id, stream_id, alias, role) VALUES (\x27'$domain_id'\x27, \x27\(.stream_id)\x27, \x27\(.alias)\x27, \x27\(.role)\x27);"' "$config_file")
EOF
)

    echo "$sql" | dcx timescaledb psql -U postgres -d ndp

    log "Objectives synced for domain: $domain_id"
}
```

---

## Acceptance Criteria (Given/When/Then)

### Scenario: Sync Objectives from Config

```gherkin
Given domain.yaml has 2 objectives defined
When deploy.sh applies the domain config
Then data_dictionary.objectives should have 2 rows for this domain
And each row should match the config values
```

### Scenario: Upsert Behavior

```gherkin
Given an objective "healthy_co2" exists with threshold 800
When the config is updated to threshold 1000
And deploy.sh applies the domain config
Then the objective threshold should be 1000
And there should still be only 1 row for "healthy_co2"
```

### Scenario: Query Objectives via MCP

```gherkin
Given objectives are stored for indoor-air-quality domain
When MCP queries for high-priority objectives
Then the response should include healthy_co2 and healthy_pm25
And should include threshold values for dashboard annotation
```

### Scenario: Remove Deleted Objectives

```gherkin
Given 3 objectives exist for a domain
When the config is updated to have only 2 objectives
And deploy.sh applies the domain config
Then data_dictionary.objectives should have exactly 2 rows
And the deleted objective should not exist
```

### Scenario: Constraint Storage

```gherkin
Given domain.yaml has a constraint "outdoor_air_safe"
When deploy.sh applies the domain config
Then data_dictionary.constraints should have 1 row
And the constraint stream should be "outdoor-air-quality"
```

---

## London TDD Interfaces

### IObjectivesSyncer (deploy/pi or tools)

```rust
/// Syncs objectives from domain config to data dictionary
pub trait IObjectivesSyncer {
    /// Sync all objectives for a domain
    fn sync_objectives(&self, domain_id: &str, objectives: &[Objective]) -> Result<(), SyncError>;

    /// Sync all constraints for a domain
    fn sync_constraints(&self, domain_id: &str, constraints: &[Constraint]) -> Result<(), SyncError>;

    /// Full domain sync (domain + streams + objectives + constraints)
    fn sync_domain(&self, domain: &DomainConfig) -> Result<(), SyncError>;
}
```

### IObjectivesQuery (MCP tools)

```rust
/// Query interface for objectives
pub trait IObjectivesQuery {
    /// Get all objectives for a domain
    fn get_objectives(&self, domain_id: &str) -> Result<Vec<Objective>, QueryError>;

    /// Get objectives filtered by priority
    fn get_objectives_by_priority(
        &self,
        domain_id: &str,
        min_priority: Priority,
    ) -> Result<Vec<Objective>, QueryError>;

    /// Get objectives for a specific stream/metric
    fn get_objectives_for_metric(
        &self,
        domain_id: &str,
        stream_id: &str,
        metric: &str,
    ) -> Result<Vec<Objective>, QueryError>;
}
```

### Objective Domain Type (core/src/gold/config.rs)

```rust
/// An objective (target to optimize toward)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Objective {
    pub id: String,
    pub description: Option<String>,
    pub target: ObjectiveTarget,
    pub priority: Priority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectiveTarget {
    pub stream: String,
    pub metric: String,
    pub condition: Condition,
    pub threshold: f64,
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Condition {
    LessThan,       // <
    GreaterThan,    // >
    LessOrEqual,    // <=
    GreaterOrEqual, // >=
    Equal,          // ==
    NotEqual,       // !=
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

/// A constraint (condition that must be met for action)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
    pub id: String,
    pub description: Option<String>,
    pub stream: String,
    pub metric: String,
    pub condition: Condition,
    pub threshold: f64,
    pub unit: Option<String>,
}
```

---

## Dependencies

| Dependency | Type | Required By |
|------------|------|-------------|
| v11-A05: Objectives JSON Schema | Phase A | Validation |
| data_dictionary schema | V1.0 | Table creation |
| Domain config file | Phase A | Source data |
| yq tool | Deployment | YAML parsing |

---

## Test Cases

### Unit Tests

| Test | Description | Expected |
|------|-------------|----------|
| `test_objective_condition_parsing` | Parse condition strings | Correct enum values |
| `test_objective_sql_generation` | Generate INSERT SQL | Valid SQL, escaped values |
| `test_constraint_sql_generation` | Generate constraint INSERT | Valid SQL |
| `test_domain_sync_sql` | Full domain sync SQL | Upsert pattern |

### Integration Tests

| Test | Description | Verification |
|------|-------------|--------------|
| `test_objectives_sync_e2e` | Sync from YAML to DB | Count matches, values correct |
| `test_objectives_upsert` | Update existing | Threshold updated, count same |
| `test_objectives_delete_removed` | Remove deleted | Count decreases |
| `test_mcp_query_objectives` | MCP query | Returns correct data |

---

## Phase E Integration

Objectives enable threshold crossing detection (v11-012):

```sql
-- In gold.threshold_crossings (Phase E)
SELECT
    o.observation_time AS event_time,
    obj.target_stream AS stream_id,
    o.ndp_id AS entity_id,
    obj.objective_id,
    obj.target_metric AS metric,
    obj.threshold,
    obj.condition,
    o.value AS current_value,
    LAG(o.value) OVER (PARTITION BY o.ndp_id ORDER BY o.observation_time) AS previous_value,
    CASE
        WHEN obj.condition = '<' AND o.value >= obj.threshold
             AND LAG(o.value) OVER w < obj.threshold THEN 'rising_above'
        WHEN obj.condition = '<' AND o.value < obj.threshold
             AND LAG(o.value) OVER w >= obj.threshold THEN 'falling_below'
        -- ... other conditions
    END AS crossing_type
FROM silver.air_quality_observations o
JOIN data_dictionary.objectives obj
    ON obj.target_stream = 'air-quality'
    AND obj.target_metric = 'co2'  -- Example metric
WHERE crossing_type IS NOT NULL;
```

---

## Data Dictionary DDL

**File:** `deploy/pi/init-timescaledb.sql` (or similar, extended)

```sql
-- Domain table
CREATE TABLE IF NOT EXISTS data_dictionary.domains (
    domain_id TEXT PRIMARY KEY,
    description TEXT,
    stream_count INTEGER,
    config_path TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Domain-stream mapping
CREATE TABLE IF NOT EXISTS data_dictionary.domain_streams (
    domain_id TEXT NOT NULL REFERENCES data_dictionary.domains(domain_id) ON DELETE CASCADE,
    stream_id TEXT NOT NULL,  -- May reference non-existent stream (future)
    alias TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('primary', 'context', 'actuator', 'constraint')),
    PRIMARY KEY (domain_id, stream_id)
);

-- Objectives table
CREATE TABLE IF NOT EXISTS data_dictionary.objectives (
    objective_id TEXT NOT NULL,
    domain_id TEXT NOT NULL REFERENCES data_dictionary.domains(domain_id) ON DELETE CASCADE,
    description TEXT,
    target_stream TEXT NOT NULL,
    target_metric TEXT NOT NULL,
    condition TEXT NOT NULL CHECK (condition IN ('<', '>', '<=', '>=', '==', '!=')),
    threshold NUMERIC NOT NULL,
    unit TEXT,
    priority TEXT DEFAULT 'medium' CHECK (priority IN ('low', 'medium', 'high', 'critical')),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (domain_id, objective_id)
);

-- Constraints table
CREATE TABLE IF NOT EXISTS data_dictionary.constraints (
    constraint_id TEXT NOT NULL,
    domain_id TEXT NOT NULL REFERENCES data_dictionary.domains(domain_id) ON DELETE CASCADE,
    description TEXT,
    constraint_stream TEXT NOT NULL,
    constraint_metric TEXT NOT NULL,
    condition TEXT NOT NULL CHECK (condition IN ('<', '>', '<=', '>=', '==', '!=')),
    threshold NUMERIC NOT NULL,
    unit TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (domain_id, constraint_id)
);

-- Indexes for query performance
CREATE INDEX IF NOT EXISTS idx_objectives_domain ON data_dictionary.objectives(domain_id);
CREATE INDEX IF NOT EXISTS idx_objectives_stream ON data_dictionary.objectives(target_stream);
CREATE INDEX IF NOT EXISTS idx_constraints_domain ON data_dictionary.constraints(domain_id);
CREATE INDEX IF NOT EXISTS idx_domain_streams_domain ON data_dictionary.domain_streams(domain_id);
```

---

## References

- [SCOPE.md - v11-007](/workspaces/neural-data-platform/product/features/fe-001/SCOPE.md)
- [ADR-FE001-002: Domain-Centric Configuration](/workspaces/neural-data-platform/product/features/fe-001/architecture/ADR-FE001-002-domain-centric-config.md)
- [DECISIONS.md - Decision 6](/workspaces/neural-data-platform/product/features/fe-001/architecture/DECISIONS.md)
- [PHASE-C-OVERVIEW.md](./PHASE-C-OVERVIEW.md) - Phase C overview
