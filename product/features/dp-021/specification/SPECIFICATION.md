# dp-021: Config Lifecycle & Release Management - SPARC Specification

**Document Type**: SPARC Specification (Phase S)
**Feature**: dp-021 Config Lifecycle & Release Management
**Version**: 1.0
**Date**: 2026-02-02
**Parent**: dp-016 Configuration Architecture Review
**Dependencies**: dp-018 JSON Config Foundation, dp-019 Config Validation Pipeline, dp-020 Declarative Deploy

---

## 1. Executive Summary

This specification defines the requirements for implementing Phases 4 and 5 of the dp-016 Configuration Architecture roadmap, plus formalizing the Release Methodology. These capabilities complete the configuration lifecycle management story.

### Key Outcomes

1. **Hot-reload sources** - MQTT/HTTP source configuration changes without application restart
2. **Schema migration** - Clean transition from v1.1 to v2.0 (entity_schemas elimination)
3. **Release methodology** - Formalized semantic versioning tied to deployment manifests
4. **Deployed version tracking** - Device state queryable via `/var/ndp/deployed-version`
5. **Webhook foundation** - Specification for future automated deployment triggers

### Core Architecture Principles

**Hot-Reload**: Minimize downtime by allowing source reconfiguration without restart.

**Clean Break**: v2.0 schema enforces single source of field metadata (enriched fields only).

**Traceability**: Every deployment is tied to a versioned release with manifest alignment.

---

## 2. Requirements Analysis

### 2.1 Phase 4: Hot-Reload (Sources Only)

#### FR-4.1: etcd Watch Connection to SourceManager

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-4.1.1** | Wire etcd watch to SourceManager | HIGH | ConfigWatcher notifies SourceManager on stream config change | Task 4.1 |
| **FR-4.1.2** | Stream-specific watch filtering | HIGH | Only notify SourceManager for changed stream_id, not all streams | Task 4.1 |
| **FR-4.1.3** | Watch reconnection handling | HIGH | Automatic reconnection on etcd connection loss with exponential backoff | Task 4.1 |
| **FR-4.1.4** | Log watch events | MEDIUM | Log config change detection with stream_id and timestamp | Task 4.1 |

#### FR-4.2: SourceManager::update_sources_for_stream()

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-4.2.1** | Implement update method | CRITICAL | `SourceManager::update_sources_for_stream(stream_id: &str, config: &StreamConfig)` exists | Task 4.2 |
| **FR-4.2.2** | Source comparison | HIGH | Compare old vs new source configs; only update changed sources | Task 4.2 |
| **FR-4.2.3** | Graceful source shutdown | HIGH | Stop old source cleanly before starting new source | Task 4.2 |
| **FR-4.2.4** | Source startup with new config | HIGH | New source starts with updated configuration | Task 4.2 |
| **FR-4.2.5** | Error isolation | MEDIUM | Failed source update does not affect other sources in same stream | Task 4.2 |
| **FR-4.2.6** | Update metrics | LOW | Emit metric for source update duration and success/failure | Task 4.2 |

#### FR-4.3: MQTT Graceful Reconnection

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-4.3.1** | No message loss guarantee | CRITICAL | In-flight messages are processed before disconnect | Task 4.3 |
| **FR-4.3.2** | Clean disconnect | HIGH | Send MQTT DISCONNECT packet before closing connection | Task 4.3 |
| **FR-4.3.3** | Reconnect with new config | HIGH | New connection uses updated broker/topic/credentials | Task 4.3 |
| **FR-4.3.4** | QoS preservation | MEDIUM | Maintain same QoS level after reconnection | Task 4.3 |
| **FR-4.3.5** | Subscription restoration | HIGH | Resubscribe to topics after reconnection | Task 4.3 |
| **FR-4.3.6** | Reconnection timeout | MEDIUM | Fail update if reconnection takes longer than 30 seconds | Task 4.3 |

#### FR-4.4: HTTP Polling Interval Update

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-4.4.1** | Immediate interval change | HIGH | New polling interval takes effect within one poll cycle | Task 4.4 |
| **FR-4.4.2** | URL change handling | HIGH | HTTP source can update target URL without restart | Task 4.4 |
| **FR-4.4.3** | Header/auth update | MEDIUM | HTTP source can update headers and authentication | Task 4.4 |
| **FR-4.4.4** | No double-poll | MEDIUM | Interval change does not cause immediate duplicate poll | Task 4.4 |

#### FR-4.5: Optional Reload HTTP Endpoint

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-4.5.1** | Reload endpoint path | LOW | `POST /api/streams/{stream_id}/reload` triggers source reload | Task 4.5 |
| **FR-4.5.2** | Authentication required | LOW | Endpoint requires valid API key or local-only access | Task 4.5 |
| **FR-4.5.3** | Response format | LOW | Returns JSON with reload status and timing | Task 4.5 |
| **FR-4.5.4** | Reload all endpoint | LOW | `POST /api/streams/reload-all` reloads all stream sources | Task 4.5 |

