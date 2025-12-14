# AIR-001 Memory Coordination Guide

## Overview
This document defines the ReasoningBank memory coordination strategy for the AIR-001 air quality data ingestion system. All swarm agents must use these memory keys to maintain consistency and share knowledge across the implementation.

## ReasoningBank Database
- **Location**: `/workspaces/neural-data-platform/.swarm/memory.db`
- **Mode**: ReasoningBank with local embeddings
- **Semantic Search**: Enabled (hash-based in NPX environment)

## Memory Key Structure

### Architecture Decisions
Keys storing core architectural choices that all agents must follow:

| Key | Value | Purpose | Agents Using |
|-----|-------|---------|--------------|
| `air001/arch/pattern` | `hexagonal-ports-adapters` | Primary architecture pattern (Hexagonal/Ports & Adapters) | All |
| `air001/arch/layers` | `core-domains-apps` | Layer structure (core/domains/apps separation) | Backend, Architect |

**Usage**:
```bash
npx claude-flow@alpha memory retrieve "air001/arch/pattern" --reasoningbank
```

### Implementation Phase Tracking
Keys tracking current implementation status:

| Key | Value | Purpose | Agents Using |
|-----|-------|---------|--------------|
| `air001/phase/current` | `phase-1-foundation` | Current implementation phase | Coordinator, Planner |
| `air001/phase/status` | `in-progress` | Phase completion status | All |

**Update Protocol**:
```bash
# When phase completes, update both:
npx claude-flow@alpha memory store "air001/phase/current" "phase-2-core-logic" --reasoningbank
npx claude-flow@alpha memory store "air001/phase/status" "completed" --reasoningbank
```

### Test-Driven Development Requirements
Keys defining TDD methodology and targets:

| Key | Value | Purpose | Agents Using |
|-----|-------|---------|--------------|
| `air001/tdd/methodology` | `london-school-mock-driven` | TDD approach (London School with mocks) | Tester, Coder |
| `air001/tdd/coverage-target` | `90-percent-minimum` | Minimum test coverage requirement | Tester, Reviewer |

**Testing Protocol**:
- All agents writing tests must retrieve `air001/tdd/methodology` first
- Coverage reports must verify against `air001/tdd/coverage-target`
- Use mocks/stubs for external dependencies (London School approach)

### Validation Rules
Keys defining data validation ranges and constraints:

| Key | Value | Purpose | Valid Range |
|-----|-------|---------|-------------|
| `air001/validation/co2-range` | `380-10000-ppm` | CO2 concentration limits | 380-10,000 ppm |
| `air001/validation/pm25-range` | `0-500-ugm3` | PM2.5 particle limits | 0-500 µg/m³ |
| `air001/validation/tvoc-range` | `1-500-index` | TVOC index limits | 1-500 index |
| `air001/validation/nox-range` | `1-500-index` | NOx index limits | 1-500 index |
| `air001/validation/temp-range` | `negative-10-to-50-celsius` | Temperature limits | -10 to 50°C |
| `air001/validation/humidity-range` | `0-100-percent` | Relative humidity limits | 0-100% |

**Validation Protocol**:
```bash
# Retrieve all validation rules before implementing validators:
npx claude-flow@alpha memory retrieve "air001/validation" --reasoningbank
```

**Implementing Validation**:
1. Retrieve validation range for metric
2. Implement boundary checks (min/max)
3. Handle edge cases (null, undefined, NaN)
4. Return descriptive error messages
5. Test boundary values (min-1, min, max, max+1)

### Storage Configuration
Keys defining data storage strategy:

| Key | Value | Purpose | Agents Using |
|-----|-------|---------|--------------|
| `air001/storage/format` | `parquet-snappy-compressed` | File format and compression | Backend, Data Engineer |
| `air001/storage/partitioning` | `daily-by-location` | Partitioning strategy | Backend, Data Engineer |

**Storage Implementation**:
```bash
# Retrieve storage config before implementing persistence:
npx claude-flow@alpha memory retrieve "air001/storage/format" --reasoningbank
npx claude-flow@alpha memory retrieve "air001/storage/partitioning" --reasoningbank
```

