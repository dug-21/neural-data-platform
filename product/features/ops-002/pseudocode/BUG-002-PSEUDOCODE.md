# BUG-002 Pseudocode: Domain Objectives Sync Migration to Rust

## Overview

Migrate `sync_domains_to_data_dictionary()` (~200 lines of dead Bash) to the `ndp domain sync` Rust command. Follows the exact same architecture as `ndp dictionary sync` (reference implementation in `crates/ndp-lib/src/dictionary/mod.rs`).

**Target tables** (from `005_domain_objectives.sql`):
- `data_dictionary.domains` -- domain metadata (UPSERT)
- `data_dictionary.domain_streams` -- stream-to-domain mappings (DELETE+INSERT)
- `data_dictionary.objectives` -- target metrics (DELETE+INSERT)
- `data_dictionary.constraints` -- action preconditions (DELETE+INSERT)

---

## 1. Domain Config Types (`crates/ndp-lib/src/domain/types.rs`)

These are the **sync entry** structs consumed by `sync_domain()`. They are parsed structs, not raw JSON -- the caller is responsible for deserialization.

```
STRUCT DomainSyncEntry
    domain_id:      String          -- PK in domains table
    description:    Option<String>  -- human description
    stream_count:   i32             -- len(streams), stored for quick access
    config_path:    String          -- "config/domains/{id}/domain.json"
    streams:        Vec<StreamMappingEntry>
    objectives:     Vec<ObjectiveSyncEntry>
    constraints:    Vec<ConstraintSyncEntry>
END STRUCT

STRUCT StreamMappingEntry
    stream_id:      String          -- references a stream config
    alias:          String          -- short name for aligned views
    role:           String          -- "primary" | "context" | "actuator" | "constraint"
END STRUCT

STRUCT ObjectiveSyncEntry
    objective_id:   String          -- PK (with domain_id)
    description:    Option<String>
    target_stream:  String          -- which stream to monitor
    target_metric:  String          -- which metric within that stream
    condition:      String          -- "<" | ">" | "<=" | ">=" | "==" | "!=" | "between"
    threshold:      f64             -- numeric threshold (lower bound for "between")
    threshold_upper: Option<f64>    -- upper bound (only for "between")
    unit:           Option<String>  -- "ppm", "ug/m3", "celsius", "percent"
    priority:       String          -- "low" | "medium" | "high" | "critical"
END STRUCT

STRUCT ConstraintSyncEntry
    constraint_id:      String      -- PK (with domain_id)
    description:        Option<String>
    constraint_stream:  String
    constraint_metric:  String
    condition:          String      -- "<" | ">" | "<=" | ">=" | "==" | "!="
    threshold:          f64
    unit:               Option<String>
END STRUCT
```

### Serde derivations

All structs derive `Debug, Clone, Serialize, Deserialize` to match the `dictionary/types.rs` pattern. Default values:
- `priority` defaults to `"medium"` via serde default function
- `threshold_upper` defaults to `None`

---

## 2. ConfigLoader Extension (`crates/ndp-lib/src/config.rs`)

### 2a. DomainConfig deserialization struct

This struct matches the shape of `config/domains/*/domain.json` -- flat keys, not nested under `"domain"`.

```
STRUCT DomainConfig
    id:             String                  -- "indoor-air-quality"
    description:    Option<String>          -- "Maintain healthy indoor air quality"
    streams:        Vec<DomainStreamConfig> -- stream mappings
    alignment:      Option<Value>           -- pass-through, not needed for sync
    events:         Option<Value>           -- pass-through, not needed for sync
    objectives:     Vec<DomainObjectiveConfig>
    constraints:    Vec<DomainConstraintConfig>   -- serde(default) = empty vec
END STRUCT

STRUCT DomainStreamConfig
    stream_id:      String
    alias:          String
    role:           String
    null_handling:  Option<String>          -- not synced, but must be parseable
END STRUCT

STRUCT DomainObjectiveConfig
    id:             String
    description:    Option<String>
    target:         ObjectiveTarget
    priority:       Option<String>          -- default "medium"
END STRUCT

STRUCT ObjectiveTarget
    stream:         String
    metric:         String
    condition:      String
    threshold:      ThresholdValue          -- single number OR [lower, upper] array
    unit:           Option<String>
END STRUCT

-- ThresholdValue must handle both:
--   "threshold": 800          (single numeric)
--   "threshold": [20, 24]     (array for "between")
ENUM ThresholdValue
    Single(f64)
    Range(f64, f64)
END ENUM

-- Custom Deserialize for ThresholdValue:
-- If JSON is a number -> Single(n)
-- If JSON is an array of 2 numbers -> Range(arr[0], arr[1])

STRUCT DomainConstraintConfig
    id:             String
    description:    Option<String>
    stream:         String
    metric:         String
    condition:      String
    threshold:      f64
    unit:           Option<String>
END STRUCT
```

