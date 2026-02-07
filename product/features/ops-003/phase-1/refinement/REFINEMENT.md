# OPS-003 Phase 1 Refinement: v1.1.14 Gold Migration

> **Feature**: ops-003 (Unified Action Library)
> **Release**: v1.1.14
> **Date**: 2026-02-07
> **Status**: Refinement
> **Patterns Used**: ID 19 (release-workflow), ID 35 (integration-e2e-testing), ID 18 (unified-validation-types-migration), ID 25 (ops-002-completion)

---

## 1. Risk Mitigation Plan

### Risk 1: Gold Test Breakage (376 tests)

**Likelihood**: Low | **Impact**: High

**Root Cause**: Moving source files from `tools/ndp-gold-ddl/src/` to `crates/ndp-lib/src/gold/` changes `use` paths in every file and every test.

**Specific Trait Method Differences (DbClient)**:

| Method | ndp-lib (`crates/ndp-lib/src/db.rs`) | ndp-gold-ddl (`tools/ndp-gold-ddl/src/db/client.rs`) |
|--------|--------------------------------------|-------------------------------------------------------|
| `query()` | Returns `Result<Vec<Row>>` (NdpLibError) | Returns `Result<Vec<Row>, DbError>` |
| `execute()` | Returns `Result<u64>` (NdpLibError) | **Not defined** |
| `batch_execute()` | Returns `Result<()>` (NdpLibError) | **Not defined** |
| Error type | `NdpLibError::Database(String)` | `DbError` enum (ConnectionFailed, QueryFailed, InvalidUrl, Timeout) |

**Resolution**: The gold module code (CaChecker, SyncPlanner) only calls `query()`. The signature is compatible -- both take `&str` + `&[&(dyn ToSql + Sync)]` and return `Vec<Row>`. The difference is the error type. Two approaches:

1. **Preferred**: Make gold module code use `ndp_lib::DbClient` directly. The `CaChecker` trait and `PostgresCaChecker<C: DbClient>` become generic over `ndp_lib::DbClient` instead of `ndp_gold_ddl::db::DbClient`. The mock tests in planner already use a custom `MockCaChecker` that does not implement DbClient at all, so they are unaffected.

2. **Fallback**: Keep gold's `DbError` as a separate type and add a `From<DbError> for NdpLibError` impl. This is more surgical but leaves two error types.

**Rollback Procedure**:
```bash
# If gold tests fail after move, revert the move:
git stash  # or git checkout -- crates/ndp-lib/src/gold/
# Gold tests still pass at their original location:
cargo test -p ndp-gold-ddl
```

**Mitigation Steps**:
1. Move source files FIRST, fix `use` paths, run `cargo test -p ndp-lib` after each file group
2. Move tests SECOND (unit tests embedded in source move automatically; integration tests in `tools/ndp-gold-ddl/tests/` move to `crates/ndp-lib/tests/gold/`)
3. Run `cargo test -p ndp-lib` continuously during migration
4. Do NOT delete original files until all 376 tests pass in new location
5. Keep `tools/ndp-gold-ddl/` buildable by making it re-export from ndp-lib

### Risk 2: deploy.sh Regression

**Likelihood**: Medium | **Impact**: High (production deployment)

**Root Cause**: Two dispatch sites change from `ndp-gold-ddl` to `ndp gold`. Flag names change (`--database-url` to `--db-url`). The new code does `error + return 1` instead of `warn + return 0`, so a missing binary now halts deployment.

**Exact Test Sequence Before Committing** (see Section 3 for full protocol):

```bash
# 1. Build the new ndp binary
cargo build -p ndp-cli

# 2. Verify ndp gold commands produce identical output
diff <(./target/debug/ndp-gold-ddl --config-dir config/base generate --stream air-quality) \
     <(./target/debug/ndp gold generate --stream air-quality --config-dir config/base)

# 3. Verify with --domain flag
diff <(./target/debug/ndp-gold-ddl --config-dir config/base generate --domain indoor-air-quality) \
     <(./target/debug/ndp gold generate --domain indoor-air-quality --config-dir config/base)

# 4. Verify --events flag
diff <(./target/debug/ndp-gold-ddl --config-dir config/base generate --domain indoor-air-quality --events) \
     <(./target/debug/ndp gold generate --domain indoor-air-quality --events --config-dir config/base)

# 5. Dry-run deploy.sh with modified dispatch
DEPLOY_ENV=integration DRY_RUN=true ./deploy.sh apply .deploy/releases/v1.1.14.manifest.json

# 6. Full integration deploy
docker compose -f docker-compose.integration.yml up -d
DEPLOY_ENV=integration ./deploy.sh apply .deploy/releases/v1.1.14.manifest.json
```

**Rollback Procedure**:
```bash
# Revert deploy.sh to v1.1.13 state:
git checkout v1.1.13 -- deploy/pi/deploy.sh
# Standalone ndp-gold-ddl still exists and works:
cargo build -p ndp-gold-ddl --release
```

### Risk 3: DbClient Incompatibility

**Detailed Analysis**:

The `CaChecker` trait (`tools/ndp-gold-ddl/src/db/queries.rs`) is parameterized on `C: DbClient`:
```rust
pub struct PostgresCaChecker<C: DbClient> {
    client: C,
}
```

Where `DbClient` currently means `ndp_gold_ddl::db::DbClient` (query-only). After migration, `DbClient` will mean `ndp_lib::DbClient` (query + execute + batch_execute).

