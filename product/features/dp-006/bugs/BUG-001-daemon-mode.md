# BUG-001: Add Daemon Mode to Silver ETL

## Type
Enhancement

## Status
Open

## Priority
Medium

## Description

The silver-etl currently runs as a one-shot batch job. For consistency with other NDP services (like air-quality-app), it should support a daemon mode that:

1. Runs continuously as a long-lived process
2. Executes ETL on a configurable interval
3. Integrates with docker-compose as a regular service (not just `run --rm`)
4. Provides health checks and metrics

## Current Behavior

```bash
./deploy.sh silver-etl  # Runs once and exits
```

Must be scheduled externally via cron or systemd timer.

## Expected Behavior

```bash
# One-shot mode (existing)
silver-etl run --stream air-quality

# Daemon mode (new)
silver-etl daemon --interval 5m

# Or via environment variable
SILVER_ETL_MODE=daemon
SILVER_ETL_INTERVAL=5m
```

## Acceptance Criteria

- [ ] Add `daemon` subcommand to CLI
- [ ] Configurable interval via `--interval` flag or `SILVER_ETL_INTERVAL` env
- [ ] Graceful shutdown on SIGTERM/SIGINT
- [ ] Health check endpoint (optional, for docker healthcheck)
- [ ] Metrics for ETL runs (count, duration, errors)
- [ ] Update docker-compose to run as daemon by default
- [ ] Update deploy.sh to support both modes
- [ ] Tests for daemon loop logic

## Implementation Notes

- Follow air-quality-app pattern for daemon loop
- Use tokio interval for scheduling
- Ensure proper error handling (don't crash on single ETL failure)

## Related

- dp-006: Silver Layer ETL
- air-quality-app daemon pattern
