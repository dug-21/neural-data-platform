# YAML-Based Configuration Architecture

## Overview

This document defines a comprehensive YAML-based configuration system for the generic platform, enabling zero-code domain deployment through hierarchical configuration files. All domain-specific details are externalized into YAML, with components self-configuring on startup.

## Core Principles

1. **Single Source of Truth**: Each configuration element exists in exactly one location
2. **Self-Configuring Components**: All services read their configuration from YAML on startup
3. **Domain Registration**: New domains are registered purely through YAML files
4. **Zero-Code Deployment**: Complete domain functionality without writing code
5. **Hierarchical Configuration**: Inheritance and override capabilities
6. **Schema Validation**: JSON Schema ensures configuration correctness

## Architecture Components

### 1. Domain Registry

The platform discovers domains by scanning the `/domains` directory for YAML files:

```
/domains/
  ├── trading.yaml      # Trading domain configuration
  ├── logs.yaml         # Log analysis domain
  ├── iot.yaml          # IoT monitoring domain
  └── custom/           # User-defined domains
```

### 2. Configuration Hierarchy

```yaml
# Global defaults (platform.yaml)
platform:
  defaults:
    neural:
      type: "feedforward"
      layers: [64, 32, 16]
    
# Domain-specific (trading.yaml)
domain:
  extends: "platform.defaults"
  neural:
    layers: [128, 64, 32]  # Overrides default
```

### 3. Component Configuration

Each component reads its configuration section:

- **Neural Engine**: Reads `neural:` section
- **DAA Coordinator**: Reads `agents:` section
- **Data Pipeline**: Reads `pipeline:` section
- **API Gateway**: Reads `api:` section

## Configuration Structure

### Domain Configuration Sections

1. **Metadata**: Domain identification and versioning
2. **Neural Models**: Model architecture and parameters
3. **DAA Agents**: Agent types and behaviors
4. **Data Pipeline**: Sources, transformations, sinks
5. **API Endpoints**: REST/GraphQL/WebSocket definitions
6. **Monitoring**: Metrics, alerts, dashboards
7. **Security**: Authentication, authorization, encryption

### Configuration Loading Process

1. **Discovery Phase**:
   - Scan `/domains` directory
   - Load and validate YAML files
   - Register domains with platform

2. **Initialization Phase**:
   - Components read their configuration sections
   - Apply inheritance and overrides
   - Validate against schemas

3. **Runtime Phase**:
   - Hot-reload on configuration changes
   - Dynamic reconfiguration without restart
   - Configuration versioning and rollback

## Key Features

### 1. Variable Interpolation

```yaml
variables:
  model_size: "large"
  base_layers: [64, 32]

neural:
  type: "feedforward_${model_size}"
  layers: "${base_layers}"
```

### 2. Environment Override

```yaml
neural:
  layers: ${NEURAL_LAYERS:-[64, 32, 16]}
  learning_rate: ${LEARNING_RATE:-0.001}
```

### 3. Conditional Configuration

```yaml
neural:
  $if: "${environment == 'production'}"
  then:
    layers: [256, 128, 64]
  else:
    layers: [64, 32, 16]
```

### 4. Schema References

```yaml
data:
  schema: 
    $ref: "schemas/trading/market_data.json"
```

### 5. Plugin System

```yaml
plugins:
  - name: "custom-indicator"
    source: "file://plugins/indicators.wasm"
    config:
      window_size: 20
```

## Component Integration

### Neural Engine Integration

```yaml
neural:
  models:
    - name: "price_predictor"
      type: "lstm"
      architecture:
        input_size: 10
        hidden_layers: [128, 64]
        output_size: 1
      training:
        epochs: 100
        batch_size: 32
        optimizer: "adam"
```

### DAA Agent Configuration

```yaml
agents:
  - type: "market_analyzer"
    capabilities:
      - "technical_analysis"
      - "pattern_recognition"
    resources:
      cpu_limit: "2"
      memory_limit: "4Gi"
    behavior:
      strategy: "aggressive"
      risk_tolerance: 0.8
```

### Data Pipeline Configuration

```yaml
pipeline:
  sources:
    - type: "websocket"
      url: "${MARKET_DATA_URL}"
      format: "json"
  
  transformations:
    - type: "normalize"
      fields: ["price", "volume"]
    - type: "aggregate"
      window: "1m"
      
  sinks:
    - type: "timeseries"
      database: "influxdb"
      retention: "30d"
```

## Implementation Guidelines

### 1. Configuration Loading

```rust
// Pseudo-code for configuration loading
fn load_domain_config(domain_name: &str) -> DomainConfig {
    let yaml_path = format!("/domains/{}.yaml", domain_name);
    let config = load_yaml(yaml_path)?;
    
    // Apply inheritance
    if let Some(extends) = config.get("extends") {
        let base = load_config(extends);
        config = merge_configs(base, config);
    }
    
    // Variable interpolation
    config = interpolate_variables(config);
    
    // Environment overrides
    config = apply_env_overrides(config);
    
    // Validation
    validate_against_schema(config)?;
    
    config
}
```

### 2. Component Self-Configuration

```rust
// Each component implements ConfigurableComponent trait
trait ConfigurableComponent {
    fn configure(&mut self, config: &Config) -> Result<()>;
    fn reconfigure(&mut self, new_config: &Config) -> Result<()>;
    fn validate_config(&self, config: &Config) -> Result<()>;
}
```

### 3. Hot Reload Support

```rust
// File watcher for configuration changes
fn watch_config_changes() {
    let watcher = FileWatcher::new("/domains");
    
    watcher.on_change(|path| {
        let domain = extract_domain_name(path);
        let new_config = load_domain_config(domain)?;
        
        // Notify components of configuration change
        broadcast_config_update(domain, new_config);
    });
}
```

## Best Practices

1. **Modularity**: Split large configurations into multiple files using includes
2. **Versioning**: Always specify configuration version for compatibility
3. **Documentation**: Include inline comments explaining complex configurations
4. **Validation**: Use JSON Schema for strict validation
5. **Defaults**: Provide sensible defaults for all optional parameters
6. **Security**: Never hardcode secrets, use environment variables or secret stores
7. **Testing**: Include test configurations for CI/CD pipelines

## Migration Strategy

1. **Phase 1**: Implement YAML loading and validation infrastructure
2. **Phase 2**: Migrate one component at a time to YAML configuration
3. **Phase 3**: Remove hardcoded domain logic from codebase
4. **Phase 4**: Enable hot reload and dynamic reconfiguration
5. **Phase 5**: Full zero-code domain deployment

## Monitoring and Debugging

### Configuration Metrics

- Configuration load time
- Validation errors
- Hot reload frequency
- Configuration drift detection

### Debugging Tools

```bash
# Validate configuration
platform validate-config trading.yaml

# Show effective configuration (after inheritance/overrides)
platform show-config trading --effective

# Diff configurations
platform diff-config trading.yaml trading-v2.yaml

# Test configuration without applying
platform test-config trading.yaml --dry-run
```

## Security Considerations

1. **Schema Validation**: Prevent injection attacks through strict schemas
2. **Access Control**: Limit who can modify configurations
3. **Audit Trail**: Log all configuration changes
4. **Encryption**: Encrypt sensitive configuration values
5. **Sandboxing**: Run user-provided configurations in isolated environments

## Conclusion

This YAML-based configuration architecture enables true zero-code domain deployment while maintaining flexibility, security, and performance. By externalizing all domain-specific logic into configuration files, the platform becomes genuinely generic and infinitely extensible.