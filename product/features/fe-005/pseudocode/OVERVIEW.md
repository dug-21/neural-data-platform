# fe-005: Pseudocode Overview

## Components

| Component | Location | Role |
|-----------|----------|------|
| ndp-lib | `crates/ndp-lib/src/gold/embeddings/` | TextEmbedder trait, OnnxEmbedder, preprocessing, model management, config types |
| ndp-embedder | `apps/ndp-embedder/` (NEW) | Embedding service binary -- reads Gold text view, embeds, stores |
| deploy | `docker/embedder/`, `deploy/pi/` | Dockerfile, compose entry, deploy.sh integration |
| config | `config/schemas/` | Domain schema update for text_embedding block |
| database | `deploy/pi/init-scripts/` | gold.text_embeddings DDL (init-script) |

## Data Flow

```
domain.json (etcd)
    |
    v
[ndp-embedder startup]
    |
    +--> read text_embedding config
    +--> ModelManager.ensure_model() --> volume mount
    +--> load OnnxEmbedder(model.onnx, tokenizer.json)
    +--> create TextPreprocessor (passthrough)
    |
    v
[poll loop]
    |
    +--> query gold.{domain}_text_latest WHERE bucket > last_processed
    |       |
    |       v
    |    for each (bucket, stream_id, column_name, text_value):
    |       |
    |       +--> preprocessor.preprocess(text_value)
    |       +--> embedder.embed(&[preprocessed_text])
    |       +--> INSERT INTO gold.text_embeddings (...)
    |       |
    |       v
    |    update last_processed = max(bucket)
    |
    +--> sleep(poll_interval)
    +--> repeat
```

## Component Interactions

1. **ndp-lib** provides all library types. ndp-embedder depends on ndp-lib.
2. **ndp-embedder** is the only runtime consumer of TextEmbedder/OnnxEmbedder.
3. **deploy** creates the container and integrates with deploy.sh.
4. **config** schema validates domain.json before etcd sync.
5. **database** init-script creates gold.text_embeddings at bootstrap.

## Cross-Component Dependencies

- ndp-embedder imports from ndp-lib: `TextEmbedder`, `OnnxEmbedder`, `TextPreprocessor`, `ModelManager`, `TextEmbeddingConfig`
- ndp-embedder imports from config-client: etcd config loading
- ndp-embedder imports from ndp-types: shared types
- deploy/docker-compose.yml references docker/embedder/Dockerfile
- deploy/deploy.sh Phase 6 references init-script DDL
