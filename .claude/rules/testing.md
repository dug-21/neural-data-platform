---
paths:
  - "tools/**/*.rs"
  - "crates/**/*.rs"
  - "core/**/*.rs"
  - "apps/**/*.rs"
  - "tests/**/*"
  - "product/features/**/refinement/**/*"
  - "product/features/**/completion/**/*"
---

# Testing and Integration Environment

## Integration Environment (USE IT)

A fully functioning integration stack exists. All SPARC Refinement and Completion phases MUST validate against it.

- **Config**: `docker-compose.integration.yml`
- **Switch**: `DEPLOY_ENV=integration` changes config paths and container names
- **Start**: `docker-compose -f docker-compose.integration.yml up -d`

### Services

| Service | Port | Purpose |
|---------|------|---------|
| etcd | 2379 | Configuration store |
| TimescaleDB | 5432 | Silver/Gold layer database |
| mosquitto | 1883 | MQTT broker |
| air-quality-app | 8080 | Domain application |
| MCP server | 9100 | Management Control Plane |
| Grafana | 3000 | Dashboards |

## When to use integration env

- All SPARC Refinement phases (TDD against live stack)
- All SPARC Completion phases (integration verification)
- Any schema change (verify DDL against TimescaleDB)
- Any ETL change (verify data flow end-to-end)
- Any config change that affects runtime behavior

## Testing Conventions

- London TDD style (mock-driven, outside-in)
- Tests live alongside source in standard Rust locations
- Integration tests use the integration environment
- `cargo test --workspace` runs all unit tests
- See AgentDB pattern ID 16 for London TDD details
