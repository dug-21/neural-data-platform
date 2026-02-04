# Configuration Directory Structure

This directory contains all configuration files for the Neural Data Platform.

## Environment-Based Configuration

Configuration is organized by environment:

| Environment | Path | Description |
|-------------|------|-------------|
| Production | `base/`, `domains/`, `grafana/` | Deployed to Raspberry Pi |
| Integration | `integration/base/`, `integration/domains/` | CI testing |
| Development | `development/base/`, `development/domains/` | Local dev (optional) |

**Default**: Production paths are used when `DEPLOY_ENV` is unset or set to `pi`.

## Directory Structure

```
config/
├── base/                    # Production streams (synced to Pi)
│   └── streams/
│       └── {stream-id}/
│           ├── config.yaml  # Human-readable config
│           └── config.json  # Machine-readable (synced to etcd)
├── domains/                 # Production Gold layer domains
│   └── {domain-id}/
│       └── domain.yaml
├── grafana/                 # Grafana provisioning (prod)
│   ├── dashboards/
│   └── datasources/
├── schemas/                 # Validation schemas (all environments)
│   └── stream-config.schema.json
├── samples/                 # Documentation examples only
├── integration/             # CI/Integration environment
│   ├── base/streams/        # Minimal test streams
│   └── domains/             # Test domains (if needed)
└── development/             # Local development (optional)
    ├── base/streams/
    └── domains/
```

## Sync Behavior

When running `deploy.sh sync`:

| DEPLOY_ENV | Streams Source | etcd Container |
|------------|----------------|----------------|
| `pi` (default) | `config/base/streams/` | `etcd` |
| `integration` | `config/integration/base/streams/` | `integration-etcd` |
| `development` | `config/development/base/streams/` | `development-etcd` |

**Fallback**: If environment-specific directory doesn't exist, falls back to production.

## Usage

```bash
# Production (default)
./deploy/pi/deploy.sh sync

# Integration testing
DEPLOY_ENV=integration ./deploy/pi/deploy.sh sync

# Local development
DEPLOY_ENV=development ./deploy/pi/deploy.sh sync
```

## Adding New Streams

1. Create stream directory: `config/base/streams/{stream-id}/`
2. Create `config.yaml` with stream definition
3. Generate JSON: `scripts/migrate-yaml-to-json.sh` or manual conversion
4. Sync to etcd: `./deploy/pi/deploy.sh sync`

For integration testing, create a simplified version in `config/integration/base/streams/`.