**This is ADDITIVE and SAFE**: `PostgresCaChecker` only calls `self.client.query()`. The additional methods (`execute`, `batch_execute`) on `ndp_lib::DbClient` are unused by CaChecker code. Any type implementing `ndp_lib::DbClient` also satisfies CaChecker's requirements.

**The mock tests use `MockCaChecker`** (a direct mock of the `CaChecker` trait), NOT `MockDbClient`. These tests do not construct a `PostgresCaChecker` at all, so the DbClient trait change is invisible to them.

**Resolution**: Replace `use crate::db::DbClient` with `use ndp_lib::DbClient` in the gold module. Delete `tools/ndp-gold-ddl/src/db/client.rs` (the local DbClient) and re-export from ndp-lib instead.

### Risk 4: Config Type Conflicts

**Which types are Gold-specific vs shared**:

| Type | Location | Gold-specific? | Action in v1.1.14 |
|------|----------|---------------|-------------------|
| `StreamConfig` (gold) | `tools/ndp-gold-ddl/src/config/types.rs` | YES -- has `gold_etl`, `transitions`, etc. | Move to `ndp_lib::gold::config` |
| `GoldEtlConfig` | `tools/ndp-gold-ddl/src/config/types.rs` | YES | Move to `ndp_lib::gold::config` |
| `DomainConfig` (gold) | `tools/ndp-gold-ddl/src/config/domain.rs` | YES -- has `alignment`, `objectives`, `events` | Move to `ndp_lib::gold::config` |
| `Action` enum | `tools/ndp-gold-ddl/src/config/types.rs` | YES (Sync/Recreate) | Move to `ndp_lib::gold::config` |
| `StreamConfig` (lib) | `crates/ndp-lib/src/config.rs` | NO -- sync-focused | DO NOT TOUCH |
| `DomainConfig` (lib) | `crates/ndp-lib/src/config.rs` | NO -- sync-focused | DO NOT TOUCH |
| `StreamConfig` (core) | `core/src/config.rs` | NO -- runtime ingestion | DO NOT TOUCH |

**There is NO conflict**: Gold types move into `ndp_lib::gold::config`, a separate namespace from `ndp_lib::config`. Consumers must use the fully qualified path. The two `StreamConfig` types coexist under different modules:
- `ndp_lib::config::StreamConfig` (sync operations)
- `ndp_lib::gold::config::StreamConfig` (Gold DDL generation)

This is acceptable in v1.1.14. Unification is explicitly deferred to V1.3.

---

## 2. Migration Order (Critical Path)

### Phase Diagram

```
                    +------------------+
                    | CHECKPOINT 0     |
                    | All 376 tests    |
                    | pass at origin   |
                    +--------+---------+
                             |
              +--------------+--------------+
              |                             |
    +---------v---------+         +---------v---------+
    | A. Create gold    |         | B. Add deps to    |
    | module structure  |         | ndp-lib Cargo.toml|
    | in ndp-lib        |         | (mockall, pretty  |
    | (empty mod.rs)    |         | assertions)       |
    +---------+---------+         +---------+---------+
              |                             |
              +--------------+--------------+
                             |
                    +--------v---------+
                    | C. Move source   |
                    | files in order:  |
                    | 1. error.rs      |
                    | 2. config/       |
                    | 3. db/ (queries) |
                    | 4. registry/     |
                    | 5. validation/   |
                    | 6. generators/   |
                    | 7. planner/      |
                    +--------+---------+
                             |
                    +--------v---------+
                    | CHECKPOINT 1     |
                    | cargo test       |
                    | -p ndp-lib       |
                    | (376 gold tests) |
                    +--------+---------+
                             |
                    +--------v---------+
                    | D. Move          |
                    | integration      |
                    | tests to         |
                    | ndp-lib/tests/   |
                    +--------+---------+
                             |
                    +--------v---------+
                    | CHECKPOINT 2     |
                    | All tests pass   |
                    | (unit + integ)   |
                    +--------+---------+
                             |
              +--------------+--------------+
              |                             |
    +---------v---------+         +---------v---------+
    | E. Update ndp-    |         | F. Add commands/  |
    | gold-ddl to thin  |         | gold.rs to        |
    | wrapper           |         | ndp-cli           |
    +---------+---------+         +---------+---------+
              |                             |
              +--------------+--------------+
                             |
                    +--------v---------+
                    | CHECKPOINT 3     |
                    | ndp gold output  |
                    | == ndp-gold-ddl  |
                    | output           |
                    +--------+---------+
                             |
                    +--------v---------+   <-- POINT OF NO RETURN
                    | G. Update        |       (deploy.sh changes)
                    | deploy.sh        |
                    | (2 dispatch      |
                    | sites)           |
                    +--------+---------+
                             |
                    +--------v---------+
                    | CHECKPOINT 4     |
                    | Integration      |
                    | deploy succeeds  |
                    +--------+---------+
                             |
                    +--------v---------+
                    | H. Release       |
                    | v1.1.14          |
                    +--------+---------+
```

### What Can Be Done in Parallel

- **A + B**: Creating module structure and adding Cargo.toml dependencies are independent.
- **E + F**: Updating ndp-gold-ddl as thin wrapper and adding ndp-cli gold commands are independent (both depend on Checkpoint 2).

### What Has Dependencies

- **C depends on A + B**: Cannot move source files until module structure and dependencies exist.
- **D depends on C**: Integration tests reference source modules.
- **E, F depend on D**: Thin wrapper and CLI commands must reference the migrated code.
- **G depends on E + F**: deploy.sh must not switch until both ndp-gold-ddl wrapper and ndp gold commands work.