## Cross-Agent Communication Protocol

### 1. Agent Initialization
Every agent MUST retrieve relevant memory on startup:

```bash
# Retrieve all air001 memories:
npx claude-flow@alpha memory retrieve "air001" --reasoningbank

# Or retrieve specific category:
npx claude-flow@alpha memory retrieve "air001/arch" --reasoningbank
npx claude-flow@alpha memory retrieve "air001/validation" --reasoningbank
```

### 2. Decision Making
Before making architectural or implementation decisions:

1. Check if decision already exists in memory
2. If exists, follow stored decision
3. If new decision needed:
   - Discuss with coordinator
   - Store decision in appropriate namespace
   - Notify other agents

### 3. Progress Updates
Update phase status when completing major milestones:

```bash
# Example: Completing foundation phase
npx claude-flow@alpha memory store "air001/phase/current" "phase-2-core-logic" --reasoningbank
npx claude-flow@alpha memory store "air001/phase/status" "completed" --reasoningbank
npx claude-flow@alpha memory store "air001/progress/foundation" "completed-2025-12-13" --reasoningbank
```

### 4. Issue Tracking
Store encountered issues for other agents:

```bash
# Store blocking issue:
npx claude-flow@alpha memory store "air001/issues/validation-edge-case" "handling-null-pm25-readings" --reasoningbank

# Store resolution:
npx claude-flow@alpha memory store "air001/resolved/validation-edge-case" "return-error-for-null-readings" --reasoningbank
```

## Memory Retrieval Patterns

### Semantic Search
ReasoningBank supports semantic search with local embeddings:

```bash
# Find all validation-related memories:
npx claude-flow@alpha memory retrieve "air001/validation" --reasoningbank

# Find architecture decisions:
npx claude-flow@alpha memory retrieve "air001/arch" --reasoningbank

# Find all air001 memories:
npx claude-flow@alpha memory retrieve "air001" --reasoningbank
```

### Pattern Matching
Use hierarchical key structure for organized retrieval:

- `air001/*` - All project memories
- `air001/arch/*` - Architecture decisions
- `air001/validation/*` - Validation rules
- `air001/phase/*` - Phase tracking
- `air001/tdd/*` - Testing requirements
- `air001/storage/*` - Storage configuration

## Agent-Specific Responsibilities

### Backend-Dev Agent
- MUST retrieve: `air001/arch/*`, `air001/storage/*`, `air001/validation/*`
- MUST store: Implementation decisions, API contracts
- Updates: Progress on core/domains layers

### Tester Agent
- MUST retrieve: `air001/tdd/*`, `air001/validation/*`
- MUST store: Test coverage reports, failing test cases
- Updates: Coverage metrics, test suite status

### Reviewer Agent
- MUST retrieve: `air001/arch/*`, `air001/tdd/*`
- MUST store: Code review findings, quality metrics
- Updates: Review status, improvement suggestions

### Coordinator Agent
- MUST retrieve: `air001/phase/*`, all other namespaces
- MUST store: Phase transitions, agent assignments
- Updates: Overall project status, bottlenecks

## Best Practices

### 1. Memory Key Naming
- Use hierarchical structure: `project/category/subcategory/item`
- Use lowercase with hyphens: `air001/validation/co2-range`
- Be specific and descriptive: `phase-1-foundation` not `phase1`
- Include units in values: `380-10000-ppm` not `380-10000`

### 2. Value Format
- Use hyphens for ranges: `0-100-percent`
- Include units: `celsius`, `ppm`, `ugm3`
- Use descriptive names: `hexagonal-ports-adapters`
- Be consistent with format across related keys

### 3. Retrieval Efficiency
- Retrieve category prefixes to get multiple related keys
- Cache frequently accessed values locally during task
- Use semantic search for exploratory queries
- Use exact keys for known requirements

### 4. Update Synchronization
- Always update related keys together (e.g., phase/current + phase/status)
- Notify coordinator when making significant updates
- Document reason for changes in separate key if needed
- Keep audit trail for major decisions