### 2b. ConfigLoader trait extension

```
TRAIT ConfigLoader
    EXISTING: load_stream_configs() -> Result<Vec<StreamConfig>>
    EXISTING: load_dimension_config(dimension_id) -> Result<DimensionConfig>
    NEW:      load_domain_configs() -> Result<Vec<DomainConfig>>
END TRAIT
```

### 2c. FileSystemConfigLoader implementation

```
ALGORITHM: FileSystemConfigLoader.discover_domain_ids
INPUT: self (has domains_dir: PathBuf)
OUTPUT: Result<Vec<String>>

BEGIN
    IF NOT self.domains_dir.exists() THEN
        RETURN Err(NdpLibError::ConfigNotFound { path: domains_dir })
    END IF

    ids <- empty Vec<String>

    FOR EACH entry IN read_dir(self.domains_dir) DO
        path <- entry.path()
        IF path.is_dir() THEN
            config_path <- path.join("domain.json")
            IF config_path.exists() THEN
                name <- path.file_name()
                ids.push(name)
            END IF
        END IF
    END FOR

    ids.sort()
    RETURN Ok(ids)
END
```

```
ALGORITHM: FileSystemConfigLoader.load_domain_config
INPUT: self, domain_id: &str
OUTPUT: Result<DomainConfig>

BEGIN
    config_path <- self.domains_dir.join(domain_id).join("domain.json")

    IF NOT config_path.exists() THEN
        RETURN Err(NdpLibError::ConfigNotFound { path: config_path })
    END IF

    content <- fs::read_to_string(config_path)
    config <- serde_json::from_str::<DomainConfig>(content)
        .map_err(|e| NdpLibError::ConfigParse { message })

    RETURN Ok(config)
END
```

```
ALGORITHM: FileSystemConfigLoader.load_domain_configs
INPUT: self
OUTPUT: Result<Vec<DomainConfig>>

BEGIN
    ids <- self.discover_domain_ids()
    configs <- empty Vec with capacity ids.len()

    FOR EACH id IN ids DO
        MATCH self.load_domain_config(id)
            Ok(config) => configs.push(config)
            Err(e)     => tracing::warn("Skipping domain config: {id}: {e}")
    END FOR

    RETURN Ok(configs)
END
```

### 2d. FileSystemConfigLoader constructor change

The `FileSystemConfigLoader` struct needs a `domains_dir` field. Add it alongside the existing `streams_dir` and `dimensions_dir`.

```
STRUCT FileSystemConfigLoader
    streams_dir:    PathBuf     -- EXISTING
    dimensions_dir: PathBuf     -- EXISTING
    domains_dir:    PathBuf     -- NEW
END STRUCT
```

The `from_base_dir(base)` constructor sets `domains_dir = base.join("domains")`.

The `new()` constructor adds a `domains_dir` parameter. Since changing the existing `new()` signature is a breaking change for existing callers, two options:

- **Option A**: Add a `with_domains_dir()` builder method (non-breaking)
- **Option B**: Add `domains_dir` param to `new()` and update the 2 call sites

**Decision**: Option B. There are only 2 call sites (`dictionary.rs` and `dimension.rs` CLI commands). Both already pass `base_config_dir.join("dimensions")`, so adding `base_config_dir.join("domains")` is trivial. The `from_base_dir()` constructor already handles it automatically.

---

## 3. Conversion Function (`crates/ndp-lib/src/convert.rs`)