### Point of No Return

**Step G (deploy.sh modification)** is the point of no return. Before G, the original ndp-gold-ddl binary and deploy.sh are untouched. After G, deploy.sh expects `ndp gold` to exist.

However, G is easily revertable: a single `git checkout` of deploy.sh restores the old dispatch. The true point of no return is the release tag + push.

### Checkpoints

| Checkpoint | Gate | Command | Expected |
|-----------|------|---------|----------|
| 0 | Baseline | `cargo test -p ndp-gold-ddl` | 376 tests pass |
| 1 | Source moved | `cargo test -p ndp-lib` | 376+ tests pass (gold + existing) |
| 2 | Integration tests moved | `cargo test -p ndp-lib` | All tests pass |
| 3 | CLI parity | `diff <(ndp-gold-ddl generate --stream air-quality) <(ndp gold generate --stream air-quality)` | Empty diff |
| 4 | Integration deploy | `DEPLOY_ENV=integration ./deploy.sh apply <manifest>` | Exit 0, Gold phases succeed |

---

## 3. deploy.sh Safety Protocol

### How to Test deploy.sh Changes Without Affecting Production

1. **Production deploy.sh is on the Pi device**, accessed via `git pull`. Changes exist only in the repo until pulled.
2. **Integration environment is isolated**: `docker-compose.integration.yml` uses separate container names and ports.
3. **`DEPLOY_ENV=integration` switch**: Changes config paths, database URLs, and container prefixes. No overlap with production.
4. **`DRY_RUN=true` mode**: deploy.sh prints what it would do without executing. Test dispatch logic without side effects.

### Integration Environment Verification Steps

```bash
# Step 1: Start integration stack
docker compose -f docker-compose.integration.yml up -d

# Step 2: Wait for TimescaleDB
docker compose -f docker-compose.integration.yml exec timescaledb \
  pg_isready -U postgres -d ndp

# Step 3: Build ndp binary with gold module
cargo build -p ndp-cli

# Step 4: Verify ndp binary is in PATH or target/debug/
ls -la target/debug/ndp

# Step 5: Dry-run deploy
DEPLOY_ENV=integration DRY_RUN=true \
  ./deploy.sh apply .deploy/releases/v1.1.14.manifest.json

# Step 6: Full integration deploy
DEPLOY_ENV=integration \
  ./deploy.sh apply .deploy/releases/v1.1.14.manifest.json

# Step 7: Verify Gold tables exist in integration DB
docker compose -f docker-compose.integration.yml exec timescaledb \
  psql -U postgres -d ndp -c \
  "SELECT view_schema, view_name FROM timescaledb_information.continuous_aggregates WHERE view_schema = 'gold'"

# Step 8: Tear down
docker compose -f docker-compose.integration.yml down
```

### Exact Commands to Verify Each Dispatch Site

**Site 1: `handle_gold_tables()` (line ~1936)**

This site handles per-stream Gold DDL with database connectivity.

```bash
# Before (standalone):
./target/debug/ndp-gold-ddl --config-dir config/base \
  --database-url "postgresql://postgres:ndp_secure_password@localhost:5432/ndp" \
  --db-timeout 10 \
  generate --stream air-quality --action sync

# After (subcommand):
./target/debug/ndp gold sync --stream air-quality \
  --config-dir config/base \
  --db-url "postgresql://postgres:ndp_secure_password@localhost:5432/ndp" \
  --db-timeout 10

# Parity check:
diff <(./target/debug/ndp-gold-ddl --config-dir config/base generate --stream air-quality) \
     <(./target/debug/ndp gold generate --stream air-quality --config-dir config/base)
```

**Site 2: `handle_domain()` gold dispatch (line ~2069)**

This site handles per-domain aligned view DDL (no database URL needed for domain generation).

```bash
# Before (standalone):
./target/debug/ndp-gold-ddl --config-dir config/base \
  generate --domain indoor-air-quality --action sync

# After (subcommand):
./target/debug/ndp gold generate --domain indoor-air-quality \
  --config-dir config/base

# Before (events):
./target/debug/ndp-gold-ddl --config-dir config/base \
  generate --domain indoor-air-quality --events --action sync

# After (events):
./target/debug/ndp gold generate --domain indoor-air-quality \
  --events --config-dir config/base

# Parity checks:
diff <(./target/debug/ndp-gold-ddl --config-dir config/base generate --domain indoor-air-quality) \
     <(./target/debug/ndp gold generate --domain indoor-air-quality --config-dir config/base)

diff <(./target/debug/ndp-gold-ddl --config-dir config/base generate --domain indoor-air-quality --events) \
     <(./target/debug/ndp gold generate --domain indoor-air-quality --events --config-dir config/base)
```

### Rollback Procedure if deploy.sh Breaks

```bash
# Immediate: revert deploy.sh only
git checkout HEAD~1 -- deploy/pi/deploy.sh
# Verify standalone binary still works
cargo build -p ndp-gold-ddl
./target/debug/ndp-gold-ddl --config-dir config/base generate --stream air-quality
# Recommit with fixed deploy.sh
```

If the issue is discovered AFTER the release tag:
```bash
# Revert to v1.1.13
git checkout v1.1.13 -- deploy/pi/deploy.sh
git commit -m "revert: deploy.sh Gold dispatch back to ndp-gold-ddl (v1.1.14 regression)"
# Do NOT retag v1.1.14 -- create v1.1.14.1 or bump to v1.1.15
```

