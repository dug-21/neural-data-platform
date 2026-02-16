# Implementation Launch Prompt: ops-008

## Proposed Prompt

> Implement ops-008: Database Bootstrap & Init-Script Consolidation
> GitHub Issue: #22
> Brief: product/features/ops-008/IMPLEMENTATION-BRIEF.md
> Pattern IDs from planning: 3, 18, 21, 22, 25, 26, 27, 28, 29, 30, 31, 32
> Constraints: No Rust app changes, integration environment first, same scripts for Pi and integration, NNN-description.sql naming
> Wave structure: Wave 1 (9 new init-scripts, parallel) -> Wave 2 (delete old + migrations + deploy.sh) -> Wave 3 (integration testing)

## Reminders for User

- Review ALIGNMENT-REPORT.md for the WARN on domain-specific Silver functions (accepted)
- Verify acceptance criteria in SCOPE.md match ACCEPTANCE-MAP.md
- ops-007 (integration testbed) depends on ops-008 clean-slate capability

## Gotchas Discovered During Planning

- **C locale sort is the root cause**: `002_` sorts between `00-` and `01-` because underscore (0x5F) > all digits. The new `NNN-` convention avoids this entirely.
- **grafana_reader grant on silver schema**: Current `02-create-users.sql` grants on `silver` schema which did not exist at init time. In new scripts, `002-schemas.sql` creates `silver` before `004-roles.sql` grants on it. Order matters.
- **Default privileges timing**: `ALTER DEFAULT PRIVILEGES IN SCHEMA silver GRANT SELECT ON TABLES TO grafana_reader` must run BEFORE deploy.sh creates Silver tables, otherwise the auto-grant misses them. Placing it in `004-roles.sql` (init-scripts) ensures it runs before any table creation.
- **deploy.sh Phase 3 migrations**: Verify that deploy.sh has a mechanism to run SQL files from `deploy/pi/migrations/`. If not, one must be added. The existing `silver-migrate` command runs Silver-specific migrations; a general migrations directory may need to be introduced.
- **Dimension sync ensure_table**: Verify the Rust feature flag exists and what it takes to activate. If it does not exist as described in SCOPE.md, a small Rust change to make `ndp dimension sync` create tables idempotently will be needed.
- **analytics schema**: Must be created by init-scripts even though analytics views are created later by deploy.sh. The schema must exist for default privileges to be set.
- **No \echo in Docker init**: Docker entrypoint init-scripts should use `RAISE NOTICE` in DO blocks rather than `\echo` for output, since `\echo` is a psql meta-command that may not work in all execution contexts.
