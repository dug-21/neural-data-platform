# fe-005 Test Plan: ndp-embedder

## Location: `apps/ndp-embedder/`

### Service Tests (using MockTextEmbedder)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // T-030: EmbeddingService processes text rows
    #[tokio::test]
    async fn test_service_processes_rows() {
        // Setup: create test DB with gold.test_domain_text_latest view
        // Create EmbeddingService with MockTextEmbedder
        // Run one cycle
        // Assert: embeddings_stored > 0, correct dimensions in gold.text_embeddings
    }

    // T-031: EmbeddingService tracks last_processed
    #[tokio::test]
    async fn test_service_tracks_last_processed() {
        // First cycle processes all rows
        // Second cycle processes only new rows (after last_processed)
        // Assert: no duplicate embeddings
    }

    // T-032: EmbeddingService handles missing Gold text view
    #[tokio::test]
    async fn test_service_missing_view_graceful() {
        // Create service pointing to nonexistent view
        // Run cycle
        // Assert: returns Ok with 0 rows_read (graceful degradation)
    }

    // T-033: EmbeddingService handles empty result set
    #[tokio::test]
    async fn test_service_empty_results() {
        // Create view with no rows matching WHERE clause
        // Run cycle
        // Assert: summary shows 0 rows_read, 0 embeddings_stored
    }

    // T-034: EmbeddingService applies preprocessing
    #[tokio::test]
    async fn test_service_applies_preprocessing() {
        // Use a counting preprocessor that records calls
        // Run cycle with test data
        // Assert: preprocessor was called once per text row
    }

    // T-035: EmbeddingService handles embedding failure
    #[tokio::test]
    async fn test_service_handles_embedding_error() {
        // Use a FailingTextEmbedder that returns Err
        // Run cycle with test data
        // Assert: errors count > 0, no crash
    }

    // T-036: EmbeddingService inserts correct provenance columns
    #[tokio::test]
    async fn test_service_provenance_columns() {
        // Run cycle with known test data
        // Query gold.text_embeddings
        // Assert: source_stream, source_column, source_text, model_id match input
    }

    // T-037: is_relation_not_found detects 42P01
    #[test]
    fn test_relation_not_found_detection() {
        let err = anyhow::anyhow!("ERROR: relation \"gold.test\" does not exist (42P01)");
        assert!(is_relation_not_found(&err));

        let other = anyhow::anyhow!("connection refused");
        assert!(!is_relation_not_found(&other));
    }
}
```

### AppConfig Tests

```rust
#[cfg(test)]
mod config_tests {
    use super::*;

    // T-038: AppConfig loads from environment
    #[test]
    fn test_app_config_from_env() {
        std::env::set_var("DATABASE_URL", "postgresql://test");
        std::env::set_var("EMBEDDER_DOMAIN", "test-domain");
        let config = AppConfig::from_env().unwrap();
        assert_eq!(config.domain_id, "test-domain");
        assert_eq!(config.poll_interval_secs, 1200); // default
        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("EMBEDDER_DOMAIN");
    }

    // T-039: AppConfig requires DATABASE_URL
    #[test]
    fn test_app_config_requires_db_url() {
        std::env::remove_var("DATABASE_URL");
        let result = AppConfig::from_env();
        assert!(result.is_err());
    }

    // T-040: AppConfig requires EMBEDDER_DOMAIN
    #[test]
    fn test_app_config_requires_domain() {
        std::env::set_var("DATABASE_URL", "postgresql://test");
        std::env::remove_var("EMBEDDER_DOMAIN");
        let result = AppConfig::from_env();
        assert!(result.is_err());
        std::env::remove_var("DATABASE_URL");
    }

