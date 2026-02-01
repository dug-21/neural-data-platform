# ADR-017-001: Integration Environment Design

**Status**: Accepted
**Date**: 2026-02-01
**Decision Makers**: Human + AI Architecture Review
**Feature**: dp-017 Integration Test Harness for Deployment Evolution

---

## Context

The Neural Data Platform deployment workflow relies on `deploy/pi/deploy.sh` as the single entry point for all deployment operations. This script already supports environment switching via `DEPLOY_ENV`:

```bash
DEPLOY_ENV=integration ./deploy.sh deploy   # Local integration testing
DEPLOY_ENV=pi ./deploy.sh deploy            # Production on Raspberry Pi
```

However, the integration environment (`docker-compose.integration.yml`) had drifted from production (`deploy/pi/docker-compose.yml`), making it impossible to validate deployment changes locally before deploying to the Pi.

**Key Problems**:

| Problem | Impact |
|---------|--------|
| Service mismatch | Integration had obsolete silver-etl-daemon, missing ndp-mcp-server |
| Init script paths | Volume mounts pointed to wrong locations |
| Config drift | Mosquitto config used deprecated options |
| No parity guarantee | Changes tested locally might fail in production |

**Existing Patterns** (from AgentDB):
- Pattern ID 19: Docker-Based Deployment Architecture - deploy.sh is primary entry point
- Pattern ID 9: GitOps Configuration Pattern - split between static (git) and dynamic (etcd)
- Pattern ID 72: ENV-controlled optional feature pattern

---

## Decision

**The integration environment mirrors production topology exactly. A single compose file change must maintain parity.**

### Core Principle: Topology Parity

Both environments must have the same:
1. **Service names** - `etcd`, `timescaledb`, `mosquitto`, `air-quality-app`, `ndp-mcp-server`, `grafana`
2. **Network architecture** - Internal service discovery via compose networking
3. **Volume mounts** - Same paths inside containers
4. **Dependency ordering** - Same health check chains
5. **Profile support** - Optional services use same profile names

### Environment Differences (Intentional)

| Aspect | Production (pi) | Integration | Rationale |
|--------|-----------------|-------------|-----------|
| Container names | `etcd`, `pi5-timescaledb` | `integration-*` | Avoid conflicts when both run |
| Ports | localhost-bound where secure | All exposed | Enable local testing tools |
| Network name | `neural-network` | `integration-network` | Isolation |
| Passwords | From secrets/env | Hardcoded defaults | Simplify dev setup |
| Resource limits | Enforced for Pi constraints | Relaxed | Dev machines have more resources |
| Restart policy | `unless-stopped` | None (default) | Faster iteration |
| Image tags | `:latest` | `:integration` | Distinguish builds |

### Container Naming Convention

Integration containers use `integration-*` prefix:

```
integration-etcd
integration-timescaledb
integration-mosquitto
integration-air-quality
integration-mcp-server
integration-grafana
```

This allows:
1. Clear identification in `docker ps`
2. Targeted cleanup with `docker rm integration-*`
3. Running both environments simultaneously (different ports)

### Compose File Relationship

```
docker-compose.integration.yml (root level)
    |
    +-- Mirrors services from:
    |       deploy/pi/docker-compose.yml
    |
    +-- Shares init scripts:
    |       deploy/pi/init-scripts/ (volume mount)
    |
    +-- Shares configuration:
            config/base/streams/
            config/grafana/
```

---

## Consequences

### Positive

1. **Safe testing** - Deploy changes validated locally before Pi deployment
2. **Single entry point** - `deploy.sh` works identically in both environments
3. **Namespace isolation** - Integration containers clearly identifiable
4. **CI-ready** - Integration compose file suitable for GitHub Actions
5. **Fast iteration** - Build once locally, deploy many times

### Negative

1. **Maintenance burden** - Two compose files to keep in sync
2. **Slight divergence** - Intentional differences (passwords, ports) add complexity
3. **No ARM testing** - Integration runs on x86_64, production on ARM64

### Neutral

1. **Shared init scripts** - Volume mount from deploy/pi/init-scripts means changes apply to both
2. **Profile parity** - Both use same profile names (`dashboards`, `silver`, etc.)

---

## Implementation Requirements

### Compose File Maintenance

When modifying production compose, also update integration:

1. Add new service? Add to both with appropriate container name prefix
2. Change volumes? Verify paths work from repo root (integration) and deploy/pi (production)
3. Add environment variable? Add to both with appropriate defaults

### Verification Protocol

After any compose change:

```bash
# Test integration environment
DEPLOY_ENV=integration ./deploy/pi/deploy.sh deploy
DEPLOY_ENV=integration ./deploy/pi/deploy.sh status
DEPLOY_ENV=integration ./deploy/pi/deploy.sh stop
```

### Parity Checklist

When reviewing compose changes, verify:

- [ ] Same service set (accounting for profile differences)
- [ ] Same health check definitions
- [ ] Same dependency chains
- [ ] Same volume mount semantics
- [ ] Same environment variable names (different defaults OK)

---

## Alternatives Considered

### Alternative 1: Single Compose File with Overrides

Use `docker-compose.yml` + `docker-compose.integration.yml` as override:

```bash
docker compose -f docker-compose.yml -f docker-compose.integration.yml up
```

**Rejected because**:
- Override semantics are confusing (merge vs replace)
- Production compose lives in `deploy/pi/`, not repo root
- Harder to reason about final configuration

### Alternative 2: Environment Variables Only

Single compose file with all differences controlled by env vars.

**Rejected because**:
- Container names would need templating (not supported)
- Would require extensive `${VAR:-default}` throughout
- Harder to read and maintain

### Alternative 3: Separate Test Infrastructure

Build dedicated test containers with mocked services.

**Rejected because**:
- Defeats purpose of testing real deployment
- More code to maintain
- Test environment wouldn't match production

---

## Related Decisions

- **ADR-016-001**: Config Source of Truth (JSON primary, etcd cache)
- **ADR-016-002**: Declarative Deploy Architecture (manifest-driven)
- **ADR-017-002**: Test Harness Strategy (how to validate deploy.sh commands)

---

## References

- `deploy/pi/deploy.sh` - Deployment entry point (supports DEPLOY_ENV switching)
- `deploy/pi/docker-compose.yml` - Production compose file
- `docker-compose.integration.yml` - Integration compose file (mirrors production)
- `product/features/dp-017/SCOPE.md` - Feature scope definition
