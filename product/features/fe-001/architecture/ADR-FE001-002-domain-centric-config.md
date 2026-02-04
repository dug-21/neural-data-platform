# ADR-FE001-002: Domain-Centric Configuration

**Status**: Accepted
**Date**: 2026-02-04
**Decision Makers**: NDP Architecture Team
**Feature**: FE-001 Gold Layer Foundation
**Parent ADRs**: ADR-016-001 (Config Source of Truth), ADR-018-001 (Config Loader Design)

---

## Context

### The Problem

Gold layer introduces cross-stream concepts that don't fit in individual stream configs:

| Concept | Description | Scope |
|---------|-------------|-------|
| **Alignment** | Join multiple streams on time buckets | Cross-stream |
| **Objectives** | Targets to optimize (e.g., CO2 < 800 ppm) | Cross-stream |
| **Constraints** | Conditions that prevent action (e.g., outdoor AQI > 35) | Cross-stream |
| **Roles** | Which stream is primary, context, actuator | Cross-stream |

The question: Where should this cross-stream configuration live?

### Key Insight

Streams are **domain-agnostic building blocks**. The same stream (e.g., `outdoor-weather`) can serve multiple domains:
- Indoor Air Quality domain (weather affects ventilation decisions)
- Energy Efficiency domain (weather affects HVAC optimization)
- Plant Care domain (weather affects watering schedules)

**Domains are where intelligence and analytics happen.** Streams just provide data.

### Current Architecture

```
config/base/streams/
├── air-quality/config.json           # Per-stream config
├── outdoor-weather/config.json       # Per-stream config
└── home-assistant-state/config.json  # Per-stream config
```

Streams contain their own Silver/Gold ETL configs. But alignment, objectives, and roles span streams.

---

## Decision

**Cross-stream configuration lives in domain-centric files under `config/domains/`.**

### Directory Structure

```
config/
├── base/streams/                    # Data building blocks (domain-agnostic)
│   ├── air-quality/config.json      # Includes gold_etl section
│   ├── outdoor-weather/config.json
│   └── home-assistant-state/config.json
│
└── domains/                         # Intelligence contexts (NEW)
    ├── indoor-air-quality/
    │   └── domain.yaml              # Streams, alignment, objectives
    └── energy-efficiency/           # Future domain
        └── domain.yaml
```

### Domain Config Schema

```yaml
domain:
  id: indoor-air-quality
  description: "Maintain healthy indoor air quality"

  # Which streams this domain uses
  streams:
    - stream_id: air-quality
      alias: indoor
      role: primary           # What we're optimizing
    - stream_id: outdoor-weather
      alias: outdoor
      role: context           # Environmental context
    - stream_id: home-assistant-state
      alias: state
      role: actuator          # Potential causes/actions
    - stream_id: outdoor-air-quality
      alias: outdoor_aqi
      role: constraint        # Limiting conditions

  # Domain-specific aligned view
  alignment:
    view_name: indoor_air_quality_aligned
    granularity: "1 hour"
    join_strategy: full_outer
    null_handling: preserve

  # What we're trying to achieve
  objectives:
    - id: healthy_co2
      target:
        stream: air-quality
        metric: co2
        condition: "<"
        threshold: 800
        unit: ppm
      priority: high

    - id: healthy_pm25
      target:
        stream: air-quality
        metric: pm25
        condition: "<"
        threshold: 12
        unit: ug/m3
      priority: high

  # When NOT to take action
  constraints:
    - id: outdoor_air_safe
      description: "Don't open window if outdoor air is bad"
      stream: outdoor-air-quality
      metric: pm25
      condition: "<"
      threshold: 35
```

### Stream vs Domain Configuration

| Config Type | Location | Scope | Contains |
|-------------|----------|-------|----------|
| Stream Config | `config/base/streams/{id}/config.json` | Single stream | Fields, sources, silver_etl, gold_etl |
| Domain Config | `config/domains/{id}/domain.yaml` | Cross-stream | Streams, alignment, objectives, constraints |

### What Stays in Stream Config

Per-stream Gold ETL configuration stays in the stream config:

```json
{
  "stream_id": "air-quality",
  "gold_etl": {
    "enabled": true,
    "aggregates": {
      "granularities": ["1 hour", "1 day"],
      "fields": {
        "pm25": { "metrics": ["mean", "std", "min", "max", "p95"] },
        "co2": { "metrics": ["mean", "std", "min", "max"] }
      }
    },
    "features": {
      "lag": { "enabled": true, "lags_hours": [1, 6, 24], "fields": ["pm25", "co2"] },
      "rolling": { "enabled": true, "windows": ["4 hours"], "stats": ["mean", "std"], "fields": ["pm25"] }
    }
  }
}
```

**Rationale**: `gold_etl` references fields defined in the same config. Keeping them together enables atomic validation.

---

## Consequences

### Positive

1. **Stream Reusability** - Same stream serves multiple domains without modification
2. **Domain Self-Containment** - Everything needed for a domain's analytics is in one file
3. **Clear Mental Model** - "Streams provide data, domains provide intelligence"
4. **Flexibility Preserved** - Can create one super-wide domain if desired
5. **Easy Domain Addition** - Adding a new domain doesn't touch existing stream configs
6. **GitOps Friendly** - One file per domain, clear change tracking
7. **Matches Roadmap** - Aligns with FEATURE-ROADMAP.md mental model

