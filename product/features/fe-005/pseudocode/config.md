# fe-005 Pseudocode: config

## Location: `config/schemas/domain.schema.json`

### Schema Addition

Add a `text_embedding` definition and property to the domain schema.

#### New Definition: `text_embedding`

```json
{
  "definitions": {
    "text_embedding": {
      "type": "object",
      "additionalProperties": false,
      "description": "Text embedding configuration for the ndp-embedder service. Separate from intelligence.embedding (metric embeddings).",
      "required": ["model", "dimensions"],
      "properties": {
        "model": {
          "type": "string",
          "description": "Model identifier for download and loading (e.g., 'BAAI/bge-small-en-v1.5')"
        },
        "quantization": {
          "type": "string",
          "enum": ["fp32", "int8"],
          "default": "int8",
          "description": "Model quantization level"
        },
        "dimensions": {
          "type": "integer",
          "minimum": 1,
          "maximum": 4096,
          "description": "Output embedding vector dimensions (must match model)"
        },
        "preprocessing": {
          "type": "object",
          "additionalProperties": false,
          "description": "Text preprocessing pipeline configuration",
          "properties": {
            "type": {
              "type": "string",
              "enum": ["passthrough"],
              "default": "passthrough",
              "description": "Preprocessing strategy"
            }
          }
        }
      }
    }
  }
}
```

#### New Property on `domain_content`

Add `text_embedding` as an optional property alongside `intelligence`:

```json
{
  "properties": {
    "text_embedding": {
      "$ref": "#/definitions/text_embedding",
      "description": "Text embedding configuration (optional, zero cost if omitted)"
    }
  }
}
```

The property is NOT in the `required` array -- domains without text embedding pay zero cost.

### Example Domain Config with Text Embedding

```json
{
  "id": "indoor-air-quality",
  "description": "Indoor air quality monitoring domain",
  "streams": [...],
  "alignment": {...},
  "objectives": [...],
  "intelligence": {
    "enabled": true,
    "embedding": {
      "type": "metric",
      "fields": {...}
    },
    "search": {...}
  },
  "text_embedding": {
    "model": "BAAI/bge-small-en-v1.5",
    "quantization": "int8",
    "dimensions": 384,
    "preprocessing": {
      "type": "passthrough"
    }
  }
}
```

Note: `intelligence.embedding` (metric) and `text_embedding` coexist at the same level. They serve different purposes:
- `intelligence.embedding`: configures MetricEmbedder (z-score normalization of numeric Gold row fields)
- `text_embedding`: configures TextEmbedder/OnnxEmbedder (ONNX inference on text data)
