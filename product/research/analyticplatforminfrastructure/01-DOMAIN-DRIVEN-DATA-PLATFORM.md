# Domain-Driven Data Platform Design

## Research Context

Date: 2026-01-01
Status: In Progress
Related Features: DP-004 (Bronze), DP-005 (Silver - future)

## The Universal Challenge

Every data platform faces the same challenge:

```
Raw API Response → ??? → Actionable Insights
                    ↑
              "The Hard Part"
```

The mistake is going straight from "I have this JSON" to "let me design a table" without understanding the domain.

## The Process

```
1. DOMAIN MODELING
   └── What entities exist in this domain?
   └── What relationships matter?
   └── What use cases will we answer?

2. SCHEMA DESIGN (from domain, not from API)
   └── Key dimensions that enable analytics
   └── Metrics that matter for use cases
   └── Joins that enable cross-entity analysis

3. ETL MAPPING (config-driven)
   └── Maps API structure → domain schema
   └── Extract-level DQ in stream config
   └── Transform-level DQ in silver config

4. LIFECYCLE-AWARE IMPLEMENTATION
   └── Early: Bronze + DuckDB exploration
   └── DevStage: Silver schema iteration
   └── Stable: Locked schemas, Gold layer
```

## Key Insight

The schema should be driven by:
- **Domain entities** (not API response structure)
- **Analytics use cases** (not storage convenience)
- **Relationships** (how entities connect for insights)

The API format is an implementation detail - it determines ETL mapping, not schema design.

## Related Documents

- [02-WEATHER-DOMAIN-MODEL.md](./02-WEATHER-DOMAIN-MODEL.md) - Weather domain specifics
- [03-DATA-PLATFORM-LIFECYCLE.md](./03-DATA-PLATFORM-LIFECYCLE.md) - Early/DevStage/Stable phases
- [04-LAYERED-DQ-STRATEGY.md](./04-LAYERED-DQ-STRATEGY.md) - Data quality approach
- [05-FORECAST-EVALUATION-SCHEMA.md](./05-FORECAST-EVALUATION-SCHEMA.md) - Forecast vs observation analysis