```
ALGORITHM: domain_config_to_sync_entry
INPUT: config: &DomainConfig
OUTPUT: DomainSyncEntry

BEGIN
    config_path <- format!("config/domains/{}/domain.json", config.id)

    streams <- []
    FOR EACH s IN config.streams DO
        streams.push(StreamMappingEntry {
            stream_id:  s.stream_id,
            alias:      s.alias,
            role:       s.role
        })
    END FOR

    objectives <- []
    FOR EACH o IN config.objectives DO
        -- Flatten nested target.* fields to top-level sync entry fields
        (threshold, threshold_upper) <- MATCH o.target.threshold
            ThresholdValue::Single(v)    => (v, None)
            ThresholdValue::Range(lo,hi) => (lo, Some(hi))
        END MATCH

        priority <- o.priority.unwrap_or("medium")

        objectives.push(ObjectiveSyncEntry {
            objective_id:    o.id,
            description:     o.description,
            target_stream:   o.target.stream,
            target_metric:   o.target.metric,
            condition:       o.target.condition,
            threshold:       threshold,
            threshold_upper: threshold_upper,
            unit:            o.target.unit,
            priority:        priority
        })
    END FOR

    constraints <- []
    FOR EACH c IN config.constraints DO
        constraints.push(ConstraintSyncEntry {
            constraint_id:      c.id,
            description:        c.description,
            constraint_stream:  c.stream,
            constraint_metric:  c.metric,
            condition:          c.condition,
            threshold:          c.threshold,
            unit:               c.unit
        })
    END FOR

    RETURN DomainSyncEntry {
        domain_id:    config.id,
        description:  config.description,
        stream_count: config.streams.len() as i32,
        config_path:  config_path,
        streams:      streams,
        objectives:   objectives,
        constraints:  constraints
    }
END
```

### Complexity

- Time: O(s + o + c) where s = streams, o = objectives, c = constraints
- Space: O(s + o + c)

---

## 4. SQL Constants (`crates/ndp-lib/src/domain/sql.rs`)

All SQL uses parameterized queries (`$1`, `$2`, ...). No string concatenation.

```
CONSTANT UPSERT_DOMAIN:
    "INSERT INTO data_dictionary.domains
        (domain_id, description, stream_count, config_path, updated_at)
     VALUES ($1, $2, $3, $4, NOW())
     ON CONFLICT (domain_id) DO UPDATE SET
        description  = EXCLUDED.description,
        stream_count = EXCLUDED.stream_count,
        config_path  = EXCLUDED.config_path,
        updated_at   = NOW()"

    Parameters: [domain_id, description, stream_count, config_path]
    Types:      [&str,      &Option<String>, &i32,   &str]

CONSTANT DELETE_DOMAIN_STREAMS:
    "DELETE FROM data_dictionary.domain_streams WHERE domain_id = $1"

    Parameters: [domain_id]

CONSTANT INSERT_DOMAIN_STREAM:
    "INSERT INTO data_dictionary.domain_streams
        (domain_id, stream_id, alias, role)
     VALUES ($1, $2, $3, $4)"

    Parameters: [domain_id, stream_id, alias, role]
    Types:      [&str,      &str,      &str,  &str]

CONSTANT DELETE_OBJECTIVES:
    "DELETE FROM data_dictionary.objectives WHERE domain_id = $1"

    Parameters: [domain_id]

CONSTANT INSERT_OBJECTIVE:
    "INSERT INTO data_dictionary.objectives
        (domain_id, objective_id, description, target_stream, target_metric,
         condition, threshold, threshold_upper, unit, priority, updated_at)
     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NOW())"

    Parameters: [domain_id, objective_id, description, target_stream,
                 target_metric, condition, threshold, threshold_upper,
                 unit, priority]
    Types:      [&str, &str, &Option<String>, &str,
                 &str, &str, &f64, &Option<f64>,
                 &Option<String>, &str]

CONSTANT DELETE_CONSTRAINTS:
    "DELETE FROM data_dictionary.constraints WHERE domain_id = $1"

    Parameters: [domain_id]

CONSTANT INSERT_CONSTRAINT:
    "INSERT INTO data_dictionary.constraints
        (domain_id, constraint_id, description, constraint_stream,
         constraint_metric, condition, threshold, unit)
     VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"

    Parameters: [domain_id, constraint_id, description, constraint_stream,
                 constraint_metric, condition, threshold, unit]
    Types:      [&str, &str, &Option<String>, &str,
                 &str, &str, &f64, &Option<String>]
```

### SQL ordering rationale

Per domain, the sync sequence is:
1. UPSERT domain (parent must exist before children)
2. DELETE domain_streams, then INSERT (full refresh of child table)
3. DELETE objectives, then INSERT (full refresh)
4. DELETE constraints, then INSERT (full refresh)

CASCADE on the FK means DELETE FROM domains would also remove children. But we UPSERT domains (not delete), so we explicitly DELETE+INSERT each child table to handle additions, removals, and modifications.

---

## 5. Sync Function (`crates/ndp-lib/src/domain/mod.rs`)