    // T-041: AppConfig custom poll interval
    #[test]
    fn test_app_config_custom_poll() {
        std::env::set_var("DATABASE_URL", "postgresql://test");
        std::env::set_var("EMBEDDER_DOMAIN", "test");
        std::env::set_var("EMBEDDER_POLL_INTERVAL_SECS", "60");
        let config = AppConfig::from_env().unwrap();
        assert_eq!(config.poll_interval_secs, 60);
        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("EMBEDDER_DOMAIN");
        std::env::remove_var("EMBEDDER_POLL_INTERVAL_SECS");
    }

    // T-042: AppConfig default model volume path
    #[test]
    fn test_app_config_default_model_path() {
        std::env::set_var("DATABASE_URL", "postgresql://test");
        std::env::set_var("EMBEDDER_DOMAIN", "test");
        let config = AppConfig::from_env().unwrap();
        assert_eq!(config.model_volume_path.to_str().unwrap(), "/models");
        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("EMBEDDER_DOMAIN");
    }
}
```

### CycleSummary Tests

```rust
#[cfg(test)]
mod summary_tests {
    use super::*;

    // T-043: CycleSummary default is zeroed
    #[test]
    fn test_cycle_summary_default() {
        let s = CycleSummary::default();
        assert_eq!(s.rows_read, 0);
        assert_eq!(s.embeddings_stored, 0);
        assert_eq!(s.errors, 0);
    }

    // T-044: CycleSummary display
    #[test]
    fn test_cycle_summary_display() {
        let s = CycleSummary {
            rows_read: 5,
            embeddings_stored: 4,
            errors: 1,
            duration: Duration::from_millis(100),
        };
        let display = format!("{}", s);
        assert!(display.contains("5"));
        assert!(display.contains("4"));
    }
}
```

### Integration Tests (require running database)

```rust
// tests/integration/embedder_pipeline.rs

#[cfg(test)]
mod integration {
    use super::*;

    // T-045: End-to-end: Gold text view -> embed -> gold.text_embeddings
    #[tokio::test]
    #[ignore] // Requires running TimescaleDB with pgvector
    async fn test_full_pipeline() {
        // 1. Create test Gold text view with sample NWS forecast text
        // 2. Create EmbeddingService with MockTextEmbedder (384D)
        // 3. Run cycle
        // 4. Query gold.text_embeddings
        // 5. Assert: correct number of rows, correct dimensions, correct provenance
    }

    // T-046: Idempotency: re-running cycle does not duplicate embeddings
    #[tokio::test]
    #[ignore]
    async fn test_idempotency() {
        // Run cycle twice with same data
        // Assert: same number of rows in gold.text_embeddings (ON CONFLICT DO NOTHING)
    }

    // T-047: Multiple text columns from same stream
    #[tokio::test]
    #[ignore]
    async fn test_multiple_columns() {
        // Gold text view with both short_forecast and detailed_forecast
        // Run cycle
        // Assert: separate embeddings for each column
    }
}
```

## Test Summary

| ID | Test | Type | Component |
|----|------|------|-----------|
| T-030 | Service processes text rows | Integration | service.rs |
| T-031 | Service tracks last_processed | Integration | service.rs |
| T-032 | Service handles missing view | Integration | service.rs |
| T-033 | Service handles empty results | Integration | service.rs |
| T-034 | Service applies preprocessing | Integration | service.rs |
| T-035 | Service handles embedding error | Integration | service.rs |
| T-036 | Service provenance columns | Integration | service.rs |
| T-037 | is_relation_not_found | Unit | service.rs |
| T-038 | AppConfig from env | Unit | main.rs |
| T-039 | AppConfig requires DB URL | Unit | main.rs |
| T-040 | AppConfig requires domain | Unit | main.rs |
| T-041 | AppConfig custom poll | Unit | main.rs |
| T-042 | AppConfig default model path | Unit | main.rs |
| T-043 | CycleSummary default | Unit | service.rs |
| T-044 | CycleSummary display | Unit | service.rs |
| T-045 | Full pipeline e2e | Integration | tests/ |
| T-046 | Idempotency | Integration | tests/ |
| T-047 | Multiple text columns | Integration | tests/ |