#### FR-4.6: Integration Test Requirements

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-4.6.1** | MQTT topic change test | HIGH | Config change in etcd triggers MQTT resubscription | Task 4.6 |
| **FR-4.6.2** | HTTP interval change test | HIGH | Config change immediately updates polling interval | Task 4.6 |
| **FR-4.6.3** | No message loss test | CRITICAL | Messages published during reload are not lost | Task 4.6 |
| **FR-4.6.4** | Error recovery test | MEDIUM | Invalid config update is rejected; old config continues | Task 4.6 |
| **FR-4.6.5** | Concurrent update test | MEDIUM | Multiple rapid config changes are handled correctly | Task 4.6 |

---

### 2.2 Phase 5: Schema Migration (v1.1 to v2.0)

#### FR-5.1: Migration Script (shell+jq)

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-5.1.1** | Script location | HIGH | `scripts/ndp-migrate-config.sh` exists and is executable | Task 5.1 |
| **FR-5.1.2** | Idempotent execution | HIGH | Running migration twice produces same result | Task 5.1 |
| **FR-5.1.3** | Single config migration | HIGH | `ndp-migrate-config.sh config.json` migrates one file | Task 5.1 |
| **FR-5.1.4** | Batch migration | HIGH | `ndp-migrate-config.sh --all` migrates all configs | Task 5.1 |
| **FR-5.1.5** | Exit codes | MEDIUM | Exit 0 on success, 1 on validation error, 2 on system error | Task 5.1 |
| **FR-5.1.6** | Progress output | MEDIUM | Show progress when migrating multiple files | Task 5.1 |

#### FR-5.2: v2.0 JSON Schema Definition

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-5.2.1** | Schema file location | HIGH | `schemas/stream-config.v2.schema.json` exists | Task 5.2 |
| **FR-5.2.2** | config_version required | HIGH | `config_version` must be `2` (integer) | Task 5.2 |
| **FR-5.2.3** | entity_schemas forbidden | CRITICAL | Schema rejects any config with `entity_schemas` field | Task 5.2 |
| **FR-5.2.4** | fields.description required | HIGH | Each field in `fields[]` must have `description` | Task 5.2 |
| **FR-5.2.5** | additionalProperties false | HIGH | No unknown fields allowed at any level | Task 5.2 |
| **FR-5.2.6** | Backward incompatible | HIGH | v1.1 configs fail validation against v2.0 schema | Task 5.2 |

#### FR-5.3: entity_schemas Removal Transform

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-5.3.1** | Remove entity_schemas | CRITICAL | Migration deletes `entity_schemas` section entirely | Task 5.3 |
| **FR-5.3.2** | Bump config_version | HIGH | Migration sets `config_version` to `2` | Task 5.3 |
| **FR-5.3.3** | Preserve other fields | CRITICAL | All other config fields remain unchanged | Task 5.3 |
| **FR-5.3.4** | Validate fields enriched | HIGH | Migration fails if fields lack description (should have been enriched in dp-018) | Task 5.3 |
| **FR-5.3.5** | JSON formatting | MEDIUM | Output maintains consistent JSON formatting (2-space indent) | Task 5.3 |

#### FR-5.4: Dictionary Loader Update

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-5.4.1** | Remove entity_schemas fallback | HIGH | Dictionary loader reads ONLY from `fields[]` | Task 5.4 |
| **FR-5.4.2** | Error on missing description | HIGH | Loader errors if field lacks description in v2.0 config | Task 5.4 |
| **FR-5.4.3** | Code cleanup | MEDIUM | Remove all `entity_schemas` references from loader code | Task 5.4 |
| **FR-5.4.4** | Update tests | HIGH | All dictionary loader tests updated for v2.0 schema | Task 5.4 |

#### FR-5.5: Dry-Run Mode

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-5.5.1** | Dry-run flag | HIGH | `--dry-run` flag shows changes without writing files | Task 5.5 |
| **FR-5.5.2** | Diff output | HIGH | Dry-run shows unified diff of proposed changes | Task 5.5 |
| **FR-5.5.3** | Validation in dry-run | HIGH | Dry-run validates result would be valid v2.0 | Task 5.5 |
| **FR-5.5.4** | No side effects | CRITICAL | Dry-run makes no file system changes | Task 5.5 |

#### FR-5.6: Validator v2.0 Enforcement

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-5.6.1** | Schema version detection | HIGH | Validator detects config_version and applies correct schema | Task 5.6 |
| **FR-5.6.2** | Reject entity_schemas | CRITICAL | Validator fails configs with entity_schemas in v2.0 mode | Task 5.6 |
| **FR-5.6.3** | --schema-version flag | MEDIUM | `ndp-validate --schema-version 2` forces v2.0 validation | Task 5.6 |
| **FR-5.6.4** | Clear error message | HIGH | Error explains entity_schemas was removed in v2.0 | Task 5.6 |