```
ALGORITHM: sync_domain
INPUT:
    entries:  &[DomainSyncEntry]    -- parsed domain configs
    db:       &impl DbClient        -- database client (real or mock)
    options:  &SyncOptions          -- { dry_run: bool }
OUTPUT:
    Result<SyncReport>

SUBROUTINES:
    insert_domain_entry()
    build_dry_run_report()

STRUCT DomainSyncCounts
    domains:     i32     -- domains upserted
    streams:     i32     -- domain_stream rows inserted
    objectives:  i32     -- objective rows inserted
    constraints: i32     -- constraint rows inserted
END STRUCT

BEGIN
    start <- Instant::now()
    counts <- DomainSyncCounts::default()
    errors <- empty Vec<SyncError>

    -- Step 0: Short-circuit for dry_run
    IF options.dry_run THEN
        RETURN Ok(build_dry_run_report(entries))
    END IF

    -- Step 1: BEGIN transaction
    db.batch_execute("BEGIN")

    -- Step 2: Process each domain
    FOR EACH entry IN entries DO
        MATCH insert_domain_entry(db, entry, &mut counts)
            Ok(())  => { }
            Err(e)  =>
                tracing::error("Failed to sync domain {entry.domain_id}: {e}")
                errors.push(SyncError {
                    item: entry.domain_id,
                    message: format!("Domain sync failed: {e}")
                })
        END MATCH
    END FOR

    -- Step 3: COMMIT transaction
    db.batch_execute("COMMIT")

    duration <- start.elapsed()

    tracing::info(
        domains     = counts.domains,
        streams     = counts.streams,
        objectives  = counts.objectives,
        constraints = counts.constraints,
        duration_ms = duration.as_millis(),
        "Domain sync complete"
    )

    RETURN Ok(SyncReport {
        entity:          "domain",
        items_processed: entries.len(),
        items_created:   (counts.domains + counts.streams
                         + counts.objectives + counts.constraints) as usize,
        items_updated:   0,
        items_deleted:   0,
        errors:          errors,
        duration:        duration
    })
END
```

### Insert single domain entry

```
ALGORITHM: insert_domain_entry
INPUT:
    db:     &impl DbClient
    entry:  &DomainSyncEntry
    counts: &mut DomainSyncCounts
OUTPUT: Result<()>

BEGIN
    -- 2a. UPSERT domain
    db.execute(sql::UPSERT_DOMAIN, &[
        &entry.domain_id,
        &entry.description,
        &entry.stream_count,
        &entry.config_path
    ])
    counts.domains += 1

    -- 2b. DELETE + INSERT domain_streams
    db.execute(sql::DELETE_DOMAIN_STREAMS, &[&entry.domain_id])

    FOR EACH stream IN entry.streams DO
        db.execute(sql::INSERT_DOMAIN_STREAM, &[
            &entry.domain_id,
            &stream.stream_id,
            &stream.alias,
            &stream.role
        ])
        counts.streams += 1
    END FOR

    -- 2c. DELETE + INSERT objectives
    db.execute(sql::DELETE_OBJECTIVES, &[&entry.domain_id])

    FOR EACH obj IN entry.objectives DO
        db.execute(sql::INSERT_OBJECTIVE, &[
            &entry.domain_id,
            &obj.objective_id,
            &obj.description,
            &obj.target_stream,
            &obj.target_metric,
            &obj.condition,
            &obj.threshold,
            &obj.threshold_upper,
            &obj.unit,
            &obj.priority
        ])
        counts.objectives += 1
    END FOR

    -- 2d. DELETE + INSERT constraints
    db.execute(sql::DELETE_CONSTRAINTS, &[&entry.domain_id])

    FOR EACH con IN entry.constraints DO
        db.execute(sql::INSERT_CONSTRAINT, &[
            &entry.domain_id,
            &con.constraint_id,
            &con.description,
            &con.constraint_stream,
            &con.constraint_metric,
            &con.condition,
            &con.threshold,
            &con.unit
        ])
        counts.constraints += 1
    END FOR

    RETURN Ok(())
END
```

### Dry run report builder

```
ALGORITHM: build_dry_run_report
INPUT: entries: &[DomainSyncEntry]
OUTPUT: SyncReport

BEGIN
    streams    <- 0
    objectives <- 0
    constraints <- 0

    FOR EACH entry IN entries DO
        streams    += entry.streams.len()
        objectives += entry.objectives.len()
        constraints += entry.constraints.len()
    END FOR

    RETURN SyncReport {
        entity:          "domain",
        items_processed: entries.len(),
        items_created:   entries.len() + streams + objectives + constraints,
        items_updated:   0,
        items_deleted:   0,
        errors:          [],
        duration:        Duration::ZERO
    }
END
```

