# ops-003 Analysis: deploy.sh Binary Dispatch Audit

> **Date**: 2026-02-06
> **Purpose**: Map every `command -v` dispatch site in deploy.sh

---

## Current Binary Dispatch Sites

### `ndp` (3 sites)

| Line | Function | Command | Fallback |
|------|----------|---------|----------|
| ~386 | `sync_to_data_dictionary()` | `ndp dictionary sync --config-dir ... --db-url ...` | `_sync_to_data_dictionary_bash` |
| ~894 | `sync_domains_to_data_dictionary()` | `ndp domain sync --domains-dir ... --db-url ...` | Warns and skips |
| ~1063 | `load_dimension_data()` | `ndp dimension sync <id> --config ... --source ... --db-url ...` | `import_dimension_sql` |

### `ndp-validate` (2 sites)

| Line | Function | Command | Fallback |
|------|----------|---------|----------|
| ~1535 | (validation phase) | `ndp-validate --all --config-dir ...` | Warns and skips validation |
| ~2035 | (pre-deploy check) | `ndp-validate <config-path>` | Warns and skips |

### `ndp-gold-ddl` (2 sites)

| Line | Function | Command | Fallback |
|------|----------|---------|----------|
| ~1938 | `handle_gold_tables()` | `ndp-gold-ddl generate --stream <id> --config-dir ... --database-url ...` | Warns and skips |
| ~2071 | (gold domain sync) | `ndp-gold-ddl generate --domain <id> --config-dir ... --database-url ...` | Warns and skips |

---

## After ops-003 Consolidation

All 7 sites become `command -v ndp`:

| Line | Current | After ops-003 |
|------|---------|---------------|
| ~386 | `ndp dictionary sync ...` | Unchanged |
| ~894 | `ndp domain sync ...` | Unchanged |
| ~1063 | `ndp dimension sync ...` | Unchanged |
| ~1535 | `ndp-validate --all ...` | `ndp validate config --all ...` |
| ~2035 | `ndp-validate <path>` | `ndp validate config <path>` |
| ~1938 | `ndp-gold-ddl generate --stream ...` | `ndp gold generate --stream ...` |
| ~2071 | `ndp-gold-ddl generate --domain ...` | `ndp gold generate --domain ...` |

**Benefit**: Single `if command -v ndp` check at the top of deploy.sh, not 7 scattered checks for 3 different binaries.

---

## Flag Mapping: Standalone → Subcommand

### ndp-validate flags

| Standalone | Subcommand | Notes |
|-----------|------------|-------|
| `ndp-validate <path>` | `ndp validate config <path>` | Positional → positional |
| `ndp-validate --all` | `ndp validate config --all` | Same |
| `ndp-validate --domain <path>` | `ndp validate domain <path>` | Separate subcommand |
| `ndp-validate --domain-all` | `ndp validate domain --all` | Matches pattern |
| `ndp-validate --generate-schema` | `ndp validate schema --generate` | Separate subcommand |
| `ndp-validate --verify-schema <path>` | `ndp validate schema --verify <path>` | Same |
| `--config-dir` | `--config-dir` (global) | Already in ndp-cli |
| `--schema-path` | `--schema-path` | Forward to ndp-validate |
| `--format json` | `--format json` | Forward to ndp-validate |
| `--strict` | `--strict` | Forward to ndp-validate |

### ndp-gold-ddl flags

| Standalone | Subcommand | Notes |
|-----------|------------|-------|
| `ndp-gold-ddl generate --stream <id>` | `ndp gold generate --stream <id>` | Same |
| `ndp-gold-ddl generate --domain <id>` | `ndp gold generate --domain <id>` | Same |
| `ndp-gold-ddl generate --transitions` | `ndp gold generate --transitions` | Forward |
| `ndp-gold-ddl generate --events` | `ndp gold generate --events` | Forward |
| `ndp-gold-ddl validate --stream <id>` | `ndp gold validate --stream <id>` | Same |
| `ndp-gold-ddl validate --domain <id>` | `ndp gold validate --domain <id>` | Same |
| `--config-dir` | `--config-dir` (global) | Already in ndp-cli |
| `--database-url` | `--db-url` (global) | Already in ndp-cli (harmonize name) |
| `--action sync\|recreate` | `--action sync\|recreate` | Forward |
| `--verbose` | `--verbose` (global) | Add to ndp-cli |

### Flag name harmonization needed

| ndp-gold-ddl | ndp-cli | Recommendation |
|-------------|---------|----------------|
| `--database-url` | `--db-url` | Use `--db-url` (shorter, consistent) |
| `--db-timeout` | (none) | Add to ndp-cli global flags |
| `--verbose` | (none) | Add to ndp-cli global flags |
