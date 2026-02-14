---
paths:
  - "product/features/**/*"
  - ".claude/agents/**/*"
---

# Agent Routing and Team Formation

## NDP Agent Preference (always use NDP-specific agents)

| Instead of | Use | Why |
|------------|-----|-----|
| `coder` | `ndp-rust-dev` | Knows Rust patterns, project structure |
| `system-architect` | `ndp-architect` | Knows Domain Adapter pattern, ADRs |
| `tester` | `ndp-tester` | Knows test patterns, mocking approach |
| `planner` | `ndp-scrum-master` | Knows feature lifecycle, SPARC phases |

## Task Type Routing

| Code | Task | Agents | Topology |
|------|------|--------|----------|
| 1 | Bug Fix | ndp-scrum-master, researcher, ndp-rust-dev, ndp-tester | hierarchical |
| 3 | Feature | ndp-scrum-master, ndp-architect, ndp-rust-dev, ndp-tester, reviewer | hierarchical |
| 5 | Refactor | ndp-scrum-master, ndp-architect, ndp-rust-dev, reviewer | hierarchical |
| 7 | Performance | ndp-scrum-master, perf-engineer, ndp-rust-dev | hierarchical |
| 9 | Security | ndp-scrum-master, security-architect, auditor | hierarchical |
| 11 | Docs | researcher, api-docs | mesh |

## Initiative-Based Team Formation

| Initiative | Core Team | Domain Specialists |
|------------|-----------|-------------------|
| Schema/ETL | ndp-architect, ndp-timescale-dev, ndp-dq-engineer | ndp-meteorologist or ndp-air-quality-specialist |
| Analytics/Dashboards | ndp-analytics-engineer, ndp-grafana-dev | Domain specialist for metrics |
| New Data Source | ndp-architect, ndp-rust-dev, ndp-parquet-dev | Domain specialist for validation |
| ML/Predictions | ndp-feature-engineer, ndp-ml-engineer | Domain specialist for feature logic |
| Alerts/Triggers | ndp-alert-engineer, ndp-rust-dev | ndp-air-quality-specialist for thresholds |

## Team Formation Rules

1. **Always include domain specialist** when working with data domain areas
2. **Always include ndp-dq-engineer** when schema or ETL changes affect data quality
3. **Always include ndp-architect** for cross-cutting or schema changes
4. **Consult domain specialists first** before implementing domain logic in code