### Complexity Analysis

```
Time Complexity:
    Per domain:
        - UPSERT domain:           O(1) amortized (single row, PK index)
        - DELETE domain_streams:    O(s) where s = existing stream mappings
        - INSERT domain_streams:    O(s') where s' = new stream mappings
        - DELETE objectives:        O(o) where o = existing objectives
        - INSERT objectives:        O(o') where o' = new objectives
        - DELETE constraints:       O(c) where c = existing constraints
        - INSERT constraints:       O(c') where c' = new constraints
    Total per domain: O(s + o + c)
    Total for N domains: O(N * (s + o + c))

    In practice: indoor-air-quality has 4 streams, 6 objectives, 0 constraints.
    Expected total domains: 1-5. This is microseconds of work.

Space Complexity:
    - Input entries:     O(N * (s + o + c))  -- caller-owned
    - SQL params:        O(max_params) = O(10)  -- per statement, reused
    - SyncCounts:        O(1)
    - Total additional:  O(1)

Network/IO:
    - 1 + N * (1 + 1 + s' + 1 + o' + 1 + c') + 1 SQL round-trips per sync
    - For 1 domain with 4 streams, 6 objectives, 0 constraints:
      BEGIN + UPSERT + DELETE + 4*INSERT + DELETE + 6*INSERT + DELETE + 0*INSERT + COMMIT
      = 16 round-trips
    - Could batch INSERTs for optimization (out of scope for v1.1.12)
```

---

## 6. CLI Command (`tools/ndp-cli/src/commands/domain.rs`)

### Clap argument structure

```
STRUCT DomainArgs
    command: DomainCommands       -- clap subcommand

ENUM DomainCommands
    Sync {
        config_dir: Option<PathBuf>  -- --config-dir, overrides default
        dry_run: bool                -- --dry-run flag
    }
END ENUM
```

### Command execution

```
ALGORITHM: domain::run
INPUT:
    args:            DomainArgs
    base_config_dir: &Path          -- resolved from CLI --config-dir or --env
    db_url:          &str           -- resolved from CLI --db-url or TIMESCALE_URL
OUTPUT: Result<()>

BEGIN
    MATCH args.command
        DomainCommands::Sync { config_dir, dry_run } =>

            -- 1. Resolve domains directory
            domains_dir <- config_dir.unwrap_or(base_config_dir.join("domains"))

            tracing::info(
                domains_dir = domains_dir,
                db_url = db_url,
                dry_run = dry_run,
                "Starting domain sync"
            )

            -- 2. Create FileSystemConfigLoader
            loader <- FileSystemConfigLoader::new(
                base_config_dir.join("streams"),
                base_config_dir.join("dimensions"),
                domains_dir
            )

            -- 3. Load domain configs
            configs <- loader.load_domain_configs()

            tracing::info(domain_count = configs.len(), "Loaded domain configurations")

            IF configs.is_empty() THEN
                println("No domain configs found. Nothing to sync.")
                RETURN Ok(())
            END IF

            -- 4. Convert DomainConfig -> DomainSyncEntry
            entries <- configs.iter()
                .map(domain_config_to_sync_entry)
                .collect::<Vec<DomainSyncEntry>>()

            options <- SyncOptions { dry_run }

            -- 5. Handle dry_run vs live
            IF dry_run THEN
                report <- sync_domain(&entries, &NoOpDbClient, &options)

                println("DRY RUN domain sync:")
                println("  Domains:      {}", report.items_processed)
                println("  Total items:  {}", report.items_created)

                FOR EACH config IN configs DO
                    obj_count <- config.objectives.len()
                    stream_count <- config.streams.len()
                    println("  - {} ({} streams, {} objectives)",
                            config.id, stream_count, obj_count)
                END FOR

                RETURN Ok(())
            END IF

            -- 6. Connect to DB and run sync
            tracing::info(db_url = db_url, "Connecting to database")
            db <- PostgresClient::connect(db_url, 10)

            report <- sync_domain(&entries, &db, &options)

            -- 7. Print report
            println("Domain sync complete:")
            println("  Domains synced: {}", report.items_processed)
            println("  Items created:  {}", report.items_created)
            println("  Duration:       {:.2}s", report.duration.as_secs_f64())

            IF NOT report.errors.is_empty() THEN
                println("  Warnings:       {}", report.errors.len())
                FOR EACH err IN report.errors DO
                    println("    - {}: {}", err.item, err.message)
                END FOR
            END IF

            RETURN Ok(())
    END MATCH
END
```