### Negative

1. **Cross-File References** - Domain config references streams by ID; validation must check existence
2. **Two Config Locations** - Operators must understand streams vs domains distinction
3. **Sync Complexity** - May need separate `sync-domains-to-etcd.sh` script
4. **Discovery** - Finding all configs for a stream requires checking domains too

### Neutral

1. **etcd Key Structure** - Domains get their own key prefix: `/domains/{id}/config`
2. **Schema Files** - Need separate `domain.schema.json` for domain validation
3. **Data Dictionary** - Need `data_dictionary.domains` table for domain metadata

---

## Alternatives Considered

### Alternative 1: Platform-Wide Alignment Config

Single global file for all cross-stream configuration:

```yaml
# config/alignment.yaml (NOT CHOSEN)
alignments:
  - view_name: platform_wide_aligned
    streams: [air-quality, outdoor-weather, home-assistant-state, ...]
    granularity: "1 hour"
```

**Rejected because:**
- Forces all streams into single alignment strategy
- Cannot have different granularities for different use cases
- Becomes unwieldy as platform grows
- Cannot start simple and add domains incrementally
- **Design principle**: Designing for domain-centric allows platform-wide as special case; reverse is hard

### Alternative 2: Embed Alignment in Stream Config

Add cross-references directly in stream configs:

```json
{
  "stream_id": "air-quality",
  "alignment": {
    "join_with": ["outdoor-weather", "home-assistant-state"],
    "granularity": "1 hour"
  }
}
```

**Rejected because:**
- Circular references (air-quality references outdoor-weather, which references air-quality?)
- No clear owner for alignment settings
- Breaks single-file-per-stream atomic validation
- Objectives have no natural home

### Alternative 3: Separate Objectives File

Keep alignment in streams, put objectives in separate file:

```
config/
├── streams/...
└── objectives/
    └── indoor-air.yaml
```

**Rejected because:**
- Splits related concepts (alignment affects how objectives are evaluated)
- Objectives depend on alignment strategy (which stream is primary?)
- Two-step lookup for related information
- Domain as a concept is cleaner

---

## Implementation

### etcd Key Structure

```
/streams/{stream_id}/config   -> Stream config JSON (existing)
/domains/{domain_id}/config   -> Domain config JSON (NEW)
```

### Validation Rules (Layer 2 Semantic)

| Error Code | Rule | Message |
|------------|------|---------|
| 404 | `InvalidDomainStream` | Domain references stream `{stream_id}` which does not exist |
| 405 | `CircularDomainDependency` | Domain `{id}` references itself |
| 406 | `DuplicateStreamAlias` | Alias `{alias}` used for multiple streams in domain |
| 407 | `InvalidObjectiveStream` | Objective references stream `{stream_id}` not in domain's stream list |
| 408 | `InvalidConstraintStream` | Constraint references stream `{stream_id}` not in domain's stream list |

### Sync Script

New script follows existing pattern:

```bash
# scripts/sync-domains-to-etcd.sh
for domain_dir in config/domains/*/; do
    domain_id=$(basename "$domain_dir")
    config_file="$domain_dir/domain.yaml"

    if [ -f "$config_file" ]; then
        # Convert YAML to JSON, sync to etcd
        yq -o=json "$config_file" | \
            etcdctl put "/domains/$domain_id/config"
    fi
done
```

### Data Dictionary Integration

New tables for domain metadata:

```sql
CREATE TABLE data_dictionary.domains (
    domain_id TEXT PRIMARY KEY,
    description TEXT,
    stream_count INTEGER,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE data_dictionary.domain_streams (
    domain_id TEXT REFERENCES data_dictionary.domains(domain_id),
    stream_id TEXT REFERENCES data_dictionary.streams(stream_id),
    alias TEXT NOT NULL,
    role TEXT NOT NULL,  -- 'primary', 'context', 'actuator', 'constraint'
    PRIMARY KEY (domain_id, stream_id)
);

CREATE TABLE data_dictionary.objectives (
    objective_id TEXT PRIMARY KEY,
    domain_id TEXT REFERENCES data_dictionary.domains(domain_id),
    description TEXT,
    target_stream TEXT,
    target_metric TEXT,
    condition TEXT,
    threshold NUMERIC,
    priority TEXT DEFAULT 'medium',
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

---

## Related Decisions

- **Decision 6 (DECISIONS.md)**: Domain-Centric Configuration - source decision
- **Decision 7 (DECISIONS.md)**: Aligned Views Are Domain-Scoped - consequence of this ADR
- **ADR-016-001**: Config Source of Truth - JSON/YAML standards
- **ADR-018-001**: Config Loader Design - how configs are loaded

---

## References

- `/workspaces/neural-data-platform/product/features/fe-001/architecture/DECISIONS.md` - Source decision (Decision 6)
- `/workspaces/neural-data-platform/product/features/fe-001/architecture/CONFIG-DEPLOYMENT-FLOW.md` - Deployment integration
- `/workspaces/neural-data-platform/config/base/streams/` - Existing stream config pattern
- `/workspaces/neural-data-platform/scripts/sync-streams-to-etcd.sh` - Existing sync pattern

---

*Architecture decision created: 2026-02-04*
*Feature: FE-001 Gold Layer Foundation*