---

### 2.3 Phase R: Release Methodology

#### FR-R.1: Semantic Versioning Standard

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-R.1.1** | Document versioning rules | HIGH | `docs/procedures/RELEASE-POLICY.md` defines SemVer 2.0.0 usage | Task R.1 |
| **FR-R.1.2** | MAJOR version rules | HIGH | Breaking changes (schema v2.0, API changes) bump MAJOR | Task R.1 |
| **FR-R.1.3** | MINOR version rules | HIGH | New features (new stream, new MCP tool) bump MINOR | Task R.1 |
| **FR-R.1.4** | PATCH version rules | HIGH | Bug fixes, config corrections bump PATCH | Task R.1 |
| **FR-R.1.5** | Pre-release tags | MEDIUM | Support `-alpha`, `-beta`, `-rc.N` suffixes | Task R.1 |

#### FR-R.2: Manifest Naming Convention

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-R.2.1** | Naming pattern | HIGH | Release manifests named `v{MAJOR}.{MINOR}.{PATCH}.manifest.json` | Task R.2 |
| **FR-R.2.2** | Location | HIGH | Release manifests stored in `.deploy/releases/` directory | Task R.2 |
| **FR-R.2.3** | Template manifest | MEDIUM | `.deploy/releases/TEMPLATE.manifest.json` exists for new releases | Task R.2 |
| **FR-R.2.4** | Validation | HIGH | deploy.sh validates manifest filename matches `release_version` field | Task R.2 |

#### FR-R.3: Git Tag Alignment

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-R.3.1** | Tag format | HIGH | Git tags follow `v{MAJOR}.{MINOR}.{PATCH}` format | Task R.3, R.4 |
| **FR-R.3.2** | Tag-manifest alignment | HIGH | Git tag `vX.Y.Z` has corresponding `vX.Y.Z.manifest.json` | Task R.3, R.4 |
| **FR-R.3.3** | Annotated tags | MEDIUM | Release tags are annotated (not lightweight) | Task R.3 |
| **FR-R.3.4** | Tag message | LOW | Tag message includes release description | Task R.3 |
| **FR-R.3.5** | Release checklist | HIGH | `docs/procedures/RELEASE-CHECKLIST.md` documents release steps | Task R.3 |

#### FR-R.4: Device Deployed-Version Tracking

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-R.4.1** | Version file path | HIGH | `/var/ndp/deployed-version` contains release version | Task R.6 |
| **FR-R.4.2** | Format | HIGH | File contains plain text version string (e.g., `v1.2.0`) | Task R.6 |
| **FR-R.4.3** | Atomic write | MEDIUM | Write via temp file + rename for atomicity | Task R.6 |
| **FR-R.4.4** | Queryable state | HIGH | Operators can query device version via file or API | Task R.6 |
| **FR-R.4.5** | Manifest hash tracking | MEDIUM | `/var/ndp/manifest-applied` contains SHA256 of applied manifest | Task R.6 |
| **FR-R.4.6** | Deployment timestamp | HIGH | `/var/ndp/deployed-at` contains ISO 8601 timestamp | Task R.6 |

#### FR-R.5: Webhook Trigger Specification

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-R.5.1** | Webhook spec document | HIGH | `docs/procedures/WEBHOOK-DEPLOYMENT-SPEC.md` exists | Task R.8 |
| **FR-R.5.2** | Trigger event | HIGH | Spec defines: GitHub tag push `v*` triggers deployment | Task R.8 |
| **FR-R.5.3** | Receiver interface | HIGH | Spec defines Pi-side webhook receiver requirements | Task R.8 |
| **FR-R.5.4** | Deployment flow | HIGH | Spec defines: git pull, locate manifest, apply, report status | Task R.8 |
| **FR-R.5.5** | Security considerations | MEDIUM | Spec addresses webhook authentication and validation | Task R.8 |
| **FR-R.5.6** | Error handling | MEDIUM | Spec defines rollback behavior on deployment failure | Task R.8 |

---

### 2.4 Non-Functional Requirements

#### Performance

| ID | Category | Requirement | Measurement | Traces To |
|----|----------|-------------|-------------|-----------|
| **NFR-001** | Performance | Hot-reload completes in <5 seconds per source | Benchmark reload time | Phase 4 |
| **NFR-002** | Performance | MQTT reconnection <3 seconds | Measure reconnection time | FR-4.3 |
| **NFR-003** | Performance | Migration script processes config in <100ms | Benchmark single config | Phase 5 |
| **NFR-004** | Performance | Batch migration <10 seconds for 20 configs | Benchmark --all mode | FR-5.1.4 |