### CLI registration in main.rs

```
-- Add to Commands enum:
ENUM Commands
    Dictionary(DictionaryArgs)      -- EXISTING
    Dimension(DimensionArgs)        -- EXISTING
    Domain(DomainArgs)              -- NEW
END ENUM

-- Add to match block:
MATCH cli.command
    Commands::Dictionary(args) => dictionary::run(args, &config_dir, &db_url)
    Commands::Dimension(args)  => dimension::run(args, &config_dir, &db_url)
    Commands::Domain(args)     => domain::run(args, &config_dir, &db_url)       -- NEW
END MATCH
```

### Module registration in commands/mod.rs

```
pub mod dictionary;     -- EXISTING
pub mod dimension;      -- EXISTING
pub mod domain;         -- NEW
```

---

## 7. deploy.sh Integration

Replace the body of `sync_domains_to_data_dictionary()` (lines 883-1086) with the `command -v ndp` fallback pattern already proven by `sync_to_data_dictionary()` (dictionary sync, lines 384-414).

```
ALGORITHM: sync_domains_to_data_dictionary (replacement)

BEGIN
    log "Syncing Domain Objectives to Data Dictionary..."

    -- Wait for TimescaleDB
    WHILE NOT timescaledb pg_isready DO
        warn "Waiting for TimescaleDB..."
        sleep 2
    END WHILE

    -- Locate ndp CLI (same pattern as dictionary sync)
    ndp_tool <- ""
    IF command -v ndp THEN
        ndp_tool <- "ndp"
    ELSE IF exists "/opt/ndp/bin/ndp" THEN
        ndp_tool <- "/opt/ndp/bin/ndp"
    ELSE IF exists "$REPO_ROOT/target/release/ndp" THEN
        ndp_tool <- "$REPO_ROOT/target/release/ndp"
    ELSE IF exists "$REPO_ROOT/target/debug/ndp" THEN
        ndp_tool <- "$REPO_ROOT/target/debug/ndp"
    END IF

    IF ndp_tool is empty THEN
        warn "ndp CLI not available, skipping domain objectives sync"
        warn "Build with: cargo build --release -p ndp-cli"
        RETURN 0
    END IF

    log "Using ndp CLI for domain sync ($ndp_tool)..."

    -- Build DB URL (same pattern as dictionary sync)
    db_password <- ${POSTGRES_PASSWORD:-ndp_secure_password}
    ndp_args <- "domain sync --db-url postgresql://postgres:$db_password@localhost:5432/ndp"

    -- Append config dir if non-default
    IF CONFIG_DOMAINS_DIR is set THEN
        ndp_args <- "$ndp_args --config-dir $CONFIG_DOMAINS_DIR"
    END IF

    IF $ndp_tool $ndp_args succeeds THEN
        log "Domain objectives sync successful (via ndp CLI)"
        RETURN 0
    ELSE
        error "Domain objectives sync failed"
        RETURN 1
    END IF
END
```

### Key differences from the old Bash

| Aspect | Old (dead) Bash | New Rust via deploy.sh |
|--------|----------------|----------------------|
| Config format | `domain.yaml` | `domain.json` |
| Key paths | `domain.id` (nested YAML) | `id` (flat JSON) |
| SQL generation | String interpolation (`'$domain_id'`) | Parameterized (`$1, $2, ...`) |
| Transaction | Generated into temp file, piped to psql | `BEGIN`/`COMMIT` via `DbClient` |
| Error handling | Silent skip on YAML parse failure | `SyncReport.errors` with item-level detail |
| Testability | None | London TDD with `MockDbClient` |

---

## 8. Test Plan (London TDD)

Tests follow the same pattern as `crates/ndp-lib/src/dictionary/mod.rs` tests (lines 482-1254): `MockDbClient` records SQL calls, assertions verify query strings and parameter counts.

### Unit tests (`crates/ndp-lib/src/domain/mod.rs`)

