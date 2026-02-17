# fe-007 Pseudocode: Domain Config (Schema + Config Types)

## Files Modified

- `config/schemas/domain.schema.json` -- Add granger to intelligence definition
- `crates/ndp-lib/src/gold/embeddings/config.rs` -- Add GrangerConfig struct
- `config/integration/domains/indoor-air-quality/domain.json` -- Add granger block

## domain.schema.json Changes

```pseudocode
// In the "intelligence" definition, add "granger" to properties:
{
  "intelligence": {
    "type": "object",
    "additionalProperties": false,
    "required": ["enabled", "embedding", "search"],
    "properties": {
      "enabled": { "type": "boolean" },
      "embedding": { /* existing */ },
      "search": { /* existing */ },
      "anomaly": { /* existing */ },
      "granger": {                          // NEW
        "type": "object",
        "additionalProperties": false,
        "properties": {
          "candidate_count": {
            "type": "integer",
            "minimum": 1,
            "default": 10
          },
          "lag_hours": {
            "type": "array",
            "items": { "type": "integer", "minimum": 1 },
            "minItems": 1,
            "default": [1, 2, 4]
          },
          "significance_level": {
            "type": "number",
            "minimum": 0,
            "exclusiveMaximum": 1,
            "default": 0.05
          },
          "test_method": {
            "type": "string",
            "enum": ["classical", "toda_yamamoto"],
            "default": "classical"
          },
          "preprocessing": {
            "type": "string",
            "enum": ["adaptive", "raw", "difference", "seasonal"],
            "default": "adaptive"
          },
          "evidence_window_days": {
            "type": "integer",
            "minimum": 1,
            "default": 7
          },
          "scan_interval_hours": {
            "type": "integer",
            "minimum": 1,
            "default": 24
          },
          "min_observations": {
            "type": "integer",
            "minimum": 10,
            "default": 48
          }
        }
      }
    }
  }
}
```

## GrangerConfig Rust Type

```pseudocode
// In crates/ndp-lib/src/gold/embeddings/config.rs:

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GrangerConfig {
    #[serde(default = "default_candidate_count")]
    pub candidate_count: usize,

    #[serde(default = "default_lag_hours")]
    pub lag_hours: Vec<u32>,

    #[serde(default = "default_significance")]
    pub significance_level: f64,

    #[serde(default = "default_test_method")]
    pub test_method: String,

    #[serde(default = "default_preprocessing")]
    pub preprocessing: String,

    #[serde(default = "default_evidence_window")]
    pub evidence_window_days: u32,

    #[serde(default = "default_scan_interval")]
    pub scan_interval_hours: u32,

    #[serde(default = "default_min_observations")]
    pub min_observations: usize,
}

fn default_candidate_count() -> usize { 10 }
fn default_lag_hours() -> Vec<u32> { vec![1, 2, 4] }
fn default_significance() -> f64 { 0.05 }
fn default_test_method() -> String { "classical".to_string() }
fn default_preprocessing() -> String { "adaptive".to_string() }
fn default_evidence_window() -> u32 { 7 }
fn default_scan_interval() -> u32 { 24 }
fn default_min_observations() -> usize { 48 }

// Add to IntelligenceConfig:
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IntelligenceConfig {
    pub enabled: bool,
    pub embedding: EmbeddingConfig,
    pub search: SearchConfig,
    #[serde(default)]
    pub anomaly: Option<AnomalyConfig>,
    #[serde(default)]                     // NEW
    pub granger: Option<GrangerConfig>,   // NEW
}
```

## Integration Domain Config Update

```pseudocode
// In config/integration/domains/indoor-air-quality/domain.json:
// Add granger block to intelligence section:
{
  "intelligence": {
    "enabled": true,
    "embedding": { /* existing */ },
    "search": { /* existing */ },
    "granger": {
      "candidate_count": 10,
      "lag_hours": [1, 2, 4],
      "significance_level": 0.05,
      "test_method": "classical",
      "preprocessing": "adaptive",
      "evidence_window_days": 7,
      "scan_interval_hours": 24,
      "min_observations": 48
    }
  }
}
```

## Compose File Update

```pseudocode
// In deploy/pi/docker-compose.*.yml, ndp-intelligence service:
// Add environment variable:
  ndp-intelligence:
    environment:
      - NDP_GRANGER_ENABLED=false    // NEW - default disabled
      - DATABASE_URL=...
      - INTELLIGENCE_DOMAIN=...
      - ETCD_ENDPOINTS=...
```