---

## 4. Release Checklist (v1.1.14)

### Pre-Release Verification

- [ ] `cargo test -p ndp-gold-ddl` -- 376 tests pass (baseline, before any changes)
- [ ] `cargo test -p ndp-lib` -- gold module tests pass in new location
- [ ] `cargo test --workspace` -- all workspace tests pass
- [ ] ndp gold generate output matches ndp-gold-ddl output (all streams)
- [ ] ndp gold generate output matches ndp-gold-ddl output (all domains)
- [ ] ndp gold generate output matches ndp-gold-ddl output (events)
- [ ] ndp gold generate output matches ndp-gold-ddl output (transitions)
- [ ] deploy.sh dry-run succeeds with `DEPLOY_ENV=integration DRY_RUN=true`
- [ ] Integration deploy succeeds: `DEPLOY_ENV=integration ./deploy.sh apply <manifest>`
- [ ] Gold tables verified in integration DB after deploy
- [ ] No calls to `ndp-gold-ddl` remain in deploy.sh (grep verification)
- [ ] `git status` is clean
- [ ] On `main` branch

### Manifest File

Location: `.deploy/releases/v1.1.14.manifest.json`

```json
{
  "$schema": "../schemas/manifest.schema.json",
  "version": "1.0",
  "release_version": "1.1.14",
  "description": "Release v1.1.14: Gold DDL generation consolidated into ndp-lib and ndp CLI (ops-003 Phase 1)",
  "changes": [
    {
      "type": "tool",
      "id": "ndp-cli",
      "action": "build",
      "profile": "release"
    },
    {
      "type": "gold-tables",
      "stream_id": "air-quality",
      "action": "sync"
    },
    {
      "type": "domain",
      "domain_id": "indoor-air-quality",
      "action": "sync"
    }
  ]
}
```

Note: The manifest includes `gold-tables` and `domain` declarations to verify the new dispatch path end-to-end during deployment. The `tool` declaration builds `ndp-cli` (not `ndp-gold-ddl`, which is no longer the deployment tool).

### CHANGELOG Entry

```markdown
## [1.1.14] - 2026-02-XX

Gold DDL generation consolidated into ndp-lib and ndp CLI (ops-003 Phase 1).

### Changed

- **Gold module migrated to ndp-lib** -- 29 source files and 376 tests moved from `tools/ndp-gold-ddl/src/` to `crates/ndp-lib/src/gold/`
- **`ndp gold` subcommands** -- `ndp gold generate`, `ndp gold sync`, `ndp gold recreate` replace standalone `ndp-gold-ddl` binary
- **deploy.sh Gold dispatch** -- 2 dispatch sites switched from `command -v ndp-gold-ddl` to `command -v ndp`
  - `handle_gold_tables()`: now calls `ndp gold sync --stream <id> --db-url <url>`
  - `handle_domain()` gold part: now calls `ndp gold generate --domain <id>`
- **No-fallback policy** -- deploy.sh now errors (`return 1`) instead of warning and skipping when `ndp` is not found
- **Flag harmonization** -- `--database-url` renamed to `--db-url` in deploy.sh Gold calls (matches ndp-cli convention)

### Added

- `crates/ndp-lib/src/gold/` module with full Gold DDL generation capability
- `tools/ndp-cli/src/commands/gold.rs` -- CLI routing for gold subcommands
- `--db-timeout` global flag in ndp-cli

### Technical Notes

- 376 Gold tests migrated, all passing under `cargo test -p ndp-lib`
- ndp-gold-ddl standalone remains buildable as a thin wrapper over ndp-lib
- Integration verified: `DEPLOY_ENV=integration ./deploy.sh apply v1.1.14.manifest.json`
```

### Git Tag

```bash
git tag -a v1.1.14 -m "Release v1.1.14: Gold DDL consolidated into ndp-lib and ndp CLI (ops-003)"
```

### Deploy Verification on Integration Env

```bash
# On integration environment:
cargo build -p ndp-cli --release
DEPLOY_ENV=integration ./deploy.sh apply .deploy/releases/v1.1.14.manifest.json

# Verify:
# 1. Phase 2.5 builds ndp-cli (tool build)
# 2. Phase 5 generates Gold tables via ndp gold sync
# 3. Phase 6 generates domain views via ndp gold generate --domain
# 4. Phase 11 updates device state
```

---

## 5. Edge Cases

### Edge Case 1: ndp-gold-ddl standalone still in PATH when deploy.sh calls `ndp gold`

**Scenario**: After v1.1.14, both `ndp` and `ndp-gold-ddl` binaries exist on the Pi. deploy.sh only calls `ndp`. If someone manually runs `ndp-gold-ddl`, it still works because ndp-gold-ddl is now a thin wrapper around ndp-lib.

**Risk**: None. deploy.sh uses `command -v ndp` which resolves to the `ndp` binary. The `ndp-gold-ddl` binary is ignored by deploy.sh but remains functional for manual use.

**However**: If a cron job or external script still calls `ndp-gold-ddl` directly, that script is unaffected -- ndp-gold-ddl still works (thin wrapper). No action needed.

### Edge Case 2: `$action` variable is something other than "sync" or "recreate"

**Current behavior in deploy.sh**: The `$action` value comes from `jq -r '.action // "sync"'` on the manifest declaration. It defaults to `"sync"` if not specified. The manifest schema only allows `"sync"` or `"recreate"`.

**ndp-gold-ddl behavior**: The `Action` enum's `FromStr` implementation fails on unknown values with an error message.