```
TEST 1: test_sync_empty_entries
    INPUT:  entries = []
    EXPECT: report.items_processed == 0
    EXPECT: report.items_created == 0
    EXPECT: db has BEGIN + COMMIT only

TEST 2: test_sync_single_domain_upsert
    INPUT:  1 domain, 0 streams, 0 objectives, 0 constraints
    EXPECT: 1 UPSERT_DOMAIN call
    EXPECT: query contains "ON CONFLICT (domain_id) DO UPDATE"
    EXPECT: 4 parameters ($1-$4)

TEST 3: test_sync_domain_streams
    INPUT:  1 domain, 4 streams
    EXPECT: 1 DELETE_DOMAIN_STREAMS call
    EXPECT: 4 INSERT_DOMAIN_STREAM calls
    EXPECT: each INSERT has 4 parameters ($1-$4)

TEST 4: test_sync_objectives
    INPUT:  1 domain, 6 objectives (matches indoor-air-quality config)
    EXPECT: 1 DELETE_OBJECTIVES call
    EXPECT: 6 INSERT_OBJECTIVE calls
    EXPECT: each INSERT has 10 parameters ($1-$10)

TEST 5: test_sync_objectives_with_between_condition
    INPUT:  1 objective with condition="between", threshold=20, threshold_upper=24
    EXPECT: INSERT_OBJECTIVE called with threshold_upper = Some(24.0)

TEST 6: test_sync_objectives_without_between
    INPUT:  1 objective with condition="<", threshold=800
    EXPECT: INSERT_OBJECTIVE called with threshold_upper = None

TEST 7: test_sync_constraints
    INPUT:  1 domain, 2 constraints
    EXPECT: 1 DELETE_CONSTRAINTS call
    EXPECT: 2 INSERT_CONSTRAINT calls
    EXPECT: each INSERT has 8 parameters ($1-$8)

TEST 8: test_sync_transaction_wrapping
    INPUT:  1 domain
    EXPECT: first query == "BEGIN"
    EXPECT: last query == "COMMIT"

TEST 9: test_sync_ordering_per_domain
    INPUT:  1 domain, 2 streams, 1 objective, 1 constraint
    EXPECT: UPSERT domain before DELETE domain_streams
    EXPECT: DELETE domain_streams before INSERT domain_stream
    EXPECT: DELETE objectives before INSERT objective
    EXPECT: DELETE constraints before INSERT constraint

TEST 10: test_sync_multiple_domains
    INPUT:  2 domains, each with different stream/objective counts
    EXPECT: report.items_processed == 2
    EXPECT: 2 UPSERT_DOMAIN calls
    EXPECT: correct total counts across both

TEST 11: test_dry_run_no_sql
    INPUT:  1 domain, options.dry_run = true
    EXPECT: db.calls() is empty
    EXPECT: report.items_processed == 1
    EXPECT: report.items_created > 0
    EXPECT: report.duration == Duration::ZERO

TEST 12: test_sync_report_counts
    INPUT:  1 domain, 4 streams, 6 objectives, 0 constraints
    EXPECT: report.entity == "domain"
    EXPECT: report.items_processed == 1
    EXPECT: report.items_created == 1 + 4 + 6 + 0 == 11

TEST 13: test_sync_domain_error_is_non_fatal
    INPUT:  MockDbClient that fails on 2nd domain's UPSERT
    EXPECT: report.errors.len() == 1
    EXPECT: report.errors[0].item == failed domain_id
    EXPECT: other domain still synced successfully
```

### Conversion tests (`crates/ndp-lib/src/convert.rs`)

```
TEST 14: test_convert_real_domain_config
    INPUT:  Load config/domains/indoor-air-quality/domain.json
    EXPECT: entry.domain_id == "indoor-air-quality"
    EXPECT: entry.stream_count == 4
    EXPECT: entry.config_path == "config/domains/indoor-air-quality/domain.json"
    EXPECT: entry.objectives.len() == 6
    EXPECT: entry.constraints.len() == 0

TEST 15: test_convert_objective_fields_flattened
    INPUT:  Load real domain.json
    EXPECT: first objective.objective_id == "healthy_co2"
    EXPECT: first objective.target_stream == "air-quality"
    EXPECT: first objective.target_metric == "co2"
    EXPECT: first objective.condition == "<"
    EXPECT: first objective.threshold == 800.0
    EXPECT: first objective.threshold_upper == None
    EXPECT: first objective.unit == Some("ppm")
    EXPECT: first objective.priority == "high"

TEST 16: test_convert_between_condition
    INPUT:  synthetic domain config with "between" condition
    EXPECT: threshold == lower_bound
    EXPECT: threshold_upper == Some(upper_bound)

TEST 17: test_convert_empty_constraints
    INPUT:  Load real domain.json (has no constraints)
    EXPECT: entry.constraints is empty

TEST 18: test_convert_stream_mappings
    INPUT:  Load real domain.json
    EXPECT: 4 stream mappings
    EXPECT: streams[0].alias == "indoor", role == "primary"
    EXPECT: streams[2].alias == "state",  role == "actuator"
```

