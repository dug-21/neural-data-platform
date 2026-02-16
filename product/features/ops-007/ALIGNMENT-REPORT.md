# Alignment Report: ops-007

> Reviewed: 2026-02-16
> Artifacts: SPECIFICATION.md, TASK-DECOMPOSITION.md, ARCHITECTURE.md, PSEUDOCODE.md
> Vision Criteria: product/vision/ALIGNMENT-CRITERIA.md

## Summary

| Principle | Status | Notes |
|-----------|--------|-------|
| Edge-Only | PASS | All processing local, no cloud dependencies, shell scripts + Docker only |
| Config-Driven | PASS | Uses existing config hierarchy, fixes config path bug (ADR-007-003), no new hardcoded values |
| Domain-Portable | PASS | Testbed framework is domain-agnostic; fixtures are per-stream, not per-domain |
| Resource-Constrained | PASS | Shell scripts, no new binaries, no new memory-heavy dependencies |
| Integration-First | PASS | Extends deploy.sh (not replacing it), uses existing compose infrastructure |
| Privacy by Architecture | PASS | All data stays local, synthetic test data only |
| Self-Learning | N/A | Infrastructure/ops feature -- no learning pipeline interaction (intelligence daemon is tested but not modified) |

## Scope Alignment

| Type | Item | Details |
|------|------|---------|
| -- | -- | No gaps, additions, or simplifications detected |

All 5 workstreams from SCOPE.md are addressed in the specification. All 12 acceptance criteria have corresponding tasks in the decomposition. The wave sequencing matches SCOPE.md planning guidance exactly.

## Variances Requiring Approval

None. No VARIANCE or FAIL findings.

## Detailed Findings

### 1. Edge-Only
The testbed framework runs entirely within the local Docker environment. All tools used (mosquitto_pub, psql, etcdctl, docker) are already present in the integration stack. No external services are contacted. The deploy.sh fixes (etcd sync, Gold config path) operate on local infrastructure.

**Evidence**: PSEUDOCODE.md shows all operations use `docker exec` against local containers. No HTTP calls to external services.

### 2. Config-Driven
The specification explicitly avoids hardcoded values. Container names, database credentials, and connection details are read from compose configuration or environment variables (PSEUDOCODE.md Section 2: `TIMESCALE_CONTAINER`, `PG_USER`, etc. are variables). ADR-007-003 specifically fixes a hardcoded config path in deploy.sh, improving config-driven behavior.

MQTT injection parameters (topic, rate, count) are all configurable via command-line arguments (PSEUDOCODE.md Section 1). Testbed manifests are declarative JSON files.

**Evidence**: ADR-007-003 replaces `$REPO_ROOT/config/base` with `$(dirname "$CONFIG_STREAMS_DIR")`.

### 3. Domain-Portable
The testbed framework operates at the infrastructure level. Message templates are per-stream (not per-domain). The assertion library checks generic properties (row counts, key existence, service health). New domains add new stream configs and templates without modifying the framework.

**Evidence**: SPECIFICATION.md FR-03.1 defines templates tied to stream configs, not domain logic.

### 4. Resource-Constrained
No new compiled binaries. Shell scripts have negligible memory footprint. The stress testbed (ADR-007-005) monitors RSS to ensure containers stay within bounds -- this directly validates the resource constraint.

No banned dependencies (DuckDB, Polars) are introduced. All tools are standard Linux utilities (bash, jq, bc, sed, grep) already present on Pi.

**Evidence**: SPECIFICATION.md NFR-05: "No dependencies outside docker, bash, jq, mosquitto_pub."

### 5. Integration-First
The design extends existing infrastructure rather than creating parallel systems:
- deploy.sh is modified (2 surgical fixes), not replaced
- docker-compose.integration.yml is extended via compose overrides, not duplicated
- Manifest format matches production release manifests
- Config sync uses existing `deploy.sh sync` and `deploy.sh apply` commands

ADR-007-006 specifically ensures testbed manifests use the production manifest format and deploy path.

**Evidence**: ADR-007-001 dispatch-and-compose pattern builds on existing deploy.sh commands.

### 6. Privacy by Architecture
All test data is synthetic (generated from templates with random values). No real sensor data is used. All processing is local. No telemetry or external reporting.

**Evidence**: PSEUDOCODE.md Section 3: `randomize_message()` generates synthetic values from templates.

### 7. Self-Learning
N/A -- This is an infrastructure/ops feature. The intelligence daemon is tested (verified to start, connect, read config) but not modified. No changes to the learning pipeline.

## Technical Constraints Check

| Constraint | Status | Evidence |
|------------|--------|----------|
| ARM64 compatible | PASS | Shell scripts are architecture-independent; all Docker images already support ARM64 |
| No banned deps | PASS | No DuckDB, no Polars; only bash, jq, bc, mosquitto_pub |
| TimescaleDB (not DuckDB) | PASS | Silver assertions query TimescaleDB directly |
| Config-driven (not hardcoded) | PASS | ADR-007-003 fixes the one remaining hardcoded path |
| Version target correct | PASS | ops-007 is operations tooling, version-independent |