**ndp gold behavior**: Must implement the same `Action` parsing. If `$action` is invalid:
- ndp-gold-ddl: exits with code 1, deploy.sh catches it at `if [ $exit_code -ne 0 ]`
- ndp gold: must exit with non-zero code, deploy.sh catches it the same way

**Mitigation**: In `commands/gold.rs`, parse `--action` using the same `Action::from_str` logic. Unknown values produce a clap error or an explicit error message. deploy.sh already handles non-zero exit codes.

### Edge Case 3: `--db-url` is missing but `--action` is sync (requires DB)

**Current ndp-gold-ddl behavior**: When `--database-url` is omitted and action is sync, the tool generates ALL DDL without existence checks (dry-run mode). This is documented behavior.

**ndp gold sync behavior**: `ndp gold sync` implies database connectivity. Two approaches:

1. **Require `--db-url` for sync/recreate**: If `ndp gold sync` is called without `--db-url`, emit error: "sync requires --db-url (or set TIMESCALE_URL)". The `generate` subcommand works without DB.
2. **Fallback to dry-run**: Same as ndp-gold-ddl, generate all DDL. Less safe but compatible.

**Recommendation**: Approach 1 (require `--db-url`). deploy.sh always passes `--db-url` for sync operations. The `generate` subcommand remains a dry-run tool. This is a cleaner API and matches user expectations: sync = apply to DB, generate = print DDL.

However, the deploy.sh invocation at Site 2 (domain generation) does NOT pass `--database-url`:
```bash
# Current Site 2 call:
"$gold_ddl_tool" --config-dir "$REPO_ROOT/config" generate --domain "$domain_id" --action "$action" 2>&1
```
This uses `generate`, not `sync`. So domain generation does NOT need `--db-url`. The CLI must route:
- `ndp gold generate` -- no DB required
- `ndp gold sync` -- DB required
- `ndp gold recreate` -- DB required

### Edge Case 4: Config directory does not exist

**Current behavior**: `FileSystemConfigLoader::new()` does not validate the directory at construction time. It fails at load time with a file-not-found error.

**ndp gold behavior**: If `--config-dir` points to a non-existent directory, `ndp gold generate --stream air-quality` fails with: `"Config file not found: {path}/base/streams/air-quality/config.json"`.

**Mitigation**: Add an early check in `commands/gold.rs`:
```rust
if !config_dir.exists() {
    return Err(format!("Config directory not found: {}", config_dir.display()).into());
}
```

This provides a clearer error message than a cryptic file-not-found deep in the config loader.

### Edge Case 5: `--db-timeout` flag -- does ndp-cli support it?

**Current state**: ndp-cli does NOT have a `--db-timeout` global flag. The existing commands (dictionary, dimension, domain) use a hardcoded timeout of 10 seconds in `PostgresClient::connect()`.

**ndp-gold-ddl has it**: `--db-timeout` with default value 10 seconds.

**deploy.sh passes it**: `--db-timeout 10` at Site 1.

**Resolution**: Add `--db-timeout` as a global flag to ndp-cli's `Cli` struct:
```rust
/// Database connection timeout in seconds.
#[arg(long, default_value = "10", global = true)]
db_timeout: u64,
```

This must be done as part of v1.1.14 since deploy.sh will pass `--db-timeout 10` to `ndp gold sync`. Without it, ndp would error on the unknown flag.

### Edge Case 6: `--action sync` vs subcommand `sync`

**Current deploy.sh (Site 1)** passes:
```bash
ndp-gold-ddl --config-dir ... --database-url ... --db-timeout 10 \
  generate --stream "$stream_id" --action "$action"
```

**SCOPE.md proposes** deploy.sh changes to:
```bash
ndp gold "$action" --stream "$stream_id" --config-dir ... --db-url ... --db-timeout 10
```

This transforms `--action sync` (a flag) into `sync` (a subcommand). The `$action` variable comes from jq and is `"sync"` or `"recreate"`. Using `"$action"` as a clap subcommand name works because clap matches positional subcommand names.

**However**, `generate` is also needed (domain DDL at Site 2 uses generate, not sync). The CLI must have three subcommands under `ndp gold`:
- `ndp gold generate` -- print DDL (no DB)
- `ndp gold sync` -- idempotent apply (needs DB)
- `ndp gold recreate` -- drop and recreate (needs DB)

**Verify**: deploy.sh Site 2 uses `--action "$action"` where `$action` defaults to `"sync"`. But it calls `generate --domain`, not `sync --domain`. The SCOPE.md AFTER code shows:
```bash
ddl=$("$ndp_tool" gold "$action" --domain "$domain_id" --config-dir ... 2>&1)
```
This would call `ndp gold sync --domain ...`, which would try to apply DDL to DB -- but there is no `--db-url` at Site 2. This needs careful handling. The resolution: `ndp gold sync --domain ...` without `--db-url` should behave like `generate` (print DDL without DB connectivity). Or, deploy.sh should call `ndp gold generate --domain ...` instead of `ndp gold "$action" --domain ...`.

**Recommendation**: deploy.sh Site 2 should call `ndp gold generate --domain "$domain_id"` explicitly, since domain DDL generation does not use DB checks today. The `$action` variable is irrelevant for domain generation -- it is always "generate and apply". Adjust SCOPE's proposed deploy.sh change accordingly.

---

## 6. Rollback Plan

### If v1.1.14 Fails in Production

**Symptoms**: deploy.sh Gold phases fail, Gold tables not created, domain views not generated.

