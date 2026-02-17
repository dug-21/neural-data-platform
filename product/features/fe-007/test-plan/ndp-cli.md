# fe-007 Test Plan: ndp-cli (DDL Generation via ndp-lib)

## Unit Tests

| Test | Input | Expected | Assertion |
|------|-------|----------|-----------|
| `test_causal_candidates_ddl_contains_create` | generate_ddl() | Contains "CREATE TABLE IF NOT EXISTS gold.causal_candidates" | String match |
| `test_causal_candidates_ddl_has_unique_constraint` | generate_ddl() | Contains "UNIQUE (domain_id, source_stream, target_stream, lag_hours)" | String match |
| `test_causal_candidates_ddl_has_indexes` | generate_ddl() | Contains both CREATE INDEX statements | String match |

## Integration Tests

| Test | Setup | Action | Assertion |
|------|-------|--------|-----------|
| `test_causal_candidates_table_creation` | Empty gold schema | Execute generate_ddl() | Table exists in information_schema |
| `test_causal_candidates_idempotent` | Table already exists | Execute generate_ddl() again | No error (IF NOT EXISTS) |

## DDL Validation

The generated DDL should be validated by:
1. Parsing with a SQL parser (or executing against test database)
2. Verifying all column types are valid PostgreSQL types
3. Verifying the UPSERT ON CONFLICT clause matches the unique constraint
4. Verifying indexes reference existing columns
