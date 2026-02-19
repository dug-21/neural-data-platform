# Implementation Launch Prompt: fe-005

## Proposed Prompt
> Implement fe-005: Text Embeddings
> GitHub Issue: #39 (https://github.com/dug-21/neural-data-platform/issues/39)
> Brief: product/features/fe-005/IMPLEMENTATION-BRIEF.md
> Pattern IDs from planning: 25 (TextEmbedder trait), 26 (OnnxEmbedder), 27 (container arch), 28 (model management), 29 (text embeddings schema), 30 (dp-023 interface), 31 (preprocessing), 32 (domain schema)
> Constraints: ARM64, 512MB container limit, no ONNX types in trait, config-driven, dp-023 dependency
> Wave structure: Wave 1 (ndp-lib traits + OnnxEmbedder + preprocessing + model manager + config), Wave 2 (DDL + schema + ndp-embedder crate + Dockerfile), Wave 3 (service impl + compose + deploy.sh + integration tests)

## Reminders for User
- Review ALIGNMENT-REPORT.md for WARN-001 (model download requires internet) and WARN-002 (ort ARM64 unverified)
- dp-023 must be implemented first (creates Gold text view that fe-005 reads from)
- Verify acceptance criteria in SCOPE.md match ACCEPTANCE-MAP.md

## Gotchas Discovered During Planning
- The existing `Embedder` trait in ndp-lib operates on `GoldRow` (numeric BTreeMap). TextEmbedder is a SEPARATE trait operating on `&[&str]`. Do not try to extend the existing trait.
- `intelligence.embedding` in domain.json is for MetricEmbedder (fe-004). The new text config is `text_embedding` at the TOP LEVEL of domain.json -- not nested under `intelligence`.
- The init-script for `gold.text_embeddings` must use `PERFORM create_hypertable(...)` inside a DO block, or call it as a bare SELECT at the top level. See ops-008 experience with events DDL.
- The `ort` crate links to ONNX Runtime C++ library. On ARM64, the `ort` crate should download pre-built binaries automatically, but this needs Pi 5 validation.
- The `tokenizers` crate is large and may add significant compile time. Consider feature-gating if it becomes a problem.
- Model download URLs follow HuggingFace pattern: `https://huggingface.co/{org}/{model}/resolve/main/{file}`. The model_id in domain config maps directly to this URL structure.