### 5. Memory Cleanup
- Mark obsolete keys with `deprecated-` prefix
- Store resolution timestamp with completed tasks
- Archive old phase data before transitions
- Maintain history of major decisions

## Verification Commands

### Check All Stored Memories
```bash
npx claude-flow@alpha memory retrieve "air001" --reasoningbank
```

### Verify Specific Category
```bash
npx claude-flow@alpha memory retrieve "air001/validation" --reasoningbank
npx claude-flow@alpha memory retrieve "air001/arch" --reasoningbank
npx claude-flow@alpha memory retrieve "air001/tdd" --reasoningbank
```

### Database Status
The ReasoningBank database includes:
- 3 tables (memories, embeddings, metadata)
- Automatic migrations on first use
- SQLite-based persistent storage
- Local hash-based embeddings (NPX mode)

## Memory IDs Reference

All stored memories have unique IDs for tracking:

| Memory Key | Memory ID | Size |
|------------|-----------|------|
| `air001/arch/pattern` | 32fe95a4-3955-4b6d-90a2-63d884a1898f | 24 bytes |
| `air001/arch/layers` | ba928a92-dc6c-4439-88cc-1bdee78b1f17 | 17 bytes |
| `air001/phase/current` | 5ed3c091-31ec-47a5-9176-fb01ec729d03 | 18 bytes |
| `air001/phase/status` | a590c1c8-a128-4f16-933d-9c79b32de070 | 11 bytes |
| `air001/tdd/methodology` | 0f6b0635-8ef4-4350-8125-af108ccfcaec | 25 bytes |
| `air001/tdd/coverage-target` | cfbf348d-9302-491b-a2d1-43c8883fa4ee | 18 bytes |
| `air001/validation/co2-range` | 9c2a1be4-651f-4664-ae87-f46d2b47f959 | 13 bytes |
| `air001/validation/pm25-range` | 7d704ba7-b286-4d5f-9cd3-47b05882442d | 10 bytes |
| `air001/validation/tvoc-range` | 159b8546-5fb1-443b-8d4d-f075f6e0ae14 | 11 bytes |
| `air001/validation/nox-range` | 0eeb5d31-df35-43aa-9b53-e8adb7e352fe | 11 bytes |
| `air001/validation/temp-range` | 3fff6519-0ccc-4d95-b80c-73b3c0058ff8 | 25 bytes |
| `air001/validation/humidity-range` | f140bb7c-ede2-431a-86ae-46d9f5cb4cbe | 13 bytes |
| `air001/storage/format` | 903c0af4-f4c1-4560-bd0c-57776dcb9c97 | 25 bytes |
| `air001/storage/partitioning` | 6af3e89a-6ef9-43d1-9b1f-c78e1ad52ca6 | 17 bytes |

**Total Memories**: 14 keys stored
**Total Size**: 228 bytes
**Database**: /workspaces/neural-data-platform/.swarm/memory.db

## Troubleshooting

### Memory Not Found
```bash
# Verify key exists:
npx claude-flow@alpha memory retrieve "air001" --reasoningbank

# Check exact key name (case-sensitive):
npx claude-flow@alpha memory retrieve "air001/validation/co2-range" --reasoningbank
```

### Stale Data
```bash
# Update stale value:
npx claude-flow@alpha memory store "air001/phase/status" "completed" --reasoningbank

# Verify update:
npx claude-flow@alpha memory retrieve "air001/phase/status" --reasoningbank
```

### Semantic Search Not Working
In NPX environment, semantic search uses hash-based embeddings (limited semantic capability).
For better semantic search, install claude-flow globally:
```bash
npm install -g claude-flow
```

## Next Steps

1. All agents should retrieve their relevant memories on initialization
2. Coordinator should verify all agents have access to required keys
3. Implement progress tracking using phase keys
4. Store new decisions as they emerge during implementation
5. Maintain audit trail of major architectural choices

---

**Last Updated**: 2025-12-13
**Swarm**: air-001-implementation
**Memory Coordinator**: Active
**Database Status**: Operational (14 keys stored)