**Step 1: Revert deploy.sh immediately**
```bash
# On Pi:
git checkout v1.1.13 -- deploy/pi/deploy.sh
```

This restores the `command -v ndp-gold-ddl` dispatch. Since ndp-gold-ddl is still built on the Pi (it was not removed), the standalone binary works immediately.

**Step 2: Verify standalone binary still available**
```bash
# On Pi:
command -v ndp-gold-ddl || ls /opt/ndp/bin/ndp-gold-ddl || ls target/release/ndp-gold-ddl
```

If not available (unlikely -- v1.1.14 does not remove it):
```bash
cargo build -p ndp-gold-ddl --release
```

**Step 3: Re-deploy using v1.1.13 manifest**
```bash
./deploy.sh apply .deploy/releases/v1.1.13.manifest.json
```

**Step 4: Verify Gold tables**
```bash
# Verify continuous aggregates exist:
docker exec timescaledb psql -U postgres -d ndp -c \
  "SELECT view_schema, view_name FROM timescaledb_information.continuous_aggregates"

# Verify aligned views:
docker exec timescaledb psql -U postgres -d ndp -c \
  "SELECT schemaname, viewname FROM pg_views WHERE schemaname = 'gold'"
```

**Step 5: Communication**
- Note the failure in `product/features/ops-003/bugs/BUG-001-{slug}.md`
- Update `product/features/ops-003/STATUS.md` to indicate rollback
- Record a reflexion entry with low reward for the pattern that failed

**Step 6: Root cause analysis**
- Check deploy.sh logs for error message
- Compare `ndp gold generate` output on Pi vs dev machine
- Verify config directory structure on Pi matches expectations
- Check if `--db-timeout` or `--db-url` flags are correctly forwarded

### Binary Availability After Rollback

| Binary | Available? | Notes |
|--------|-----------|-------|
| ndp-gold-ddl | YES | Standalone binary not deleted in v1.1.14 |
| ndp | YES | But gold subcommand may have issues |
| ndp-validate | YES | Unchanged in v1.1.14 |

---

## 7. Validation Criteria

v1.1.14 is "done" when ALL of the following are true:

### Code Migration

- [ ] All 376 ndp-gold-ddl tests pass in ndp-lib (`cargo test -p ndp-lib`)
- [ ] ndp-gold-ddl standalone still builds and passes its own tests (`cargo test -p ndp-gold-ddl`)
- [ ] `cargo test --workspace` passes (no regressions in other crates)
- [ ] No compilation warnings in moved code (allow warnings only for known issues documented in comments)

### CLI Parity

- [ ] `ndp gold generate --stream air-quality` == `ndp-gold-ddl generate --stream air-quality` (DDL output)
- [ ] `ndp gold generate --domain indoor-air-quality` == `ndp-gold-ddl generate --domain indoor-air-quality` (DDL output)
- [ ] `ndp gold generate --domain indoor-air-quality --events` == `ndp-gold-ddl generate --domain indoor-air-quality --events` (DDL output)
- [ ] `ndp gold generate --stream air-quality` exits 0
- [ ] `ndp gold generate --stream nonexistent` exits non-zero with error message
- [ ] `ndp gold sync --stream air-quality --db-url <url>` performs DB-aware sync
- [ ] `ndp gold sync --stream air-quality` without `--db-url` produces clear error or falls back to generate
- [ ] `--db-timeout` flag is accepted and forwarded
- [ ] `--config-dir` flag is accepted and forwarded
- [ ] `--verbose` flag is accepted (if implemented)

### deploy.sh

- [ ] Zero calls to `ndp-gold-ddl` remain in deploy.sh (verified by grep)
- [ ] `handle_gold_tables()` calls `ndp gold sync --stream <id> --db-url <url>`
- [ ] `handle_domain()` gold part calls `ndp gold generate --domain <id>`
- [ ] `handle_domain()` events part calls `ndp gold generate --domain <id> --events`
- [ ] Missing `ndp` binary causes `error` + `return 1` (not `warn` + `return 0`)
- [ ] Integration deploy passes: `DEPLOY_ENV=integration ./deploy.sh apply v1.1.14.manifest.json`
- [ ] Gold tables present in integration DB after deploy
- [ ] Domain aligned views present in integration DB after deploy

### Release Artifacts

- [ ] Manifest: `.deploy/releases/v1.1.14.manifest.json` exists and is valid JSON
- [ ] Manifest `release_version` is `"1.1.14"`
- [ ] CHANGELOG.md updated with v1.1.14 entry
- [ ] Git tag `v1.1.14` created (annotated)
- [ ] Tag message matches manifest description

### Regression Checks

- [ ] `ndp dictionary sync` still works (unchanged)
- [ ] `ndp dimension sync` still works (unchanged)
- [ ] `ndp domain sync` still works (unchanged)
- [ ] `ndp-validate --all` still works (unchanged, not in scope for v1.1.14)
- [ ] deploy.sh `ndp` dispatch sites (lines ~386, ~894, ~1063) unchanged and still work

---

## 8. Dependency on v1.1.15 / v1.1.16

### What is Explicitly NOT Done in v1.1.14

