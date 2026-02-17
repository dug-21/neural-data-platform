# fe-007 Pseudocode: ndp-cli (DDL Generation via ndp-lib)

## Files Created/Modified

- `crates/ndp-lib/src/gold/generators/causal_candidates.rs` (NEW)
- `crates/ndp-lib/src/gold/generators/mod.rs` (add re-export)
- `tools/ndp-cli/src/gold.rs` (add causal-candidates subcommand invocation)

## CausalCandidatesGenerator

```pseudocode
// crates/ndp-lib/src/gold/generators/causal_candidates.rs

/// DDL generator for the gold.causal_candidates table.
/// This is a global table (not per-domain) with domain_id column for multi-tenancy.
/// Unlike aligned views or continuous aggregates, this is a regular table with UPSERT semantics.
pub struct CausalCandidatesGenerator;

impl CausalCandidatesGenerator:
    /// Generate the CREATE TABLE DDL for gold.causal_candidates.
    pub fn generate_ddl() -> String:
        return "
            CREATE TABLE IF NOT EXISTS gold.causal_candidates (
                id                BIGSERIAL PRIMARY KEY,
                domain_id         TEXT NOT NULL,
                source_stream     TEXT NOT NULL,
                target_stream     TEXT NOT NULL,
                test_method       TEXT NOT NULL,
                lag_hours         INTEGER NOT NULL,
                f_statistic       DOUBLE PRECISION NOT NULL,
                p_value           DOUBLE PRECISION NOT NULL,
                p_value_adjusted  DOUBLE PRECISION,
                is_significant    BOOLEAN NOT NULL,
                bic               DOUBLE PRECISION,
                preprocessing     TEXT NOT NULL,
                evidence_count    INTEGER NOT NULL DEFAULT 1,
                stability_score   DOUBLE PRECISION,
                first_seen        TIMESTAMPTZ NOT NULL DEFAULT now(),
                last_seen         TIMESTAMPTZ NOT NULL DEFAULT now(),
                scan_window_start TIMESTAMPTZ,
                scan_window_end   TIMESTAMPTZ,
                metadata          JSONB DEFAULT '{}'::jsonb,
                UNIQUE (domain_id, source_stream, target_stream, lag_hours)
            );

            CREATE INDEX IF NOT EXISTS idx_causal_candidates_domain
                ON gold.causal_candidates (domain_id);

            CREATE INDEX IF NOT EXISTS idx_causal_candidates_significant
                ON gold.causal_candidates (domain_id, is_significant)
                WHERE is_significant = true;
        "

    /// Check if the table already exists.
    pub async fn table_exists(client: &dyn DbClient) -> Result<bool>:
        let result = client.query_one(
            "SELECT EXISTS (
                SELECT 1 FROM information_schema.tables
                WHERE table_schema = 'gold' AND table_name = 'causal_candidates'
            )",
            &[]
        ).await?
        return result.get::<_, bool>(0)
```

## Integration with ndp gold CLI

```pseudocode
// The causal_candidates table is created during deploy.sh Phase 6 (Gold DDL).
// Option A: Add as a subcommand to ndp-cli:
//   ndp gold causal-candidates --domain <domain-id>
//
// Option B: Add to the existing intelligence DDL generation:
//   ndp gold intelligence schema --domain <domain-id>
//   (already creates pgvector tables, add causal_candidates)
//
// Recommendation: Option B -- extend existing intelligence DDL path
// since causal_candidates is part of the intelligence layer.
//
// In the PgVectorSchemaGenerator or a new intelligence DDL entry point:
fn generate_intelligence_ddl(domain_id: &str, config: &IntelligenceConfig) -> Vec<String>:
    let mut ddl = Vec::new()

    // Existing: pgvector extension, metric_embeddings table, predictions table
    ddl.extend(PgVectorSchemaGenerator::generate(domain_id, &config.embedding))

    // New: causal_candidates table (only if granger config exists)
    if config.granger.is_some():
        ddl.push(CausalCandidatesGenerator::generate_ddl())

    return ddl
```

## deploy.sh Integration

```pseudocode
// In deploy.sh Phase 6 (Gold DDL):
// The existing intelligence DDL step already runs:
//   ndp gold intelligence schema --domain "$DOMAIN_ID"
//
// If we extend that command to include causal_candidates DDL,
// no deploy.sh changes are needed.
//
// If we use a separate subcommand:
//   ndp gold causal-candidates --domain "$DOMAIN_ID"
// Then add after the intelligence schema step in handle_domain().
```