#### Reliability

| ID | Category | Requirement | Measurement | Traces To |
|----|----------|-------------|-------------|-----------|
| **NFR-010** | Reliability | Zero message loss during MQTT hot-reload | Integration test verification | FR-4.3.1 |
| **NFR-011** | Reliability | Failed hot-reload preserves previous working config | Error recovery test | FR-4.2.5 |
| **NFR-012** | Reliability | Migration is idempotent | Run twice, compare results | FR-5.1.2 |
| **NFR-013** | Reliability | Dry-run produces accurate preview | Compare dry-run to actual | FR-5.5 |

#### Maintainability

| ID | Category | Requirement | Measurement | Traces To |
|----|----------|-------------|-------------|-----------|
| **NFR-020** | Maintainability | Migration script uses only shell+jq (no Python) | Code review | Platform constraint |
| **NFR-021** | Maintainability | Release procedure documented with examples | Documentation review | Phase R |
| **NFR-022** | Maintainability | Version tracking files have defined schema | Schema documented | FR-R.4 |

#### Traceability

| ID | Category | Requirement | Measurement | Traces To |
|----|----------|-------------|-------------|-----------|
| **NFR-030** | Traceability | Every deployment linked to git tag | Query deployed-version | FR-R.4 |
| **NFR-031** | Traceability | Every manifest has release_version field | Schema validation | FR-R.2 |
| **NFR-032** | Traceability | Deployment logs include version applied | Log inspection | Phase R |

---

## 3. Acceptance Criteria

### AC-001: Hot-Reload Source Configuration

```gherkin
Feature: Hot-Reload MQTT Source

  Scenario: MQTT topic change without restart
    Given air-quality-app is running with MQTT source subscribed to "sensors/pm25"
    And etcd contains config for stream "air-quality"
    When I update the MQTT topic to "sensors/air-quality/pm25" in etcd
    Then within 5 seconds, the source resubscribes to the new topic
    And messages on "sensors/air-quality/pm25" are received
    And messages on old topic "sensors/pm25" are ignored
    And no application restart occurred
    And log shows "Source reloaded for stream air-quality"

  Scenario: MQTT broker change without restart
    Given air-quality-app is running connected to mqtt://broker-a:1883
    When I update the MQTT broker to "mqtt://broker-b:1883" in etcd config
    Then source disconnects from broker-a
    And source connects to broker-b
    And no messages are lost during transition

  Scenario: HTTP polling interval change
    Given air-quality-app is running with HTTP source polling every 60 seconds
    When I update the polling interval to 30 seconds in etcd config
    Then the next poll occurs within 30 seconds (not 60)
    And subsequent polls use 30-second interval
```

### AC-002: Migration v1.1 to v2.0

```gherkin
Feature: Schema Migration

  Scenario: Successful v1.1 to v2.0 migration
    Given a config file at v1.1 with entity_schemas section
    And fields[] already enriched with description/device_class
    When I run `ndp-migrate-config.sh config.json`
    Then the entity_schemas section is removed
    And config_version is set to 2
    And all other fields are preserved
    And the file passes v2.0 schema validation

  Scenario: Dry-run shows changes without modifying
    Given a v1.1 config file
    When I run `ndp-migrate-config.sh --dry-run config.json`
    Then output shows unified diff of proposed changes
    And the original file is unchanged
    And exit code is 0

  Scenario: Migration fails on unenriched fields
    Given a v1.1 config file where fields[] lacks description
    When I run `ndp-migrate-config.sh config.json`
    Then migration fails with error
    And error message indicates which field lacks description
    And original file is unchanged

  Scenario: Batch migration
    Given 5 config files at v1.1 in config/base/streams/
    When I run `ndp-migrate-config.sh --all`
    Then all 5 files are migrated to v2.0
    And progress is shown for each file
    And summary shows "5 configs migrated successfully"
```

### AC-003: Validator v2.0 Enforcement

```gherkin
Feature: v2.0 Schema Enforcement

  Scenario: Validator rejects entity_schemas in v2.0 config
    Given a config file with config_version: 2
    And the file contains an entity_schemas section
    When I run `ndp-validate config.json`
    Then validation fails with error code FORBIDDEN_FIELD
    And error path is "$.entity_schemas"
    And error message explains "entity_schemas removed in v2.0"

  Scenario: Validator accepts clean v2.0 config
    Given a config file with config_version: 2
    And no entity_schemas section
    And all fields have description
    When I run `ndp-validate config.json`
    Then validation passes
    And exit code is 0
```

### AC-004: Release Methodology