| Item | Release | Why Deferred |
|------|---------|-------------|
| Validate module migration (ndp-validate -> ndp-lib) | v1.1.15 | Separate codebase, separate risk. Gold first. |
| `ndp validate` subcommands | v1.1.15 | Depends on validate module migration |
| deploy.sh validate dispatch switchover (2 sites) | v1.1.15 | Cannot switch until ndp validate commands exist |
| Shared constants (`VALID_METRICS`, `VALID_ROLLING_STATS`) | v1.1.16 | Requires both gold and validate in ndp-lib to share |
| Cross-cutting validation (`gold::sync()` calls `validate::gold_config()`) | v1.1.16 | Requires validate module to exist in ndp-lib |
| Gold validation unification (remove duplicate `ConfigValidator`) | v1.1.16 | Requires cross-cutting validation wiring |
| `NoOpDbClient` dedup (3 copies -> 1) | v1.1.16 | Low priority, no functional impact |
| Standalone binary thin wrappers (final form) | v1.1.16 | ndp-gold-ddl becomes thin wrapper in v1.1.14, but ndp-validate is untouched until v1.1.15 |

### Temporary State Between v1.1.14 and v1.1.15

After v1.1.14 deploys but before v1.1.15:

| Component | State | Notes |
|-----------|-------|-------|
| deploy.sh Gold dispatch | Uses `ndp gold` | New behavior |
| deploy.sh Validate dispatch | Uses `ndp-validate` standalone | Old behavior, untouched |
| deploy.sh Dictionary/Dimension/Domain dispatch | Uses `ndp` | Unchanged from ops-001/ops-002 |
| ndp-gold-ddl binary | Thin wrapper around ndp-lib | Still works standalone |
| ndp-validate binary | Independent standalone | No changes in v1.1.14 |
| ndp binary | Has gold subcommands | New subcommands added |
| Gold config validation | Exists in BOTH `ndp_lib::gold::validation` AND `ndp-validate semantic/gold.rs` | Duplication persists until v1.1.16 |
| `VALID_METRICS` constants | Defined in BOTH `ndp_lib::gold::config` AND `ndp-validate semantic/gold.rs` | Duplication persists until v1.1.16 |
| `DbClient` trait | Defined in BOTH `ndp_lib::db` AND `ndp-gold-ddl::db` (re-export) | ndp-gold-ddl re-exports from ndp-lib now |
| `NoOpDbClient` | 3 copies in ndp-cli commands | Dedup deferred to v1.1.16 |

**Key Point**: This temporary state is SAFE. deploy.sh works because Gold uses `ndp` and validation uses `ndp-validate` -- they don't interact. The duplication is a code quality issue, not a functional issue. Constants cannot drift between v1.1.14 and v1.1.16 because no one is modifying ndp-validate in that window.

### Risk of Delayed v1.1.15

If v1.1.15 is delayed, the system operates correctly with:
- Gold via `ndp gold` (v1.1.14)
- Validation via `ndp-validate` (pre-ops-003)
- Dictionary/Dimension/Domain via `ndp` (ops-001/ops-002)

There is no deadline pressure on v1.1.15 shipping. Each release is independently deployable.

---

## Appendix A: File Migration Manifest

### Source Files (29 files to move)

| Source | Destination | Notes |
|--------|-------------|-------|
| `tools/ndp-gold-ddl/src/error.rs` | `crates/ndp-lib/src/gold/error.rs` | Rename `GoldDdlError` or keep as-is |
| `tools/ndp-gold-ddl/src/config/mod.rs` | `crates/ndp-lib/src/gold/config/mod.rs` | |
| `tools/ndp-gold-ddl/src/config/types.rs` | `crates/ndp-lib/src/gold/config/types.rs` | Contains Gold StreamConfig, GoldEtlConfig |
| `tools/ndp-gold-ddl/src/config/domain.rs` | `crates/ndp-lib/src/gold/config/domain.rs` | Gold DomainConfig |
| `tools/ndp-gold-ddl/src/config/loader.rs` | `crates/ndp-lib/src/gold/config/loader.rs` | FileSystemConfigLoader (Gold-specific) |
| `tools/ndp-gold-ddl/src/db/mod.rs` | DELETE (or replace with re-export) | Use ndp_lib::db instead |
| `tools/ndp-gold-ddl/src/db/client.rs` | DELETE | Replaced by ndp_lib::db::DbClient |
| `tools/ndp-gold-ddl/src/db/queries.rs` | `crates/ndp-lib/src/gold/db.rs` | CaChecker, CaInfo, PostgresCaChecker |
| `tools/ndp-gold-ddl/src/generators/mod.rs` | `crates/ndp-lib/src/gold/generators/mod.rs` | |
| `tools/ndp-gold-ddl/src/generators/continuous_aggregate.rs` | `crates/ndp-lib/src/gold/generators/continuous_aggregate.rs` | |
| `tools/ndp-gold-ddl/src/generators/aligned_view.rs` | `crates/ndp-lib/src/gold/generators/aligned_view.rs` | |
| `tools/ndp-gold-ddl/src/generators/state_transitions.rs` | `crates/ndp-lib/src/gold/generators/state_transitions.rs` | |
| `tools/ndp-gold-ddl/src/generators/events.rs` | `crates/ndp-lib/src/gold/generators/events.rs` | |
| `tools/ndp-gold-ddl/src/generators/refresh_policy.rs` | `crates/ndp-lib/src/gold/generators/refresh_policy.rs` | |
| `tools/ndp-gold-ddl/src/generators/classification.rs` | `crates/ndp-lib/src/gold/generators/classification.rs` | |
| `tools/ndp-gold-ddl/src/generators/column_builder.rs` | `crates/ndp-lib/src/gold/generators/column_builder.rs` | |
| `tools/ndp-gold-ddl/src/generators/join_builder.rs` | `crates/ndp-lib/src/gold/generators/join_builder.rs` | |
| `tools/ndp-gold-ddl/src/generators/null_handler.rs` | `crates/ndp-lib/src/gold/generators/null_handler.rs` | |
| `tools/ndp-gold-ddl/src/generators/constants.rs` | `crates/ndp-lib/src/gold/generators/constants.rs` | Move to ndp_lib::constants in v1.1.16 |
| `tools/ndp-gold-ddl/src/planner/mod.rs` | `crates/ndp-lib/src/gold/planner/mod.rs` | |
| `tools/ndp-gold-ddl/src/planner/sync.rs` | `crates/ndp-lib/src/gold/planner/sync.rs` | |
| `tools/ndp-gold-ddl/src/registry/mod.rs` | `crates/ndp-lib/src/gold/registry/mod.rs` | |
| `tools/ndp-gold-ddl/src/registry/trait_def.rs` | `crates/ndp-lib/src/gold/registry/trait_def.rs` | |
| `tools/ndp-gold-ddl/src/registry/lag.rs` | `crates/ndp-lib/src/gold/registry/lag.rs` | |
| `tools/ndp-gold-ddl/src/registry/rolling.rs` | `crates/ndp-lib/src/gold/registry/rolling.rs` | |
| `tools/ndp-gold-ddl/src/registry/trend.rs` | `crates/ndp-lib/src/gold/registry/trend.rs` | |
| `tools/ndp-gold-ddl/src/validation/mod.rs` | `crates/ndp-lib/src/gold/validation/mod.rs` | |
| `tools/ndp-gold-ddl/src/validation/config_validator.rs` | `crates/ndp-lib/src/gold/validation/config_validator.rs` | |
| `tools/ndp-gold-ddl/src/lib.rs` | REWRITE (thin wrapper) | Re-exports from ndp_lib::gold |