### Config loader tests (`crates/ndp-lib/src/config.rs`)

```
TEST 19: test_discover_domain_ids
    INPUT:  tempdir with 2 domain subdirs containing domain.json
    EXPECT: returns sorted vec of 2 domain IDs

TEST 20: test_load_domain_configs
    INPUT:  tempdir with real indoor-air-quality/domain.json
    EXPECT: returns 1 DomainConfig
    EXPECT: config.id == "indoor-air-quality"
    EXPECT: config.objectives.len() == 6

TEST 21: test_domain_config_not_found
    INPUT:  empty tempdir
    EXPECT: Err(NdpLibError::ConfigNotFound)

TEST 22: test_parse_real_domain_config
    INPUT:  include_str!(config/domains/indoor-air-quality/domain.json)
    EXPECT: all fields parsed correctly
    EXPECT: ThresholdValue deserialized correctly for single values
```

---

## 9. File Manifest

Summary of files to create or modify.

### New files

| File | Purpose |
|------|---------|
| `crates/ndp-lib/src/domain/mod.rs` | `sync_domain()` function + London TDD tests |
| `crates/ndp-lib/src/domain/types.rs` | `DomainSyncEntry`, `ObjectiveSyncEntry`, `StreamMappingEntry`, `ConstraintSyncEntry` |
| `crates/ndp-lib/src/domain/sql.rs` | SQL constants: `UPSERT_DOMAIN`, `INSERT_DOMAIN_STREAM`, `INSERT_OBJECTIVE`, etc. |
| `tools/ndp-cli/src/commands/domain.rs` | `ndp domain sync` CLI command |

### Modified files

| File | Change |
|------|--------|
| `crates/ndp-lib/src/lib.rs` | Add `pub mod domain;` |
| `crates/ndp-lib/src/config.rs` | Add `DomainConfig` struct, `load_domain_configs()` to `ConfigLoader` trait, impl in `FileSystemConfigLoader`, add `domains_dir` field |
| `crates/ndp-lib/src/convert.rs` | Add `domain_config_to_sync_entry()` function |
| `tools/ndp-cli/src/main.rs` | Add `Domain(DomainArgs)` variant to `Commands` enum |
| `tools/ndp-cli/src/commands/mod.rs` | Add `pub mod domain;` |
| `deploy/pi/deploy.sh` | Replace body of `sync_domains_to_data_dictionary()` (lines 883-1086) |

### Unchanged files

| File | Reason |
|------|--------|
| `deploy/pi/init-scripts/005_domain_objectives.sql` | Tables already exist, no DDL changes needed |
| `config/domains/indoor-air-quality/domain.json` | Config already has all needed fields |
| `crates/ndp-lib/src/db.rs` | `DbClient` trait already has `execute()` and `batch_execute()` |
| `crates/ndp-lib/src/types.rs` | `SyncReport`, `SyncOptions`, `SyncError` already sufficient |
| `crates/ndp-lib/src/error.rs` | Existing error variants sufficient |

---

## 10. Design Pattern Summary

| Pattern | Where Applied | Reference |
|---------|--------------|-----------|
| **Parsed structs, not file paths** | `sync_domain()` takes `&[DomainSyncEntry]` | `sync_dictionary()` takes `&[StreamDictionaryEntry]` |
| **Trait-based DB** | `&impl DbClient` for mockability | Same as dictionary and dimension sync |
| **Parameterized SQL** | All `$1, $2, ...` constants in `sql.rs` | Same as `dictionary/sql.rs` |
| **Transaction wrapping** | `BEGIN`/`COMMIT` around all mutations | Same as dictionary sync |
| **ConfigLoader trait** | `load_domain_configs()` added | Alongside `load_stream_configs()` |
| **Conversion layer** | `domain_config_to_sync_entry()` | Alongside `stream_config_to_dictionary_entry()` |
| **Entity/verb CLI** | `ndp domain sync` | Alongside `ndp dictionary sync`, `ndp dimension sync` |
| **deploy.sh fallback** | `command -v ndp` pattern | Lines 384-414 (dictionary), 1219-1227 (dimension) |
| **London TDD** | `MockDbClient` records SQL calls | Same as dictionary tests (lines 482-1254) |
| **Non-fatal errors** | Per-domain errors collected, sync continues | Same as per-stream errors in dictionary sync |