```gherkin
Feature: Release Versioning

  Scenario: Create release with proper artifacts
    Given I have made changes ready for release
    When I create a release following RELEASE-CHECKLIST.md
    Then .deploy/releases/v1.2.0.manifest.json exists
    And manifest contains "release_version": "1.2.0"
    And git tag v1.2.0 is created
    And tag is annotated with release description

  Scenario: Deploy release to device
    Given git tag v1.2.0 exists with manifest
    And device has previous release deployed
    When I run `git pull && ./deploy.sh apply .deploy/releases/v1.2.0.manifest.json`
    Then deployment completes successfully
    And /var/ndp/deployed-version contains "v1.2.0"
    And /var/ndp/deployed-at contains current timestamp
    And /var/ndp/manifest-applied contains SHA256 of manifest

  Scenario: Query deployed version
    Given device has v1.2.0 deployed
    When I run `cat /var/ndp/deployed-version`
    Then output is "v1.2.0"
```

### AC-005: Integration Test - Hot-Reload End-to-End

```gherkin
Feature: Hot-Reload Integration Test

  Scenario: Full hot-reload cycle with message continuity
    Given docker-compose.integration.yml is running
    And air-quality-app is processing MQTT messages
    And messages are being published to "sensors/test"
    When I update stream config in etcd to change topic to "sensors/test-v2"
    And simultaneously continue publishing to both topics
    Then app reloads sources within 5 seconds
    And messages from "sensors/test-v2" are received
    And total message count shows no gaps (all messages processed)
    And no ERROR logs during reload
```

---

## 4. Data Model Specification

### 4.1 Device State Files

| File Path | Content | Format | Example |
|-----------|---------|--------|---------|
| `/var/ndp/deployed-version` | Release version | Plain text | `v1.2.0` |
| `/var/ndp/deployed-at` | Deployment timestamp | ISO 8601 | `2026-02-02T14:30:00Z` |
| `/var/ndp/manifest-applied` | Manifest hash | SHA256 hex | `a1b2c3d4e5f6...` |

### 4.2 Release Manifest Schema Extension

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "manifest.release.schema.json",
  "title": "NDP Release Manifest",
  "type": "object",
  "required": ["version", "release_version", "changes"],
  "additionalProperties": false,
  "properties": {
    "version": {
      "type": "string",
      "enum": ["1.0"],
      "description": "Manifest schema version"
    },
    "release_version": {
      "type": "string",
      "pattern": "^[0-9]+\\.[0-9]+\\.[0-9]+(-[a-zA-Z0-9.]+)?$",
      "description": "SemVer release version"
    },
    "description": {
      "type": "string",
      "description": "Human-readable release description"
    },
    "changes": {
      "type": "array",
      "description": "Declaration of changes in this release"
    }
  }
}
```

### 4.3 v2.0 Stream Config Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "stream-config.v2.schema.json",
  "title": "NDP Stream Configuration v2.0",
  "type": "object",
  "required": ["config_version", "stream_id", "description", "fields", "sources"],
  "additionalProperties": false,
  "properties": {
    "config_version": {
      "const": 2,
      "description": "Must be 2 for v2.0 schema"
    },
    "stream_id": {
      "type": "string",
      "pattern": "^[a-z][a-z0-9-]*$"
    },
    "description": {
      "type": "string"
    },
    "fields": {
      "type": "array",
      "minItems": 1,
      "items": {
        "type": "object",
        "required": ["name", "type", "description"],
        "additionalProperties": false,
        "properties": {
          "name": {"type": "string", "pattern": "^[a-z_][a-z0-9_]*$"},
          "type": {"type": "string"},
          "description": {"type": "string"},
          "device_class": {"type": "string"},
          "unit": {"type": "string"},
          "nullable": {"type": "boolean"},
          "range": {"type": "array", "items": {"type": "number"}, "minItems": 2, "maxItems": 2}
        }
      }
    },
    "sources": {
      "type": "array",
      "minItems": 1
    },
    "silver_etl": {
      "type": "object"
    }
  }
}
```

**Key v2.0 Changes**:
- `config_version` must be exactly `2`
- `entity_schemas` field is NOT in properties (rejected by additionalProperties: false)
- `fields[].description` is REQUIRED

---

## 5. Interface Specification

### 5.1 SourceManager API

```rust
/// Extended SourceManager interface for hot-reload
impl SourceManager {
    /// Update sources for a specific stream without full restart
    ///
    /// # Arguments
    /// * `stream_id` - The stream to update
    /// * `config` - New stream configuration
    ///
    /// # Returns
    /// * `Ok(ReloadResult)` - Reload succeeded with metrics
    /// * `Err(ReloadError)` - Reload failed; previous config still active
    pub async fn update_sources_for_stream(
        &mut self,
        stream_id: &str,
        config: &StreamConfig,
    ) -> Result<ReloadResult, ReloadError>;

    /// Check if hot-reload is supported for given config change
    ///
    /// # Returns
    /// * `true` if only source-level changes detected
    /// * `false` if changes require full restart (e.g., silver_etl changes)
    pub fn supports_hot_reload(
        old_config: &StreamConfig,
        new_config: &StreamConfig,
    ) -> bool;
}

pub struct ReloadResult {
    pub stream_id: String,
    pub sources_updated: Vec<String>,
    pub duration_ms: u64,
}

pub enum ReloadError {
    SourceConnectionFailed(String),
    Timeout,
    ConfigValidationFailed(String),
    InternalError(String),
}
```

