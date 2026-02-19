# fe-005 Acceptance Criteria Map

| AC-ID | Description | Verification Method | Verification Detail | Status |
|-------|-------------|--------------------|--------------------|--------|
| AC-01 | TextEmbedder trait is model-agnostic | grep | `grep -r "ort\|onnx\|Session" crates/ndp-lib/src/gold/embeddings/text.rs` returns no matches (no ONNX types in trait file) | PENDING |
| AC-02 | OnnxEmbedder produces vectors | test | `cargo test -p ndp-lib test_onnx_embedder_dimensions` -- verifies 384D Vec<f32> output | PENDING |
| AC-03 | Model loads from volume | test | `cargo test -p ndp-lib test_model_manager_existing_model` -- volume path resolution | PENDING |
| AC-04 | Domain config drives model selection | test | `cargo test -p ndp-lib test_text_embedding_config_deserialize` -- config deserialization | PENDING |
| AC-05 | Preprocessing pipeline exists | test | `cargo test -p ndp-lib test_passthrough_identity` + `test_factory_passthrough` | PENDING |
| AC-06 | Text embeddings stored | test | `cargo test -p ndp-embedder test_full_pipeline` -- end-to-end Gold view to gold.text_embeddings | PENDING |
| AC-07 | Retention tier column present | shell | `psql -c "SELECT column_name, is_nullable FROM information_schema.columns WHERE table_schema='gold' AND table_name='text_embeddings' AND column_name='retention_tier'"` returns YES | PENDING |
| AC-08 | Inference latency (cold) | manual | Run ndp-embedder on Pi 5, measure first embedding time < 500ms | PENDING |
| AC-09 | ndp-embedder container runs | shell | `docker compose --profile intelligence up -d ndp-embedder && docker ps --filter name=ndp-embedder --format '{{.Status}}'` shows Up | PENDING |
| AC-10 | Existing metric cycle unaffected | test | `cargo test -p ndp-intelligence` -- all existing tests pass, no modifications | PENDING |