### Integration Test Files (10 files to move)

| Source | Destination |
|--------|-------------|
| `tools/ndp-gold-ddl/tests/aligned_view_tests.rs` | `crates/ndp-lib/tests/gold_aligned_view_tests.rs` |
| `tools/ndp-gold-ddl/tests/golden_master_test.rs` | `crates/ndp-lib/tests/gold_golden_master_test.rs` |
| `tools/ndp-gold-ddl/tests/objectives_tests.rs` | `crates/ndp-lib/tests/gold_objectives_tests.rs` |
| `tools/ndp-gold-ddl/tests/ops002_config_driven_tests.rs` | `crates/ndp-lib/tests/gold_ops002_config_driven_tests.rs` |
| `tools/ndp-gold-ddl/tests/ops002_hardcoding_tests.rs` | `crates/ndp-lib/tests/gold_ops002_hardcoding_tests.rs` |
| `tools/ndp-gold-ddl/tests/ops002_source_scan_tests.rs` | `crates/ndp-lib/tests/gold_ops002_source_scan_tests.rs` |
| `tools/ndp-gold-ddl/tests/state_transitions_tests.rs` | `crates/ndp-lib/tests/gold_state_transitions_tests.rs` |
| `tools/ndp-gold-ddl/tests/fixtures/mod.rs` | `crates/ndp-lib/tests/gold_fixtures/mod.rs` |
| `tools/ndp-gold-ddl/tests/fixtures/phase_c.rs` | `crates/ndp-lib/tests/gold_fixtures/phase_c.rs` |
| `tools/ndp-gold-ddl/tests/fixtures/energy_monitoring.rs` | `crates/ndp-lib/tests/gold_fixtures/energy_monitoring.rs` |

### Cargo.toml Changes (v1.1.14)

**`crates/ndp-lib/Cargo.toml` additions:**
```toml
[dev-dependencies]
mockall = "0.11"
pretty_assertions = "1.4"
sha2 = "0.10"
```

**`tools/ndp-gold-ddl/Cargo.toml` additions:**
```toml
[dependencies]
ndp-lib = { path = "../../crates/ndp-lib" }
```

**`tools/ndp-cli/Cargo.toml`** -- no changes needed (already depends on ndp-lib).

---

## Appendix B: deploy.sh Grep Verification

After v1.1.14, run this to verify no ndp-gold-ddl references remain:

```bash
grep -n 'ndp-gold-ddl' deploy/pi/deploy.sh
# Expected: zero results

grep -n 'command -v ndp-gold-ddl' deploy/pi/deploy.sh
# Expected: zero results

grep -n 'gold_ddl_tool' deploy/pi/deploy.sh
# Expected: zero results

# Verify ndp dispatch sites exist:
grep -n 'command -v ndp' deploy/pi/deploy.sh
# Expected: 5+ lines (dictionary, domain, dimension, gold-site-1, gold-site-2)
```

---

## Appendix C: Flag Mapping Reference

| deploy.sh (v1.1.13) | deploy.sh (v1.1.14) | Notes |
|---------------------|---------------------|-------|
| `"$gold_ddl_tool" --config-dir ... --database-url ... --db-timeout 10 generate --stream ... --action ...` | `"$ndp_tool" gold sync --stream ... --config-dir ... --db-url ... --db-timeout 10` | sync subcommand replaces generate + --action sync |
| `"$gold_ddl_tool" --config-dir ... generate --domain ... --action ...` | `"$ndp_tool" gold generate --domain ... --config-dir ...` | Domain always uses generate (no DB) |
| `"$gold_ddl_tool" --config-dir ... generate --domain ... --events --action ...` | `"$ndp_tool" gold generate --domain ... --events --config-dir ...` | Events flag forwarded |