### 5.2 Migration Script CLI

```bash
# Usage
ndp-migrate-config.sh [OPTIONS] [CONFIG_PATH]

# Options
  --from <VERSION>      Source schema version (default: auto-detect)
  --to <VERSION>        Target schema version (default: 2)
  --dry-run             Preview changes without writing
  --all                 Migrate all configs in config/base/streams/
  --verbose             Show detailed progress
  --help                Show usage information

# Examples
ndp-migrate-config.sh config/base/streams/air-quality/config.json
ndp-migrate-config.sh --dry-run --all
ndp-migrate-config.sh --from 1.1 --to 2 config.json

# Exit Codes
  0  Success
  1  Migration/validation error
  2  System error (file not found, permission denied)
```

### 5.3 Deploy.sh Release Commands

```bash
# Apply a specific release manifest
./deploy.sh apply .deploy/releases/v1.2.0.manifest.json

# Show current deployed version
./deploy.sh version

# Output:
# Deployed Version: v1.2.0
# Deployed At: 2026-02-02T14:30:00Z
# Manifest Hash: a1b2c3d4...
```

### 5.4 Reload HTTP Endpoint (Optional)

```yaml
openapi: 3.0.0
paths:
  /api/streams/{stream_id}/reload:
    post:
      summary: Trigger hot-reload for a specific stream
      parameters:
        - name: stream_id
          in: path
          required: true
          schema:
            type: string
      responses:
        200:
          description: Reload successful
          content:
            application/json:
              schema:
                type: object
                properties:
                  status: {type: string, enum: [success]}
                  stream_id: {type: string}
                  sources_reloaded: {type: array, items: {type: string}}
                  duration_ms: {type: integer}
        400:
          description: Reload not supported (requires restart)
        500:
          description: Reload failed
```

---

## 6. Hot-Reload Architecture

### 6.1 Watch-to-Reload Flow

```
etcd key: /streams/{stream_id}/config
    │
    │ (etcd watch)
    ▼
ConfigWatcher::on_change(stream_id, new_config)
    │
    ├── Parse JSON config
    ├── Validate config (fast path)
    │
    ▼
SourceManager::update_sources_for_stream(stream_id, config)
    │
    ├── Compare old vs new source configs
    │   │
    │   ├── No change? → Skip, log "no changes"
    │   │
    │   ├── MQTT source changed?
    │   │   ├── Drain in-flight messages
    │   │   ├── Disconnect cleanly
    │   │   ├── Connect with new config
    │   │   └── Resubscribe to topics
    │   │
    │   └── HTTP source changed?
    │       ├── Update URL/interval
    │       └── Continue polling with new config
    │
    └── Log reload result + emit metrics
```

### 6.2 MQTT Graceful Reconnection Sequence

```
┌──────────────┐    ┌──────────────┐    ┌──────────────┐
│   Publisher  │    │    Old       │    │    New       │
│              │    │   Broker     │    │   Broker     │
└──────┬───────┘    └──────┬───────┘    └──────┬───────┘
       │                   │                   │
       │ Publish msg       │                   │
       │──────────────────>│                   │
       │                   │                   │
       │            ┌──────┴───────┐           │
       │            │ MqttSource   │           │
       │            │ receives msg │           │
       │            └──────┬───────┘           │
       │                   │                   │
       │            [Reload triggered]         │
       │                   │                   │
       │            Drain queue               │
       │            ────────────              │
       │                   │                   │
       │            DISCONNECT                │
       │            ──────────>               │
       │                   │                   │
       │                   │    CONNECT        │
       │                   │    ──────────────>│
       │                   │                   │
       │                   │    SUBSCRIBE      │
       │                   │    ──────────────>│
       │                   │                   │
       │ Publish msg       │                   │
       │──────────────────────────────────────>│
       │                   │                   │
       │                   │           Message │
       │                   │    <──────────────│
```

---

## 7. Migration Transform Specification

### 7.1 v1.1 to v2.0 Transform (jq)

```bash
# Core transform: remove entity_schemas, bump version
jq '
  # Verify all fields have descriptions (fail if not)
  if (.fields | all(has("description"))) then
    # Remove entity_schemas, set version
    del(.entity_schemas) | .config_version = 2
  else
    error("Fields missing description - migration cannot proceed")
  end
'
```

### 7.2 Migration Validation Rules

Before migration:
1. **config_version == 1.1** or **config_version == 1** (allow both)
2. **All fields have description** (enriched in dp-018)
3. **entity_schemas exists** (otherwise nothing to migrate)

After migration:
1. **config_version == 2**
2. **entity_schemas does not exist**
3. **Validates against v2.0 schema**

### 7.3 Migration Error Messages

| Error | Cause | Message |
|-------|-------|---------|
| UNENRICHED_FIELD | Field lacks description | `Field '{name}' missing description. Run dp-018 enrichment first.` |
| ALREADY_V2 | Config is already v2.0 | `Config is already at v2.0. No migration needed.` |
| PARSE_ERROR | Invalid JSON | `Failed to parse config: {json_error}` |
| WRITE_ERROR | Cannot write file | `Cannot write to {path}: {os_error}` |

---

## 8. Release Workflow Specification

### 8.1 Release Artifacts

```
Git tag: v1.2.0
    │
    ├── .deploy/releases/v1.2.0.manifest.json  (what to deploy)
    │   └── Contains: {
    │         "version": "1.0",
    │         "release_version": "1.2.0",
    │         "description": "Add weather-station stream",
    │         "changes": [...]
    │       }
    │
    ├── config/base/streams/*/config.json      (configurations)
    │
    └── CHANGELOG.md entry                      (human description)
```

### 8.2 Release Checklist (Procedure)

```markdown
## Release Checklist

### Pre-Release
- [ ] All feature work complete and merged to main
- [ ] All tests passing (CI green)
- [ ] CHANGELOG.md updated with release notes
- [ ] Version number determined (MAJOR.MINOR.PATCH)

### Create Release
1. [ ] Create manifest: `.deploy/releases/vX.Y.Z.manifest.json`
2. [ ] Set `release_version` in manifest to `X.Y.Z`
3. [ ] List all changes in manifest
4. [ ] Commit: `git add .deploy/releases/vX.Y.Z.manifest.json && git commit -m "release: vX.Y.Z"`
5. [ ] Tag: `git tag -a vX.Y.Z -m "Release vX.Y.Z: <description>"`
6. [ ] Push: `git push origin main --tags`

### Deploy to Device
1. [ ] SSH to device
2. [ ] `cd /opt/ndp && git pull`
3. [ ] `./deploy.sh apply .deploy/releases/vX.Y.Z.manifest.json`
4. [ ] Verify: `./deploy.sh version`

### Post-Deploy
- [ ] Verify services running
- [ ] Check logs for errors
- [ ] Test key functionality
```

### 8.3 Webhook Deployment Flow (Future)

```
GitHub: Tag push (v*)
    │
    │ POST /webhook
    ▼
Pi Webhook Receiver (dp-023)
    │
    ├── Validate webhook signature
    ├── Extract tag version
    │
    ▼
Deployment Steps:
    │
    ├── 1. git fetch --tags
    ├── 2. git checkout {tag}
    ├── 3. Locate .deploy/releases/{tag}.manifest.json
    ├── 4. ./deploy.sh apply {manifest}
    └── 5. POST status back to GitHub
            │
            └── Status: success/failure
```

---

## 9. Testing Strategy

### 9.1 Unit Tests

| Component | Test Focus | Location |
|-----------|------------|----------|
| SourceManager.update_sources | Config comparison, source recreation | `core/src/sources/tests/` |
| MqttSource hot-reload | Graceful disconnect/reconnect | `core/src/sources/mqtt/tests/` |
| HttpSource hot-reload | Interval update, URL change | `core/src/sources/http/tests/` |
| Migration transform | jq correctness, edge cases | `scripts/tests/` |
| Version file writer | Atomic write, format | `apps/air-quality-app/tests/` |

### 9.2 Integration Tests

| Test ID | Scenario | Verification |
|---------|----------|--------------|
| INT-4.1 | MQTT topic hot-reload | Messages received on new topic |
| INT-4.2 | MQTT broker hot-reload | Connection switches brokers |
| INT-4.3 | HTTP interval change | Poll timing matches new interval |
| INT-4.4 | No message loss | Message count continuous during reload |
| INT-4.5 | Invalid config rejected | Old config continues working |
| INT-5.1 | v1.1 to v2.0 migration | All configs pass v2.0 validation |
| INT-5.2 | Dry-run accuracy | Dry-run matches actual migration |
| INT-5.3 | Batch migration | All streams migrated successfully |
| INT-R.1 | Release deployment | Version files updated correctly |
| INT-R.2 | Tag-manifest alignment | Validation passes for proper releases |

### 9.3 Test Infrastructure

```bash
# Start integration environment
./scripts/integration-test.sh start

# Run hot-reload tests
DEPLOY_ENV=integration ./scripts/test-hot-reload.sh

# Run migration tests
./scripts/test-migration.sh

# Test release workflow
./scripts/test-release.sh

# Clean up
./scripts/integration-test.sh clean
```

---

## 10. Validation Checklist

Before completing dp-021:

**Phase 4: Hot-Reload**:
- [ ] etcd watch wired to SourceManager
- [ ] SourceManager::update_sources_for_stream() implemented
- [ ] MQTT graceful reconnection with no message loss
- [ ] HTTP polling interval update immediate
- [ ] Reload endpoint functional (optional)
- [ ] Integration tests pass for hot-reload

**Phase 5: Schema Migration**:
- [ ] ndp-migrate-config.sh script created
- [ ] v2.0 JSON Schema created
- [ ] Migration removes entity_schemas correctly
- [ ] Dictionary loader updated for v2.0
- [ ] Dry-run mode works correctly
- [ ] Validator enforces v2.0 schema

**Phase R: Release Methodology**:
- [ ] RELEASE-POLICY.md documents versioning
- [ ] Manifest naming convention enforced
- [ ] Git tag alignment documented
- [ ] Device version tracking implemented
- [ ] WEBHOOK-DEPLOYMENT-SPEC.md created
- [ ] Release template created

**Documentation**:
- [ ] RELEASE-CHECKLIST.md created
- [ ] WEBHOOK-DEPLOYMENT-SPEC.md created
- [ ] deploy.sh help updated
- [ ] AgentDB pattern stored

---

## 11. Dependencies and Prerequisites

| Dependency | Type | Status | Notes |
|------------|------|--------|-------|
| dp-018: JSON Config Foundation | REQUIRED | Complete | JSON configs, v1.1 schema with enriched fields |
| dp-019: Config Validation Pipeline | REQUIRED | Complete | Validator binary, two-layer validation |
| dp-020: Declarative Deploy | REQUIRED | Complete | Manifest-driven deployment, device state |
| etcd | Runtime | Available | Config storage and watch |
| TimescaleDB | Runtime | Available | Silver layer database |

---

## 12. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Message loss during MQTT hot-reload | Medium | High | Drain queue before disconnect; integration tests |
| Migration corrupts configs | Low | High | Dry-run mode; backup before migration |
| Hot-reload edge cases cause instability | Medium | Medium | Scope limited to sources only |
| Version tracking files corrupted | Low | Low | Atomic writes via temp file |
| Release workflow adoption | Medium | Low | Clear documentation; checklist |

---

## 13. Success Metrics

| Metric | Current State | After dp-021 | Measurement |
|--------|---------------|--------------|-------------|
| Source config change downtime | Full restart required | <5 seconds | Measure reload time |
| MQTT message loss during reload | N/A (requires restart) | Zero | Integration test |
| Config schema version | v1.1 (transitional) | v2.0 (clean) | All configs validated |
| entity_schemas in configs | Present (deprecated) | Removed | Grep for "entity_schemas" |
| Device version queryable | No | Yes | `cat /var/ndp/deployed-version` |
| Release-manifest alignment | Informal | Enforced | Validation script |
| Webhook spec | None | Documented | Doc exists |

---

## 14. Glossary

| Term | Definition |
|------|------------|
| **Hot-Reload** | Updating configuration without restarting the application |
| **Source** | A data ingestion component (MQTT subscriber, HTTP poller) |
| **v1.1 Schema** | Transitional schema supporting both entity_schemas and enriched fields |
| **v2.0 Schema** | Clean schema with enriched fields only; entity_schemas forbidden |
| **entity_schemas** | Legacy section for field metadata; deprecated in v1.1, removed in v2.0 |
| **Enriched Fields** | Fields with description and device_class directly in `fields[]` array |
| **Release Manifest** | JSON file declaring all changes in a versioned release |
| **Deployed Version** | The release version currently running on a device |

---

## 15. References

| Document | Path | Relevance |
|----------|------|-----------|
| dp-021 SCOPE.md | `product/features/dp-021/SCOPE.md` | Feature scope definition |
| dp-016 IMPLEMENTATION-ROADMAP.md | `product/features/dp-016/IMPLEMENTATION-ROADMAP.md` | Phase 4, 5 details |
| dp-020 SPECIFICATION.md | `product/features/dp-020/specification/SPECIFICATION.md` | Declarative deploy patterns |
| dp-019 SPECIFICATION.md | `product/features/dp-019/specification/SPECIFICATION.md` | Validation pipeline |
| dp-018 ADR-018-001 | `product/features/dp-018/architecture/ADR-018-001-config-loader-design.md` | JSON pass-through architecture |

---

*Specification created: 2026-02-02*
*SPARC Phase: Specification (S)*
*Next Phase: Pseudocode (P)*
